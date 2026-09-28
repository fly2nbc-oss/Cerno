//! Index of analysis results and thumbnails. Scores live here – never in the photos.
//!
//! `files` maps a path (valid while size and mtime match) to a content fingerprint; `images`
//! holds everything computed from the pixels, keyed by that fingerprint, so a renamed or
//! re-rated file keeps its scores. `feedback` remembers deleted photos as negative examples
//! for the personal taste model.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::metadata::{Label, Rating};

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS files (
        path        TEXT PRIMARY KEY,
        size        INTEGER NOT NULL,
        mtime_ns    INTEGER NOT NULL,
        fingerprint INTEGER NOT NULL,
        rating      INTEGER
    );
    CREATE TABLE IF NOT EXISTS images (
        fingerprint       INTEGER PRIMARY KEY,
        sharpness         REAL,
        sharpness_version INTEGER NOT NULL DEFAULT 0,
        aesthetic         REAL,
        aesthetic_model   TEXT,
        -- CLIP image embedding (768 × f32 LE): personal taste model, CLIP attributes.
        embedding         BLOB,
        thumbnail         BLOB
    );
    CREATE TABLE IF NOT EXISTS settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS feedback (
        fingerprint INTEGER PRIMARY KEY,
        label       REAL NOT NULL,
        at          INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS taste_skip (
        fingerprint INTEGER PRIMARY KEY
    );
";

/// Columns added after the first release; `migrate` adds whichever an index lacks.
const ADDED_IMAGE_COLUMNS: &[(&str, &str)] = &[
    ("aesthetic25", "REAL"),
    ("aesthetic25_model", "TEXT"),
    ("highlights", "REAL"),
    ("shadows", "REAL"),
    ("exposure_version", "INTEGER NOT NULL DEFAULT 0"),
    ("eyes", "REAL"),
    ("faces", "INTEGER"),
    ("faces_version", "INTEGER NOT NULL DEFAULT 0"),
    ("taken_ms", "INTEGER"),
    ("metadata_version", "INTEGER NOT NULL DEFAULT 0"),
];

/// Columns added to `files` after the first release.
const ADDED_FILE_COLUMNS: &[(&str, &str)] = &[("label", "TEXT")];

/// Everything `ImageRecord` needs, in the order `image_from_row` reads it.
const IMAGE_COLUMNS: &str = "i.sharpness, i.sharpness_version, i.aesthetic, i.aesthetic_model,
    i.aesthetic25, i.aesthetic25_model, i.highlights, i.shadows, i.exposure_version,
    i.eyes, i.faces, i.faces_version, i.thumbnail IS NOT NULL, i.embedding,
    i.taken_ms, i.metadata_version";

/// Cheap identity check for a file: if size or mtime changed, the fingerprint is recomputed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    pub size: u64,
    pub mtime_ns: i64,
}

impl FileStamp {
    pub fn of(path: &Path) -> std::io::Result<Self> {
        let meta = std::fs::metadata(path)?;
        let mtime = meta
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Ok(Self {
            size: meta.len(),
            mtime_ns: i64::try_from(mtime.as_nanos()).unwrap_or(i64::MAX),
        })
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Scores {
    /// Laplacian variance of the sharpest tiles.
    pub sharpness: Option<f32>,
    /// LAION predictor on CLIP ViT-L/14, ~1–10.
    pub aesthetic: Option<f32>,
    /// Aesthetic Predictor V2.5 on SigLIP, ~1–10.
    pub aesthetic25: Option<f32>,
    /// Share of clipped highlight / shadow pixels, 0..1.
    pub highlights: Option<f32>,
    pub shadows: Option<f32>,
    /// Laplacian variance around the eyes of the largest face; `None` without a usable face.
    pub eyes: Option<f32>,
    /// Faces found; `None` until face detection ran.
    pub faces: Option<u8>,
}

/// What was computed from an image's pixels. Capture time lives here too, so a renamed file
/// keeps it.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ImageRecord {
    pub scores: Scores,
    pub sharpness_version: i64,
    pub aesthetic_model: Option<String>,
    pub aesthetic25_model: Option<String>,
    pub exposure_version: i64,
    pub faces_version: i64,
    pub has_thumbnail: bool,
    pub embedding: Option<Vec<f32>>,
    /// Local wall-clock milliseconds; `None` until metadata of `metadata_version` was read,
    /// or when the file has no capture time.
    pub taken_ms: Option<i64>,
    pub metadata_version: i64,
}

/// What the index knows about one path.
#[derive(Debug, Clone, PartialEq)]
pub struct FileRecord {
    pub fingerprint: u64,
    /// Rating read from the file when it was indexed (kept current by the rating writer).
    /// Stored as in the file: 1–5, -1 for rejected, NULL for unrated.
    pub rating: Rating,
    /// Colour label Cerno understands (`Red` …). A foreign `xmp:Label` is stored as none.
    pub label: Option<Label>,
    pub image: ImageRecord,
}

fn opt_f32(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<f32>> {
    Ok(row.get::<_, Option<f64>>(index)?.map(|v| v as f32))
}

fn blob_to_f32(blob: &[u8]) -> Vec<f32> {
    blob.as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
}

/// Reads `IMAGE_COLUMNS` starting at `at`. Columns are NULL when there is no images row.
fn image_from_row(row: &Row<'_>, at: usize) -> rusqlite::Result<ImageRecord> {
    Ok(ImageRecord {
        scores: Scores {
            sharpness: opt_f32(row, at)?,
            aesthetic: opt_f32(row, at + 2)?,
            aesthetic25: opt_f32(row, at + 4)?,
            highlights: opt_f32(row, at + 6)?,
            shadows: opt_f32(row, at + 7)?,
            eyes: opt_f32(row, at + 9)?,
            faces: row
                .get::<_, Option<i64>>(at + 10)?
                .map(|n| n.clamp(0, 255) as u8),
        },
        sharpness_version: row.get::<_, Option<i64>>(at + 1)?.unwrap_or(0),
        aesthetic_model: row.get(at + 3)?,
        aesthetic25_model: row.get(at + 5)?,
        exposure_version: row.get::<_, Option<i64>>(at + 8)?.unwrap_or(0),
        faces_version: row.get::<_, Option<i64>>(at + 11)?.unwrap_or(0),
        has_thumbnail: row.get::<_, Option<bool>>(at + 12)?.unwrap_or(false),
        embedding: row
            .get::<_, Option<Vec<u8>>>(at + 13)?
            .map(|b| blob_to_f32(&b)),
        taken_ms: row.get(at + 14)?,
        metadata_version: row.get::<_, Option<i64>>(at + 15)?.unwrap_or(0),
    })
}

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("cannot open database {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

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

    /// Capture time for a fingerprint. The images row already exists (the thumbnail does).
    pub fn put_metadata(
        &self,
        fingerprint: u64,
        taken_ms: Option<i64>,
        version: i64,
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "UPDATE images SET taken_ms = ?2, metadata_version = ?3 WHERE fingerprint = ?1",
            )?
            .execute(params![fingerprint as i64, taken_ms, version])?;
        Ok(())
    }

    /// A deleted photo becomes a negative example (label 0) for the taste model. Must run
    /// before its `files` row goes – that row is the only link to the fingerprint.
    pub fn record_deletion(&self, path: &str) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let fingerprint: Option<i64> = tx
            .query_row(
                "SELECT fingerprint FROM files WHERE path = ?1",
                [path],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(fp) = fingerprint {
            tx.execute("DELETE FROM taste_skip WHERE fingerprint = ?1", [fp])?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO feedback (fingerprint, label, at)
             SELECT fingerprint, 0.0, CAST(strftime('%s', 'now') AS INTEGER)
             FROM files WHERE path = ?1",
            [path],
        )?;
        tx.execute("DELETE FROM files WHERE path = ?1", [path])?;
        tx.commit()?;
        Ok(())
    }

    /// Forgets taste training data while keeping star ratings in the photo files.
    pub fn reset_taste_learning(&self) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO taste_skip (fingerprint)
             SELECT fingerprint FROM files WHERE rating IS NOT NULL",
            [],
        )?;
        tx.execute("DELETE FROM feedback", [])?;
        tx.commit()?;
        Ok(())
    }

    /// A new rating or rejection counts again for the taste model.
    pub fn allow_taste_for(&self, fingerprint: u64) -> Result<()> {
        self.conn().execute(
            "DELETE FROM taste_skip WHERE fingerprint = ?1",
            [fingerprint as i64],
        )?;
        Ok(())
    }

    /// Training data for the taste model: (CLIP embedding, label 0–5). Rejected photos count
    /// as 0 like deleted ones; explicit ratings win over deletion feedback for the same pixels.
    pub fn taste_examples(&self) -> Result<Vec<(Vec<f32>, f32)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT i.embedding, CAST(MAX(MAX(f.rating, 0)) AS REAL)
             FROM files f JOIN images i ON i.fingerprint = f.fingerprint
             WHERE (f.rating BETWEEN 1 AND 5 OR f.rating = -1) AND i.embedding IS NOT NULL
               AND f.fingerprint NOT IN (SELECT fingerprint FROM taste_skip)
             GROUP BY f.fingerprint
             UNION ALL
             SELECT i.embedding, fb.label
             FROM feedback fb JOIN images i ON i.fingerprint = fb.fingerprint
             WHERE i.embedding IS NOT NULL
               AND fb.fingerprint NOT IN
                   (SELECT fingerprint FROM files WHERE rating BETWEEN 1 AND 5 OR rating = -1)
               AND fb.fingerprint NOT IN (SELECT fingerprint FROM taste_skip)",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                blob_to_f32(&row.get::<_, Vec<u8>>(0)?),
                row.get::<_, f64>(1)? as f32,
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn put_sharpness(&self, fingerprint: u64, value: f32, version: i64) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, sharpness, sharpness_version) VALUES (?1, ?2, ?3)
                 ON CONFLICT(fingerprint) DO UPDATE
                 SET sharpness = excluded.sharpness, sharpness_version = excluded.sharpness_version",
            )?
            .execute(params![fingerprint as i64, f64::from(value), version])?;
        Ok(())
    }

    pub fn put_aesthetic(
        &self,
        fingerprint: u64,
        value: f32,
        model: &str,
        embedding: &[f32],
    ) -> Result<()> {
        let embedding: Vec<u8> = embedding.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, aesthetic, aesthetic_model, embedding)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(fingerprint) DO UPDATE
                 SET aesthetic = excluded.aesthetic, aesthetic_model = excluded.aesthetic_model,
                     embedding = excluded.embedding",
            )?
            .execute(params![
                fingerprint as i64,
                f64::from(value),
                model,
                embedding
            ])?;
        Ok(())
    }

    pub fn put_aesthetic25(&self, fingerprint: u64, value: f32, model: &str) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, aesthetic25, aesthetic25_model) VALUES (?1, ?2, ?3)
                 ON CONFLICT(fingerprint) DO UPDATE
                 SET aesthetic25 = excluded.aesthetic25,
                     aesthetic25_model = excluded.aesthetic25_model",
            )?
            .execute(params![fingerprint as i64, f64::from(value), model])?;
        Ok(())
    }

    pub fn put_exposure(
        &self,
        fingerprint: u64,
        highlights: f32,
        shadows: f32,
        version: i64,
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, highlights, shadows, exposure_version)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(fingerprint) DO UPDATE
                 SET highlights = excluded.highlights, shadows = excluded.shadows,
                     exposure_version = excluded.exposure_version",
            )?
            .execute(params![
                fingerprint as i64,
                f64::from(highlights),
                f64::from(shadows),
                version
            ])?;
        Ok(())
    }

    pub fn put_faces(
        &self,
        fingerprint: u64,
        eyes: Option<f32>,
        faces: u8,
        version: i64,
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, eyes, faces, faces_version) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(fingerprint) DO UPDATE
                 SET eyes = excluded.eyes, faces = excluded.faces,
                     faces_version = excluded.faces_version",
            )?
            .execute(params![
                fingerprint as i64,
                eyes.map(f64::from),
                faces,
                version
            ])?;
        Ok(())
    }

    pub fn put_thumbnail(&self, fingerprint: u64, jpeg: &[u8]) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, thumbnail) VALUES (?1, ?2)
                 ON CONFLICT(fingerprint) DO UPDATE SET thumbnail = excluded.thumbnail",
            )?
            .execute(params![fingerprint as i64, jpeg])?;
        Ok(())
    }

    pub fn thumbnail(&self, fingerprint: u64) -> Result<Option<Vec<u8>>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare_cached("SELECT thumbnail FROM images WHERE fingerprint = ?1")?;
        let thumb = stmt
            .query_row(params![fingerprint as i64], |row| {
                row.get::<_, Option<Vec<u8>>>(0)
            })
            .optional()?;
        Ok(thumb.flatten())
    }

    pub fn setting(&self, key: &str) -> Option<String> {
        self.conn()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .ok()
            .flatten()
    }

    pub fn put_setting(&self, key: &str, value: &str) {
        let result = self.conn().execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            [key, value],
        );
        if let Err(err) = result {
            log::warn!("cannot save setting {key}: {err}");
        }
    }
}

/// Adds columns introduced after an index was created.
fn migrate(conn: &Connection) -> Result<()> {
    add_columns(conn, "images", ADDED_IMAGE_COLUMNS)?;
    add_columns(conn, "files", ADDED_FILE_COLUMNS)?;
    Ok(())
}

fn add_columns(conn: &Connection, table: &str, columns: &[(&str, &str)]) -> Result<()> {
    let existing: HashSet<String> = conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?;
    for (name, declaration) in columns {
        if !existing.contains(*name) {
            conn.execute_batch(&format!(
                "ALTER TABLE {table} ADD COLUMN {name} {declaration}"
            ))?;
            log::info!("index: added column {table}.{name}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAMP: FileStamp = FileStamp {
        size: 1000,
        mtime_ns: 42,
    };

    #[test]
    fn lookup_requires_matching_stamp() {
        let db = Db::open_in_memory().unwrap();
        db.put_file(
            "a.jpg",
            STAMP,
            u64::MAX - 7,
            Rating::Stars(3),
            Some(Label::Red),
        )
        .unwrap();
        let record = db.lookup("a.jpg", STAMP).unwrap().unwrap();
        assert_eq!(
            record.fingerprint,
            u64::MAX - 7,
            "u64 survives the i64 column"
        );
        assert_eq!(record.rating, Rating::Stars(3));
        assert_eq!(record.label, Some(Label::Red));
        assert_eq!(record.image, ImageRecord::default());

        let changed = FileStamp {
            size: 1001,
            ..STAMP
        };
        assert!(db.lookup("a.jpg", changed).unwrap().is_none());
        db.update_after_write("a.jpg", changed, Rating::Rejected, None)
            .unwrap();
        let updated = db.lookup("a.jpg", changed).unwrap().unwrap();
        assert_eq!(updated.rating, Rating::Rejected);
        assert_eq!(updated.label, None);
    }

    #[test]
    fn scores_follow_the_fingerprint() {
        let db = Db::open_in_memory().unwrap();
        db.put_sharpness(7, 123.5, 1).unwrap();
        db.put_aesthetic(7, 6.25, "m1", &[0.5; 768]).unwrap();
        db.put_aesthetic25(7, 5.5, "v25").unwrap();
        db.put_exposure(7, 0.01, 0.2, 1).unwrap();
        db.put_faces(7, Some(88.0), 2, 1).unwrap();
        db.put_thumbnail(7, &[1, 2, 3]).unwrap();
        // A renamed file pointing at the same pixels sees the same scores.
        db.put_file("renamed.jpg", STAMP, 7, Rating::Unrated, None)
            .unwrap();
        let image = db.lookup("renamed.jpg", STAMP).unwrap().unwrap().image;
        assert_eq!(image.scores.sharpness, Some(123.5));
        assert_eq!(image.scores.aesthetic, Some(6.25));
        assert_eq!(image.scores.aesthetic25, Some(5.5));
        assert_eq!(image.scores.highlights, Some(0.01));
        assert_eq!(image.scores.shadows, Some(0.2));
        assert_eq!(image.scores.eyes, Some(88.0));
        assert_eq!(image.scores.faces, Some(2));
        assert_eq!(image.sharpness_version, 1);
        assert_eq!(image.exposure_version, 1);
        assert_eq!(image.faces_version, 1);
        assert_eq!(image.aesthetic_model.as_deref(), Some("m1"));
        assert_eq!(image.aesthetic25_model.as_deref(), Some("v25"));
        assert_eq!(image.embedding.as_deref(), Some(&[0.5f32; 768][..]));
        assert!(image.has_thumbnail);
        assert_eq!(db.thumbnail(7).unwrap(), Some(vec![1, 2, 3]));
        assert_eq!(db.image(7).unwrap(), image);
        assert_eq!(db.image(8).unwrap(), ImageRecord::default());
    }

    #[test]
    fn settings_round_trip() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.setting("sort"), None);
        db.put_setting("sort", "aesthetics");
        assert_eq!(db.setting("sort").as_deref(), Some("aesthetics"));
    }

    /// An index written by the first release (without the newer columns) must open and work.
    #[test]
    fn migrates_an_index_from_the_first_release() {
        const FIRST_RELEASE: &str = "
            CREATE TABLE files (path TEXT PRIMARY KEY, size INTEGER NOT NULL,
                mtime_ns INTEGER NOT NULL, fingerprint INTEGER NOT NULL, rating INTEGER);
            CREATE TABLE images (fingerprint INTEGER PRIMARY KEY, sharpness REAL,
                sharpness_version INTEGER NOT NULL DEFAULT 0, aesthetic REAL,
                aesthetic_model TEXT, embedding BLOB, thumbnail BLOB);
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO images (fingerprint, sharpness, sharpness_version) VALUES (9, 50.0, 1);
            INSERT INTO files VALUES ('old.jpg', 1000, 42, 9, 4);";
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(FIRST_RELEASE).unwrap();
        let db = Db::init(conn).unwrap();

        let record = db.lookup("old.jpg", STAMP).unwrap().unwrap();
        assert_eq!(record.rating, Rating::Stars(4));
        assert_eq!(record.image.scores.sharpness, Some(50.0));
        assert_eq!(record.image.exposure_version, 0);
        assert_eq!(record.image.metadata_version, 0);
        assert_eq!(record.label, None);
        db.put_metadata(9, Some(1_000), 1).unwrap();
        assert_eq!(db.image(9).unwrap().taken_ms, Some(1_000));
        assert_eq!(db.image(9).unwrap().metadata_version, 1);
        db.put_faces(9, None, 0, 1).unwrap();
        assert_eq!(db.image(9).unwrap().scores.faces, Some(0));
    }

    #[test]
    fn deletions_and_rejections_become_negative_examples() {
        let db = Db::open_in_memory().unwrap();
        for (fp, path, rating) in [
            (1, "liked.jpg", Rating::Stars(5)),
            (2, "binned.jpg", Rating::Unrated),
            (3, "rejected.jpg", Rating::Rejected),
            (4, "unrated.jpg", Rating::Unrated),
        ] {
            db.put_aesthetic(fp, 5.0, "m", &[fp as f32; 768]).unwrap();
            db.put_file(path, STAMP, fp, rating, None).unwrap();
        }
        db.record_deletion("binned.jpg").unwrap();
        assert!(db.lookup("binned.jpg", STAMP).unwrap().is_none());

        let mut examples: Vec<(f32, f32)> = db
            .taste_examples()
            .unwrap()
            .into_iter()
            .map(|(embedding, label)| (embedding[0], label))
            .collect();
        examples.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(examples, [(1.0, 5.0), (2.0, 0.0), (3.0, 0.0)]);
    }

    #[test]
    fn taste_reset_skips_old_ratings_until_they_change() {
        let db = Db::open_in_memory().unwrap();
        db.put_aesthetic(1, 5.0, "m", &[1.0; 768]).unwrap();
        db.put_file("a.jpg", STAMP, 1, Rating::Stars(4), None)
            .unwrap();
        assert_eq!(db.taste_examples().unwrap().len(), 1);
        db.reset_taste_learning().unwrap();
        assert!(db.taste_examples().unwrap().is_empty());
        db.allow_taste_for(1).unwrap();
        db.put_file("a.jpg", STAMP, 1, Rating::Stars(3), None)
            .unwrap();
        assert_eq!(db.taste_examples().unwrap().len(), 1);
    }

    #[test]
    fn retarget_path_keeps_the_fingerprint() {
        let db = Db::open_in_memory().unwrap();
        db.put_file("old.jpg", STAMP, 9, Rating::Stars(4), Some(Label::Red))
            .unwrap();
        db.put_file("stale.jpg", STAMP, 1, Rating::Unrated, None)
            .unwrap();
        db.retarget_path("old.jpg", "stale.jpg").unwrap();
        assert!(db.lookup("old.jpg", STAMP).unwrap().is_none());
        let moved = db.lookup("stale.jpg", STAMP).unwrap().unwrap();
        assert_eq!(moved.fingerprint, 9);
        assert_eq!(moved.rating, Rating::Stars(4));
        assert_eq!(moved.label, Some(Label::Red));
        db.retarget_path("missing.jpg", "other.jpg").unwrap();
    }
}
