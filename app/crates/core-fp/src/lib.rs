//! Fingerprint sets (blueprint file 01).
//!
//! v1 ships the **native-family** mode only (file 01 §10.3, the blueprint's default): a profile
//! claims the host's OS family and GPU render class, because pixels, fonts, emoji and audio DSP
//! come from the real host and cannot be faked coherently. Per-profile variation comes from the
//! profile secret (seeded noise keys, media-device labels/ids), from realistic hardware buckets
//! that never exceed the host, and from locale/timezone/geolocation matched to the proxy exit.
//! Signed real-device packs (file 01 §9) plug in later through `FingerprintSet::from_pack`.

pub mod geo;
pub mod host;
pub mod seeds;
pub mod validate;

use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub use host::{GpuVendorClass, HostMonitor, HostProfile};
pub use seeds::Seeds;
pub use validate::{Issue, Report, Severity};

/// Chrome's `navigator.deviceMemory` buckets (capped at 8 by the spec/Chrome).
pub const DEVICE_MEMORY_BUCKETS: [f64; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
/// Logical core counts seen on real consumer machines.
pub const CORE_BUCKETS: [u32; 9] = [2, 4, 6, 8, 10, 12, 16, 20, 24];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EngineVersion { pub major: u32, pub full: String }

impl EngineVersion {
    pub fn parse(full: &str) -> Option<Self> {
        let major = full.split('.').next()?.parse().ok()?;
        if full.split('.').count() != 4 { return None; }
        Some(Self { major, full: full.to_string() })
    }
}

/// One coherent identity document (schema of file 01 §5). Stored as canonical JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FingerprintSet(pub Value);

/// Deterministic byte stream from a 32-byte key (HKDF-expand counter mode).
struct Prf { key: [u8; 32], ctr: u32 }
impl Prf {
    fn new(key: [u8; 32]) -> Self { Self { key, ctr: 0 } }
    fn next_u32(&mut self) -> u32 {
        let hk = Hkdf::<Sha256>::from_prk(&self.key).expect("32-byte PRK");
        let mut b = [0u8; 4];
        hk.expand(&self.ctr.to_be_bytes(), &mut b).unwrap();
        self.ctr += 1;
        u32::from_be_bytes(b)
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T { &items[(self.next_u32() as usize) % items.len()] }
}

impl FingerprintSet {
    /// Build a native-family set for this host. `misc_key` comes from the profile's seeds, so the
    /// same profile always gets the same set, and different profiles spread over the buckets.
    pub fn native_for_host(host: &HostProfile, misc_key: [u8; 32]) -> Self {
        let mut prf = Prf::new(misc_key);
        // Cores: a realistic bucket <= host logical cores (HW-CPU-002), biased to the host value.
        let cores_opts: Vec<u32> = CORE_BUCKETS.iter().copied().filter(|c| *c <= host.logical_cores.max(2)).collect();
        let cores = if prf.next_u32() % 2 == 0 { host.logical_cores.max(2) } else { *prf.pick(&cores_opts) };
        let host_bucket = DEVICE_MEMORY_BUCKETS.iter().copied().filter(|b| *b <= host.physical_mem_gb.max(0.25)).fold(0.25, f64::max);
        let mem_opts: Vec<f64> = DEVICE_MEMORY_BUCKETS.iter().copied().filter(|b| *b >= 4.0_f64.min(host_bucket) && *b <= host_bucket).collect();
        let memory = *prf.pick(&mem_opts);
        let mon = host.primary_monitor();
        let (sw, sh, dpr) = (mon.width, mon.height, mon.dpr);
        let taskbar = if host.os_family == "windows" { (48.0 / dpr).round() as u32 } else { 0 };
        let audio_sr = if prf.next_u32() % 4 == 0 { 44100 } else { 48000 };
        let has_cam = prf.next_u32() % 3 != 0;
        let mut devices = vec![
            json!({"kind": "audioinput", "label": "", "group": 1}),
            json!({"kind": "audiooutput", "label": "", "group": 1}),
        ];
        if has_cam { devices.push(json!({"kind": "videoinput", "label": "", "group": 2})); }
        let form = if host.is_laptop { "laptop" } else { "desktop" };
        FingerprintSet(json!({
            "meta": {"schema_version": 3, "mode": "native_family", "label": format!("{} · {} · {}", host.os_label(), host.gpu_label(), form),
                     "provenance": {"generator": "vanta-fpgen/native-v1", "host_class": host.host_class()}},
            "device": {"form_factor": form, "cpu_arch": host.cpu_arch, "bitness": "64", "hardware_concurrency": cores,
                       "device_memory_gb": memory, "max_touch_points": host.max_touch_points},
            "os": {"family": host.os_family, "version": host.os_version},
            "browser": {"brand": "Google Chrome",
                        "user_agent_template": host::ua_template(&host.os_family, &host.cpu_arch),
                        "full_version_template": "{{engine.full}}", "pdf_viewer_enabled": true, "webdriver": false},
            "screen": {"width": sw, "height": sh, "avail": {"left": 0, "top": 0, "width": sw, "height": sh.saturating_sub(taskbar)},
                       "dpr": dpr, "color_depth": 24},
            "locale": {"mode": "auto_from_ip"},
            "gpu": {"vendor_class": host.gpu_vendor.as_str(), "render_class": host.gpu_render_class, "source": "host"},
            "rendering": {"canvas_mode": "stable_noise", "webgl_readback_mode": "stable_noise", "text_metrics_mode": "native", "client_rects_mode": "native"},
            "audio": {"sample_rate": audio_sr, "max_channel_count": 2, "noise_mode": "stable_noise"},
            "fonts": {"source": "host"},
            "media": {"devices": devices},
            "network": {"webrtc_mode": "proxy_only"},
            "battery": {"present": host.is_laptop},
            "css": {"system_colors_family": host.os_family, "pointer": "fine"}
        }))
    }

    pub fn get(&self, path: &str) -> Option<&Value> { self.0.pointer(path) }
    pub fn os_family(&self) -> &str { self.get("/os/family").and_then(Value::as_str).unwrap_or("unknown") }
    pub fn os_version(&self) -> &str { self.get("/os/version").and_then(Value::as_str).unwrap_or("") }

    /// SHA-256 over canonical JSON (serde_json with sorted keys = RFC 8785 for our value types:
    /// strings, integers and finite floats).
    pub fn canonical_hash(&self) -> [u8; 32] {
        let bytes = canonical_json(&self.0);
        Sha256::digest(bytes).into()
    }
    pub fn id(&self) -> String { hex::encode(self.canonical_hash()) }
}

pub fn canonical_json(v: &Value) -> Vec<u8> {
    fn sort(v: &Value) -> Value {
        match v {
            Value::Object(m) => {
                let mut keys: Vec<_> = m.keys().collect();
                keys.sort();
                let mut out = serde_json::Map::new();
                for k in keys { out.insert(k.clone(), sort(&m[k])); }
                Value::Object(out)
            }
            Value::Array(a) => Value::Array(a.iter().map(sort).collect()),
            o => o.clone(),
        }
    }
    serde_json::to_vec(&sort(v)).expect("serializable")
}

/// Locale/geo inputs decided at launch (manual or from the proxy exit).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LocaleInputs {
    pub timezone: Option<String>,
    pub languages: Vec<String>,
    pub country: Option<String>,
    pub geo: Option<(f64, f64, f64)>,
}

/// What the engine will receive, shown in the editor (no secrets). Mirrors `ResolvedFingerprint` in file 04 §6.
pub fn resolve(set: &FingerprintSet, engine: &EngineVersion, overrides: &Value, locale: &LocaleInputs, seeds: &Seeds) -> Value {
    let mut s = set.0.clone();
    merge(&mut s, overrides);
    let set = FingerprintSet(s);
    let tpl = set.get("/browser/user_agent_template").and_then(Value::as_str).unwrap_or("");
    let ua = tpl.replace("{{engine.major}}", &engine.major.to_string());
    let languages = if locale.languages.is_empty() { vec!["en-US".to_string(), "en".to_string()] } else { locale.languages.clone() };
    let num = |p: &str| set.get(p).and_then(Value::as_f64).unwrap_or(0.0);
    let devices = set.get("/media/devices").and_then(Value::as_array).cloned().unwrap_or_default();
    let count = |k: &str| devices.iter().filter(|d| d["kind"] == k).count();
    json!({
        "userAgent": ua,
        "uaCh": {
            "brands": brands(engine.major),
            "fullVersionList": brands_full(engine),
            "grease": "added by engine",
            "platform": host::uach_platform(set.os_family()),
            "mobile": false,
            "architecture": if set.get("/device/cpu_arch").and_then(Value::as_str) == Some("arm") { "arm" } else { "x86" },
            "bitness": "64",
            "platformVersion": "native",
        },
        "navigator": {
            "platform": host::navigator_platform(set.os_family()),
            "hardwareConcurrency": num("/device/hardware_concurrency"),
            "deviceMemory": num("/device/device_memory_gb"),
            "maxTouchPoints": num("/device/max_touch_points"),
            "vendor": "Google Inc.",
            "languages": languages,
        },
        "screen": {
            "width": num("/screen/width"), "height": num("/screen/height"),
            "availWidth": num("/screen/avail/width"), "availHeight": num("/screen/avail/height"),
            "colorDepth": num("/screen/color_depth"), "dpr": num("/screen/dpr"),
        },
        "timezone": locale.timezone,
        "acceptLanguage": geo::accept_language(&languages),
        "geolocation": locale.geo.map(|(a, b, c)| json!({"lat": a, "lon": b, "accuracyM": c})),
        "webgl": {"source": "host", "renderClass": set.get("/gpu/render_class")},
        "audio": {"sampleRate": num("/audio/sample_rate"), "channelCount": num("/audio/max_channel_count")},
        "mediaDevices": {"audioinput": count("audioinput"), "audiooutput": count("audiooutput"), "videoinput": count("videoinput")},
        "noise": {"canvas": "stable", "webgl": "stable", "audio": "stable"},
        "seedFingerprintHex": hex::encode(&Sha256::digest(seeds.canvas)[..6]),
    })
}

/// Brand list preview. The engine adds the GREASE entry and orders the list itself
/// (Chromium's own `user_agent_utils.cc` algorithm), so the core never re-implements it.
fn brands(major: u32) -> Value {
    json!([{"brand": "Google Chrome", "version": major.to_string()}, {"brand": "Chromium", "version": major.to_string()}])
}
fn brands_full(e: &EngineVersion) -> Value {
    json!([{"brand": "Google Chrome", "version": e.full}, {"brand": "Chromium", "version": e.full}])
}

/// Deep-merge `patch` into `base` (objects merge, everything else replaces).
pub fn merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            for (k, v) in p { merge(b.entry(k.clone()).or_insert(Value::Null), v); }
        }
        (b, p) => *b = p.clone(),
    }
}

/// Uniqueness guard (file 04 §10.2): hash over the distinguishing tuple. Two live profiles may not share it.
pub fn identity_hash(set: &FingerprintSet, seeds: &Seeds) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(set.canonical_hash());
    h.update(seeds.canvas);
    h.update(seeds.audio);
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> HostProfile { HostProfile::example_windows() }

    #[test]
    fn deterministic_and_host_bounded() {
        let a = FingerprintSet::native_for_host(&host(), [1; 32]);
        let b = FingerprintSet::native_for_host(&host(), [1; 32]);
        assert_eq!(a, b);
        assert_eq!(a.id(), b.id());
        for k in 0..64u8 {
            let s = FingerprintSet::native_for_host(&host(), [k; 32]);
            let c = s.get("/device/hardware_concurrency").unwrap().as_u64().unwrap() as u32;
            assert!(c <= host().logical_cores && c >= 2);
            let m = s.get("/device/device_memory_gb").unwrap().as_f64().unwrap();
            assert!(DEVICE_MEMORY_BUCKETS.contains(&m) && m <= 8.0);
            let r = validate::validate(&s, &host(), &EngineVersion::parse("157.0.8103.38").unwrap(), &LocaleInputs::default(), &Default::default());
            assert!(r.errors().next().is_none(), "{:?}", r.issues);
        }
    }

    #[test]
    fn resolve_ua_matches_engine() {
        let e = EngineVersion::parse("157.0.8103.38").unwrap();
        let s = FingerprintSet::native_for_host(&host(), [3; 32]);
        let seeds = Seeds::derive(&[9; 32], &s.id(), 0);
        let r = resolve(&s, &e, &json!({}), &LocaleInputs { languages: vec!["de-DE".into(), "de".into()], ..Default::default() }, &seeds);
        assert_eq!(r["userAgent"], "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/157.0.0.0 Safari/537.36");
        assert_eq!(r["acceptLanguage"], "de-DE,de;q=0.9");
        assert_eq!(r["uaCh"]["brands"].as_array().unwrap().len(), 2);
    }
}
