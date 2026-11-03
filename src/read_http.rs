//! Socket side of zapfast-reader: one connection per thread behind a
//! global cap, a shared upstream and rate limiter, and the checks that
//! must run before authentication (web-origin refusal, bounded head).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::read_api::{RateLimiter, Registry, Upstream, handle, hash_token};
use crate::scheduler_api::ServerSender;

/// Most connections served at once; extra ones are refused with 429.
pub const MAX_CONNECTIONS: usize = 32;
const MAX_REQUEST: usize = 16 * 1024;

impl Upstream for ServerSender {
    fn get(&mut self, target: &str) -> Result<(u16, String), String> {
        self.get_bounded(target)
    }
}

pub struct Shared {
    pub upstream: Box<dyn Upstream + Send>,
    pub limiter: RateLimiter,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn respond(stream: &mut TcpStream, status: u16, body: &str) {
    let text = match status {
        200 => "200 OK",
        400 => "400 Bad Request",
        401 => "401 Unauthorized",
        403 => "403 Forbidden",
        404 => "404 Not Found",
        405 => "405 Method Not Allowed",
        429 => "429 Too Many Requests",
        _ => "502 Bad Gateway",
    };
    let response = format!(
        "HTTP/1.1 {text}\r\ncontent-type: application/json\r\ncache-control: no-store\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            return Some(String::from_utf8_lossy(&buf).into_owned());
        }
        if buf.len() > MAX_REQUEST {
            return None;
        }
    }
}

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then_some(value.trim())
    })
}

pub fn serve_connection(stream: &mut TcpStream, path: &Path, shared: &Mutex<Shared>) {
    let Some(head) = read_head(stream) else {
        return respond(stream, 400, r#"{"error":"bad request"}"#);
    };
    if header(&head, "origin").is_some() || header(&head, "referer").is_some() {
        return respond(stream, 403, r#"{"error":"web-originated request refused"}"#);
    }
    let mut parts = head.lines().next().unwrap_or("").split_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
        return respond(stream, 400, r#"{"error":"bad request"}"#);
    };
    let Ok(registry) = Registry::load(path) else {
        eprintln!("zapfast-reader: client registry unusable, refusing all requests");
        return respond(stream, 401, r#"{"error":"unauthorized"}"#);
    };
    let bearer = header(&head, "authorization").and_then(|v| v.strip_prefix("Bearer "));
    let at = now();
    // Keyed by token hash, so unknown tokens cannot use a real client's
    // allowance or reach the Server.
    let key = bearer.map(hash_token).unwrap_or_default();
    let mut guard = shared.lock().unwrap_or_else(|e| e.into_inner());
    if !guard.limiter.allow(&key, at) {
        drop(guard);
        return respond(stream, 429, r#"{"error":"rate limited"}"#);
    }
    let (status, body, client, label) =
        handle(&registry, &mut *guard.upstream, bearer, method, target, at);
    drop(guard);
    eprintln!(
        "zapfast-reader: client={} route={label} status={status}",
        client.as_deref().unwrap_or("-")
    );
    respond(stream, status, &body);
}

/// Serves until the listener fails. Each connection gets a thread, so a
/// slow client holds only its own slot; the cap bounds the total.
pub fn run(listener: TcpListener, path: PathBuf, shared: Arc<Mutex<Shared>>, max: usize) {
    let active = Arc::new(AtomicUsize::new(0));
    for mut stream in listener.incoming().flatten() {
        if active.fetch_add(1, Ordering::SeqCst) >= max {
            active.fetch_sub(1, Ordering::SeqCst);
            respond(&mut stream, 429, r#"{"error":"too many connections"}"#);
            continue;
        }
        let path = path.clone();
        let shared = Arc::clone(&shared);
        let active = Arc::clone(&active);
        std::thread::spawn(move || {
            serve_connection(&mut stream, &path, &shared);
            active.fetch_sub(1, Ordering::SeqCst);
        });
    }
}
