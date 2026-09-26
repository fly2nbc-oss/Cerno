//! Index of analysis results and thumbnails. Scores live here – never in the photos.
//!
//! Two tables: `files` maps a path (valid while size and mtime match) to a content
//! fingerprint; `images` holds everything computed from the pixels, keyed by that fingerprint.
//! A renamed or re-rated file therefore keeps its scores.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use rusqlite::{Connection, OptionalExtension, params};

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
        -- CLIP image embedding (768 × f32 LE) – basis for a personal taste model later.
        embedding         BLOB,
        thumbnail         BLOB
    );
    CREATE TABLE IF NOT EXISTS settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
";

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
    pub sharpness: Option<f32>,
    pub aesthetic: Option<f32>,
}

/// What was computed from an image's pixels.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ImageRecord {
    pub scores: Scores,
    pub sharpness_version: i64,
    pub aesthetic_model: Option<String>,
    pub has_thumbnail: bool,
}

/// What the index knows about one path.
#[derive(Debug, Clone, PartialEq)]
pub struct FileRecord {
    pub fingerprint: u64,
    /// Rating read from the file when it was indexed (kept current by the rating writer).
    pub rating: Option<u8>,
    pub image: ImageRecord,
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
        let mut stmt = conn.prepare_cached(
            "SELECT f.fingerprint, f.rating, i.sharpness, i.sharpness_version, i.aesthetic,
                    i.aesthetic_model, i.thumbnail IS NOT NULL
             FROM files f LEFT JOIN images i ON i.fingerprint = f.fingerprint
             WHERE f.path = ?1 AND f.size = ?2 AND f.mtime_ns = ?3",
        )?;
        let record = stmt
            .query_row(params![path, stamp.size as i64, stamp.mtime_ns], |row| {
                Ok(FileRecord {
                    fingerprint: row.get::<_, i64>(0)? as u64,
                    rating: row
                        .get::<_, Option<i64>>(1)?
                        .and_then(|r| u8::try_from(r).ok())
                        .filter(|r| (1..=5).contains(r)),
                    image: ImageRecord {
                        scores: Scores {
                            sharpness: row.get::<_, Option<f64>>(2)?.map(|v| v as f32),
                            aesthetic: row.get::<_, Option<f64>>(4)?.map(|v| v as f32),
                        },
                        sharpness_version: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                        aesthetic_model: row.get(5)?,
                        has_thumbnail: row.get::<_, Option<bool>>(6)?.unwrap_or(false),
                    },
                })
            })
            .optional()?;
        Ok(record)
    }

    /// Everything known for a fingerprint, e.g. after the file was renamed.
    pub fn image(&self, fingerprint: u64) -> Result<ImageRecord> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT sharpness, sharpness_version, aesthetic, aesthetic_model, thumbnail IS NOT NULL
             FROM images WHERE fingerprint = ?1",
        )?;
        let record = stmt
            .query_row(params![fingerprint as i64], |row| {
                Ok(ImageRecord {
                    scores: Scores {
                        sharpness: row.get::<_, Option<f64>>(0)?.map(|v| v as f32),
                        aesthetic: row.get::<_, Option<f64>>(2)?.map(|v| v as f32),
                    },
                    sharpness_version: row.get(1)?,
                    aesthetic_model: row.get(3)?,
                    has_thumbnail: row.get(4)?,
                })
            })
            .optional()?;
        Ok(record.unwrap_or_default())
    }

    pub fn put_file(
        &self,
        path: &str,
        stamp: FileStamp,
        fingerprint: u64,
        rating: Option<u8>,
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT OR REPLACE INTO files (path, size, mtime_ns, fingerprint, rating)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?
            .execute(params![
                path,
                stamp.size as i64,
                stamp.mtime_ns,
                fingerprint as i64,
                rating
            ])?;
        Ok(())
    }

    /// After Cerno wrote a rating: the size changed, the mtime deliberately did not. Updating
    /// the stamp here avoids re-fingerprinting the file.
    pub fn update_after_rating_write(
        &self,
        path: &str,
        stamp: FileStamp,
        rating: Option<u8>,
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "UPDATE files SET size = ?2, mtime_ns = ?3, rating = ?4 WHERE path = ?1",
            )?
            .execute(params![path, stamp.size as i64, stamp.mtime_ns, rating])?;
        Ok(())
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
        db.put_file("a.jpg", STAMP, u64::MAX - 7, Some(3)).unwrap();
        let record = db.lookup("a.jpg", STAMP).unwrap().unwrap();
        assert_eq!(
            record.fingerprint,
            u64::MAX - 7,
            "u64 survives the i64 column"
        );
        assert_eq!(record.rating, Some(3));
        assert_eq!(record.image, ImageRecord::default());

        let changed = FileStamp {
            size: 1001,
            ..STAMP
        };
        assert!(db.lookup("a.jpg", changed).unwrap().is_none());
        db.update_after_rating_write("a.jpg", changed, Some(5))
            .unwrap();
        assert_eq!(
            db.lookup("a.jpg", changed).unwrap().unwrap().rating,
            Some(5)
        );
    }

    #[test]
    fn scores_follow_the_fingerprint() {
        let db = Db::open_in_memory().unwrap();
        db.put_sharpness(7, 123.5, 1).unwrap();
        db.put_aesthetic(7, 6.25, "m1", &[0.5; 768]).unwrap();
        db.put_thumbnail(7, &[1, 2, 3]).unwrap();
        // A renamed file pointing at the same pixels sees the same scores.
        db.put_file("renamed.jpg", STAMP, 7, None).unwrap();
        let image = db.lookup("renamed.jpg", STAMP).unwrap().unwrap().image;
        assert_eq!(image.scores.sharpness, Some(123.5));
        assert_eq!(image.scores.aesthetic, Some(6.25));
        assert_eq!(image.sharpness_version, 1);
        assert_eq!(image.aesthetic_model.as_deref(), Some("m1"));
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
}
