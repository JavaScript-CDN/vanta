//! Proxy storage (file 04 §4.3 `proxies`). Passwords are sealed with K_secrets on top of SQLCipher.
use crate::{Db, new_id, now_ms, sql_err};
use core_model::{AppError, CmdResult, Proxy, ProxyCheckResult, ProxyDraft, ProxyKind, ProxyStatus};
use rusqlite::{OptionalExtension, params};

/// Credentials needed to connect; only ever used inside the core.
pub struct ProxySecret { pub kind: ProxyKind, pub host: String, pub port: u16, pub username: Option<String>, pub password_enc: Option<Vec<u8>> }

const COLS: &str = "px.id, px.label, px.kind, px.host, px.port, px.username, px.password_enc IS NOT NULL, px.rotate_url, px.last_status, px.last_latency_ms, px.last_check_at, px.exit_ip, px.exit_country, px.exit_city, px.exit_timezone, px.exit_asn, px.exit_isp, (SELECT count(*) FROM profiles p WHERE p.proxy_id = px.id AND p.deleted_at IS NULL)";

fn map(r: &rusqlite::Row) -> rusqlite::Result<Proxy> {
    Ok(Proxy {
        id: r.get(0)?, label: r.get(1)?,
        kind: ProxyKind::parse(&r.get::<_, String>(2)?).unwrap_or(ProxyKind::Http),
        host: r.get(3)?, port: r.get::<_, Option<i64>>(4)?.map(|p| p as u16), username: r.get(5)?,
        has_password: r.get(6)?, rotate_url: r.get(7)?,
        last_status: r.get::<_, Option<String>>(8)?.as_deref().and_then(ProxyStatus::parse).unwrap_or(ProxyStatus::Unknown),
        last_latency_ms: r.get(9)?, last_check_at: r.get(10)?, exit_ip: r.get(11)?, exit_country: r.get(12)?,
        exit_city: r.get(13)?, exit_timezone: r.get(14)?, exit_asn: r.get(15)?, exit_isp: r.get(16)?,
        profile_count: r.get::<_, i64>(17)? as u64,
    })
}

impl Db {
    pub fn proxies_list(&self) -> CmdResult<Vec<Proxy>> {
        let c = self.conn();
        let mut s = c.prepare(&format!("SELECT {COLS} FROM proxies px ORDER BY px.created_at DESC")).map_err(sql_err)?;
        s.query_map([], map).map_err(sql_err)?.collect::<Result<_, _>>().map_err(sql_err)
    }

    pub fn proxy_get(&self, id: &str) -> CmdResult<Proxy> {
        self.conn().query_row(&format!("SELECT {COLS} FROM proxies px WHERE px.id=?1"), [id], map).optional().map_err(sql_err)?.ok_or_else(|| AppError::NotFound(id.into()))
    }

    pub fn proxy_secret(&self, id: &str) -> CmdResult<ProxySecret> {
        self.conn().query_row("SELECT kind, host, port, username, password_enc FROM proxies WHERE id=?1", [id], |r| Ok(ProxySecret {
            kind: ProxyKind::parse(&r.get::<_, String>(0)?).unwrap_or(ProxyKind::Http),
            host: r.get::<_, Option<String>>(1)?.unwrap_or_default(), port: r.get::<_, Option<i64>>(2)?.unwrap_or(0) as u16,
            username: r.get(3)?, password_enc: r.get(4)?,
        })).optional().map_err(sql_err)?.ok_or_else(|| AppError::NotFound(id.into()))
    }

    /// `seal` encrypts the password for a given proxy id (context-bound).
    pub fn proxy_upsert(&self, d: &ProxyDraft, seal: impl Fn(&str, &[u8]) -> Vec<u8>) -> CmdResult<String> {
        if d.host.trim().is_empty() || d.port == 0 { return Err(AppError::invalid("host", "proxy.invalid_host")); }
        let id = d.id.clone().unwrap_or_else(new_id);
        let now = now_ms();
        let enc = d.password.as_ref().map(|p| seal(&id, p.as_bytes()));
        let c = self.conn();
        let exists: bool = c.query_row("SELECT count(*) FROM proxies WHERE id=?1", [&id], |r| r.get::<_, i64>(0)).map_err(sql_err)? > 0;
        if exists {
            c.execute("UPDATE proxies SET label=?1, kind=?2, host=?3, port=?4, username=?5, rotate_url=?6, updated_at=?7 WHERE id=?8",
                params![d.label, d.kind.as_str(), d.host.trim(), d.port, d.username, d.rotate_url, now, id]).map_err(sql_err)?;
            if let Some(e) = enc { c.execute("UPDATE proxies SET password_enc=?1 WHERE id=?2", params![e, id]).map_err(sql_err)?; }
        } else {
            c.execute("INSERT INTO proxies(id,label,kind,host,port,username,password_enc,rotate_url,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",
                params![id, d.label, d.kind.as_str(), d.host.trim(), d.port, d.username, enc, d.rotate_url, now]).map_err(sql_err)?;
        }
        Ok(id)
    }

    pub fn proxies_delete(&self, ids: &[String]) -> CmdResult<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(sql_err)?;
        for id in ids { tx.execute("DELETE FROM proxies WHERE id=?1", [id]).map_err(sql_err)?; }
        tx.commit().map_err(sql_err)
    }

    pub fn proxy_record_check(&self, id: &str, r: &ProxyCheckResult) -> CmdResult<()> {
        self.conn().execute(
            "UPDATE proxies SET last_check_at=?1, last_status=?2, last_latency_ms=?3, exit_ip=COALESCE(?4,exit_ip), exit_country=COALESCE(?5,exit_country), exit_region=COALESCE(?6,exit_region), exit_city=COALESCE(?7,exit_city), exit_timezone=COALESCE(?8,exit_timezone), exit_asn=COALESCE(?9,exit_asn), exit_isp=COALESCE(?10,exit_isp), updated_at=?1 WHERE id=?11",
            params![now_ms(), r.status.as_str(), r.latency_ms, r.exit_ip, r.country, r.region, r.city, r.timezone, r.asn, r.isp, id],
        ).map(|_| ()).map_err(sql_err)
    }
}
