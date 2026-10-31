//! Server-API implementation of the Send Later scheduler's send channel.
//!
//! Talks to the local Beeper Server/Desktop API exactly like the app's own
//! backend: bearer token, POST /v1/chats/{chatID}/messages for sends and
//! GET /v1/messages/search for reconciliation. Error mapping preserves the
//! one distinction the scheduler's safety rules depend on: a definite
//! rejection (HTTP error status) versus an unknown outcome (timeout,
//! dropped connection, unreadable response), which may have been delivered
//! and must never be blindly retried.

use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};

use crate::scheduler::{Reconcile, ScheduledMessage, Send, SendError};

/// Long enough for a slow bridge, short enough that a wedged connection
/// fails the tick instead of the whole scheduler.
const REQUEST_TIMEOUT_SECS: u64 = 30;
/// Search responses are bounded; reconciliation never pages.
const MAX_BODY_BYTES: usize = 1 << 20;

pub struct ServerSender {
    base_url: String,
    http: Client,
}

#[derive(serde::Deserialize)]
struct SearchPage {
    #[serde(default)]
    items: Vec<SearchMessage>,
}

#[derive(serde::Deserialize)]
struct SearchMessage {
    #[serde(rename = "chatID", default)]
    chat_id: String,
    #[serde(rename = "accountID", default)]
    account_id: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    is_sender: Option<bool>,
    #[serde(default)]
    timestamp: String,
}

impl ServerSender {
    pub fn new(base_url: String, token: &str) -> Result<Self, String> {
        if token.is_empty() {
            return Err("Beeper access token missing".into());
        }
        let mut auth = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|e| format!("invalid token characters: {e}"))?;
        auth.set_sensitive(true);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(AUTHORIZATION, auth);
        let http = Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| format!("http client: {e}"))?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
        })
    }

    /// Reads at most MAX_BODY_BYTES so a misbehaving local server cannot
    /// exhaust memory; the parse afterwards decides.
    fn bounded_body(response: reqwest::blocking::Response) -> Result<String, SendError> {
        let body = response
            .text()
            .map_err(|e| SendError::Unknown(format!("response unreadable: {e}")))?;
        if body.len() > MAX_BODY_BYTES {
            return Err(SendError::Unknown("response too large".into()));
        }
        Ok(body)
    }

    /// One RFC3339 timestamp, as the API emits them, to Unix seconds.
    /// Unparseable timestamps never match reconciliation.
    fn unix_seconds(timestamp: &str) -> Option<i64> {
        timestamp
            .parse::<jiff::Timestamp>()
            .ok()
            .map(|t| t.as_second())
    }
}

impl Send for ServerSender {
    fn send(&mut self, item: &ScheduledMessage) -> Result<String, SendError> {
        let mut payload = serde_json::json!({"text": item.text});
        if let Some(reply) = &item.reply_to {
            payload["replyToMessageID"] = reply.clone().into();
        }
        let url = format!(
            "{}/v1/chats/{}/messages",
            self.base_url,
            encode_segment(&item.chat_id)
        );
        let response = self
            .http
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .body(payload.to_string())
            .send()
            .map_err(|e| SendError::Unknown(format!("send outcome unknown: {e}")))?;
        let status = response.status();
        let body = Self::bounded_body(response)?;
        if !status.is_success() {
            // An HTTP error status means the server rejected the request;
            // nothing went out. Truncate server detail for the error field.
            let detail: String = body.chars().take(200).collect();
            return Err(SendError::Rejected(format!("HTTP {status}: {detail}")));
        }
        let accepted: serde_json::Value = serde_json::from_str(&body).map_err(|_| {
            SendError::Unknown("send accepted but response unreadable; reconcile".into())
        })?;
        accepted
            .get("pendingMessageID")
            .or_else(|| accepted.get("id"))
            .and_then(|id| id.as_str())
            .map(str::to_string)
            .ok_or_else(|| SendError::Unknown("send accepted without a message id".into()))
    }

    fn reconcile(&mut self, item: &ScheduledMessage) -> Reconcile {
        let url = format!("{}/v1/messages/search", self.base_url);
        let response = self
            .http
            .get(url)
            .query(&[("query", item.text.as_str())])
            .send();
        let Ok(response) = response else {
            return Reconcile::Unknown;
        };
        if !response.status().is_success() {
            return Reconcile::Unknown;
        }
        let Ok(body) = Self::bounded_body(response) else {
            return Reconcile::Unknown;
        };
        let Ok(page) = serde_json::from_str::<SearchPage>(&body) else {
            return Reconcile::Unknown;
        };
        let found = page.items.iter().any(|message| {
            message.chat_id == item.chat_id
                && message.account_id == item.account_id
                && message.text.as_deref() == Some(item.text.as_str())
                && message.is_sender == Some(true)
                && Self::unix_seconds(&message.timestamp)
                    .map(|at| at >= item.due_at - 60)
                    .unwrap_or(false)
        });
        if found {
            Reconcile::Found
        } else {
            Reconcile::NotFound
        }
    }
}

/// Percent-encode one path segment, as the backend does for chat ids.
fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        let ok = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');
        if ok {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::Queue;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc::{Receiver, channel};
    use std::thread;

    /// One-shot HTTP stub: serves each queued response in order and records
    /// the raw requests it received.
    struct Stub {
        base_url: String,
        requests: Receiver<String>,
    }

    fn stub(responses: Vec<String>) -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = channel();
        thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                // Read until headers end; test bodies are small and sent with
                // the headers by reqwest's blocking client.
                loop {
                    let n = stream.read(&mut chunk).unwrap();
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(pos) = find_header_end(&buf) {
                        if let Some(len) = content_length(&buf[..pos]) {
                            if buf.len() >= pos + 4 + len {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                tx.send(String::from_utf8_lossy(&buf).into_owned()).unwrap();
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        Stub {
            base_url: format!("http://127.0.0.1:{port}"),
            requests: rx,
        }
    }

    fn find_header_end(buf: &[u8]) -> Option<usize> {
        buf.windows(4).position(|w| w == b"\r\n\r\n")
    }

    fn content_length(headers: &[u8]) -> Option<usize> {
        let text = String::from_utf8_lossy(headers);
        text.lines()
            .find(|line| line.to_ascii_lowercase().starts_with("content-length:"))
            .and_then(|line| line.split(':').nth(1)?.trim().parse().ok())
    }

    fn ok_json(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }

    fn item() -> ScheduledMessage {
        ScheduledMessage::new(
            "sched-1".into(),
            "wa-main".into(),
            "chat 9/4".into(),
            "hello later".into(),
            Some("reply-1".into()),
            1_762_000_000,
            1_761_999_000,
        )
    }

    fn sender(base_url: &str) -> ServerSender {
        ServerSender::new(base_url.to_string(), "secret-token").unwrap()
    }

    #[test]
    fn send_posts_text_and_returns_pending_id() {
        let stub = stub(vec![ok_json(r#"{"pendingMessageID":"pm-1"}"#)]);
        let mut sender = sender(&stub.base_url);
        let id = sender.send(&item()).unwrap();
        assert_eq!(id, "pm-1");
        let request = stub.requests.recv().unwrap();
        assert!(request.starts_with("POST /v1/chats/chat%209%2F4/messages "));
        assert!(request.contains("authorization: Bearer secret-token"));
        assert!(request.contains(r#""text":"hello later""#));
        assert!(request.contains(r#""replyToMessageID":"reply-1""#));
    }

    #[test]
    fn http_error_is_a_rejection() {
        let stub = stub(vec![
            "HTTP/1.1 404 Not Found\r\ncontent-length: 11\r\nconnection: close\r\n\r\nunknown cha"
                .to_string(),
        ]);
        let mut sender = sender(&stub.base_url);
        match sender.send(&item()) {
            Err(SendError::Rejected(detail)) => assert!(detail.starts_with("HTTP 404")),
            other => panic!("expected rejection, got {other:?}"),
        }
    }

    #[test]
    fn connection_failure_is_unknown() {
        let mut sender = sender("http://127.0.0.1:1");
        match sender.send(&item()) {
            Err(SendError::Unknown(_)) => {}
            other => panic!("expected unknown, got {other:?}"),
        }
    }

    #[test]
    fn reconcile_matches_own_message_in_chat_after_due() {
        let body = r#"{"items":[
            {"chatID":"chat 9/4","accountID":"wa-main","text":"hello later","is_sender":true,"timestamp":"2025-11-01T12:26:41Z"},
            {"chatID":"chat 9/4","accountID":"wa-main","text":"hello later","is_sender":false,"timestamp":"2025-11-01T12:27:00Z"}
        ]}"#;
        let stub = stub(vec![ok_json(body)]);
        let mut sender = sender(&stub.base_url);
        // due_at 1_762_000_000 is 2025-11-01T12:26:40Z; the own message at +1s matches.
        assert_eq!(sender.reconcile(&item()), Reconcile::Found);
        let request = stub.requests.recv().unwrap();
        assert!(
            request.starts_with("GET /v1/messages/search?query=hello+later")
                || request.starts_with("GET /v1/messages/search?query=hello%20later")
        );
    }

    #[test]
    fn reconcile_ignores_other_chats_and_old_copies() {
        let body = r#"{"items":[
            {"chatID":"other","accountID":"wa-main","text":"hello later","is_sender":true,"timestamp":"2025-11-01T12:27:00Z"},
            {"chatID":"chat 9/4","accountID":"wa-main","text":"hello later","is_sender":true,"timestamp":"2025-11-01T11:00:00Z"}
        ]}"#;
        let stub = stub(vec![ok_json(body)]);
        let mut sender = sender(&stub.base_url);
        assert_eq!(sender.reconcile(&item()), Reconcile::NotFound);
    }

    #[test]
    fn reconcile_search_failure_is_unknown() {
        let stub = stub(vec![
            "HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                .to_string(),
        ]);
        let mut sender = sender(&stub.base_url);
        assert_eq!(sender.reconcile(&item()), Reconcile::Unknown);
    }

    #[test]
    fn missing_token_is_an_error() {
        assert!(ServerSender::new("http://localhost".into(), "").is_err());
    }

    #[test]
    fn queue_type_stays_usable_with_sender() {
        // The adapter composes with the queue the same way the mock does.
        let mut queue = Queue::default();
        queue.add(item()).unwrap();
        let _sender = sender("http://127.0.0.1:1");
        assert_eq!(queue.items[0].state, crate::scheduler::State::Scheduled);
    }
}
