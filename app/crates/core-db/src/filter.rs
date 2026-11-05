//! FilterQuery → parameterised SQL (file 04 §9.4). No user text is concatenated into SQL.
use core_model::{FilterQuery, SortDir, SortField};
use rusqlite::types::Value as SqlValue;

pub(crate) struct Compiled { pub where_sql: String, pub params: Vec<SqlValue>, pub order_sql: String }

fn placeholders(n: usize, start: usize) -> String {
    (0..n).map(|i| format!("?{}", start + i)).collect::<Vec<_>>().join(",")
}

/// Turn free text into a safe FTS5 prefix query: every token quoted, `*` appended.
pub(crate) fn fts_query(text: &str) -> Option<String> {
    let toks: Vec<String> = text.split_whitespace()
        .map(|t| t.chars().filter(|c| !matches!(c, '"' | '*' | '^' | ':' | '(' | ')')).collect::<String>())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    if toks.is_empty() { None } else { Some(toks.join(" AND ")) }
}

pub(crate) fn compile(q: &FilterQuery) -> Compiled {
    let mut w: Vec<String> = Vec::new();
    let mut p: Vec<SqlValue> = Vec::new();
    let push_in = |w: &mut Vec<String>, p: &mut Vec<SqlValue>, col: &str, vals: Vec<String>| {
        if vals.is_empty() { return; }
        let ph = placeholders(vals.len(), p.len() + 1);
        w.push(format!("{col} IN ({ph})"));
        p.extend(vals.into_iter().map(SqlValue::Text));
    };

    if q.in_trash.unwrap_or(false) { w.push("p.deleted_at IS NOT NULL".into()); } else { w.push("p.deleted_at IS NULL".into()); }

    if let Some(t) = q.text.as_deref().and_then(fts_query) {
        p.push(SqlValue::Text(t));
        w.push(format!("p.id IN (SELECT profile_id FROM profiles_fts WHERE profiles_fts MATCH ?{})", p.len()));
    }
    if let Some(ids) = &q.folder_ids {
        if q.include_subfolders.unwrap_or(true) && !ids.is_empty() {
            let ph = placeholders(ids.len(), p.len() + 1);
            p.extend(ids.iter().cloned().map(SqlValue::Text));
            w.push(format!("p.folder_id IN (WITH RECURSIVE sub(id) AS (SELECT id FROM folders WHERE id IN ({ph}) UNION SELECT f.id FROM folders f JOIN sub ON f.parent_id = sub.id) SELECT id FROM sub)"));
        } else {
            push_in(&mut w, &mut p, "p.folder_id", ids.clone());
        }
    }
    if let Some(s) = &q.statuses { push_in(&mut w, &mut p, "p.status", s.iter().map(|x| x.as_str().to_string()).collect()); }
    if let Some(s) = &q.platforms { push_in(&mut w, &mut p, "p.platform", s.iter().map(|x| x.as_str().to_string()).collect()); }
    if let Some(s) = &q.os_families { push_in(&mut w, &mut p, "fs.os_family", s.clone()); }
    if let Some(s) = &q.health_levels { push_in(&mut w, &mut p, "p.health_level", s.iter().map(|x| x.as_str().to_string()).collect()); }
    if let Some(s) = &q.colors { push_in(&mut w, &mut p, "p.color", s.clone()); }
    if let Some(s) = &q.proxy_countries { push_in(&mut w, &mut p, "px.exit_country", s.iter().map(|c| c.to_ascii_uppercase()).collect()); }
    if let Some(v) = q.health_score_min { p.push(SqlValue::Integer(v)); w.push(format!("p.health_score >= ?{}", p.len())); }
    if let Some(v) = q.health_score_max { p.push(SqlValue::Integer(v)); w.push(format!("p.health_score <= ?{}", p.len())); }
    if let Some(b) = q.has_proxy { w.push(if b { "p.proxy_id IS NOT NULL".into() } else { "p.proxy_id IS NULL".into() }); }
    if let Some(b) = q.pinned { p.push(SqlValue::Integer(b as i64)); w.push(format!("p.pinned = ?{}", p.len())); }
    if let Some(tags) = &q.tag_ids_all {
        for t in tags { p.push(SqlValue::Text(t.clone())); w.push(format!("EXISTS (SELECT 1 FROM profile_tags pt WHERE pt.profile_id=p.id AND pt.tag_id=?{})", p.len())); }
    }
    if let Some(tags) = &q.tag_ids_any {
        if !tags.is_empty() {
            let ph = placeholders(tags.len(), p.len() + 1);
            p.extend(tags.iter().cloned().map(SqlValue::Text));
            w.push(format!("EXISTS (SELECT 1 FROM profile_tags pt WHERE pt.profile_id=p.id AND pt.tag_id IN ({ph}))"));
        }
    }
    if let Some(tags) = &q.tag_ids_none {
        if !tags.is_empty() {
            let ph = placeholders(tags.len(), p.len() + 1);
            p.extend(tags.iter().cloned().map(SqlValue::Text));
            w.push(format!("NOT EXISTS (SELECT 1 FROM profile_tags pt WHERE pt.profile_id=p.id AND pt.tag_id IN ({ph}))"));
        }
    }

    let order_sql = match q.sort {
        Some(s) => {
            let col = match s.field {
                SortField::Name => "p.name COLLATE NOCASE", SortField::UpdatedAt => "p.updated_at",
                SortField::CreatedAt => "p.created_at", SortField::LastLaunchedAt => "p.last_launched_at",
                SortField::HealthScore => "p.health_score", SortField::Status => "p.status",
            };
            let dir = match s.dir { SortDir::Asc => "ASC", SortDir::Desc => "DESC" };
            format!("ORDER BY p.pinned DESC, {col} {dir}, p.id {dir}")
        }
        None => "ORDER BY p.pinned DESC, p.updated_at DESC, p.id DESC".into(),
    };
    Compiled { where_sql: w.join(" AND "), params: p, order_sql }
}

#[cfg(test)]
mod t {
    #[test]
    fn fts_escapes() {
        assert_eq!(super::fts_query(r#"shop "x" OR*"#).unwrap(), r#""shop"* AND "x"* AND "OR"*"#);
        assert!(super::fts_query("  \" * ").is_none());
    }
}
