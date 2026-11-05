//! Profile launch pipeline (file 00 §7). Every step that can fail aborts *before* any traffic
//! leaves the machine: proxy egress probe → geo-match → validator → forwarder → prefs → spawn.

use crate::state::AppState;
use core_fp::{EngineVersion, FingerprintSet, LocaleInputs, geo, validate::LaunchCtx};
use core_launch::{LaunchSpec, Upstream, UpstreamKind, prefs};
use core_model::*;
use std::path::PathBuf;
use tauri::State;
use zeroize::Zeroize;

pub fn engine_exe(s: &AppState, configured: Option<String>) -> PathBuf {
    if let Some(p) = configured.filter(|p| !p.trim().is_empty()) { return PathBuf::from(p); }
    let base = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_default();
    let candidates = [base.join("engine").join("chrome.exe"), s.data_root.join("engine").join("chrome.exe")];
    candidates.iter().find(|p| p.exists()).cloned().unwrap_or_else(|| candidates[0].clone())
}

/// Default concurrency: ~1 profile per 1.5 GB RAM, at least 1, at most 50 (file 04 §11.1).
fn default_max_concurrent(mem_gb: f64) -> usize { ((mem_gb / 1.5) as usize).clamp(1, 50) }

pub async fn launch_many(s: &State<'_, AppState>, ids: Vec<ProfileId>) -> CmdResult<Vec<String>> {
    let settings = s.with_db(|db| db.settings_all()).await?;
    let max = settings.get("maxConcurrent").and_then(|v| v.as_u64()).map(|n| n as usize).unwrap_or_else(|| default_max_concurrent(s.host.physical_mem_gb));
    let exe = engine_exe(s, settings.get("enginePath").and_then(|v| v.as_str().map(String::from)));
    if !exe.exists() { return Err(AppError::Launch("launch.engine_missing".into())); }
    let mut results = Vec::new();
    for id in ids {
        if s.supervisor.is_running(&id) { results.push(format!("{id}:already_running")); continue; }
        if s.supervisor.running_count() >= max { results.push(format!("{id}:limit")); continue; }
        match launch_one(s, &id, &exe).await {
            Ok(()) => results.push(format!("{id}:started")),
            Err(e) => {
                s.with_db(|db| { db.log("error", "launch", Some(&id), "launch.failed", serde_json::json!({"reason": e.to_string()})); db.set_status(&id, ProfileStatus::Idle) }).await.ok();
                results.push(format!("{id}:{}", match &e { AppError::Launch(k) | AppError::Proxy(k) => k.clone(), other => other.to_string() }));
            }
        }
    }
    Ok(results)
}

async fn launch_one(s: &State<'_, AppState>, id: &str, exe: &PathBuf) -> CmdResult<()> {
    s.with_db(|db| db.set_status(id, ProfileStatus::Starting)).await?;
    let m = s.with_db(|db| db.launch_material(id)).await?;
    let set = FingerprintSet(serde_json::from_slice(&m.set_payload).map_err(|_| AppError::Internal)?);

    // 4. Network up: decrypt credentials, probe egress through the proxy.
    let mut upstream = None;
    let mut ctx = LaunchCtx { webrtc_real: m.webrtc_mode == "real", has_proxy: m.proxy_id.is_some(), ..Default::default() };
    let mut probe = None;
    if let Some(pid) = &m.proxy_id {
        let sec = s.with_db(|db| db.proxy_secret(pid)).await?;
        let password = match &sec.password_enc {
            Some(blob) => {
                let vault = s.vault.read().await;
                let mut pt = vault.require_unlocked()?.open_secret(&format!("proxy:{pid}"), blob)?;
                let p = String::from_utf8(pt.clone()).map_err(|_| AppError::Internal);
                pt.zeroize();
                Some(p?)
            }
            None => None,
        };
        let kind = match sec.kind { ProxyKind::Socks5 => UpstreamKind::Socks5, ProxyKind::Http | ProxyKind::Https => UpstreamKind::HttpConnect,
                                     _ => return Err(AppError::Launch("launch.proxy_kind_unsupported".into())) };
        let ua = format!("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36", s.engine.major);
        let target = core_proxy::CheckTarget { kind: sec.kind, host: &sec.host, port: sec.port, username: sec.username.as_deref(), password: password.as_deref() };
        let res = core_proxy::check(&target, core_proxy::DEFAULT_ECHO, &ua).await;
        s.with_db(|db| db.proxy_record_check(pid, &res)).await.ok();
        if !matches!(res.status, ProxyStatus::Ok | ProxyStatus::Slow) {
            // Kill switch: no working upstream → no launch (never DIRECT).
            return Err(AppError::Proxy(res.error.unwrap_or_else(|| "proxy.connect_failed".into())));
        }
        ctx.proxy_country = res.country.clone();
        ctx.proxy_timezone = res.timezone.clone();
        upstream = Some(Upstream { kind, host: sec.host.clone(), port: sec.port, username: sec.username.clone(), password });
        probe = Some(res);
    }

    // 5. Geo-match from the exit (auto mode), persisted for the next launch.
    let (mut tz, mut langs) = (m.timezone.clone(), m.languages.clone());
    if m.geo_mode == "auto_from_proxy" {
        if let Some(p) = &probe {
            tz = p.timezone.clone().or_else(|| p.country.as_deref().and_then(geo::single_timezone).map(String::from));
            if let Some(cc) = &p.country { langs = geo::languages_for(cc); }
            s.with_db(|db| db.set_geo(id, tz.as_deref(), &langs, p.lat, p.lon, p.country.as_deref())).await?;
        }
    }
    if langs.is_empty() { langs = vec!["en-US".into(), "en".into()]; }

    // 7. Validate (errors abort when the guard is on).
    let locale = LocaleInputs { timezone: tz.clone(), languages: langs.clone(), country: ctx.proxy_country.clone(), geo: None };
    let report = core_fp::validate::validate(&set, &s.host, &EngineVersion { major: s.engine.major, full: s.engine.full.clone() }, &locale, &ctx);
    if !report.ok() && m.block_on_mismatch {
        let first = report.errors().next().map(|i| i.message_key).unwrap_or("fp.invalid");
        return Err(AppError::Launch(first.into()));
    }

    // Local gateway (credentials stay in the core).
    let ipv6_block = m.ipv6_mode != "allow";
    let fw = match upstream { Some(up) => Some(core_launch::Forwarder::start(up, ipv6_block).await.map_err(|_| AppError::Launch("launch.gateway_failed".into()))?), None => None };

    // 9. Prefs + directories.
    let pdir = s.data_root.join("profiles").join(id);
    let ud = pdir.join("ud");
    let downloads = pdir.join("downloads");
    std::fs::create_dir_all(&ud)?;
    std::fs::create_dir_all(&downloads)?;
    let accept = geo::accept_language(&langs);
    let webrtc_policy = match m.webrtc_mode.as_str() { "real" => "default", "public_only" => "default_public_interface_only", _ => "disable_non_proxied_udp" };
    prefs::write(&ud, &prefs::PrefInputs { accept_languages: &accept, webrtc_policy, download_dir: &downloads })?;

    // 10. Spawn.
    let mon = s.host.primary_monitor();
    let spec = LaunchSpec {
        engine_exe: exe.clone(), user_data_dir: ud, socks_port: fw.as_ref().map(|f| f.port),
        ui_lang: langs[0].clone(), webrtc_block_udp: m.webrtc_mode != "real",
        window: Some(((mon.width as f64 / mon.dpr * 0.8) as u32, (mon.height as f64 / mon.dpr * 0.85) as u32)),
        start_urls: m.start_urls.clone(), config_file: None,
    };
    let keep: Box<dyn Send> = Box::new(fw);
    s.supervisor.spawn(id, &spec, m.ram_limit_mb, keep).map_err(|_| AppError::Launch("launch.spawn_failed".into()))?;
    s.with_db(|db| { db.mark_launched(id)?; db.log("info", "launch", Some(id), "launch.started", serde_json::json!({"proxy": m.proxy_id.is_some(), "tz": tz})); Ok(()) }).await?;
    Ok(())
}
