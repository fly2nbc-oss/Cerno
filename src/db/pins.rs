//! Photos marked in Cerno alone (`V`, CER-74): only in the index, never in a file. They
//! survive a restart and follow a photo moved, renamed, deleted or put back
//! (`retarget_path`, `record_deletion`, `record_restore`).

use super::*;

impl Db {
    /// Every marked photo's path.
    pub fn pins(&self) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT path FROM pins ORDER BY at_ms")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Marks `path`, or takes the mark away.
    pub fn set_pin(&self, path: &str, on: bool) -> Result<()> {
        let conn = self.conn();
        if on {
            conn.execute(
                "INSERT OR IGNORE INTO pins (path, at_ms) VALUES (?1, ?2)",
                params![path, now_ms()],
            )?;
        } else {
            conn.execute("DELETE FROM pins WHERE path = ?1", [path])?;
        }
        Ok(())
    }

    /// Every mark goes (*Clear marks*).
    pub fn clear_pins(&self) -> Result<()> {
        self.conn().execute("DELETE FROM pins", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Marks are set, follow a move and a deletion, and go all at once.
    #[test]
    fn marks_follow_the_photo() {
        let db = Db::open_in_memory().unwrap();
        db.set_pin("/a/IMG_1.jpg", true).unwrap();
        db.set_pin("/a/IMG_2.jpg", true).unwrap();
        db.set_pin("/a/IMG_2.jpg", true).unwrap();
        assert_eq!(db.pins().unwrap().len(), 2);
        db.retarget_path("/a/IMG_1.jpg", "/b/IMG_1.jpg").unwrap();
        db.record_deletion("/a/IMG_2.jpg", "/a/.originals/IMG_2.jpg")
            .unwrap();
        let mut pins = db.pins().unwrap();
        pins.sort();
        assert_eq!(pins, ["/a/.originals/IMG_2.jpg", "/b/IMG_1.jpg"]);
        db.record_restore("/a/.originals/IMG_2.jpg", "/a/IMG_2.jpg", "/a/IMG_2.jpg")
            .unwrap();
        assert!(db.pins().unwrap().contains(&"/a/IMG_2.jpg".to_owned()));
        db.set_pin("/b/IMG_1.jpg", false).unwrap();
        assert_eq!(db.pins().unwrap(), ["/a/IMG_2.jpg"]);
        db.clear_pins().unwrap();
        assert!(db.pins().unwrap().is_empty());
    }
}
