//! Folders and tags (file 04 §9).
use crate::{Db, new_id, now_ms, sql_err};
use core_model::{AppError, CmdResult, Folder, TagWithCount};
use rusqlite::params;

impl Db {
    pub fn folders_list(&self) -> CmdResult<Vec<Folder>> {
        let c = self.conn();
        let mut s = c.prepare("SELECT f.id, f.parent_id, f.name, f.color, f.sort_order, (SELECT count(*) FROM profiles p WHERE p.folder_id=f.id AND p.deleted_at IS NULL) FROM folders f ORDER BY f.sort_order, f.name COLLATE NOCASE").map_err(sql_err)?;
        s.query_map([], |r| Ok(Folder { id: r.get(0)?, parent_id: r.get(1)?, name: r.get(2)?, color: r.get(3)?, sort_order: r.get(4)?, profile_count: r.get::<_, i64>(5)? as u64 }))
            .map_err(sql_err)?.collect::<Result<_, _>>().map_err(sql_err)
    }

    pub fn folder_upsert(&self, id: Option<&str>, parent: Option<&str>, name: &str, color: Option<&str>) -> CmdResult<String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 { return Err(AppError::invalid("name", "folder.invalid_name")); }
        let c = self.conn();
        let now = now_ms();
        match id {
            Some(id) => {
                // Prevent cycles: the new parent must not be the folder itself or one of its descendants.
                if let Some(p) = parent {
                    let cyc: i64 = c.query_row("WITH RECURSIVE sub(id) AS (SELECT ?1 UNION SELECT f.id FROM folders f JOIN sub ON f.parent_id=sub.id) SELECT count(*) FROM sub WHERE id=?2", params![id, p], |r| r.get(0)).map_err(sql_err)?;
                    if cyc > 0 { return Err(AppError::invalid("parentId", "folder.cycle")); }
                }
                c.execute("UPDATE folders SET parent_id=?1, name=?2, color=?3, updated_at=?4 WHERE id=?5", params![parent, name, color, now, id]).map_err(sql_err)?;
                Ok(id.to_string())
            }
            None => {
                let id = new_id();
                c.execute("INSERT INTO folders(id,parent_id,name,color,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5)", params![id, parent, name, color, now]).map_err(sql_err)?;
                Ok(id)
            }
        }
    }

    pub fn folder_delete(&self, id: &str) -> CmdResult<()> {
        // Profiles move to "no folder" via ON DELETE SET NULL; subfolders cascade.
        self.conn().execute("DELETE FROM folders WHERE id=?1", [id]).map(|_| ()).map_err(sql_err)
    }

    pub fn tags_list(&self) -> CmdResult<Vec<TagWithCount>> {
        let c = self.conn();
        let mut s = c.prepare("SELECT t.id, t.name, t.color, (SELECT count(*) FROM profile_tags pt JOIN profiles p ON p.id=pt.profile_id WHERE pt.tag_id=t.id AND p.deleted_at IS NULL) FROM tags t ORDER BY t.name COLLATE NOCASE").map_err(sql_err)?;
        s.query_map([], |r| Ok(TagWithCount { id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, count: r.get::<_, i64>(3)? as u64 }))
            .map_err(sql_err)?.collect::<Result<_, _>>().map_err(sql_err)
    }

    pub fn tag_upsert(&self, id: Option<&str>, name: &str, color: &str) -> CmdResult<String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 { return Err(AppError::invalid("name", "tag.invalid_name")); }
        let c = self.conn();
        match id {
            Some(id) => { c.execute("UPDATE tags SET name=?1, color=?2 WHERE id=?3", params![name, color, id]).map_err(sql_err)?; Ok(id.into()) }
            None => { let id = new_id(); c.execute("INSERT INTO tags(id,name,color,created_at) VALUES(?1,?2,?3,?4)", params![id, name, color, now_ms()]).map_err(sql_err)?; Ok(id) }
        }
    }

    pub fn tag_delete(&self, id: &str) -> CmdResult<()> {
        self.conn().execute("DELETE FROM tags WHERE id=?1", [id]).map(|_| ()).map_err(sql_err)
    }
}

#[cfg(test)]
mod t {
    use crate::testutil::db;
    #[test]
    fn folder_cycle_rejected() {
        let (_d, db) = db();
        let a = db.folder_upsert(None, None, "A", None).unwrap();
        let b = db.folder_upsert(None, Some(&a), "B", None).unwrap();
        assert!(db.folder_upsert(Some(&a), Some(&b), "A", None).is_err());
        assert_eq!(db.folders_list().unwrap().len(), 2);
        let t = db.tag_upsert(None, "ads", "teal").unwrap();
        assert!(db.tag_upsert(None, "ADS", "teal").is_err(), "tag names are case-insensitive unique");
        db.tag_delete(&t).unwrap();
    }
}
