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
    ("/v1/chats/boom/messages", 500, "SECRET-DETAIL"),
    (
        "/v1/chats/%21AdaWa%3Abeeper.local",
        200,
        r#"{"id":"!AdaWa:beeper.local","accountID":"local-whatsapp_ba_Ada01","network":"WhatsApp","title":"Ada Lovelace","type":"single"}"#,
    ),
    (
        "/v1/chats/%21AdaSg%3Abeeper.local",
        200,
        r#"{"id":"!AdaSg:beeper.local","accountID":"local-signal_ba_Ada02","network":"Signal","title":"Ada Lovelace","type":"single"}"#,
    ),
    (
        "/v1/chats/%21AdaWa%3Abeeper.local/messages",
        200,
        r#"{"items":[{"id":"w2","chatID":"!AdaWa:beeper.local","accountID":"local-whatsapp_ba_Ada01","senderID":"ada","sortKey":"2","timestamp":"2026-11-03T12:00:00.000Z","text":"wa newest","isSender":false},{"id":"w1","chatID":"!AdaWa:beeper.local","accountID":"local-whatsapp_ba_Ada01","senderID":"ada","sortKey":"1","timestamp":"2026-11-03T10:00:00.000Z","text":"wa oldest","isSender":false}],"hasMore":true,"oldestCursor":"c1"}"#,
    ),
    (
        "/v1/chats/%21AdaSg%3Abeeper.local/messages",
        200,
        r#"{"items":[{"id":"s1","chatID":"!AdaSg:beeper.local","accountID":"local-signal_ba_Ada02","senderID":"ada","sortKey":"1","timestamp":"2026-11-03T11:00:00.000Z","text":"sg middle","isSender":false}]}"#,
    ),
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
    let settings = path.with_extension("settings");
    let sender = ServerSender::new(base.to_string(), "UPSTREAM-SECRET").unwrap();
    let shared = Arc::new(Shared {
        source: Box::new(sender),
        limiter: Mutex::new(RateLimiter::default()),
        settings: Some(settings.clone()),
        origins: vec!["http://127.0.0.1:23374".to_string()],
    });
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
        "people:read",
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

const SETTINGS: &str = r#"{"beeper":{"people":[
  {"name":"Ada Lovelace","members":[
    {"origin":"http://127.0.0.1:23374/","account":"a1","chat":"c-a1"},
    {"origin":"http://other-backend:8077","account":"d9","chat":"c-d9"}],
   "primary":{"origin":"http://127.0.0.1:23374/","account":"a1","chat":"c-a1"}},
  {"name":"Linked Across","members":[
    {"origin":"http://127.0.0.1:23374/","account":"a1","chat":"c-a1"},
    {"origin":"http://127.0.0.1:23374/","account":"b2","chat":"c-b2"}]}
],"local_ai_endpoint":"http://secret.example"}}"#;

fn people_ids(reader: &Reader, token: &str) -> (u16, serde_json::Value) {
    let (status, body) = get(reader, token, "/v1/people");
    (status, serde_json::from_str(&body).unwrap_or_default())
}

#[test]
fn people_are_listed_with_unreadable_members_marked_and_nothing_else_leaked() {
    let (base, _) = mock_server(false);
    let reader = start(&base, 8);
    let token = add_client(&reader, "app", &[]);
    assert_eq!(get(&reader, &token, "/v1/people").0, 502, "no settings yet");
    std::fs::write(reader.path.with_extension("settings"), SETTINGS).unwrap();
    let (status, list) = people_ids(&reader, &token);
    assert_eq!(status, 200);
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
    let text = list.to_string();
    assert!(!text.contains("secret.example"));
    let ada = &list["items"][0];
    assert_eq!(ada["members"][0]["readable"], true);
    assert_eq!(ada["members"][1]["readable"], false);
}

#[test]
fn merged_timeline_lists_what_it_could_not_read() {
    let (base, log) = mock_server(false);
    let reader = start(&base, 8);
    let token = add_client(&reader, "app", &[]);
    std::fs::write(reader.path.with_extension("settings"), SETTINGS).unwrap();
    let (_, list) = people_ids(&reader, &token);
    let id = list["items"][0]["id"].as_str().unwrap().to_string();
    let (status, body) = get(
        &reader,
        &token,
        &format!("/v1/people/{id}/messages?limit=5"),
    );
    assert_eq!(status, 200);
    let timeline: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(timeline["partial"], true);
    assert_eq!(timeline["unavailable"].as_array().unwrap().len(), 1);
    assert_eq!(timeline["items"][0]["chatID"], "c-a1");
    let seen = log.lock().unwrap().join("\n");
    assert!(
        !seen.contains("c-d9"),
        "app-only members never reach the Server"
    );
    assert_eq!(
        get(&reader, &token, "/v1/people/zzzzzzzzzzzzzzzz/messages").0,
        400
    );
    assert_eq!(
        get(&reader, &token, "/v1/people/0000000000000000/messages").0,
        404
    );
}

#[test]
fn restricted_clients_never_see_a_link_that_crosses_their_allowlist() {
    let (base, _) = mock_server(false);
    let reader = start(&base, 8);
    let token = add_client(&reader, "app", &["a1"]);
    std::fs::write(reader.path.with_extension("settings"), SETTINGS).unwrap();
    let (_, list) = people_ids(&reader, &token);
    assert_eq!(list["items"].as_array().unwrap().len(), 0);
    let all = add_client(&reader, "all", &[]);
    let (_, full) = people_ids(&reader, &all);
    let id = full["items"][1]["id"].as_str().unwrap();
    let target = format!("/v1/people/{id}/messages");
    assert_eq!(get(&reader, &token, &target).0, 404);
}

#[test]
fn people_scope_is_required() {
    let (base, _) = mock_server(false);
    let reader = start(&base, 8);
    let mut registry = Registry::load(&reader.path).unwrap();
    let token = registry
        .add("narrow", vec!["chats:read".into()], vec![], None, 0)
        .unwrap();
    registry.save(&reader.path).unwrap();
    assert_eq!(get(&reader, &token, "/v1/people").0, 403);
}

#[test]
fn a_restricted_client_with_every_member_allowed_sees_only_its_own_metadata() {
    let (base, log) = mock_server(false);
    let reader = start(&base, 8);
    let token = add_client(&reader, "app", &["a1", "d9"]);
    std::fs::write(reader.path.with_extension("settings"), SETTINGS).unwrap();
    let (_, list) = people_ids(&reader, &token);
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    let text = list.to_string();
    assert!(!text.contains("b2") && !text.contains("Linked Across"));
    let id = list["items"][0]["id"].as_str().unwrap();
    let (_, body) = get(&reader, &token, &format!("/v1/people/{id}/messages"));
    assert!(!body.contains("b2") && !body.contains("c-b2"));
    assert!(!log.lock().unwrap().join(" ").contains("c-b2"));
}

const REAL_SHAPE: &str = r#"{"beeper":{"people":[
  {"name":"Ada Lovelace","members":[
    {"origin":"http://127.0.0.1:23374","account":"local-whatsapp_ba_Ada01","chat":"!AdaWa:beeper.local"},
    {"origin":"http://127.0.0.1:23374","account":"local-signal_ba_Ada02","chat":"!AdaSg:beeper.local"}],
   "primary":{"origin":"http://127.0.0.1:23374","account":"local-whatsapp_ba_Ada01","chat":"!AdaWa:beeper.local"}}
]}}"#;

#[test]
fn merged_timeline_with_recorded_origin_schema_orders_across_members() {
    let (base, _) = mock_server(false);
    let reader = start(&base, 8);
    let token = add_client(&reader, "app", &[]);
    std::fs::write(reader.path.with_extension("settings"), REAL_SHAPE).unwrap();
    let (_, list) = people_ids(&reader, &token);
    let ada = &list["items"][0];
    assert_eq!(ada["members"][0]["readable"], true);
    assert_eq!(ada["members"][1]["readable"], true);
    let id = ada["id"].as_str().unwrap();
    let (status, body) = get(
        &reader,
        &token,
        &format!("/v1/people/{id}/messages?limit=2"),
    );
    assert_eq!(status, 200);
    let timeline: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(timeline["partial"], false);
    let texts: Vec<&str> = timeline["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["wa newest", "sg middle"]);
    assert_eq!(timeline["items"][1]["accountID"], "local-signal_ba_Ada02");
    assert!(
        timeline.get("hasMore").is_none(),
        "no upstream paging leaks"
    );
}

#[test]
fn an_allowlist_matching_the_recorded_accounts_sees_the_realistic_person() {
    let (base, _) = mock_server(false);
    let reader = start(&base, 8);
    let both = add_client(
        &reader,
        "both",
        &["local-whatsapp_ba_Ada01", "local-signal_ba_Ada02"],
    );
    let one = add_client(&reader, "one", &["local-whatsapp_ba_Ada01"]);
    std::fs::write(reader.path.with_extension("settings"), REAL_SHAPE).unwrap();
    assert_eq!(
        people_ids(&reader, &both).1["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        people_ids(&reader, &one).1["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}
