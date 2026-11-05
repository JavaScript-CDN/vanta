//! Non-fingerprint Chromium preferences written before launch (file 00 §7 step 9).
use serde_json::{Value, json};
use std::path::Path;

pub struct PrefInputs<'a> { pub accept_languages: &'a str, pub webrtc_policy: &'a str, pub download_dir: &'a Path }

/// Merge our keys into `<ud>/Default/Preferences`, keeping everything else the browser stored.
pub fn write(user_data_dir: &Path, p: &PrefInputs) -> std::io::Result<()> {
    let dir = user_data_dir.join("Default");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("Preferences");
    let mut cur: Value = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    let ours = json!({
        "intl": {"accept_languages": p.accept_languages, "selected_languages": p.accept_languages},
        "webrtc": {"ip_handling_policy": p.webrtc_policy, "multiple_routes_enabled": false, "nonproxied_udp_enabled": false},
        "browser": {"check_default_browser": false},
        "credentials_enable_service": false,
        "safebrowsing": {"enabled": true},
        "download": {"default_directory": p.download_dir.to_string_lossy(), "prompt_for_download": true},
        "profile": {"default_content_setting_values": {"geolocation": 3, "notifications": 0}},
        "translate": {"enabled": false},
    });
    merge(&mut cur, &ours);
    let tmp = dir.join("Preferences.vanta.tmp");
    std::fs::write(&tmp, serde_json::to_vec(&cur)?)?;
    std::fs::rename(tmp, path)
}

fn merge(b: &mut Value, p: &Value) {
    match (b, p) {
        (Value::Object(b), Value::Object(p)) => for (k, v) in p { merge(b.entry(k.clone()).or_insert(Value::Null), v) },
        (b, p) => *b = p.clone(),
    }
}

#[cfg(test)]
mod t {
    #[test]
    fn merges() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("Default")).unwrap();
        std::fs::write(d.path().join("Default/Preferences"), r#"{"bookmark_bar":{"show_on_all_tabs":true},"intl":{"accept_languages":"x"}}"#).unwrap();
        super::write(d.path(), &super::PrefInputs { accept_languages: "bn-BD,bn,en-US,en", webrtc_policy: "disable_non_proxied_udp", download_dir: d.path() }).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(d.path().join("Default/Preferences")).unwrap()).unwrap();
        assert_eq!(v["intl"]["accept_languages"], "bn-BD,bn,en-US,en");
        assert_eq!(v["bookmark_bar"]["show_on_all_tabs"], true);
        assert_eq!(v["webrtc"]["ip_handling_policy"], "disable_non_proxied_udp");
    }
}
