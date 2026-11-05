//! Per-profile local SOCKS5 gateway (the v1 subset of `veil-netd`, file 03).
//!
//! The engine is pointed at `socks5://127.0.0.1:<port>` and can reach the network only through
//! here. Each CONNECT is relayed through the profile's upstream (SOCKS5 with RFC 1929 auth, or
//! HTTP CONNECT with Basic auth), so credentials never go on the engine command line.
//! Fail-closed: if the upstream is unreachable the connection fails — there is no direct path.
//! UDP ASSOCIATE is refused (UDP/QUIC/WebRTC cannot leak around the proxy); IPv6 literal targets
//! are refused when `block_ipv6` is set.

use base64::Engine as _;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::{Duration, timeout};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamKind { Socks5, HttpConnect }

#[derive(Clone)]
pub struct Upstream { pub kind: UpstreamKind, pub host: String, pub port: u16, pub username: Option<String>, pub password: Option<String> }

impl std::fmt::Debug for Upstream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Upstream({:?} {}:{}, auth={})", self.kind, self.host, self.port, self.username.is_some())
    }
}

#[derive(Default)]
pub struct Stats { pub connections: AtomicU64, pub failures: AtomicU64, pub blocked: AtomicU64 }

pub struct Forwarder { pub port: u16, pub stats: Arc<Stats>, task: JoinHandle<()> }

impl Drop for Forwarder { fn drop(&mut self) { self.task.abort(); } }

#[derive(Debug)]
enum Target { Domain(String, u16), V4(Ipv4Addr, u16), V6(Ipv6Addr, u16) }

impl Forwarder {
    pub async fn start(up: Upstream, block_ipv6: bool) -> std::io::Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
        let port = listener.local_addr()?.port();
        let stats = Arc::new(Stats::default());
        let up = Arc::new(up);
        let st = stats.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((client, peer)) = listener.accept().await else { continue };
                if !peer.ip().is_loopback() { continue; }
                let (up, st) = (up.clone(), st.clone());
                tokio::spawn(async move {
                    st.connections.fetch_add(1, Ordering::Relaxed);
                    if handle(client, &up, block_ipv6, &st).await.is_err() { st.failures.fetch_add(1, Ordering::Relaxed); }
                });
            }
        });
        Ok(Self { port, stats, task })
    }
}

async fn handle(mut c: TcpStream, up: &Upstream, block_ipv6: bool, st: &Stats) -> std::io::Result<()> {
    c.set_nodelay(true).ok();
    // Greeting: VER NMETHODS METHODS…  → choose 0x00 (no auth; listener is loopback-only)
    let mut h = [0u8; 2];
    timeout(Duration::from_secs(10), c.read_exact(&mut h)).await??;
    if h[0] != 5 { return Err(bad()); }
    let mut m = vec![0u8; h[1] as usize];
    c.read_exact(&mut m).await?;
    if !m.contains(&0) { c.write_all(&[5, 0xFF]).await?; return Err(bad()); }
    c.write_all(&[5, 0]).await?;
    // Request: VER CMD RSV ATYP …
    let mut r = [0u8; 4];
    c.read_exact(&mut r).await?;
    if r[1] != 1 {
        // Only CONNECT. UDP ASSOCIATE / BIND refused → no UDP path around the proxy.
        st.blocked.fetch_add(1, Ordering::Relaxed);
        reply(&mut c, 7).await?;
        return Ok(());
    }
    let target = match r[3] {
        1 => { let mut a = [0u8; 6]; c.read_exact(&mut a).await?; Target::V4(Ipv4Addr::new(a[0], a[1], a[2], a[3]), u16::from_be_bytes([a[4], a[5]])) }
        3 => { let mut l = [0u8; 1]; c.read_exact(&mut l).await?; let mut d = vec![0u8; l[0] as usize + 2]; c.read_exact(&mut d).await?;
               let port = u16::from_be_bytes([d[d.len() - 2], d[d.len() - 1]]); d.truncate(d.len() - 2);
               Target::Domain(String::from_utf8(d).map_err(|_| bad())?, port) }
        4 => { let mut a = [0u8; 18]; c.read_exact(&mut a).await?; let mut ip = [0u8; 16]; ip.copy_from_slice(&a[..16]);
               Target::V6(Ipv6Addr::from(ip), u16::from_be_bytes([a[16], a[17]])) }
        _ => { reply(&mut c, 8).await?; return Err(bad()); }
    };
    if block_ipv6 && matches!(target, Target::V6(..)) {
        st.blocked.fetch_add(1, Ordering::Relaxed);
        reply(&mut c, 2).await?;
        return Ok(());
    }
    let upstream = match timeout(Duration::from_secs(20), connect_upstream(up, &target)).await {
        Ok(Ok(s)) => s,
        _ => { reply(&mut c, 1).await?; return Err(bad()); }   // fail closed
    };
    reply(&mut c, 0).await?;
    let (mut a, mut b) = (c, upstream);
    tokio::io::copy_bidirectional(&mut a, &mut b).await.map(|_| ())
}

async fn reply(c: &mut TcpStream, code: u8) -> std::io::Result<()> { c.write_all(&[5, code, 0, 1, 0, 0, 0, 0, 0, 0]).await }
fn bad() -> std::io::Error { std::io::Error::new(std::io::ErrorKind::InvalidData, "socks") }

async fn connect_upstream(up: &Upstream, t: &Target) -> std::io::Result<TcpStream> {
    let mut s = TcpStream::connect((up.host.as_str(), up.port)).await?;
    s.set_nodelay(true).ok();
    match up.kind {
        UpstreamKind::Socks5 => {
            let auth = up.username.is_some();
            s.write_all(if auth { &[5, 1, 2] } else { &[5, 1, 0] }).await?;
            let mut g = [0u8; 2];
            s.read_exact(&mut g).await?;
            match g[1] {
                0 => {}
                2 if auth => {
                    let u = up.username.as_deref().unwrap_or("").as_bytes();
                    let p = up.password.as_deref().unwrap_or("").as_bytes();
                    if u.len() > 255 || p.len() > 255 { return Err(bad()); }
                    let mut msg = vec![1, u.len() as u8]; msg.extend_from_slice(u); msg.push(p.len() as u8); msg.extend_from_slice(p);
                    s.write_all(&msg).await?;
                    let mut ar = [0u8; 2];
                    s.read_exact(&mut ar).await?;
                    if ar[1] != 0 { return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "proxy auth")); }
                }
                _ => return Err(bad()),
            }
            let mut req = vec![5, 1, 0];
            match t {
                // Domain names are sent to the exit for resolution — no local DNS lookup (no DNS leak).
                Target::Domain(d, p) => { req.push(3); req.push(d.len() as u8); req.extend_from_slice(d.as_bytes()); req.extend_from_slice(&p.to_be_bytes()); }
                Target::V4(ip, p) => { req.push(1); req.extend_from_slice(&ip.octets()); req.extend_from_slice(&p.to_be_bytes()); }
                Target::V6(ip, p) => { req.push(4); req.extend_from_slice(&ip.octets()); req.extend_from_slice(&p.to_be_bytes()); }
            }
            s.write_all(&req).await?;
            let mut rh = [0u8; 4];
            s.read_exact(&mut rh).await?;
            if rh[1] != 0 { return Err(bad()); }
            let skip = match rh[3] { 1 => 6, 4 => 18, 3 => { let mut l = [0u8; 1]; s.read_exact(&mut l).await?; l[0] as usize + 2 } _ => return Err(bad()) };
            let mut sink = vec![0u8; skip];
            s.read_exact(&mut sink).await?;
            Ok(s)
        }
        UpstreamKind::HttpConnect => {
            let authority = match t { Target::Domain(d, p) => format!("{d}:{p}"), Target::V4(ip, p) => format!("{ip}:{p}"), Target::V6(ip, p) => format!("[{ip}]:{p}") };
            let mut req = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n");
            if let Some(u) = &up.username {
                let tok = base64::engine::general_purpose::STANDARD.encode(format!("{u}:{}", up.password.as_deref().unwrap_or("")));
                req.push_str(&format!("Proxy-Authorization: Basic {tok}\r\n"));
            }
            req.push_str("\r\n");
            s.write_all(req.as_bytes()).await?;
            // Read the response head byte-by-byte up to CRLFCRLF (bounded) so no tunnel bytes are consumed.
            let mut head = Vec::with_capacity(256);
            let mut b = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                if head.len() > 8192 { return Err(bad()); }
                s.read_exact(&mut b).await?;
                head.push(b[0]);
            }
            let line = String::from_utf8_lossy(&head);
            let code = line.split_whitespace().nth(1).unwrap_or("");
            if code == "200" { Ok(s) } else if code == "407" { Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "proxy auth")) } else { Err(bad()) }
        }
    }
}

#[cfg(test)]
mod t {
    use super::*;

    /// Minimal HTTP CONNECT upstream that checks auth and then echoes.
    async fn fake_http_proxy() -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                tokio::spawn(async move {
                    let mut head = Vec::new(); let mut b = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") { s.read_exact(&mut b).await.unwrap(); head.push(b[0]); }
                    let h = String::from_utf8_lossy(&head).to_string();
                    let ok = h.starts_with("CONNECT example.com:443 ") && h.contains("Proxy-Authorization: Basic dTpw");
                    if !ok { s.write_all(b"HTTP/1.1 407 Proxy Auth\r\n\r\n").await.unwrap(); return; }
                    s.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await.unwrap();
                    let mut buf = [0u8; 5]; s.read_exact(&mut buf).await.unwrap(); s.write_all(&buf).await.unwrap();
                });
            }
        });
        port
    }

    async fn socks_connect(port: u16, cmd: u8) -> (TcpStream, u8) {
        let mut c = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        c.write_all(&[5, 1, 0]).await.unwrap();
        let mut g = [0u8; 2]; c.read_exact(&mut g).await.unwrap();
        let d = b"example.com";
        let mut req = vec![5, cmd, 0, 3, d.len() as u8]; req.extend_from_slice(d); req.extend_from_slice(&443u16.to_be_bytes());
        c.write_all(&req).await.unwrap();
        let mut r = [0u8; 10]; c.read_exact(&mut r).await.unwrap();
        (c, r[1])
    }

    #[tokio::test]
    async fn relays_through_http_upstream_and_refuses_udp() {
        let up = fake_http_proxy().await;
        let fw = Forwarder::start(Upstream { kind: UpstreamKind::HttpConnect, host: "127.0.0.1".into(), port: up, username: Some("u".into()), password: Some("p".into()) }, true).await.unwrap();
        let (mut c, code) = socks_connect(fw.port, 1).await;
        assert_eq!(code, 0);
        c.write_all(b"hello").await.unwrap();
        let mut b = [0u8; 5]; c.read_exact(&mut b).await.unwrap();
        assert_eq!(&b, b"hello");
        let (_c, code) = socks_connect(fw.port, 3).await; // UDP ASSOCIATE
        assert_eq!(code, 7);
    }

    #[tokio::test]
    async fn fails_closed_when_upstream_down() {
        let fw = Forwarder::start(Upstream { kind: UpstreamKind::Socks5, host: "127.0.0.1".into(), port: 1, username: None, password: None }, true).await.unwrap();
        let (_c, code) = socks_connect(fw.port, 1).await;
        assert_eq!(code, 1);
    }

    #[tokio::test]
    async fn wrong_credentials_rejected() {
        let up = fake_http_proxy().await;
        let fw = Forwarder::start(Upstream { kind: UpstreamKind::HttpConnect, host: "127.0.0.1".into(), port: up, username: Some("u".into()), password: Some("WRONG".into()) }, true).await.unwrap();
        let (_c, code) = socks_connect(fw.port, 1).await;
        assert_eq!(code, 1);
    }
}
