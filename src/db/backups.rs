//! Deleted photos set aside (`set_aside`) and kept originals (`backups`).

use super::*;

impl Db {
    /// A photo was deleted: its `files` row follows it to `aside` in `.originals` (a rename
    /// keeps size and dates, so its scores and thumbnail are found there at once) and
    /// `set_aside` remembers where it came from. It teaches the taste model nothing (since
    /// 1.9.0).
    pub fn record_deletion(&self, path: &str, aside: &str) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM files WHERE path = ?1", [aside])?;
        tx.execute(
            "UPDATE files SET path = ?1 WHERE path = ?2",
            params![aside, path],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO set_aside (aside, original, at_ms) VALUES (?1, ?2, ?3)",
            params![aside, path, now_ms()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Where the photos set aside into the folder `dir` (an `.originals`) came from.
    pub fn set_aside_in(&self, dir: &str) -> Result<Vec<(String, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT aside, original FROM set_aside WHERE substr(aside, 1, length(?1)) = ?1",
        )?;
        let rows = stmt.query_map([dir], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// A deleted photo is back, at `to` (`original`, or a free name beside it). Its row moves
    /// along. Back under another name, it takes the kept originals from before its deletion
    /// along: the photo now called `original` must not undo an edit with them.
    pub fn record_restore(&self, aside: &str, to: &str, original: &str) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let deleted_at: Option<i64> = tx
            .query_row(
                "SELECT at_ms FROM set_aside WHERE aside = ?1",
                [aside],
                |row| row.get(0),
            )
            .optional()?;
        tx.execute("DELETE FROM files WHERE path = ?1", [to])?;
        tx.execute(
            "UPDATE files SET path = ?1 WHERE path = ?2",
            params![to, aside],
        )?;
        if to != original
            && let Some(at) = deleted_at
        {
            tx.execute(
                "UPDATE backups SET path = ?1 WHERE path = ?2 AND at_ms <= ?3",
                params![to, original, at],
            )?;
        }
        tx.execute("DELETE FROM set_aside WHERE aside = ?1", [aside])?;
        tx.commit()?;
        Ok(())
    }

    /// Remembers the copy of `path` taken before an edit.
    pub fn push_backup(&self, path: &str, backup: &str, at_ms: i64) -> Result<()> {
        self.conn().execute(
            "INSERT INTO backups (path, backup, at_ms) VALUES (?1, ?2, ?3)",
            params![path, backup, at_ms],
        )?;
        Ok(())
    }

    /// Every copy of `path`, newest first: row id and the copy's file.
    pub fn backups_of(&self, path: &str) -> Result<Vec<(i64, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, backup FROM backups WHERE path = ?1 ORDER BY at_ms DESC, id DESC",
        )?;
        let rows = stmt.query_map([path], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn drop_backup(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM backups WHERE id = ?1", [id])?;
        Ok(())
    }

    /// The kept original moved (beside its photo, or with it): the row points at the new file.
    pub fn set_backup_file(&self, id: i64, backup: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE backups SET backup = ?2 WHERE id = ?1",
            params![id, backup],
        )?;
        Ok(())
    }

    /// Every row, oldest first: row id, photo and kept file.
    pub fn all_backups(&self) -> Result<Vec<(i64, String, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, path, backup FROM backups ORDER BY at_ms, id")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}
