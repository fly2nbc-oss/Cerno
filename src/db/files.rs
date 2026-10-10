//! The `files` table: a path and its stamp → fingerprint, rating and label.

use super::*;

impl Db {
    /// The record for `path`, if the file is unchanged since it was indexed.
    pub fn lookup(&self, path: &str, stamp: FileStamp) -> Result<Option<FileRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT f.fingerprint, f.rating, f.label, {IMAGE_COLUMNS}
             FROM files f LEFT JOIN images i ON i.fingerprint = f.fingerprint
             WHERE f.path = ?1 AND f.size = ?2 AND f.mtime_ns = ?3"
        ))?;
        let record = stmt
            .query_row(params![path, stamp.size as i64, stamp.mtime_ns], |row| {
                Ok(FileRecord {
                    fingerprint: row.get::<_, i64>(0)? as u64,
                    rating: row
                        .get::<_, Option<i64>>(1)?
                        .map_or(Rating::Unrated, Rating::from_value),
                    label: row
                        .get::<_, Option<String>>(2)?
                        .as_deref()
                        .and_then(Label::from_stored),
                    image: image_from_row(row, 3)?,
                })
            })
            .optional()?;
        Ok(record)
    }

    /// Everything known for a fingerprint, e.g. after the file was renamed.
    pub fn image(&self, fingerprint: u64) -> Result<ImageRecord> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {IMAGE_COLUMNS} FROM images i WHERE i.fingerprint = ?1"
        ))?;
        let record = stmt
            .query_row(params![fingerprint as i64], |row| image_from_row(row, 0))
            .optional()?;
        Ok(record.unwrap_or_default())
    }

    pub fn put_file(
        &self,
        path: &str,
        stamp: FileStamp,
        fingerprint: u64,
        rating: Rating,
        label: Option<Label>,
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT OR REPLACE INTO files (path, size, mtime_ns, fingerprint, rating, label)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?
            .execute(params![
                path,
                stamp.size as i64,
                stamp.mtime_ns,
                fingerprint as i64,
                rating.value(),
                label.map(|l| l.xmp_name()),
            ])?;
        Ok(())
    }

    /// Drops the path so the next analysis fingerprints the file again. Used after a pixel edit
    /// or a quarter turn: size and mtime are put back on purpose, so the stamp would otherwise
    /// still match and the old scores would stick. The `images` row stays; ratings history and
    /// taste feedback point at it.
    pub fn forget_file(&self, path: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM files WHERE path = ?1", [path])?;
        Ok(())
    }

    /// The file moved. Scores stay on the fingerprint; only the path changes. A stale row at
    /// the destination is removed first so the primary key is free.
    pub fn retarget_path(&self, from: &str, to: &str) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let found: i64 = tx.query_row(
            "SELECT COUNT(*) FROM files WHERE path = ?1",
            [from],
            |row| row.get(0),
        )?;
        // Ctrl+Z still finds the original of a moved photo.
        tx.execute(
            "UPDATE backups SET path = ?1 WHERE path = ?2",
            params![to, from],
        )?;
        if found == 0 {
            tx.commit()?;
            return Ok(());
        }
        tx.execute("DELETE FROM files WHERE path = ?1", [to])?;
        tx.execute(
            "UPDATE files SET path = ?1 WHERE path = ?2",
            params![to, from],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// After Cerno wrote a rating or colour label: the size changed, the mtime deliberately did
    /// not. Updating the stamp here avoids re-fingerprinting the file.
    pub fn update_after_write(
        &self,
        path: &str,
        stamp: FileStamp,
        rating: Rating,
        label: Option<Label>,
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "UPDATE files SET size = ?2, mtime_ns = ?3, rating = ?4, label = ?5 WHERE path = ?1",
            )?
            .execute(params![
                path,
                stamp.size as i64,
                stamp.mtime_ns,
                rating.value(),
                label.map(|l| l.xmp_name()),
            ])?;
        Ok(())
    }

    /// Capture time and camera model for a fingerprint. The images row already exists (the
    /// thumbnail does).
    pub fn put_metadata(
        &self,
        fingerprint: u64,
        taken_ms: Option<i64>,
        camera: Option<&str>,
        version: i64,
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "UPDATE images SET taken_ms = ?2, camera = ?3, metadata_version = ?4
                 WHERE fingerprint = ?1",
            )?
            .execute(params![fingerprint as i64, taken_ms, camera, version])?;
        Ok(())
    }

    /// Whether a JPEG ends inside its image data (`jpeg_info::is_complete`).
    pub fn put_truncated(&self, fingerprint: u64, truncated: bool) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, truncated) VALUES (?1, ?2)
                 ON CONFLICT(fingerprint) DO UPDATE SET truncated = excluded.truncated",
            )?
            .execute(params![fingerprint as i64, truncated])?;
        Ok(())
    }
}
