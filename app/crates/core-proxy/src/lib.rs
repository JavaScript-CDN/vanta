//! Proxy parsing and checking (file 03 / file 04 §8.6, task 3.1).

use core_model::{AppError, CmdResult, ProxyCheckResult, ProxyKind, ProxyStatus};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedProxy {
    pub kind: ProxyKind,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password: Option<String>,
    pub has_password: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseIssue { pub line: usize, pub message_key: &'static str }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedList { pub items: Vec<ParsedProxy>, pub issues: Vec<ParseIssue> }

fn valid_host(h: &str) -> bool {
    !h.is_empty() && h.len() <= 253
        && (h.parse::<std::net::IpAddr>().is_ok()
            || h.split('.').all(|l| !l.is_empty() && l.len() <= 63 && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !l.starts_with('-')))
}

/// Accepted line formats (one proxy per line; `#` comments and blank lines ignored):
/// * `scheme://user:pass@host:port` (http, https, socks5, socks5h)
/// * `host:port`
/// * `host:port:user:pass`
/// * `user:pass@host:port`
/// * `user:pass:host:port` (when the third field is a host and the fourth a port)
pub fn parse_line(line: &str, default_kind: ProxyKind) -> Result<ParsedProxy, &'static str> {
    let line = line.trim();
    let (kind, rest) = match line.split_once("://") {
        Some((s, r)) => (match s.to_ascii_lowercase().as_str() {
            "http" => ProxyKind::Http, "https" => ProxyKind::Https, "socks5" | "socks5h" | "socks" => ProxyKind::Socks5,
            _ => return Err("proxy.unsupported_scheme"),
        }, r.trim_end_matches('/')),
        None => (default_kind, line),
    };
    let (user, pass, host, port) = if let Some((cred, hp)) = rest.rsplit_once('@') {
        let (h, p) = hp.rsplit_once(':').ok_or("proxy.missing_port")?;
        let (u, pw) = cred.split_once(':').map(|(u, p)| (Some(u), Some(p))).unwrap_or((Some(cred), None));
        (u.map(str::to_string), pw.map(str::to_string), h.to_string(), p)
    } else {
        let parts: Vec<&str> = rest.split(':').collect();
        match parts.as_slice() {
            [h, p] => (None, None, h.to_string(), *p),
            [a, b, c, d] if b.parse::<u16>().is_ok() => (Some(c.to_string()), Some(d.to_string()), a.to_string(), *b),
            [a, b, c, d] if d.parse::<u16>().is_ok() && valid_host(c) => (Some(a.to_string()), Some(b.to_string()), c.to_string(), *d),
            _ => return Err("proxy.unrecognised_format"),
        }
    };
    let host = host.trim_start_matches('[').trim_end_matches(']').to_string();
    if !valid_host(&host) { return Err("proxy.invalid_host"); }
    let port: u16 = port.parse().map_err(|_| "proxy.invalid_port")?;
    if port == 0 { return Err("proxy.invalid_port"); }
    let user = user.filter(|u| !u.is_empty());
    let pass = pass.filter(|p| !p.is_empty());
    Ok(ParsedProxy { kind, host, port, has_password: pass.is_some(), username: user, password: pass })
}

pub fn parse_list(text: &str, default_kind: ProxyKind) -> ParsedList {
    let mut items = Vec::new();
    let mut issues = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let l = raw.trim();
        if l.is_empty() || l.starts_with('#') { continue; }
        match parse_line(l, default_kind) {
            Ok(p) => if !items.contains(&p) { items.push(p) },
            Err(k) => issues.push(ParseIssue { line: i + 1, message_key: k }),
        }
    }
    ParsedList { items, issues }
}

/// What the checker needs; the password is plaintext only for the duration of the check.
pub struct CheckTarget<'a> { pub kind: ProxyKind, pub host: &'a str, pub port: u16, pub username: Option<&'a str>, pub password: Option<&'a str> }

/// Build a proxy URL for reqwest. SOCKS uses `socks5h` so DNS resolves at the exit (no DNS leak).
fn proxy_url(t: &CheckTarget) -> CmdResult<reqwest::Proxy> {
    let scheme = match t.kind { ProxyKind::Http => "http", ProxyKind::Https => "https", ProxyKind::Socks5 => "socks5h", _ => return Err(AppError::Proxy("proxy.kind_not_checkable".into())) };
    let host = if t.host.contains(':') { format!("[{}]", t.host) } else { t.host.to_string() };
    let mut u = url::Url::parse(&format!("{scheme}://{host}:{}", t.port)).map_err(|_| AppError::Proxy("proxy.invalid_host".into()))?;
    if let Some(user) = t.username { u.set_username(user).ok(); u.set_password(t.password).ok(); }
    reqwest::Proxy::all(u.as_str()).map_err(|_| AppError::Proxy("proxy.invalid_host".into()))
}

/// Exit lookup endpoint. ipwho.is returns IP, country, region, city, timezone, ASN, ISP and
/// coordinates over HTTPS without an API key (endpoint user-configurable in Settings).
pub const DEFAULT_ECHO: &str = "https://ipwho.is/";

#[derive(Deserialize)]
struct WhoIs {
    success: Option<bool>,
    ip: Option<String>,
    country_code: Option<String>,
    region: Option<String>,
    city: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    connection: Option<Conn>,
    timezone: Option<Tz>,
}
#[derive(Deserialize)] struct Conn { asn: Option<i64>, isp: Option<String> }
#[derive(Deserialize)] struct Tz { id: Option<String> }

/// Check a proxy: connect through it, fetch the exit identity, measure latency.
/// Never logs credentials. Uses a Chrome-like User-Agent so the probe does not stand out.
pub async fn check(t: &CheckTarget<'_>, echo_url: &str, user_agent: &str) -> ProxyCheckResult {
    let fail = |status: ProxyStatus, err: &str| ProxyCheckResult { status, latency_ms: None, exit_ip: None, country: None, region: None, city: None, timezone: None, asn: None, isp: None, lat: None, lon: None, error: Some(err.to_string()) };
    let proxy = match proxy_url(t) { Ok(p) => p, Err(_) => return fail(ProxyStatus::Fail, "proxy.invalid") };
    let client = match reqwest::Client::builder().proxy(proxy).timeout(Duration::from_secs(15)).connect_timeout(Duration::from_secs(10))
        .user_agent(user_agent).https_only(true).build() { Ok(c) => c, Err(_) => return fail(ProxyStatus::Fail, "proxy.client") };
    let t0 = Instant::now();
    let resp = match client.get(echo_url).send().await {
        Ok(r) => r,
        Err(e) => {
            let s = e.to_string().to_ascii_lowercase();
            let auth = s.contains("407") || s.contains("auth");
            return fail(if auth { ProxyStatus::AuthFail } else { ProxyStatus::Fail }, if auth { "proxy.auth_failed" } else if e.is_timeout() { "proxy.timeout" } else { "proxy.connect_failed" });
        }
    };
    if resp.status().as_u16() == 407 { return fail(ProxyStatus::AuthFail, "proxy.auth_failed"); }
    let latency = t0.elapsed().as_millis() as i64;
    let who: WhoIs = match resp.json().await { Ok(w) => w, Err(_) => return fail(ProxyStatus::Fail, "proxy.echo_unreadable") };
    if who.success == Some(false) { return fail(ProxyStatus::Fail, "proxy.echo_failed"); }
    ProxyCheckResult {
        status: if latency > 3000 { ProxyStatus::Slow } else { ProxyStatus::Ok },
        latency_ms: Some(latency), exit_ip: who.ip, country: who.country_code.map(|c| c.to_ascii_uppercase()),
        region: who.region, city: who.city, timezone: who.timezone.and_then(|t| t.id),
        asn: who.connection.as_ref().and_then(|c| c.asn), isp: who.connection.and_then(|c| c.isp),
        lat: who.latitude, lon: who.longitude, error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        let k = ProxyKind::Http;
        let p = parse_line("socks5://u:p@1.2.3.4:1080", k).unwrap();
        assert_eq!((p.kind, p.host.as_str(), p.port, p.username.as_deref(), p.password.as_deref()), (ProxyKind::Socks5, "1.2.3.4", 1080, Some("u"), Some("p")));
        let p = parse_line("proxy.example.com:8080:alice:s3cr:et", k);
        assert!(p.is_err(), "5 fields is ambiguous");
        let p = parse_line("proxy.example.com:8080:alice:secret", k).unwrap();
        assert_eq!((p.host.as_str(), p.port, p.username.as_deref()), ("proxy.example.com", 8080, Some("alice")));
        let p = parse_line("alice:secret@10.0.0.1:3128", k).unwrap();
        assert_eq!((p.host.as_str(), p.port), ("10.0.0.1", 3128));
        let p = parse_line("alice:secret:gw.example.net:9000", k).unwrap();
        assert_eq!((p.host.as_str(), p.port, p.username.as_deref()), ("gw.example.net", 9000, Some("alice")));
        assert_eq!(parse_line("1.2.3.4:99999", k), Err("proxy.invalid_port"));
        assert_eq!(parse_line("ftp://1.2.3.4:21", k), Err("proxy.unsupported_scheme"));
        assert_eq!(parse_line("bad host:80", k), Err("proxy.invalid_host"));
        let l = parse_list("# list\n1.2.3.4:80\n\n1.2.3.4:80\nnope\n", k);
        assert_eq!(l.items.len(), 1);
        assert_eq!(l.issues.len(), 1);
        assert_eq!(l.issues[0].line, 5);
        // password never serialised to the UI
        let j = serde_json::to_string(&parse_line("http://u:topsecret@h.com:1", k).unwrap()).unwrap();
        assert!(!j.contains("topsecret"));
    }
}
