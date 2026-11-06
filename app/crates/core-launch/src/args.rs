//! Engine command line. Release gate (file 04 §3.1): never any remote-debugging switch.
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub engine_exe: PathBuf,
    pub user_data_dir: PathBuf,
    /// Local forwarder port (127.0.0.1). None = no proxy configured for this profile.
    pub socks_port: Option<u16>,
    pub ui_lang: String,
    pub webrtc_block_udp: bool,
    pub window: Option<(u32, u32)>,
    pub start_urls: Vec<String>,
    /// Path of the sealed launch config for the patched engine (read via --vanta-config-file).
    pub config_file: Option<PathBuf>,
}

const FORBIDDEN: &[&str] = &["--remote-debugging", "--remote-allow-origins", "--enable-automation", "--headless", "--no-sandbox", "--disable-web-security"];

pub fn build_args(s: &LaunchSpec) -> Vec<String> {
    let mut a = vec![
        format!("--user-data-dir={}", s.user_data_dir.display()),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--disable-background-networking".into(),
        "--metrics-recording-only".into(),
        format!("--lang={}", s.ui_lang),
    ];
    if let Some(port) = s.socks_port {
        a.push(format!("--proxy-server=socks5://127.0.0.1:{port}"));
        // Remove Chromium's implicit loopback bypass so nothing goes DIRECT.
        a.push("--proxy-bypass-list=<-loopback>".into());
    }
    if s.webrtc_block_udp {
        a.push("--force-webrtc-ip-handling-policy=disable_non_proxied_udp".into());
    }
    if let Some((w, h)) = s.window { a.push(format!("--window-size={w},{h}")); }
    if let Some(cfg) = &s.config_file { a.push(format!("--vanta-config-file={}", cfg.display())); }
    // URLs last; only http(s) to avoid file:// or javascript: injection via profile data.
    for u in &s.start_urls {
        let l = u.trim().to_ascii_lowercase();
        if l.starts_with("https://") || l.starts_with("http://") { a.push(u.trim().to_string()); }
    }
    // Hard filter in release builds too, not only a debug assertion.
    a.retain(|x| !FORBIDDEN.iter().any(|f| x.starts_with(f)));
    a
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn no_debug_switches_and_safe_urls() {
        let s = LaunchSpec { engine_exe: "chrome.exe".into(), user_data_dir: "C:/p/ud".into(), socks_port: Some(5555), ui_lang: "bn".into(),
            webrtc_block_udp: true, window: Some((1280, 800)), start_urls: vec!["https://example.com".into(), "javascript:alert(1)".into(), "file:///c:/x".into()], config_file: None };
        let a = build_args(&s);
        assert!(a.iter().all(|x| !FORBIDDEN.iter().any(|f| x.contains(f))));
        assert!(a.contains(&"--proxy-server=socks5://127.0.0.1:5555".to_string()));
        assert!(a.contains(&"--proxy-bypass-list=<-loopback>".to_string()));
        assert_eq!(a.iter().filter(|x| x.contains("://") && !x.starts_with("--")).count(), 1);
    }
}
