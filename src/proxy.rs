//! Proxy support for the WhatsApp connection, media, and update checks.
//!
//! The proxy comes from Settings, or from `ALL_PROXY` / `HTTPS_PROXY` when
//! the setting is empty. `http://`, `socks5://`, and `socks5h://` proxies
//! are supported, with an optional `user:password@`.

use std::sync::{OnceLock, RwLock};
use std::time::Duration;


/// Time allowed to reach the proxy and open the tunnel.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Longest proxy reply header accepted for an HTTP tunnel.
const MAX_REPLY: usize = 8 * 1024;
const ENV_VARS: [&str; 4] = ["ALL_PROXY", "all_proxy", "HTTPS_PROXY", "https_proxy"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scheme {
    Http,
    /// SOCKS5 with names resolved on this computer.
    Socks5,
    /// SOCKS5 with names resolved by the proxy.
    Socks5h,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Proxy {
    scheme: Scheme,
    host: String,
    port: u16,
    auth: Option<(String, String)>,
    /// Normalized URL for ureq and reqwest.
    url: String,
}

impl std::fmt::Debug for Proxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.redacted())
    }
}

impl Proxy {
    /// Parses a proxy URL. A bare `host:port` is an HTTP proxy.
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        let with_scheme = if value.contains("://") {
            value.to_owned()
        } else {
            format!("http://{value}")
        };
        let url = reqwest::Url::parse(&with_scheme)
            .map_err(|_| "Enter a proxy address like socks5://127.0.0.1:1080".to_owned())?;
        let scheme = match url.scheme() {
            "http" => Scheme::Http,
            "socks5" | "socks" => Scheme::Socks5,
            "socks5h" => Scheme::Socks5h,
            _ => return Err("Use an http://, socks5://, or socks5h:// proxy".to_owned()),
        };
        let host = url
            .host_str()
            .filter(|host| !host.is_empty())
            .ok_or_else(|| "The proxy address needs a host".to_owned())?
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        if !matches!(url.path(), "" | "/") || url.query().is_some() {
            return Err("A proxy address has no path".to_owned());
        }
        let port = match scheme {
            Scheme::Http => url.port_or_known_default().unwrap_or(80),
            Scheme::Socks5 | Scheme::Socks5h => url.port().unwrap_or(1080),
        };
        let auth = (!url.username().is_empty()).then(|| {
            (
                percent_decode(url.username()),
                percent_decode(url.password().unwrap_or("")),
            )
        });
        let scheme_name = match scheme {
            Scheme::Http => "http",
            Scheme::Socks5 => "socks5",
            Scheme::Socks5h => "socks5h",
        };
        let credentials = match url.password() {
            Some(password) if !url.username().is_empty() => {
                format!("{}:{password}@", url.username())
            }
            _ if !url.username().is_empty() => format!("{}@", url.username()),
            _ => String::new(),
        };
        let host_part = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.clone()
        };
        Ok(Self {
            scheme,
            url: format!("{scheme_name}://{credentials}{host_part}:{port}"),
            host,
            port,
            auth,
        })
    }

    /// The URL without its password, for logs and Settings.
    pub fn redacted(&self) -> String {
        match (&self.auth, self.url.split_once('@')) {
            (Some((user, _)), Some((_, rest))) => {
                let scheme = self.url.split("://").next().unwrap_or("http");
                format!("{scheme}://{user}:***@{rest}")
            }
            _ => self.url.clone(),
        }
    }

}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(byte) = value
                .get(index + 1..index + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether `NO_PROXY` exempts `host`.
fn bypassed(host: &str, no_proxy: &str) -> bool {
    no_proxy.split(',').map(str::trim).any(|entry| {
        let entry = entry.trim_start_matches("*.").trim_start_matches('.');
        entry == "*"
            || (!entry.is_empty() && (host == entry || host.ends_with(&format!(".{entry}"))))
    })
}

fn from_env(host: &str) -> Option<Proxy> {
    let no_proxy = std::env::var("NO_PROXY")
        .or_else(|_| std::env::var("no_proxy"))
        .unwrap_or_default();
    if bypassed(host, &no_proxy) {
        return None;
    }
    ENV_VARS
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty())
        .and_then(|value| match Proxy::parse(&value) {
            Ok(proxy) => Some(proxy),
            Err(error) => {
                log::warn!("ignoring the proxy environment variable: {error}");
                None
            }
        })
}

struct Configured {
    setting: String,
    agent: Option<ureq::Agent>,
}

fn configured() -> &'static RwLock<Configured> {
    static CONFIGURED: OnceLock<RwLock<Configured>> = OnceLock::new();
    CONFIGURED.get_or_init(|| {
        RwLock::new(Configured {
            setting: String::new(),
            agent: None,
        })
    })
}

/// Applies the Settings value. An invalid value is ignored, so Settings
/// validates before saving.
pub fn configure(setting: &str) {
    let mut configured = configured().write().unwrap_or_else(|e| e.into_inner());
    if configured.setting != setting.trim() {
        configured.setting = setting.trim().to_owned();
        configured.agent = None;
    }
}

/// The proxy chosen in Settings, if any.
pub fn from_settings() -> Option<Proxy> {
    let setting = configured()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .setting
        .clone();
    (!setting.is_empty())
        .then(|| Proxy::parse(&setting).ok())
        .flatten()
}

/// The proxy for the WhatsApp connection: Settings, then the environment.
/// Proxy from the environment for update and media downloads.
pub fn for_downloads() -> Option<Proxy> {
    from_env("github.com")
}

pub fn ureq_proxy() -> Option<ureq::Proxy> {
    match from_settings().map(|proxy| ureq::Proxy::new(&proxy.url)) {
        Some(Ok(proxy)) => Some(proxy),
        Some(Err(error)) => {
            log::warn!("the proxy is not usable for downloads: {error}");
            None
        }
        None => ureq::Proxy::try_from_env(),
    }
}

/// A shared HTTP agent that uses [`ureq_proxy`].
pub fn agent() -> ureq::Agent {
    if let Some(agent) = &configured().read().unwrap_or_else(|e| e.into_inner()).agent {
        return agent.clone();
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .proxy(ureq_proxy())
        .build()
        .into();
    configured()
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .agent = Some(agent.clone());
    agent
}

/// The Settings proxy as a reqwest proxy. reqwest reads the environment
/// itself when this is `None`.
pub fn reqwest_proxy() -> Option<reqwest::Proxy> {
    from_settings().and_then(|proxy| reqwest::Proxy::all(&proxy.url).ok())
}

/// Opens the WhatsApp WebSocket through a proxy.
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn parses_supported_schemes() {
        let proxy = Proxy::parse("socks5h://me:p%40ss@127.0.0.1:9050").unwrap();
        assert_eq!(proxy.scheme, Scheme::Socks5h);
        assert_eq!(proxy.port, 9050);
        assert_eq!(proxy.auth, Some(("me".to_owned(), "p@ss".to_owned())));
        assert_eq!(proxy.redacted(), "socks5h://me:***@127.0.0.1:9050");
        assert!(!format!("{proxy:?}").contains("p%40ss"));

        let proxy = Proxy::parse("proxy.lan:3128").unwrap();
        assert_eq!(proxy.scheme, Scheme::Http);
        assert_eq!(proxy.url, "http://proxy.lan:3128");
        assert_eq!(
            Proxy::parse("socks5://[::1]").unwrap().url,
            "socks5://[::1]:1080"
        );
        assert!(Proxy::parse("ftp://proxy.lan").is_err());
        assert!(Proxy::parse("http://proxy.lan/path").is_err());
        assert!(Proxy::parse("").is_err());
    }

    #[test]
    fn no_proxy_matches_domains() {
        assert!(bypassed("web.whatsapp.com", "localhost, .whatsapp.com"));
        assert!(bypassed("web.whatsapp.com", "*"));
        assert!(bypassed("web.whatsapp.com", "web.whatsapp.com"));
        assert!(!bypassed("web.whatsapp.com", "app.com,,"));
        assert!(!bypassed("web.whatsapp.com", ""));
    }



}
