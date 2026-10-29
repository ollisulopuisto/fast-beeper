//! Local Server sign-in. No CLI subprocesses, registration, secret reset or network linking.
use serde_json::{Value, json};
use std::sync::mpsc::{self, Receiver};
use zeroize::{Zeroize, Zeroizing};

#[derive(Default)]
pub struct SignIn {
    pub open: bool,
    pub endpoint: String,
    pub email: String,
    pub code: String,
    pub recovery: String,
    pub message: String,
    pub session: Value,
    request: String,
    token: Zeroizing<String>,
    pending: Option<Receiver<Result<Reply, String>>>,
    pub connect: Option<crate::backend::beeper::BeeperConfig>,
}
struct Reply {
    session: Value,
    request: Option<String>,
    token: Option<Zeroizing<String>>,
    warning: Option<String>,
}
enum Step {
    Check,
    Save,
    Email(String),
    Code(String, String),
    Recovery(String),
    Verify,
    Action(String, String),
}

fn local_root(raw: &str) -> Result<String, String> {
    let u = reqwest::Url::parse(raw).map_err(|_| "Invalid local Server address")?;
    if !matches!(
        u.host_str(),
        Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
    ) || !matches!(u.scheme(), "http" | "https")
        || !u.username().is_empty()
        || u.password().is_some()
        || !matches!(u.path(), "" | "/")
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err("Sign-in is limited to an installed Server on this computer. Use its loopback root URL.".into());
    }
    Ok(u.as_str().trim_end_matches('/').into())
}
fn credential(root: &str) -> Result<keyring_core::Entry, String> {
    crate::archive::encryption::platform_store()
        .map_err(|_| "Unlock the OS keyring and retry")?
        .build("rocks.zapfast.BeeperServer", root, None)
        .map_err(|_| "Could not open the OS keyring".into())
}
pub(crate) fn stored(root: &str) -> Result<Zeroizing<String>, String> {
    match credential(root)?.get_secret() {
        Ok(mut bytes) => {
            let result = String::from_utf8(bytes.clone())
                .map(Zeroizing::new)
                .map_err(|_| "Invalid saved Server credential".into());
            bytes.zeroize();
            result
        }
        Err(keyring_core::Error::NoEntry) => Ok(Zeroizing::new(String::new())),
        Err(_) => Err("Unlock the OS keyring and retry. No plaintext fallback is used.".into()),
    }
}
struct Wire {
    root: String,
    http: reqwest::blocking::Client,
    token: Zeroizing<String>,
}
impl Wire {
    fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        use std::io::Read;
        let mut r = self.http.request(method, format!("{}{path}", self.root));
        if !self.token.is_empty() {
            r = r.bearer_auth(self.token.as_str());
        }
        if let Some(body) = body {
            r = r.json(&body);
        }
        let mut response = r.send().map_err(
            |_| "Could not reach local Beeper Server. Keep it running and check the address.",
        )?;
        if !response.status().is_success() {
            return Err(format!(
                "Server returned HTTP {}. A 401/403 means authentication was rejected, not that Server is unreachable. No reinstall, token fallback or account reset was attempted. No response body was logged.",
                response.status().as_u16()
            ));
        }
        let mut bytes = Zeroizing::new(Vec::new());
        response
            .by_ref()
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Could not read Server response")?;
        if bytes.len() > 1024 * 1024 {
            return Err("Server setup response is too large".into());
        }
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| "Server returned an invalid setup response".into())
    }
}
fn perform(root: String, token: Zeroizing<String>, step: Step) -> Result<Reply, String> {
    let (get, post) = (reqwest::Method::GET, reqwest::Method::POST);
    let root = local_root(&root)?;
    let mut warning = None;
    let token = if token.is_empty() && matches!(step, Step::Check) {
        match stored(&root) {
            Ok(token) => token,
            Err(error) => {
                warning = Some(error);
                Zeroizing::new(String::new())
            }
        }
    } else {
        token
    };
    // Public email setup must not inherit a rejected previous bearer token.
    let token = if matches!(step, Step::Email(_) | Step::Code(_, _)) {
        Zeroizing::new(String::new())
    } else {
        token
    };
    let mut wire = Wire {
        root,
        token,
        http: reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|_| "Could not create local Server connection")?,
    };
    let mut request = None;
    let data = match step {
        Step::Save => {
            credential(&wire.root)?
                .set_secret(wire.token.as_bytes())
                .map_err(|_| "Unlock the OS keyring and retry saving. No plaintext fallback.")?;
            wire.call(get, "/v1/app/setup", None)?
        }
        Step::Check => wire.call(get, "/v1/app/setup", None)?,
        Step::Email(email) => {
            let started = wire.call(post.clone(), "/v1/app/setup/start", None)?;
            let id = started["setupRequestID"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("Server did not return a setup request")?
                .to_owned();
            wire.call(
                post.clone(),
                "/v1/app/setup/email",
                Some(json!({"setupRequestID":id,"email":email})),
            )?;
            request = Some(id);
            json!({"state":"email-code"})
        }
        Step::Code(id, mut code) => {
            let result = wire.call(
                post.clone(),
                "/v1/app/setup/response",
                Some(json!({"setupRequestID":id,"response":code})),
            );
            code.zeroize();
            let mut data = result?;
            if let Some(Value::String(token)) = data
                .get_mut("matrix")
                .and_then(|m| m.get_mut("accessToken"))
                .map(Value::take)
            {
                wire.token = Zeroizing::new(token);
                if credential(&wire.root)
                    .and_then(|entry| {
                        entry
                            .set_secret(wire.token.as_bytes())
                            .map_err(|_| "keyring write failed".into())
                    })
                    .is_err()
                {
                    warning = Some("Signed in for this session, but the OS keyring could not save it. Unlock the keyring and choose Save credential before closing. No plaintext token was saved.".into());
                }
            } else {
                return Err("No existing account session was returned. Account creation is not supported here; no account was created.".into());
            }
            data
        }
        Step::Recovery(mut key) => {
            let result = wire.call(
                post.clone(),
                "/v1/app/setup/verification/recovery-key",
                Some(json!({"recoveryKey":key})),
            );
            key.zeroize();
            result?
        }
        Step::Verify => wire.call(post.clone(), "/v1/app/setup/verifications", Some(json!({})))?,
        Step::Action(id, action) => {
            if !matches!(
                action.as_str(),
                "accept" | "cancel" | "sas/start" | "sas/confirm"
            ) {
                return Err("Unsupported verification action".into());
            }
            let id: String = id.bytes().map(|b| format!("%{b:02X}")).collect();
            wire.call(
                post.clone(),
                &format!("/v1/app/setup/verifications/{id}/{action}"),
                Some(json!({})),
            )?
        }
    };
    let session = data.get("session").unwrap_or(&data).clone();
    Ok(Reply {
        session,
        request,
        token: (!wire.token.is_empty()).then_some(wire.token),
        warning,
    })
}
impl SignIn {
    pub fn startup() -> Self {
        let mut s = Self::default();
        s.open = std::env::var("BEEPER_SIGN_IN").is_ok_and(|s| s == "1");
        s
    }
    fn launch(&mut self, step: Step, ctx: &egui::Context) {
        if self.pending.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        let root = self.endpoint.clone();
        let token = self.token.clone();
        let ctx = ctx.clone();
        self.message.clear();
        std::thread::spawn(move || {
            let _ = tx.send(perform(root, token, step));
            ctx.request_repaint();
        });
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        if self.token.is_empty()
            && self.endpoint.is_empty()
            && let Some(cfg) = crate::backend::beeper::BeeperConfig::from_env()
        {
            self.token = Zeroizing::new(cfg.token);
        }
        if self.endpoint.is_empty() {
            self.endpoint = crate::backend::beeper::BeeperConfig::from_env()
                .map(|c| c.base_url)
                .unwrap_or_else(|| "http://127.0.0.1:23374".into());
        }
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    match result {
                        Ok(reply) => {
                            self.message = reply.warning.unwrap_or_default();
                            self.session = reply.session;
                            if let Some(id) = reply.request {
                                self.request = id;
                            }
                            if let Some(token) = reply.token {
                                self.token = token;
                            }
                        }
                        Err(error) => self.message = error,
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.message = "Setup stopped. Retry the current step.".into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut open = true;
        egui::Window::new("Sign in to local Beeper Server").open(&mut open).collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_width(500.0).default_height(600.0).max_height((ctx.content_rect().height() - 70.0).max(240.0)).vscroll(true).resizable(false).show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 12.0);
            ui.style_mut().text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
            ui.style_mut().text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
            ui.style_mut().text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(16.0));
            ui.visuals_mut().override_text_color = Some(if ui.visuals().dark_mode { egui::Color32::from_gray(235) } else { egui::Color32::from_gray(25) });
            ui.label("Use your existing Beeper account. Server must already be running. No Desktop session is copied and no networks are linked.");
            let ctx = ui.ctx().clone();
            ui.add_enabled_ui(self.pending.is_none(), |ui| {
                ui.label("Local Server address");
                // Freeze destination after requesting a code: credentials must never switch servers mid-flow.
                ui.add_enabled(self.request.is_empty() && self.token.is_empty(), egui::TextEdit::singleline(&mut self.endpoint));
                if !self.token.is_empty() && ui.button("Save credential in OS keyring").clicked() { self.launch(Step::Save, &ctx); }
                if ui.button("Check existing session / refresh").clicked() { self.launch(Step::Check, &ctx); }
                let state = self.session["state"].as_str().unwrap_or("").to_owned();
                ui.label(format!("Setup: {}", if state.is_empty() { "not checked" } else { &state }));
                if state == "needs-login" {
                    ui.label("Existing Beeper email"); ui.text_edit_singleline(&mut self.email);
                    if ui.add_enabled(self.email.contains('@'), egui::Button::new("Send sign-in code")).clicked() { self.launch(Step::Email(self.email.trim().into()), &ctx); }
                }
                if !self.request.is_empty() && (state == "email-code" || state == "needs-login") {
                    ui.label("Email code (kept only in this app)"); ui.add(egui::TextEdit::singleline(&mut self.code).password(true));
                    if ui.add_enabled(!self.code.trim().is_empty(), egui::Button::new("Sign in")).clicked() {
                        let code = std::mem::take(&mut self.code); self.launch(Step::Code(self.request.clone(), code), &ctx);
                    }
                }
                if matches!(state.as_str(), "needs-verification" | "needs-secrets") {
                    ui.label("Verify on another signed-in device, or enter your existing recovery key. This app never resets it.");
                    if ui.add_enabled(self.session["verification"].is_null(), egui::Button::new("Request device verification")).clicked() { self.launch(Step::Verify, &ctx); }
                    ui.add(egui::TextEdit::singleline(&mut self.recovery).password(true));
                    if ui.add_enabled(!self.recovery.trim().is_empty(), egui::Button::new("Unlock with recovery key")).clicked() {
                        let key = std::mem::take(&mut self.recovery); self.launch(Step::Recovery(key), &ctx);
                    }
                }
                let v = self.session["verification"].clone();
                if let Some(state) = v["state"].as_str() {
                    ui.label(format!("Device verification: {state}"));
                    if state == "requested" { ui.label("Accept the incoming request in your already-verified Desktop or mobile Beeper app, then refresh here. Keep this request; no need to start again."); }
                }
                if let Some(actions) = v["availableActions"].as_array() {
                    for (action, label, path) in [("accept", "Accept device verification", "accept"), ("sas.start", "Compare emoji / numbers", "sas/start"), ("sas.confirm", "They match on both devices", "sas/confirm"), ("cancel", "Cancel verification", "cancel")] {
                        if actions.iter().any(|a| a.as_str() == Some(action)) {
                            let sas = v["sas"]["emojis"].as_str().or(v["sas"]["decimals"].as_str()).unwrap_or("");
                            if action == "sas.confirm" { ui.label("Compare on your other trusted device. Confirm only if both match:"); ui.label(sas); }
                            if ui.add_enabled(action != "sas.confirm" || !sas.is_empty(), egui::Button::new(label)).clicked()
                                && let Some(id) = v["id"].as_str() { self.launch(Step::Action(id.into(), path.into()), &ctx); }
                        }
                    }
                }
                if matches!(state.as_str(), "initializing" | "needs-first-sync") { ui.label("Server is syncing. Refresh when ready; this does not imply your other networks are connected."); }
                if state == "needs-cross-signing-setup" { ui.label("This account requires recovery setup. Automatic key creation/reset is intentionally unavailable; finish recovery setup in an official Beeper client."); }
                if state == "ready" && !self.token.is_empty() && ui.button("Connect fast-beeper").clicked() {
                    self.connect = Some(crate::backend::beeper::BeeperConfig { base_url: self.endpoint.clone(), token: self.token.to_string() });
                    self.open = false;
                }
                if state == "ready" && self.token.is_empty() { ui.label("Server is signed in but this app has no saved credential. Use your existing authenticated launch configuration; no Desktop token is copied."); }
            });
            if self.pending.is_some() { ui.spinner(); ui.label("Waiting for local Server..."); }
            if !self.message.is_empty() && self.pending.is_none() && ui.button("Sign in again to this Server (no account reset)").clicked() {
                self.session = json!({"state":"needs-login"}); self.request.clear(); self.token.zeroize(); self.code.zeroize(); self.recovery.zeroize(); self.message.clear();
            }
            if !self.message.is_empty() { ui.colored_label(egui::Color32::LIGHT_RED, &self.message); }
        });
        if !open {
            self.open = false;
            self.code.zeroize();
            self.recovery.zeroize();
        }
    }
}
impl Drop for SignIn {
    fn drop(&mut self) {
        self.code.zeroize();
        self.recovery.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_only() {
        for url in [
            "http://127.0.0.1:23374",
            "http://localhost:23374/",
            "https://[::1]:23374",
        ] {
            assert!(local_root(url).is_ok(), "{url}");
        }
        for url in [
            "https://example.com",
            "http://localhost.evil.test",
            "http://user:pass@localhost",
            "http://localhost/v1",
            "http://localhost?token=x",
            "file:///tmp/server",
        ] {
            assert!(local_root(url).is_err(), "{url}");
        }
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn fixture(
        status: &str,
        body: &str,
        header: Option<&str>,
    ) -> (Wire, std::thread::JoinHandle<String>) {
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        let root = format!("http://{}", socket.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{body}",
            body.len(),
            header.unwrap_or("")
        );
        let join = std::thread::spawn(move || {
            let (mut stream, _) = socket.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut bytes = vec![0; 16384];
            let n = stream.read(&mut bytes).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&bytes[..n]).to_string()
        });
        (
            Wire {
                root,
                http: reqwest::blocking::Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .unwrap(),
                token: Zeroizing::new("synthetic-token".into()),
            },
            join,
        )
    }
    #[test]
    fn exact_origin_and_path() {
        let (w, j) = fixture("200 OK", "{\"state\":\"needs-verification\"}", None);
        assert_eq!(
            w.call(reqwest::Method::GET, "/v1/app/setup", None).unwrap()["state"],
            "needs-verification"
        );
        let request = j.join().unwrap().to_lowercase();
        assert!(request.starts_with("get /v1/app/setup "));
        assert!(request.contains("authorization: bearer synthetic-token"));
    }
    #[test]
    fn auth_failure_does_not_echo_private_body() {
        let (w, j) = fixture("401 Unauthorized", "private-token-secret-code", None);
        let error = w
            .call(reqwest::Method::GET, "/v1/app/setup", None)
            .unwrap_err();
        assert!(error.contains("401"));
        assert!(!error.contains("private-token-secret-code"));
        j.join().unwrap();
    }
    #[test]
    fn redirects_are_not_followed() {
        let (w, j) = fixture(
            "302 Found",
            "",
            Some("Location: http://127.0.0.1:9/private\r\n"),
        );
        assert!(
            w.call(reqwest::Method::GET, "/v1/app/setup", None)
                .unwrap_err()
                .contains("302")
        );
        j.join().unwrap();
    }
    #[test]
    fn empty_email_response_supported() {
        let (w, j) = fixture("204 No Content", "", None);
        assert!(
            w.call(
                reqwest::Method::POST,
                "/v1/app/setup/email",
                Some(json!({"email":"test@example.invalid","setupRequestID":"fixture"}))
            )
            .unwrap()
            .is_null()
        );
        j.join().unwrap();
    }
    #[test]
    fn malformed_json_not_echoed() {
        let (w, j) = fixture("200 OK", "private-not-json", None);
        assert_eq!(
            w.call(reqwest::Method::GET, "/v1/app/setup", None)
                .unwrap_err(),
            "Server returned an invalid setup response"
        );
        j.join().unwrap();
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    #[test]
    fn renders_login_code_verification_sync_and_ready() {
        let ctx = egui::Context::default();
        for state in [
            "needs-login",
            "email-code",
            "needs-verification",
            "needs-secrets",
            "initializing",
            "needs-first-sync",
            "needs-cross-signing-setup",
            "ready",
        ] {
            let mut signin = SignIn::default();
            signin.open = true;
            signin.endpoint = "http://127.0.0.1:23374".into();
            signin.request = "synthetic".into();
            signin.token = Zeroizing::new("synthetic-token".into());
            signin.session = json!({"state":state,"verification":{"id":"fixture", "availableActions":["sas.confirm","cancel"],"sas":{"decimals":"123 456"}}});
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| signin.show(ui.ctx()));
            assert!(!output.shapes.is_empty(), "{state}");
            output.textures_delta.clear();
            assert!(signin.pending.is_none());
            assert!(signin.connect.is_none());
        }
    }
}

#[cfg(test)]
mod flow_tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn server(replies: Vec<&'static str>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        let job = std::thread::spawn(move || {
            replies
                .into_iter()
                .map(|reply| {
                    let (mut s, _) = listener.accept().unwrap();
                    s.set_read_timeout(Some(std::time::Duration::from_secs(3)))
                        .unwrap();
                    let mut data = Vec::new();
                    let mut buffer = [0; 4096];
                    loop {
                        let n = s.read(&mut buffer).unwrap();
                        if n == 0 {
                            break;
                        }
                        data.extend_from_slice(&buffer[..n]);
                        if let Some(pos) = data.windows(4).position(|x| x == b"\r\n\r\n") {
                            let header = String::from_utf8_lossy(&data[..pos]).to_lowercase();
                            let length = header
                                .lines()
                                .find_map(|x| x.strip_prefix("content-length: "))
                                .and_then(|x| x.parse::<usize>().ok())
                                .unwrap_or(0);
                            if data.len() >= pos + 4 + length {
                                break;
                            }
                        }
                    }
                    write!(
                        s,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        reply.len(),
                        reply
                    )
                    .unwrap();
                    String::from_utf8(data).unwrap()
                })
                .collect()
        });
        (root, job)
    }
    #[test]
    fn email_setup_uses_same_request_and_drops_stale_bearer() {
        let (root, job) = server(vec![r#"{"setupRequestID":"fixture-request"}"#, "null"]);
        let reply = perform(
            root,
            Zeroizing::new("stale-other-target-token".into()),
            Step::Email("synthetic@example.invalid".into()),
        )
        .unwrap();
        assert_eq!(reply.request.as_deref(), Some("fixture-request"));
        assert_eq!(reply.session["state"], "email-code");
        let calls = job.join().unwrap();
        assert!(calls[0].starts_with("POST /v1/app/setup/start "));
        assert!(calls[1].starts_with("POST /v1/app/setup/email "));
        assert!(calls[1].contains("fixture-request"));
        assert!(
            calls
                .iter()
                .all(|r| !r.contains("stale-other-target-token"))
        );
    }
    #[test]
    fn registration_response_never_creates_account() {
        let (root, job) = server(vec![
            r#"{"registrationRequired":true,"leadToken":"synthetic"}"#,
        ]);
        let result = perform(
            root,
            Zeroizing::new(String::new()),
            Step::Code("fixture".into(), "synthetic-code".into()),
        );
        assert!(
            result
                .err()
                .unwrap()
                .contains("Account creation is not supported")
        );
        let calls = job.join().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].starts_with("POST /v1/app/setup/response "));
    }
    #[test]
    fn recovery_stays_on_bound_origin() {
        let (root, job) = server(vec![r#"{"session":{"state":"ready"}}"#]);
        let reply = perform(
            root,
            Zeroizing::new("synthetic-token".into()),
            Step::Recovery("synthetic-key".into()),
        )
        .unwrap();
        assert_eq!(reply.session["state"], "ready");
        let calls = job.join().unwrap();
        assert!(calls[0].starts_with("POST /v1/app/setup/verification/recovery-key "));
        assert!(calls[0].contains("synthetic-key"));
    }
}
