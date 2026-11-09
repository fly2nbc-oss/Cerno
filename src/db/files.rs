//! The `files` table: a path and its stamp → fingerprint, rating and label.

use super::*;

impl Db {
    /// The record for `path`, if the file is unchanged since it was indexed.
    pub fn lookup(&self, path: &str, stamp: FileStamp) -> Result<Option<FileRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT f.fingerprint, f.rating, f.label, f.keywords, {IMAGE_COLUMNS}
             FROM files f LEFT JOIN images i ON i.fingerprint = f.fingerprint
             WHERE f.path = ?1 AND f.size = ?2 AND f.mtime_ns = ?3"
        ))?;
        let record = stmt
            .query_row(params![path, stamp.size as i64, stamp.mtime_ns], |row| {
                file_from_row(row, 0)
            })
            .optional()?;
        Ok(record)
    }

    /// Every file row below `dir` (its subfolders too) with its stamp, as `lookup` reads it:
    /// one range scan over the path key instead of a query per photo (CER-71 – opening a
    /// folder of 50 000 files spent 1.8 s on single queries). The caller compares the stamps.
    pub fn records_under(&self, dir: &Path) -> Result<HashMap<String, (FileStamp, FileRecord)>> {
        let (from, to) = path_range(dir);
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT f.path, f.size, f.mtime_ns, f.fingerprint, f.rating, f.label, f.keywords,
                    {IMAGE_COLUMNS}
             FROM files f LEFT JOIN images i ON i.fingerprint = f.fingerprint
             WHERE f.path >= ?1 AND f.path < ?2"
        ))?;
        let rows = stmt.query_map(params![from, to], |row| {
            let stamp = FileStamp {
                size: row.get::<_, i64>(1)? as u64,
                mtime_ns: row.get(2)?,
            };
            Ok((row.get::<_, String>(0)?, (stamp, file_from_row(row, 3)?)))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
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

    /// The file's keywords as read (CER-68); the row exists (`put_file` just wrote it).
    pub fn put_keywords(&self, path: &str, keywords: &[String]) -> Result<()> {
        self.conn()
            .prepare_cached("UPDATE files SET keywords = ?2 WHERE path = ?1")?
            .execute(params![path, crate::keywords::join(keywords)])?;
        Ok(())
    }

    /// Every keyword of the index with the number of files that have it – live ones only
    /// (`live`): the description tab's suggestions after the folder's own.
    pub fn keyword_counts(&self) -> Result<Vec<(String, usize)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT keywords FROM files WHERE keywords IS NOT NULL AND keywords != '' AND {}",
            live("path")
        ))?;
        let mut counts: HashMap<String, usize> = HashMap::new();
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for stored in rows {
            let mut seen = HashSet::new();
            for keyword in crate::keywords::split(&stored?) {
                if seen.insert(keyword.clone()) {
                    *counts.entry(keyword).or_default() += 1;
                }
            }
        }
        Ok(counts.into_iter().collect())
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

    /// After Cerno wrote a rating, colour label or keywords: the size changed, the mtime
    /// deliberately did not. Updating the stamp here avoids re-fingerprinting the file.
    pub fn update_after_write(
        &self,
        path: &str,
        stamp: FileStamp,
        rating: Rating,
        label: Option<Label>,
        keywords: &[String],
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "UPDATE files SET size = ?2, mtime_ns = ?3, rating = ?4, label = ?5, keywords = ?6
                 WHERE path = ?1",
            )?
            .execute(params![
                path,
                stamp.size as i64,
                stamp.mtime_ns,
                rating.value(),
                label.map(|l| l.xmp_name()),
                crate::keywords::join(keywords),
            ])?;
        Ok(())
    }

    /// Capture time, camera model and body serial number for a fingerprint. The images row
    /// already exists (the thumbnail does).
    pub fn put_metadata(
        &self,
        fingerprint: u64,
        taken_ms: Option<i64>,
        camera: Option<&str>,
        serial: Option<&str>,
        version: i64,
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "UPDATE images SET taken_ms = ?2, camera = ?3, camera_serial = ?4,
                     metadata_version = ?5
                 WHERE fingerprint = ?1",
            )?
            .execute(params![
                fingerprint as i64,
                taken_ms,
                camera,
                serial,
                version
            ])?;
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

/// A `FileRecord` from `fingerprint, rating, label, keywords, IMAGE_COLUMNS` at `first`.
fn file_from_row(row: &Row, first: usize) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        fingerprint: row.get::<_, i64>(first)? as u64,
        rating: row
            .get::<_, Option<i64>>(first + 1)?
            .map_or(Rating::Unrated, Rating::from_value),
        label: row
            .get::<_, Option<String>>(first + 2)?
            .as_deref()
            .and_then(Label::from_stored),
        keywords: row
            .get::<_, Option<String>>(first + 3)?
            .as_deref()
            .map(crate::keywords::split),
        image: image_from_row(row, first + 4)?,
    })
}

/// The keys of every path below `dir`, as `[from, to)` in the index's (binary) text order:
/// `D:\Fotos\` up to `D:\Fotos]` – the separator's next character –, so `D:\Fotos 2` and
/// `D:\Fotosammlung` stay out.
fn path_range(dir: &Path) -> (String, String) {
    let separator = std::path::MAIN_SEPARATOR;
    let mut from = dir.to_string_lossy().into_owned();
    if !from.ends_with(separator) {
        from.push(separator);
    }
    let mut to = from.clone();
    to.pop();
    to.push(char::from_u32(separator as u32 + 1).unwrap_or(char::MAX));
    (from, to)
}
