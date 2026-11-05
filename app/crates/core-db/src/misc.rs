//! Settings and the local activity log (file 04 §4.3). Logs carry i18n keys + redacted params only.
use crate::{Db, now_ms, sql_err};
use core_model::{CmdResult, LogRow, Page, PageReq};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;

/// Keys whose values are never written to logs.
const REDACT: &[&str] = &["password", "pass", "secret", "token", "cookie", "key"];

fn redact(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.iter().map(|(k, v)| {
            let lk = k.to_ascii_lowercase();
            if REDACT.iter().any(|r| lk.contains(r)) { (k.clone(), Value::String("[redacted]".into())) } else { (k.clone(), redact(v)) }
        }).collect()),
        Value::Array(a) => Value::Array(a.iter().map(redact).collect()),
        o => o.clone(),
    }
}

impl Db {
    pub fn setting_get(&self, key: &str) -> CmdResult<Option<Value>> {
        let v: Option<String> = self.conn().query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get(0)).optional().map_err(sql_err)?;
        Ok(v.and_then(|s| serde_json::from_str(&s).ok()))
    }
    pub fn setting_set(&self, key: &str, value: &Value) -> CmdResult<()> {
        self.conn().execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value.to_string()]).map(|_| ()).map_err(sql_err)
    }
    pub fn settings_all(&self) -> CmdResult<serde_json::Map<String, Value>> {
        let c = self.conn();
        let mut s = c.prepare("SELECT key, value FROM settings").map_err(sql_err)?;
        let rows = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(sql_err)?;
        let mut m = serde_json::Map::new();
        for row in rows { let (k, v) = row.map_err(sql_err)?; m.insert(k, serde_json::from_str(&v).unwrap_or(Value::Null)); }
        Ok(m)
    }

    pub fn log(&self, level: &str, category: &str, profile_id: Option<&str>, key: &str, params_v: Value) {
        let _ = self.conn().execute(
            "INSERT INTO logs(ts,level,category,profile_id,message_key,params) VALUES(?1,?2,?3,?4,?5,?6)",
            params![now_ms(), level, category, profile_id, key, redact(&params_v).to_string()],
        );
    }

    pub fn logs_query(&self, category: Option<&str>, page: &PageReq) -> CmdResult<Page<LogRow>> {
        let c = self.conn();
        let total: i64 = c.query_row("SELECT count(*) FROM logs WHERE (?1 IS NULL OR category=?1)", [category], |r| r.get(0)).map_err(sql_err)?;
        let mut s = c.prepare("SELECT id, ts, level, category, profile_id, message_key, params FROM logs WHERE (?1 IS NULL OR category=?1) ORDER BY ts DESC, id DESC LIMIT ?2 OFFSET ?3").map_err(sql_err)?;
        let items = s.query_map(params![category, page.limit.min(500) as i64, page.offset as i64], |r| Ok(LogRow {
            id: r.get(0)?, ts: r.get(1)?, level: r.get(2)?, category: r.get(3)?, profile_id: r.get(4)?, message_key: r.get(5)?,
            params: r.get::<_, Option<String>>(6)?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null),
        })).map_err(sql_err)?.collect::<Result<_, _>>().map_err(sql_err)?;
        Ok(Page { items, total: total as u64, offset: page.offset, limit: page.limit })
    }

    /// Trim logs older than `days` (retention).
    pub fn logs_prune(&self, days: i64) -> CmdResult<()> {
        let cutoff = now_ms() - days * 86_400_000;
        self.conn().execute("DELETE FROM logs WHERE ts < ?1", [cutoff]).map(|_| ()).map_err(sql_err)
    }
}

#[cfg(test)]
mod t {
    use crate::testutil::db;
    #[test]
    fn logs_redact_secrets() {
        let (_d, db) = db();
        db.log("info", "proxy", None, "proxy.checked", serde_json::json!({"host": "1.2.3.4", "password": "hunter2", "nested": {"apiToken": "x"}}));
        let p = db.logs_query(None, &core_model::PageReq { offset: 0, limit: 10 }).unwrap();
        let s = p.items[0].params.to_string();
        assert!(!s.contains("hunter2") && !s.contains("\"x\"") && s.contains("1.2.3.4"));
    }
}
