#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Vanta manager — Tauri 2 host. The WebView only reaches the commands registered here.

mod commands;
mod host;
mod launch;
mod state;

use core_fp::{EngineVersion, HostMonitor};
use core_model::ProfileStatus;
use tauri::{Emitter, Manager};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data_root = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data_root)?;
            let monitors: Vec<HostMonitor> = app.get_webview_window("main")
                .and_then(|w| w.available_monitors().ok())
                .map(|ms| ms.iter().enumerate().map(|(i, m)| HostMonitor {
                    width: (m.size().width as f64 / m.scale_factor()).round() as u32,
                    height: (m.size().height as f64 / m.scale_factor()).round() as u32,
                    dpr: m.scale_factor(), primary: i == 0,
                }).collect()).unwrap_or_default();
            let host = host::probe(monitors);
            let engine_ver = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("engine").join("VERSION")))
                .and_then(|p| std::fs::read_to_string(p).ok()).map(|s| s.trim().to_string())
                .unwrap_or_else(|| include_str!("../../../engine/chromium_version.txt").trim().to_string());
            let engine = EngineVersion::parse(&engine_ver).unwrap_or(EngineVersion { major: 157, full: "157.0.8103.38".into() });
            let (st, mut exits) = state::AppState::new(data_root, host, engine);
            app.manage(st);
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(ev) = exits.recv().await {
                    let s = handle.state::<state::AppState>();
                    let status = if ev.crashed { ProfileStatus::Crashed } else { ProfileStatus::Idle };
                    let _ = s.with_db(|db| db.set_status(&ev.profile_id, status)).await;
                    let _ = handle.emit("profile:state", serde_json::json!({"id": ev.profile_id, "status": status.as_str(), "code": ev.code}));
                }
            });
            Ok(())
        })
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::Destroyed = e { w.app_handle().state::<state::AppState>().supervisor.stop_all(); }
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault_status,
            commands::vault_create,
            commands::vault_unlock,
            commands::vault_unlock_recovery,
            commands::vault_lock,
            commands::vault_change_password,
            commands::profiles_query,
            commands::profiles_count,
            commands::profile_get,
            commands::profiles_ids_for,
            commands::profile_create,
            commands::profile_update,
            commands::profiles_trash,
            commands::profiles_restore,
            commands::bulk_create,
            commands::proxies_list,
            commands::proxies_parse,
            commands::proxies_import,
            commands::proxy_upsert,
            commands::proxies_delete,
            commands::proxy_check,
            commands::folders_list,
            commands::folder_upsert,
            commands::folder_delete,
            commands::tags_list,
            commands::tag_upsert,
            commands::tag_delete,
            commands::launch_profiles,
            commands::stop_profiles,
            commands::resource_snapshot,
            commands::settings_get,
            commands::setting_set,
            commands::logs_query,
            commands::engine_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vanta");
}
