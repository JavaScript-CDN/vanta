fn main() {
    // Every UI-callable command is listed here; Tauri generates an `allow-<cmd>` permission for
    // each and denies anything not granted in capabilities/ (file 04 §5.3 / task 1.1).
    const COMMANDS: &[&str] = &["vault_status", "vault_create", "vault_unlock", "vault_unlock_recovery", "vault_lock", "vault_change_password", "profiles_query", "profiles_count", "profile_get", "profiles_ids_for", "profile_create", "profile_update", "profiles_trash", "bulk_create", "proxies_list", "proxies_parse", "proxy_upsert", "proxies_delete", "proxy_check", "folders_list", "folder_upsert", "folder_delete", "tags_list", "tag_upsert", "tag_delete", "launch_profiles", "stop_profiles", "resource_snapshot", "settings_get", "setting_set", "logs_query", "engine_info"];
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)))
        .expect("tauri build");
}
