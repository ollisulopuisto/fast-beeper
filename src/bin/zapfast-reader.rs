//! zapfast-reader: a read-only API for other local apps.
//!
//! Runs beside the Beeper Server and serves a small allowlist of GET
//! endpoints to client apps that each hold their own revocable token. See
//! contrib/read-api/README.md for the model and its limits.
//!
//! Serve (environment): BEEPER_BASE_URL, BEEPER_ACCESS_TOKEN or
//! BEEPER_ACCESS_TOKEN_FILE (the upstream token, never shown to clients),
//! ZAPFAST_SETTINGS_FILE (optional: the app's settings.json, read-only, for
//! linked people), ZAPFAST_READER_LISTEN (default 127.0.0.1:23377; non-loopback addresses
//! are refused), ZAPFAST_READER_CLIENTS (default
//! ~/.config/zapfast/reader-clients.json, mode 0600).
//!
//! Manage clients (never reads the upstream token):
//!   zapfast-reader token add NAME --scopes a,b [--accounts x,y] [--days N]
//!   zapfast-reader token list
//!   zapfast-reader token revoke NAME
//! `token add` prints the new token once, on the terminal. It is not
//! stored anywhere in clear.
//!
//! Endpoints (all GET): /v1/accounts, /v1/chats,
//! /v1/chats/{id}/messages, /v1/messages/search?query=, /v1/people and
//! /v1/people/{id}/messages (needs ZAPFAST_SETTINGS_FILE)
//!
//! Requests carrying Origin or Referer are refused, so web pages cannot
//! drive it. The log holds client name, endpoint family and status only:
//! never query strings, chat ids or message content.

use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use zapfast::read_api::{RateLimiter, Registry, SCOPES};
use zapfast::read_http::{MAX_CONNECTIONS, Shared, run};
use zapfast::scheduler_api::ServerSender;

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn registry_path() -> PathBuf {
    if let Ok(path) = std::env::var("ZAPFAST_READER_CLIENTS") {
        return PathBuf::from(path);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/zapfast/reader-clients.json")
}

fn token_command(args: &[String]) -> i32 {
    let path = registry_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _lock = match Registry::lock(&path) {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!("zapfast-reader: {e}");
            return 1;
        }
    };
    let mut registry = match Registry::load(&path) {
        Ok(registry) => registry,
        Err(e) => {
            eprintln!("zapfast-reader: {e}");
            return 1;
        }
    };
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let list = |v: Option<String>| -> Vec<String> {
        v.map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
            .unwrap_or_default()
    };
    match args.first().map(String::as_str) {
        Some("add") => {
            let Some(name) = args.get(1).filter(|n| !n.starts_with("--")) else {
                eprintln!("usage: token add NAME --scopes a,b [--accounts x,y] [--days N]");
                return 2;
            };
            let scopes = list(flag("--scopes"));
            let accounts = list(flag("--accounts"));
            let days = flag("--days").and_then(|d| d.parse::<i64>().ok());
            let expires = days.map(|d| now() + d.clamp(1, 3650) * 86400);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match registry.add(name, scopes, accounts, expires, now()) {
                Ok(token) => match registry.save(&path) {
                    Ok(()) => {
                        println!("client {name} created. Token (shown once):\n{token}");
                        0
                    }
                    Err(e) => {
                        eprintln!("zapfast-reader: {e}");
                        1
                    }
                },
                Err(e) => {
                    eprintln!("zapfast-reader: {e}. Scopes: {}", SCOPES.join(", "));
                    1
                }
            }
        }
        Some("list") => {
            for client in &registry.clients {
                let accounts = if client.accounts.is_empty() {
                    "all accounts".to_string()
                } else {
                    client.accounts.join(",")
                };
                println!(
                    "{}  scopes={}  {accounts}  expires={}",
                    client.name,
                    client.scopes.join(","),
                    client
                        .expires_at
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "never".into())
                );
            }
            0
        }
        Some("revoke") => {
            let Some(name) = args.get(1) else {
                eprintln!("usage: token revoke NAME");
                return 2;
            };
            if !registry.revoke(name) {
                eprintln!("zapfast-reader: no such client");
                return 1;
            }
            match registry.save(&path) {
                Ok(()) => {
                    println!("client {name} revoked");
                    0
                }
                Err(e) => {
                    eprintln!("zapfast-reader: {e}");
                    1
                }
            }
        }
        _ => {
            eprintln!("usage: zapfast-reader token add|list|revoke");
            2
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("token") {
        std::process::exit(token_command(&args[1..]));
    }
    let base_url =
        std::env::var("BEEPER_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:23374".into());
    let token = std::env::var("BEEPER_ACCESS_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .or_else(|| {
            std::env::var("BEEPER_ACCESS_TOKEN_FILE")
                .ok()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
        })
        .unwrap_or_default();
    let listen =
        std::env::var("ZAPFAST_READER_LISTEN").unwrap_or_else(|_| "127.0.0.1:23377".into());
    let addr: SocketAddr = match listen.parse() {
        Ok(addr) => addr,
        Err(_) => {
            eprintln!("zapfast-reader: listen address must be a literal IP:port");
            std::process::exit(1);
        }
    };
    if !addr.ip().is_loopback() {
        eprintln!("zapfast-reader: refusing a non-loopback listen address");
        std::process::exit(1);
    }
    let path = registry_path();
    if let Err(e) = Registry::load(&path) {
        eprintln!("zapfast-reader: {e}");
        std::process::exit(1);
    }
    let sender = match ServerSender::new(base_url, &token) {
        Ok(sender) => sender,
        Err(e) => {
            eprintln!("zapfast-reader: {e}");
            std::process::exit(1);
        }
    };
    let listener = match TcpListener::bind(addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("zapfast-reader: cannot bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("zapfast-reader: listening on {addr}");
    let shared = Arc::new(Shared {
        source: Box::new(sender),
        limiter: Mutex::new(RateLimiter::default()),
        settings: std::env::var("ZAPFAST_SETTINGS_FILE").ok().map(PathBuf::from),
    });
    run(listener, path, shared, MAX_CONNECTIONS);
}
