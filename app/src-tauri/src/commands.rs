//! Tauri commands = the UI↔core contract (file 04 §5.3). Every data command checks the vault is
//! unlocked first. Nothing here returns a secret. All DB access goes through `AppState`.

use crate::state::AppState;
use core_db::Db;
use core_fp::{FingerprintSet, Seeds};
use core_model::*;
use core_vault::Key32;
use serde::Serialize;
use tauri::State;
use zeroize::Zeroize;

type R<T> = CmdResult<T>;

fn rand32() -> [u8; 32] { let mut b = [0u8; 32]; getrandom::fill(&mut b).expect("rng"); b }

// ---------- vault ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateResult { pub recovery_code: String }

#[tauri::command]
pub async fn vault_status(s: State<'_, AppState>) -> R<VaultStatus> {
    let v = s.vault.read().await;
    Ok(VaultStatus {
        exists: v.exists(), unlocked: v.is_unlocked(), hint: v.hint(),
        retry_after_secs: v.retry_after_secs(),
        autolock_minutes: s.with_db(|db| db.setting_get("autolock_minutes").map(|o| o.and_then(|v| v.as_u64().map(|n| n as u32)))).await.ok().flatten(),
    })
}

#[tauri::command]
pub async fn vault_create(s: State<'_, AppState>, password: String, hint: Option<String>) -> R<CreateResult> {
    let created = { s.vault.write().await.create(&password, hint, None)? };
    open_db(&s).await?;
    Ok(CreateResult { recovery_code: created.recovery_code })
}

#[tauri::command]
pub async fn vault_unlock(s: State<'_, AppState>, password: String) -> R<()> {
    { s.vault.write().await.unlock(&password)?; }
    open_db(&s).await
}

#[tauri::command]
pub async fn vault_unlock_recovery(s: State<'_, AppState>, code: String) -> R<()> {
    { s.vault.write().await.unlock_recovery(&code)?; }
    open_db(&s).await
}

#[tauri::command]
pub async fn vault_lock(s: State<'_, AppState>) -> R<()> {
    s.supervisor.stop_all();
    s.forwarders.lock().unwrap().clear();
    *s.db.write().await = None;
    s.vault.write().await.lock();
    Ok(())
}

#[tauri::command]
pub async fn vault_change_password(s: State<'_, AppState>, old: String, new: String, hint: Option<String>) -> R<()> {
    s.vault.write().await.change_password(&old, &new, hint)
}

async fn open_db(s: &State<'_, AppState>) -> R<()> {
    let key = { s.vault.read().await.require_unlocked()?.db_key() };
    std::fs::create_dir_all(&s.data_root).ok();
    let db = Db::open(&s.data_root.join("vault").join("vault.db"), &key)?;
    *s.db.write().await = Some(db);
    Ok(())
}

// ---------- profiles ----------

#[tauri::command]
pub async fn profiles_query(s: State<'_, AppState>, q: FilterQuery, page: PageReq) -> R<Page<ProfileRow>> {
    s.with_db(|db| db.profiles_query(&q, &page)).await
}
#[tauri::command]
pub async fn profiles_count(s: State<'_, AppState>, q: FilterQuery) -> R<u64> {
    s.with_db(|db| db.profiles_count(&q)).await
}
#[tauri::command]
pub async fn profile_get(s: State<'_, AppState>, id: ProfileId) -> R<ProfileDetail> {
    let mut d = s.with_db(|db| db.profile_get(&id)).await?;
    d.resolved = resolved_preview(&s, &d).await?;
    Ok(d)
}
#[tauri::command]
pub async fn profiles_ids_for(s: State<'_, AppState>, q: FilterQuery) -> R<Vec<ProfileId>> {
    s.with_db(|db| db.profile_ids_for(&q)).await
}

#[tauri::command]
pub async fn profile_create(s: State<'_, AppState>, draft: ProfileDraft) -> R<ProfileDetail> {
    let (host, engine) = (s.host.clone(), s.engine.clone());
    let id = {
        let vault = s.vault.read().await;
        let u = vault.require_unlocked()?;
        let mut secret = rand32();
        let pid = core_db::new_id();
        let wrapped = u.wrap_pdk(&pid, 1, &Key32::random());
        let r = s.with_db(|db| db.profile_create(&pid, &draft, &host, &engine, &secret, &wrapped)).await;
        secret.zeroize();
        r?
    };
    profile_get(s, id).await
}

#[tauri::command]
pub async fn profile_update(s: State<'_, AppState>, id: ProfileId, patch: ProfilePatch) -> R<ProfileDetail> {
    s.with_db(|db| db.profile_patch(&id, &patch)).await?;
    profile_get(s, id).await
}
#[tauri::command]
pub async fn profiles_restore(s: State<'_, AppState>, ids: Vec<ProfileId>) -> R<()> {
    s.with_db(|db| db.profiles_restore(&ids)).await
}
#[tauri::command]
pub async fn profiles_trash(s: State<'_, AppState>, ids: Vec<ProfileId>) -> R<()> {
    s.with_db(|db| db.profiles_trash(&ids)).await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateReq { pub count: u32, pub name_pattern: String, pub start_index: u64, pub draft: ProfileDraft }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkResult { pub created: usize, pub ids: Vec<ProfileId> }

#[tauri::command]
pub async fn bulk_create(s: State<'_, AppState>, req: BulkCreateReq) -> R<BulkResult> {
    let (host, engine) = (s.host.clone(), s.engine.clone());
    let vault = s.vault.read().await;
    let _ = vault.require_unlocked()?;
    let src = |pid: &str| {
        let secret = rand32();
        // Per-profile data key (PDK) for at-rest storage encryption, wrapped by K_wrap_pdk.
        let wrapped = vault.require_unlocked().unwrap().wrap_pdk(pid, 1, &Key32::random());
        (secret, wrapped)
    };
    let ids = s.with_db(|db| db.bulk_create(req.count, &req.name_pattern, req.start_index, &req.draft, &host, &engine, src, |_, _| {})).await?;
    Ok(BulkResult { created: ids.len(), ids })
}

// ---------- fingerprint preview ----------

async fn resolved_preview(s: &State<'_, AppState>, d: &ProfileDetail) -> R<serde_json::Value> {
    let set = s.with_db(|db| db.fp_set_payload(&d.fingerprint_set_id)).await?;
    let set = FingerprintSet(serde_json::from_slice(&set).map_err(|_| AppError::Internal)?);
    let seeds = Seeds::derive(&[0u8; 32], &d.fingerprint_set_id, 0); // display seed only; real seeds stay in core
    let locale = core_fp::LocaleInputs { timezone: d.timezone.clone(), languages: d.languages.clone(), country: None, geo: d.geo.as_ref().map(|g| (g.lat, g.lon, g.accuracy_m)) };
    Ok(core_fp::resolve(&set, &s.engine, &d.fp_overrides, &locale, &seeds))
}

// ---------- proxies ----------

#[tauri::command]
pub async fn proxies_list(s: State<'_, AppState>) -> R<Vec<Proxy>> { s.with_db(|db| db.proxies_list()).await }

#[tauri::command]
pub async fn proxies_parse(text: String, default_kind: Option<ProxyKind>) -> R<serde_json::Value> {
    let list = core_proxy::parse_list(&text, default_kind.unwrap_or(ProxyKind::Http));
    serde_json::to_value(list).map_err(|_| AppError::Internal)
}

/// Parse a pasted list and store every valid line (passwords sealed with K_secrets). Returns the count.
#[tauri::command]
pub async fn proxies_import(s: State<'_, AppState>, text: String, default_kind: Option<ProxyKind>) -> R<usize> {
    let list = core_proxy::parse_list(&text, default_kind.unwrap_or(ProxyKind::Http));
    let vault = s.vault.read().await;
    let u = vault.require_unlocked()?;
    let existing: std::collections::HashSet<(String, u16, Option<String>)> = s.with_db(|db| db.proxies_list()).await?
        .into_iter().map(|p| (p.host.unwrap_or_default(), p.port.unwrap_or(0), p.username)).collect();
    let mut n = 0;
    for p in list.items {
        if existing.contains(&(p.host.clone(), p.port, p.username.clone())) { continue; }
        let draft = ProxyDraft { id: None, label: None, kind: p.kind, host: p.host, port: p.port, username: p.username, password: p.password, rotate_url: None };
        let seal = |ctx: &str, pt: &[u8]| u.seal_secret(&format!("proxy:{ctx}"), pt);
        s.with_db(|db| db.proxy_upsert(&draft, seal)).await?;
        n += 1;
    }
    Ok(n)
}

#[tauri::command]
pub async fn proxy_upsert(s: State<'_, AppState>, draft: ProxyDraft) -> R<Proxy> {
    let id = {
        let vault = s.vault.read().await;
        let u = vault.require_unlocked()?;
        let seal = |ctx: &str, pt: &[u8]| u.seal_secret(&format!("proxy:{ctx}"), pt);
        s.with_db(|db| db.proxy_upsert(&draft, seal)).await?
    };
    s.with_db(|db| db.proxy_get(&id)).await
}

#[tauri::command]
pub async fn proxies_delete(s: State<'_, AppState>, ids: Vec<ProxyId>) -> R<()> {
    s.with_db(|db| db.proxies_delete(&ids)).await
}

#[tauri::command]
pub async fn proxy_check(s: State<'_, AppState>, id: ProxyId) -> R<ProxyCheckResult> {
    let secret = s.with_db(|db| db.proxy_secret(&id)).await?;
    let password = match &secret.password_enc {
        Some(blob) => {
            let vault = s.vault.read().await;
            let u = vault.require_unlocked()?;
            let mut pt = u.open_secret(&format!("proxy:{id}"), blob)?;
            let p = String::from_utf8(pt.clone()).map_err(|_| AppError::Internal)?;
            pt.zeroize();
            Some(p)
        }
        None => None,
    };
    let echo = s.with_db(|db| db.setting_get("proxy_echo_url").map(|o| o.and_then(|v| v.as_str().map(String::from)))).await?
        .unwrap_or_else(|| core_proxy::DEFAULT_ECHO.to_string());
    let ua = format!("Mozilla/5.0 ({}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36",
        if s.host.os_family == "windows" { "Windows NT 10.0; Win64; x64" } else { "X11; Linux x86_64" }, s.engine.major);
    let target = core_proxy::CheckTarget { kind: secret.kind, host: &secret.host, port: secret.port, username: secret.username.as_deref(), password: password.as_deref() };
    let res = core_proxy::check(&target, &echo, &ua).await;
    s.with_db(|db| db.proxy_record_check(&id, &res)).await?;
    Ok(res)
}

// ---------- organisation ----------

#[tauri::command]
pub async fn folders_list(s: State<'_, AppState>) -> R<Vec<Folder>> { s.with_db(|db| db.folders_list()).await }
#[tauri::command]
pub async fn folder_upsert(s: State<'_, AppState>, id: Option<FolderId>, parent_id: Option<FolderId>, name: String, color: Option<String>) -> R<Folder> {
    let fid = s.with_db(|db| db.folder_upsert(id.as_deref(), parent_id.as_deref(), &name, color.as_deref())).await?;
    s.with_db(|db| db.folders_list().map(|v| v.into_iter().find(|f| f.id == fid).unwrap())).await
}
#[tauri::command]
pub async fn folder_delete(s: State<'_, AppState>, id: FolderId) -> R<()> { s.with_db(|db| db.folder_delete(&id)).await }
#[tauri::command]
pub async fn tags_list(s: State<'_, AppState>) -> R<Vec<TagWithCount>> { s.with_db(|db| db.tags_list()).await }
#[tauri::command]
pub async fn tag_upsert(s: State<'_, AppState>, id: Option<TagId>, name: String, color: String) -> R<TagId> {
    s.with_db(|db| db.tag_upsert(id.as_deref(), &name, &color)).await
}
#[tauri::command]
pub async fn tag_delete(s: State<'_, AppState>, id: TagId) -> R<()> { s.with_db(|db| db.tag_delete(&id)).await }

// ---------- launch ----------

#[tauri::command]
pub async fn launch_profiles(s: State<'_, AppState>, ids: Vec<ProfileId>) -> R<Vec<String>> {
    crate::launch::launch_many(&s, ids).await
}
#[tauri::command]
pub async fn stop_profiles(s: State<'_, AppState>, ids: Vec<ProfileId>) -> R<()> {
    for id in ids { s.supervisor.stop(&id); s.forwarders.lock().unwrap().remove(&id); s.with_db(|db| db.set_status(&id, ProfileStatus::Idle)).await.ok(); }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSnapshot { pub running: usize, pub ram_used_mb: u64, pub ram_total_mb: u64 }

#[tauri::command]
pub async fn resource_snapshot(s: State<'_, AppState>) -> R<ResourceSnapshot> {
    Ok(ResourceSnapshot { running: s.supervisor.running_count(), ram_used_mb: 0, ram_total_mb: (s.host.physical_mem_gb * 1024.0) as u64 })
}

// ---------- settings / logs ----------

#[tauri::command]
pub async fn settings_get(s: State<'_, AppState>) -> R<serde_json::Value> {
    s.with_db(|db| db.settings_all().map(serde_json::Value::Object)).await
}
#[tauri::command]
pub async fn setting_set(s: State<'_, AppState>, key: String, value: serde_json::Value) -> R<()> {
    s.with_db(|db| db.setting_set(&key, &value)).await
}
#[tauri::command]
pub async fn logs_query(s: State<'_, AppState>, category: Option<String>, page: PageReq) -> R<Page<LogRow>> {
    s.with_db(|db| db.logs_query(category.as_deref(), &page)).await
}
#[tauri::command]
pub async fn engine_info(s: State<'_, AppState>) -> R<serde_json::Value> {
    let configured = s.with_db(|db| db.setting_get("enginePath")).await.ok().flatten().and_then(|v| v.as_str().map(String::from));
    let path = crate::launch::engine_exe(&s, configured);
    Ok(serde_json::json!({ "engineVersion": s.engine.full, "engineMajor": s.engine.major,
        "enginePath": path.display().to_string(), "engineInstalled": path.exists(),
        "host": { "osFamily": s.host.os_family, "osVersion": s.host.os_version, "gpu": s.host.gpu_label(),
                  "cores": s.host.logical_cores, "memGb": s.host.physical_mem_gb } }))
}
