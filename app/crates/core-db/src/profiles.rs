//! Profile CRUD, query, and bulk create (file 04 §5, §10).
use crate::{Db, filter, new_id, now_ms, sql_err};
use core_fp::{EngineVersion, FingerprintSet, HostProfile, Seeds, identity_hash};
use core_model::{
    AppError, CmdResult, FilterQuery, HealthLevel, HealthRef, Page, PageReq, Platform, ProfileDetail,
    ProfileDraft, ProfileId, ProfilePatch, ProfileRow, ProfileStatus, ProxyRef, TagRef,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};
use zeroize::Zeroize;

/// Launch inputs; `secret` is zeroized on drop.
pub struct LaunchMaterial {
    pub secret: [u8; 32], pub set_id: String, pub noise_epoch: u32, pub set_payload: Vec<u8>, pub proxy_id: Option<String>,
    pub timezone: Option<String>, pub languages: Vec<String>, pub webrtc_mode: String, pub ipv6_mode: String,
    pub start_urls: Vec<String>, pub ram_limit_mb: Option<u32>, pub geo_mode: String, pub block_on_mismatch: bool, pub overrides: Value,
}
impl Drop for LaunchMaterial { fn drop(&mut self) { self.secret.zeroize(); } }

const SELECT_ROW: &str = r#"
SELECT p.id, p.name, p.folder_id, p.color, p.status, p.platform, fs.os_family, fs.os_version,
       fs.chrome_major_min, p.notes, p.last_launched_at, p.pinned, p.updated_at,
       p.health_score, p.health_level, p.health_checked_at,
       px.id, px.label, px.exit_country, px.last_status
FROM profiles p
JOIN fingerprint_sets fs ON fs.id = p.fingerprint_set_id
LEFT JOIN proxies px ON px.id = p.proxy_id
"#;

fn row_from(r: &rusqlite::Row, tags: Vec<TagRef>) -> rusqlite::Result<ProfileRow> {
    let notes: Option<String> = r.get(9)?;
    let proxy = r.get::<_, Option<String>>(16)?.map(|id| Ok::<_, rusqlite::Error>(ProxyRef {
        id, label: r.get(17)?, country: r.get(18)?, status: r.get::<_, Option<String>>(19)?.unwrap_or_else(|| "unknown".into()),
    })).transpose()?;
    let health = r.get::<_, Option<i64>>(13)?.map(|score| Ok::<_, rusqlite::Error>(HealthRef {
        score, level: r.get::<_, Option<String>>(14)?.as_deref().and_then(HealthLevel::parse).unwrap_or(HealthLevel::Yellow), checked_at: r.get::<_, Option<i64>>(15)?.unwrap_or(0),
    })).transpose()?;
    Ok(ProfileRow {
        id: r.get(0)?, name: r.get(1)?, folder_id: r.get(2)?, color: r.get(3)?, tags,
        status: ProfileStatus::parse(&r.get::<_, String>(4)?).unwrap_or(ProfileStatus::Idle),
        platform: Platform::parse(&r.get::<_, String>(5)?).unwrap_or(Platform::Desktop),
        os_family: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
        os_version: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
        browser_major: r.get::<_, Option<i64>>(8)?.unwrap_or(0) as u32,
        proxy, health, notes_preview: notes.map(|n| n.chars().take(80).collect()),
        last_launched_at: r.get(10)?, pinned: r.get::<_, i64>(11)? != 0, updated_at: r.get(12)?,
    })
}

fn tags_for(conn: &Connection, ids: &[&str]) -> rusqlite::Result<std::collections::HashMap<String, Vec<TagRef>>> {
    let mut map: std::collections::HashMap<String, Vec<TagRef>> = std::collections::HashMap::new();
    if ids.is_empty() { return Ok(map); }
    let ph = (0..ids.len()).map(|i| format!("?{}", i + 1)).collect::<Vec<_>>().join(",");
    let sql = format!("SELECT pt.profile_id, t.id, t.name, t.color FROM profile_tags pt JOIN tags t ON t.id=pt.tag_id WHERE pt.profile_id IN ({ph})");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |r| Ok((r.get::<_, String>(0)?, TagRef { id: r.get(1)?, name: r.get(2)?, color: r.get(3)? })))?;
    for row in rows { let (pid, t) = row?; map.entry(pid).or_default().push(t); }
    Ok(map)
}

impl Db {
    pub fn profiles_count(&self, q: &FilterQuery) -> CmdResult<u64> {
        let c = filter::compile(q);
        let sql = format!("SELECT count(*) FROM profiles p JOIN fingerprint_sets fs ON fs.id=p.fingerprint_set_id LEFT JOIN proxies px ON px.id=p.proxy_id WHERE {}", c.where_sql);
        let conn = self.conn();
        conn.query_row(&sql, rusqlite::params_from_iter(c.params.iter()), |r| r.get::<_, i64>(0)).map(|n| n as u64).map_err(sql_err)
    }

    pub fn profiles_query(&self, q: &FilterQuery, page: &PageReq) -> CmdResult<Page<ProfileRow>> {
        let total = self.profiles_count(q)?;
        let c = filter::compile(q);
        let limit = page.limit.clamp(1, 1000);
        let sql = format!("{SELECT_ROW} WHERE {} {} LIMIT ?{} OFFSET ?{}", c.where_sql, c.order_sql, c.params.len() + 1, c.params.len() + 2);
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql).map_err(sql_err)?;
        let mut params = c.params.clone();
        params.push((limit as i64).into());
        params.push((page.offset as i64).into());
        let base: Vec<ProfileRow> = stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| row_from(r, vec![]))
            .map_err(sql_err)?.collect::<Result<_, _>>().map_err(sql_err)?;
        let ids: Vec<&str> = base.iter().map(|r| r.id.as_str()).collect();
        let mut tags = tags_for(&conn, &ids).map_err(sql_err)?;
        let items = base.into_iter().map(|mut r| { r.tags = tags.remove(&r.id).unwrap_or_default(); r }).collect();
        Ok(Page { items, total, offset: page.offset, limit })
    }

    /// Resolve all profile ids matching a query (for "select all matching" — file 04 §7.3).
    pub fn profile_ids_for(&self, q: &FilterQuery) -> CmdResult<Vec<ProfileId>> {
        let c = filter::compile(q);
        let sql = format!("SELECT p.id FROM profiles p JOIN fingerprint_sets fs ON fs.id=p.fingerprint_set_id LEFT JOIN proxies px ON px.id=p.proxy_id WHERE {} {}", c.where_sql, c.order_sql);
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql).map_err(sql_err)?;
        stmt.query_map(rusqlite::params_from_iter(c.params.iter()), |r| r.get(0)).map_err(sql_err)?.collect::<Result<_, _>>().map_err(sql_err)
    }

    pub fn profile_get(&self, id: &str) -> CmdResult<ProfileDetail> {
        let conn = self.conn();
        let sql = format!("{SELECT_ROW} WHERE p.id = ?1");
        let mut row = conn.query_row(&sql, [id], |r| row_from(r, vec![])).optional().map_err(sql_err)?.ok_or_else(|| AppError::NotFound(id.into()))?;
        row.tags = tags_for(&conn, &[id]).map_err(sql_err)?.remove(id).unwrap_or_default();
        let (notes, fp_set, overrides, engine, policy, geo_mode, tz, locale, langs, lat, lon, acc, webrtc, dns, ipv6, block, urls, exts, ram, cpu, prio, pw, created, resolved):
            (Option<String>, String, String, String, String, String, Option<String>, Option<String>, String, Option<f64>, Option<f64>, Option<f64>, String, String, String, i64, String, String, Option<i64>, Option<i64>, i64, i64, i64, String) =
            conn.query_row(
                "SELECT notes, fingerprint_set_id, fp_overrides, engine_version, engine_policy, geo_mode, timezone, locale, languages, geo_lat, geo_lon, geo_accuracy_m, webrtc_mode, dns_mode, ipv6_mode, block_launch_on_mismatch, start_urls, extensions, ram_limit_mb, cpu_limit_pct, launch_priority, password_protected, created_at, '' FROM profiles WHERE id=?1",
                [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?, r.get(13)?, r.get(14)?, r.get(15)?, r.get(16)?, r.get(17)?, r.get(18)?, r.get(19)?, r.get(20)?, r.get(21)?, r.get(22)?, r.get(23)?))
            ).map_err(sql_err)?;
        let jarr = |s: &str| serde_json::from_str::<Vec<String>>(s).unwrap_or_default();
        let _ = resolved;
        Ok(ProfileDetail {
            row, notes, fingerprint_set_id: fp_set, fp_overrides: serde_json::from_str(&overrides).unwrap_or(json!({})),
            resolved: json!(null), engine_version: engine, engine_policy: policy,
            geo_mode: core_model::GeoMode::parse(&geo_mode).unwrap_or(core_model::GeoMode::AutoFromProxy),
            timezone: tz, locale, languages: jarr(&langs),
            geo: match (lat, lon) { (Some(a), Some(b)) => Some(core_model::GeoPoint { lat: a, lon: b, accuracy_m: acc.unwrap_or(1000.0) }), _ => None },
            webrtc_mode: core_model::WebRtcMode::parse(&webrtc).unwrap_or(core_model::WebRtcMode::Proxied),
            dns_mode: core_model::DnsMode::parse(&dns).unwrap_or(core_model::DnsMode::ViaProxy),
            ipv6_mode: core_model::Ipv6Mode::parse(&ipv6).unwrap_or(core_model::Ipv6Mode::BlockIfProxyV4),
            block_launch_on_mismatch: block != 0, start_urls: jarr(&urls), extensions: jarr(&exts),
            ram_limit_mb: ram.map(|v| v as u32), cpu_limit_pct: cpu.map(|v| v as u32), launch_priority: prio as i32,
            password_protected: pw != 0, created_at: created,
        })
    }

    /// Create one profile. `secret` is the profile's 32-byte secret (caller generated + will store it wrapped).
    pub fn profile_create(&self, id: &str, draft: &ProfileDraft, host: &HostProfile, engine: &EngineVersion, secret: &[u8; 32], wrapped_key: &[u8]) -> CmdResult<ProfileId> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(sql_err)?;
        let id = insert_one(&tx, id, draft, host, engine, secret, wrapped_key)?;
        tx.commit().map_err(sql_err)?;
        Ok(id)
    }

    pub fn profile_patch(&self, id: &str, patch: &ProfilePatch) -> CmdResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(sql_err)?;
        macro_rules! set { ($col:literal, $v:expr) => { tx.execute(concat!("UPDATE profiles SET ", $col, "=?1, updated_at=?2 WHERE id=?3"), params![$v, now_ms(), id]).map_err(sql_err)?; }; }
        if let Some(v) = &patch.name { set!("name", v); }
        if let Some(v) = &patch.folder_id { set!("folder_id", v); }
        if let Some(v) = &patch.color { set!("color", v); }
        if let Some(v) = &patch.notes { set!("notes", v); }
        if let Some(v) = &patch.proxy_id { set!("proxy_id", v); }
        if let Some(v) = patch.pinned { set!("pinned", v as i64); }
        if let Some(v) = patch.launch_priority { set!("launch_priority", v); }
        if let Some(v) = &patch.timezone { set!("timezone", v); }
        if let Some(v) = &patch.locale { set!("locale", v); }
        if let Some(v) = &patch.languages { set!("languages", serde_json::to_string(v).unwrap()); }
        if let Some(v) = &patch.start_urls { set!("start_urls", serde_json::to_string(v).unwrap()); }
        if let Some(v) = patch.block_launch_on_mismatch { set!("block_launch_on_mismatch", v as i64); }
        if let Some(v) = patch.webrtc_mode { set!("webrtc_mode", v.as_str()); }
        if let Some(v) = patch.ram_limit_mb { set!("ram_limit_mb", v); }
        if let Some(v) = patch.cpu_limit_pct { set!("cpu_limit_pct", v); }
        if let Some(tags) = &patch.tag_ids {
            tx.execute("DELETE FROM profile_tags WHERE profile_id=?1", [id]).map_err(sql_err)?;
            for t in tags { tx.execute("INSERT OR IGNORE INTO profile_tags(profile_id, tag_id) VALUES(?1,?2)", params![id, t]).map_err(sql_err)?; }
        }
        update_fts(&tx, id)?;
        tx.commit().map_err(sql_err)?;
        Ok(())
    }

    pub fn profiles_trash(&self, ids: &[String]) -> CmdResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(sql_err)?;
        for id in ids { tx.execute("UPDATE profiles SET deleted_at=?1, updated_at=?1 WHERE id=?2", params![now_ms(), id]).map_err(sql_err)?; }
        tx.commit().map_err(sql_err)?;
        Ok(())
    }

    pub fn fp_set_payload(&self, set_id: &str) -> CmdResult<Vec<u8>> {
        self.conn().query_row("SELECT payload FROM fingerprint_sets WHERE id=?1", [set_id], |r| r.get(0)).map_err(sql_err)
    }

    /// Everything the launcher needs, read in one place (never leaves the core).
    pub fn launch_material(&self, id: &str) -> CmdResult<LaunchMaterial> {
        self.conn().query_row(
            "SELECT p.fp_seed, p.fingerprint_set_id, p.noise_epoch, fs.payload, p.proxy_id, p.timezone, p.languages, p.webrtc_mode, p.ipv6_mode, p.start_urls, p.ram_limit_mb, p.geo_mode, p.block_launch_on_mismatch, p.fp_overrides FROM profiles p JOIN fingerprint_sets fs ON fs.id=p.fingerprint_set_id WHERE p.id=?1 AND p.deleted_at IS NULL",
            [id], |r| {
                let seed: Vec<u8> = r.get(0)?;
                let mut secret = [0u8; 32];
                if seed.len() == 32 { secret.copy_from_slice(&seed); }
                Ok(LaunchMaterial {
                    secret, set_id: r.get(1)?, noise_epoch: r.get::<_, i64>(2)? as u32, set_payload: r.get(3)?, proxy_id: r.get(4)?,
                    timezone: r.get(5)?, languages: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
                    webrtc_mode: r.get(7)?, ipv6_mode: r.get(8)?, start_urls: serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or_default(),
                    ram_limit_mb: r.get::<_, Option<i64>>(10)?.map(|v| v as u32), geo_mode: r.get(11)?, block_on_mismatch: r.get::<_, i64>(12)? != 0,
                    overrides: serde_json::from_str(&r.get::<_, String>(13)?).unwrap_or(json!({})),
                })
            }).optional().map_err(sql_err)?.ok_or_else(|| AppError::NotFound(id.into()))
    }

    pub fn mark_launched(&self, id: &str) -> CmdResult<()> {
        self.conn().execute("UPDATE profiles SET last_launched_at=?1, launch_count=launch_count+1, status='running' WHERE id=?2", params![now_ms(), id]).map(|_| ()).map_err(sql_err)
    }

    pub fn set_geo(&self, id: &str, tz: Option<&str>, langs: &[String], lat: Option<f64>, lon: Option<f64>, country: Option<&str>) -> CmdResult<()> {
        self.conn().execute("UPDATE profiles SET timezone=?1, languages=?2, geo_lat=?3, geo_lon=?4, geo_accuracy_m=1500, last_exit_country=?5, updated_at=?6 WHERE id=?7",
            params![tz, serde_json::to_string(langs).unwrap(), lat, lon, country, now_ms(), id]).map(|_| ()).map_err(sql_err)
    }

    pub fn set_status(&self, id: &str, status: ProfileStatus) -> CmdResult<()> {
        self.conn().execute("UPDATE profiles SET status=?1, updated_at=?2 WHERE id=?3", params![status.as_str(), now_ms(), id]).map(|_| ()).map_err(sql_err)
    }

    /// Bulk create (file 04 §10.1). The DB-level unique index on `profile_fp_identity.identity_hash`
    /// guarantees zero collisions; this resamples a profile's set up to 32 times to find a free identity.
    pub fn bulk_create(&self, count: u32, name_pattern: &str, start_index: u64, draft: &ProfileDraft,
                       host: &HostProfile, engine: &EngineVersion,
                       mut secret_source: impl FnMut(&str) -> ([u8; 32], Vec<u8>),
                       mut progress: impl FnMut(u32, u32)) -> CmdResult<Vec<ProfileId>> {
        if count == 0 || count > 10_000 { return Err(AppError::Limit("count 1..10000".into())); }
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(sql_err)?;
        let mut ids = Vec::with_capacity(count as usize);
        let mut failed = 0u32;
        for i in 0..count {
            let name = render_name(name_pattern, start_index + i as u64, draft);
            let mut d = draft.clone();
            d.name = name;
            let pid = new_id();
            let (secret, wrapped) = secret_source(&pid);
            match insert_one(&tx, &pid, &d, host, engine, &secret, &wrapped) {
                Ok(id) => ids.push(id),
                Err(AppError::Conflict(_)) => { failed += 1; } // identity collision after retries: skip
                Err(e) => return Err(e),
            }
            if i % 100 == 0 || i + 1 == count { progress(i + 1, failed); }
        }
        if ids.is_empty() { return Err(AppError::Limit("no unique identities available for these constraints".into())); }
        tx.commit().map_err(sql_err)?;
        Ok(ids)
    }
}

fn render_name(pattern: &str, n: u64, draft: &ProfileDraft) -> String {
    let mut out = pattern.replace("{n}", &n.to_string());
    // zero-padded {n:0000}
    while let Some(start) = out.find("{n:") {
        let end = out[start..].find('}').map(|e| start + e).unwrap_or(out.len());
        let width = out[start + 3..end].chars().filter(|c| *c == '0').count();
        out.replace_range(start..=end.min(out.len() - 1), &format!("{:0width$}", n, width = width));
    }
    out.replace("{country}", draft.timezone.as_deref().unwrap_or("")).trim().to_string()
}

fn insert_one(tx: &Transaction, id: &str, draft: &ProfileDraft, host: &HostProfile, engine: &EngineVersion, secret: &[u8; 32], wrapped_key: &[u8]) -> CmdResult<ProfileId> {
    // Find a fingerprint set + seeds whose identity hash is not already taken (resample up to 32×).
    let (set, seeds, noise_epoch) = {
        let mut chosen = None;
        for epoch in 0..32u32 {
            let builder_key = Seeds::set_builder_key(secret);
            // Vary the builder key per attempt so a retry draws a different set.
            let mut k = builder_key;
            k[0] ^= epoch as u8;
            let set = FingerprintSet::native_for_host(host, k);
            let seeds = Seeds::derive(secret, &set.id(), epoch);
            let idh = identity_hash(&set, &seeds);
            let taken: bool = tx.query_row("SELECT count(*) FROM profile_fp_identity WHERE identity_hash=?1", [&idh[..]], |r| r.get::<_, i64>(0)).map_err(sql_err)? > 0;
            if !taken { chosen = Some((set, seeds, epoch)); break; }
        }
        chosen.ok_or_else(|| AppError::Conflict("identity".into()))?
    };
    ensure_fp_set(tx, &set, host, engine)?;

    let id = id.to_string();
    let now = now_ms();
    let languages = draft.languages.clone().unwrap_or_else(|| vec!["en-US".into(), "en".into()]);
    tx.execute(
        "INSERT INTO profiles (id,name,folder_id,color,notes,status,platform,fingerprint_set_id,fp_seed,fp_overrides,engine_version,engine_policy,proxy_id,geo_mode,timezone,locale,languages,webrtc_mode,start_urls,extensions,ram_limit_mb,cpu_limit_pct,storage_key_wrapped,created_at,updated_at,noise_epoch)
         VALUES (?1,?2,?3,?4,?5,'idle',?6,?7,?8,?9,?10,'follow_latest',?11,?12,?13,?14,?15,?16,'[]','[]',?17,?18,?19,?20,?20,?21)",
        params![id, draft.name, draft.folder_id, draft.color, draft.notes, draft.platform.as_str(), set.id(),
                &secret[..], draft.fp_overrides.clone().unwrap_or(json!({})).to_string(), engine.full,
                draft.proxy_id, draft.geo_mode.as_str(), draft.timezone, draft.locale,
                serde_json::to_string(&languages).unwrap(), draft.webrtc_mode.as_str(),
                draft.ram_limit_mb, draft.cpu_limit_pct, wrapped_key, now, noise_epoch],
    ).map_err(sql_err)?;
    let idh = identity_hash(&set, &seeds);
    tx.execute("INSERT INTO profile_fp_identity(profile_id, identity_hash) VALUES(?1,?2)", params![id, &idh[..]]).map_err(sql_err)?;
    for t in &draft.tag_ids { tx.execute("INSERT OR IGNORE INTO profile_tags(profile_id, tag_id) VALUES(?1,?2)", params![id, t]).map_err(sql_err)?; }
    insert_fts(tx, &id, &draft.name, draft.notes.as_deref())?;
    Ok(id)
}

fn ensure_fp_set(tx: &Transaction, set: &FingerprintSet, host: &HostProfile, engine: &EngineVersion) -> CmdResult<()> {
    let id = set.id();
    let exists: bool = tx.query_row("SELECT count(*) FROM fingerprint_sets WHERE id=?1", [&id], |r| r.get::<_, i64>(0)).map_err(sql_err)? > 0;
    if exists { return Ok(()); }
    let num = |p: &str| set.get(p).and_then(Value::as_f64).unwrap_or(0.0);
    let mut payload = zstd_json(set);
    tx.execute(
        "INSERT INTO fingerprint_sets (id,pack_id,os_family,os_version,device_class,chrome_major_min,chrome_major_max,gpu_vendor,gpu_renderer,screen_w,screen_h,dpr,cpu_cores,device_memory_gb,weight,payload,created_at)
         VALUES (?1,'native',?2,?3,?4,?5,?5,?6,?7,?8,?9,?10,?11,?12,1.0,?13,?14)",
        params![id, set.os_family(), set.os_version(),
                set.get("/device/form_factor").and_then(Value::as_str).unwrap_or("desktop"),
                engine.major as i64, host.gpu_vendor.as_str(), host.gpu_render_class,
                num("/screen/width") as i64, num("/screen/height") as i64, num("/screen/dpr"),
                num("/device/hardware_concurrency") as i64, num("/device/device_memory_gb"),
                payload, now_ms()],
    ).map_err(sql_err)?;
    payload.zeroize();
    Ok(())
}

fn zstd_json(set: &FingerprintSet) -> Vec<u8> { core_fp::canonical_json(&set.0) }

fn insert_fts(tx: &Transaction, id: &str, name: &str, notes: Option<&str>) -> CmdResult<()> {
    tx.execute("INSERT INTO profiles_fts(profile_id,name,notes,tags,proxy) VALUES(?1,?2,?3,'','')", params![id, name, notes.unwrap_or("")]).map_err(sql_err)?;
    Ok(())
}

fn update_fts(tx: &Transaction, id: &str) -> CmdResult<()> {
    let row: Option<(String, Option<String>)> = tx.query_row("SELECT name, notes FROM profiles WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(sql_err)?;
    if let Some((name, notes)) = row {
        let tags: String = {
            let mut s = tx.prepare("SELECT t.name FROM profile_tags pt JOIN tags t ON t.id=pt.tag_id WHERE pt.profile_id=?1").map_err(sql_err)?;
            let v: Vec<String> = s.query_map([id], |r| r.get(0)).map_err(sql_err)?.collect::<Result<_, _>>().map_err(sql_err)?;
            v.join(" ")
        };
        tx.execute("DELETE FROM profiles_fts WHERE profile_id=?1", [id]).map_err(sql_err)?;
        tx.execute("INSERT INTO profiles_fts(profile_id,name,notes,tags,proxy) VALUES(?1,?2,?3,?4,'')", params![id, name, notes.unwrap_or_default(), tags]).map_err(sql_err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::db;

    fn secret_src() -> impl FnMut(&str) -> ([u8; 32], Vec<u8>) {
        let mut n = 0u64;
        move |_| { n += 1; let mut s = [0u8; 32]; s[..8].copy_from_slice(&n.to_be_bytes()); (s, vec![0u8; 48]) }
    }

    #[test]
    fn create_query_bulk_unique() {
        let (_d, db) = db();
        let host = HostProfile::example_windows();
        let engine = EngineVersion::parse("157.0.8103.38").unwrap();
        let mut src = secret_src();
        let (s, w) = src("x");
        let draft = ProfileDraft { name: "First".into(), folder_id: None, color: None, notes: Some("ads shop".into()), platform: Platform::Desktop, fingerprint_set_id: None, os_family: Some("windows".into()), fp_overrides: None, proxy_id: None, geo_mode: core_model::GeoMode::AutoFromProxy, timezone: None, locale: None, languages: None, geo: None, webrtc_mode: core_model::WebRtcMode::Proxied, start_urls: vec![], tag_ids: vec![], ram_limit_mb: None, cpu_limit_pct: None };
        let id = db.profile_create(&crate::new_id(), &draft, &host, &engine, &s, &w).unwrap();
        let got = db.profile_get(&id).unwrap();
        assert_eq!(got.row.name, "First");
        assert_eq!(got.engine_version, "157.0.8103.38");

        let ids = db.bulk_create(500, "Shop-{n:0000}", 1, &draft, &host, &engine, secret_src(), |_, _| {}).unwrap();
        assert_eq!(ids.len(), 500);
        let distinct: i64 = db.conn().query_row("SELECT count(DISTINCT identity_hash) FROM profile_fp_identity", [], |r| r.get(0)).unwrap();
        assert_eq!(distinct, 501, "every identity unique");

        let q = FilterQuery { text: Some("shop".into()), ..Default::default() };
        assert_eq!(db.profiles_count(&q).unwrap(), 501);
        let page = db.profiles_query(&FilterQuery::default(), &PageReq { offset: 0, limit: 50 }).unwrap();
        assert_eq!(page.total, 501);
        assert_eq!(page.items.len(), 50);
        assert!(page.items[0].name.starts_with("Shop-") || page.items[0].name == "First");
    }

    #[test]
    fn patch_and_trash() {
        let (_d, db) = db();
        let host = HostProfile::example_windows();
        let engine = EngineVersion::parse("157.0.8103.38").unwrap();
        let draft = ProfileDraft { name: "P".into(), folder_id: None, color: None, notes: None, platform: Platform::Desktop, fingerprint_set_id: None, os_family: None, fp_overrides: None, proxy_id: None, geo_mode: core_model::GeoMode::Manual, timezone: None, locale: None, languages: None, geo: None, webrtc_mode: core_model::WebRtcMode::Proxied, start_urls: vec![], tag_ids: vec![], ram_limit_mb: None, cpu_limit_pct: None };
        let id = db.profile_create(&crate::new_id(), &draft, &host, &engine, &[1; 32], &[0; 48]).unwrap();
        db.profile_patch(&id, &ProfilePatch { name: Some("Renamed".into()), pinned: Some(true), ..Default::default() }).unwrap();
        assert_eq!(db.profile_get(&id).unwrap().row.name, "Renamed");
        db.profiles_trash(&[id.clone()]).unwrap();
        assert_eq!(db.profiles_count(&FilterQuery::default()).unwrap(), 0);
        assert_eq!(db.profiles_count(&FilterQuery { in_trash: Some(true), ..Default::default() }).unwrap(), 1);
    }
}
