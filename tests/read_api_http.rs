//! HTTP-level tests for zapfast-reader: a real loopback socket in front,
//! a real HTTP mock Server behind it (through ServerSender). Synthetic
//! data only.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use zapfast::read_api::{RateLimiter, Registry};
use zapfast::read_http::{Shared, run};
use zapfast::scheduler_api::ServerSender;

type Log = Arc<Mutex<Vec<String>>>;

const ROUTES: &[(&str, u16, &str)] = &[
    (
        "/v1/accounts",
        200,
        r#"[{"accountID":"a1"},{"accountID":"b2"}]"#,
    ),
    (
        "/v1/chats/c-a1/messages",
        200,
        r#"{"items":[{"text":"hi"}]}"#,
    ),
    ("/v1/chats/c-a1", 200, r#"{"accountID":"a1"}"#),
    ("/v1/chats/c-b2", 200, r#"{"accountID":"b2"}"#),
    ("/v1/chats/boom", 500, "SECRET-DETAIL"),
    ("/v1/chats", 200, r#"{"items":[{"accountID":"a1"}]}"#),
];

/// A mock Server: records "METHOD target|authorization" per request.
fn mock_server(redirect_accounts: bool) -> (String, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&log);
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]).into_owned();
            let mut first = head.lines().next().unwrap_or("").split_whitespace();
            let method = first.next().unwrap_or("").to_string();
            let target = first.next().unwrap_or("").to_string();
            let auth = head
                .lines()
                .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
                .unwrap_or("")
                .to_string();
            seen.lock()
                .unwrap()
                .push(format!("{method} {target}|{auth}"));
            let (status, extra, body) = if redirect_accounts && target == "/v1/accounts" {
                (302, "location: http://127.0.0.1:1/elsewhere\r\n", "")
            } else {
                match ROUTES
                    .iter()
                    .find(|(p, _, _)| target.split('?').next() == Some(*p))
                {
                    Some((_, s, b)) => (*s, "", *b),
                    None => (404, "", "{}"),
                }
            };
            let reply = format!(
                "HTTP/1.1 {status} X\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    (base, log)
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Reader {
    addr: String,
    path: PathBuf,
}

fn start(base: &str, max: usize) -> Reader {
    let path = std::env::temp_dir().join(format!(
        "zf-http-{}-{}.json",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let sender = ServerSender::new(base.to_string(), "UPSTREAM-SECRET").unwrap();
    let shared = Arc::new(Mutex::new(Shared {
        upstream: Box::new(sender),
        limiter: RateLimiter::default(),
    }));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let serve_path = path.clone();
    std::thread::spawn(move || run(listener, serve_path, shared, max));
    Reader { addr, path }
}

fn add_client(reader: &Reader, name: &str, accounts: &[&str]) -> String {
    let mut registry = Registry::load(&reader.path).unwrap();
    let scopes = [
        "accounts:read",
        "chats:read",
        "messages:read",
        "search:read",
    ];
    let token = registry
        .add(
            name,
            scopes.iter().map(|s| s.to_string()).collect(),
            accounts.iter().map(|s| s.to_string()).collect(),
            None,
            0,
        )
        .unwrap();
    registry.save(&reader.path).unwrap();
    token
}

fn request(addr: &str, method: &str, target: &str, headers: &[&str]) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).unwrap();
    let mut text = format!("{method} {target} HTTP/1.1\r\nhost: x\r\n");
    for header in headers {
        text.push_str(header);
        text.push_str("\r\n");
    }
    text.push_str("\r\n");
    stream.write_all(text.as_bytes()).unwrap();
    let mut reply = String::new();
    let _ = stream.read_to_string(&mut reply);
    let status = reply
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = reply.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

fn get(reader: &Reader, token: &str, target: &str) -> (u16, String) {
    let auth = format!("Authorization: Bearer {token}");
    request(&reader.addr, "GET", target, &[&auth])
}

#[test]
fn auth_origin_method_and_upstream_credential_handling() {
    let (base, log) = mock_server(false);
    let reader = start(&base, 8);
    let token = add_client(&reader, "app", &[]);
    assert_eq!(request(&reader.addr, "GET", "/v1/chats", &[]).0, 401);
    assert_eq!(get(&reader, "zfr_wrong", "/v1/chats").0, 401);
    let auth = format!("Authorization: Bearer {token}");
    let origin = request(
        &reader.addr,
        "GET",
        "/v1/chats",
        &[&auth, "Origin: http://evil.example"],
    );
    assert_eq!(origin.0, 403);
    let referer = request(
        &reader.addr,
        "GET",
        "/v1/chats",
        &[&auth, "Referer: http://evil.example/"],
    );
    assert_eq!(referer.0, 403);
    assert_eq!(request(&reader.addr, "POST", "/v1/chats", &[&auth]).0, 405);
    assert_eq!(get(&reader, &token, "/v1/chats").0, 200);
    let seen = log.lock().unwrap().clone();
    assert_eq!(
        seen.len(),
        1,
        "only the one valid GET reached the Server: {seen:?}"
    );
    assert!(seen[0].starts_with("GET /v1/chats?"));
    assert!(seen[0].contains("UPSTREAM-SECRET"));
    assert!(!seen[0].contains(&token));
}

#[test]
fn guessed_ids_traversal_and_query_tricks_never_reach_the_server() {
    let (base, log) = mock_server(false);
    let reader = start(&base, 8);
    let token = add_client(&reader, "app", &["a1"]);
    for target in [
        "/v1/chats/%2e%2e/messages",
        "/v1/chats/a%2Fb/messages",
        "/v1/chats/c-a1%3Fx=1/messages?limit=1",
        "/v1/chats/c-a1/messages?limit=1&accountIDs=b2",
        "/v1/chats/c-a1/read",
    ] {
        let (status, _) = get(&reader, &token, target);
        assert!(status == 400 || status == 404, "{target} gave {status}");
    }
    assert_eq!(get(&reader, &token, "/v1/chats/c-b2/messages").0, 404);
    assert_eq!(get(&reader, &token, "/v1/chats/guess/messages").0, 404);
    let seen = log.lock().unwrap().join("\n");
    assert!(!seen.contains("c-b2/messages"), "{seen}");
    assert!(!seen.contains("/read"), "{seen}");
    assert!(!seen.contains("%2e") && !seen.contains(".."), "{seen}");
}

#[test]
fn restricted_client_sees_only_items_and_allowed_chat_messages() {
    let (base, _) = mock_server(false);
    let reader = start(&base, 8);
    let token = add_client(&reader, "app", &["a1"]);
    let (status, body) = get(&reader, &token, "/v1/accounts");
    assert_eq!((status, body.as_str()), (200, r#"[{"accountID":"a1"}]"#));
    assert_eq!(get(&reader, &token, "/v1/chats/c-a1/messages").0, 200);
}

#[test]
fn redirects_and_upstream_errors_are_not_followed_or_forwarded() {
    let (base, log) = mock_server(true);
    let reader = start(&base, 8);
    let token = add_client(&reader, "app", &[]);
    let (status, body) = get(&reader, &token, "/v1/accounts");
    assert_eq!(status, 502);
    assert!(!body.contains("elsewhere"));
    let (status, body) = get(&reader, &token, "/v1/chats/boom/messages");
    assert_eq!(status, 502);
    assert!(!body.contains("SECRET-DETAIL"));
    assert!(log.lock().unwrap().iter().all(|l| !l.contains("elsewhere")));
}

#[test]
fn revocation_is_immediate_and_rate_limit_applies() {
    let (base, _) = mock_server(false);
    let reader = start(&base, 8);
    let token = add_client(&reader, "app", &[]);
    assert_eq!(get(&reader, &token, "/v1/chats").0, 200);
    let mut registry = Registry::load(&reader.path).unwrap();
    assert!(registry.revoke("app"));
    registry.save(&reader.path).unwrap();
    assert_eq!(get(&reader, &token, "/v1/chats").0, 401);
    let other = add_client(&reader, "other", &[]);
    let mut last = 0;
    for _ in 0..125 {
        last = get(&reader, &other, "/v1/chats").0;
    }
    assert_eq!(last, 429);
}

#[test]
fn slow_connections_hold_only_their_slot() {
    let (base, _) = mock_server(false);
    let reader = start(&base, 2);
    let token = add_client(&reader, "app", &[]);
    let _idle1 = TcpStream::connect(&reader.addr).unwrap();
    let _idle2 = TcpStream::connect(&reader.addr).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(get(&reader, &token, "/v1/chats").0, 429);
}

#[test]
fn loose_registry_permissions_refuse_everything() {
    let (base, _) = mock_server(false);
    let reader = start(&base, 8);
    let token = add_client(&reader, "app", &[]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&reader.path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(get(&reader, &token, "/v1/chats").0, 401);
    }
    #[cfg(not(unix))]
    let _ = token;
}
