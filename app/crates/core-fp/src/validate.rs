//! Pre-launch consistency validator (file 01 §15) — the rules that apply to native-family sets.
use crate::{EngineVersion, FingerprintSet, HostProfile, LocaleInputs, DEVICE_MEMORY_BUCKETS, geo};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity { Error, Warning, Info }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue { pub rule: &'static str, pub severity: Severity, pub message_key: &'static str }

#[derive(Debug, Clone, Serialize, Default)]
pub struct Report { pub issues: Vec<Issue> }

impl Report {
    pub fn errors(&self) -> impl Iterator<Item = &Issue> { self.issues.iter().filter(|i| i.severity == Severity::Error) }
    pub fn ok(&self) -> bool { self.errors().next().is_none() }
    fn e(&mut self, rule: &'static str, key: &'static str) { self.issues.push(Issue { rule, severity: Severity::Error, message_key: key }) }
    fn w(&mut self, rule: &'static str, key: &'static str) { self.issues.push(Issue { rule, severity: Severity::Warning, message_key: key }) }
}

/// Extra context known only at launch.
#[derive(Debug, Clone, Default)]
pub struct LaunchCtx {
    pub proxy_country: Option<String>,
    pub proxy_timezone: Option<String>,
    pub webrtc_real: bool,
    pub has_proxy: bool,
}

pub fn validate(set: &FingerprintSet, host: &HostProfile, engine: &EngineVersion, locale: &LocaleInputs, ctx: &LaunchCtx) -> Report {
    let mut r = Report::default();
    let s = |p: &str| set.get(p).and_then(Value::as_str).unwrap_or("");
    let n = |p: &str| set.get(p).and_then(Value::as_f64);

    // UA / OS
    let tpl = s("/browser/user_agent_template");
    if !tpl.contains("Chrome/{{engine.major}}.0.0.0") { r.e("UA-FMT-001", "fp.ua_format"); }
    if engine.major < 100 { r.e("UA-VER-001", "fp.engine_version"); }
    let fam = set.os_family();
    let token_ok = match fam { "windows" => tpl.contains("Windows NT 10.0"), "macos" => tpl.contains("Macintosh"), "linux" => tpl.contains("Linux"), _ => false };
    if !token_ok { r.e("UA-PLAT-001", "fp.ua_platform"); }
    if fam != host.os_family { r.w("GPU-RCLASS-002", "fp.cross_os"); }

    // Hardware
    match n("/device/hardware_concurrency") {
        Some(c) if c >= 1.0 && c.fract() == 0.0 => { if c as u32 > host.logical_cores { r.w("HW-CPU-002", "fp.cores_above_host"); } }
        _ => r.e("HW-CPU-001", "fp.cores_invalid"),
    }
    match n("/device/device_memory_gb") {
        Some(m) if DEVICE_MEMORY_BUCKETS.contains(&m) => { if m > host.physical_mem_gb.max(0.25) { r.w("HW-MEM-002", "fp.memory_above_host"); } }
        _ => r.e("HW-MEM-001", "fp.memory_bucket"),
    }

    // Screen
    let (w, h) = (n("/screen/width").unwrap_or(0.0), n("/screen/height").unwrap_or(0.0));
    let (aw, ah) = (n("/screen/avail/width").unwrap_or(-1.0), n("/screen/avail/height").unwrap_or(-1.0));
    if w < 320.0 || h < 320.0 { r.e("SCR-DIP-001", "fp.screen_size"); }
    if aw < 0.0 || ah < 0.0 || aw > w || ah > h { r.e("SCR-AVAIL-001", "fp.screen_avail"); }
    let dpr = n("/screen/dpr").unwrap_or(0.0);
    if !(0.5..=4.0).contains(&dpr) { r.e("SCR-DIP-001", "fp.dpr"); }
    if !host.monitors.iter().any(|m| (m.width as f64) >= w && (m.height as f64) >= h) && !host.monitors.is_empty() {
        r.w("SCR-HOST-001", "fp.screen_exceeds_host");
    }
    if s("/gpu/render_class") != host.gpu_render_class { r.e("GPU-RCLASS-001", "fp.render_class"); }
    if host.gpu_vendor == crate::GpuVendorClass::Software { r.w("GPU-HOST-001", "fp.software_gpu"); }

    // Locale
    if let Some(tz) = &locale.timezone {
        if !geo::plausible_iana(tz) { r.e("LOC-TZ-001", "fp.timezone_invalid"); }
        if let Some(ptz) = &ctx.proxy_timezone { if ptz != tz { r.e("LOC-TZ-002", "fp.timezone_mismatch"); } }
    }
    if let (Some(pc), Some(first)) = (&ctx.proxy_country, locale.languages.first()) {
        let expected = geo::languages_for(pc);
        let lang = first.split('-').next().unwrap_or("");
        if !expected.iter().any(|l| l.split('-').next() == Some(lang)) && lang != "en" { r.w("LOC-LANG-002", "fp.language_country"); }
    }

    // Network
    if ctx.webrtc_real && ctx.has_proxy { r.e("NET-RTC-001", "fp.webrtc_leak"); }
    r
}
