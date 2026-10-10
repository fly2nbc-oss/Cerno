//! Index of analysis results and thumbnails. Scores live here – never in the photos.
//!
//! `files` maps a path (valid while size and mtime match) to a content fingerprint; `images`
//! holds everything computed from the pixels, keyed by that fingerprint, so a renamed or
//! re-rated file keeps its scores. `feedback` held deleted photos as negative examples for the
//! personal taste model up to 1.8.0; it stays empty now (`migrate` clears it).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::metadata::{Label, Rating};

mod backups;
mod camera_offsets;
mod files;
mod images;
mod settings;
mod taste;

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
    -- Copies of originals taken before straighten, crop or a quarter turn (Ctrl+Z).
    CREATE TABLE IF NOT EXISTS backups (
        id     INTEGER PRIMARY KEY,
        path   TEXT NOT NULL,
        backup TEXT NOT NULL,
        at_ms  INTEGER NOT NULL
    );
    -- Photos deleted in Cerno lie in `.originals`: where each one came from (since 1.6).
    CREATE TABLE IF NOT EXISTS set_aside (
        aside    TEXT PRIMARY KEY,
        original TEXT NOT NULL,
        at_ms    INTEGER NOT NULL
    );
    -- Up to 1.8.0: photos put back before their fingerprint was known. Unused, cleared.
    CREATE TABLE IF NOT EXISTS restored (
        path TEXT PRIMARY KEY
    );
    -- Every face YuNet found, in 0..1 of the upright photo (the faces grid `G`, since 1.7).
    -- `landmarks`: ten f32 LE – eyes, nose, mouth corners as x, y.
    CREATE TABLE IF NOT EXISTS faces (
        fingerprint INTEGER NOT NULL,
        idx         INTEGER NOT NULL,
        score       REAL NOT NULL,
        x           REAL NOT NULL,
        y           REAL NOT NULL,
        w           REAL NOT NULL,
        h           REAL NOT NULL,
        landmarks   BLOB NOT NULL,
        eyes        REAL,
        PRIMARY KEY (fingerprint, idx)
    );
    -- A camera whose clock was off, per open folder (since 1.7): added to the capture time of
    -- every photo of that camera model (`metadata::camera_id`) there. The files keep theirs.
    CREATE TABLE IF NOT EXISTS camera_offsets (
        folder    TEXT NOT NULL,
        camera    INTEGER NOT NULL,
        offset_ms INTEGER NOT NULL,
        PRIMARY KEY (folder, camera)
    );
";

/// SQL: the column `path` lies outside every `.originals` folder. A deleted photo's row (it
/// lies there, see `record_deletion`) teaches the taste model nothing, not even the stars it
/// had. `LIKE` ignores ASCII case, like `originals::is_inside`.
fn live(path: &str) -> String {
    format!("{path} NOT LIKE '%/.originals/%' AND {path} NOT LIKE '%\\.originals\\%'")
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

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
    ("camera", "TEXT"),
    ("truncated", "INTEGER"),
];

/// Columns added to `files` after the first release.
const ADDED_FILE_COLUMNS: &[(&str, &str)] = &[("label", "TEXT")];

/// Everything `ImageRecord` needs, in the order `image_from_row` reads it.
const IMAGE_COLUMNS: &str = "i.sharpness, i.sharpness_version, i.aesthetic, i.aesthetic_model,
    i.aesthetic25, i.aesthetic25_model, i.highlights, i.shadows, i.exposure_version,
    i.eyes, i.faces, i.faces_version, i.thumbnail IS NOT NULL, i.embedding,
    i.taken_ms, i.metadata_version, i.camera, i.truncated";

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
    /// A JPEG that ends inside its image data (`jpeg_info::is_complete`); `None` until checked,
    /// and for every other format.
    pub truncated: Option<bool>,
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
    /// Camera model (`CameraInfo::model`): only photos of one camera form a series.
    pub camera: Option<String>,
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

/// One face of a photo, in 0..1 of the upright image (`faces` table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceRow {
    pub score: f32,
    /// x, y, width, height.
    pub bbox: [f32; 4],
    /// Right eye, left eye, nose, right and left mouth corner.
    pub landmarks: [[f32; 2]; 5],
    /// Laplacian variance around its eyes; `None` when it is too small to measure.
    pub eyes: Option<f32>,
}

/// One example of the taste model: the CLIP embedding and its label, 0–5 stars.
pub type TasteExample = (Vec<f32>, f32);

/// Where the taste model's examples come from (the models card names them). Deleted photos are
/// none since 1.9.0: a good photo is often deleted only because there are too many alike.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TasteSources {
    /// Photos with 1–5 stars.
    pub stars: usize,
    /// Rejected photos: 0 stars.
    pub rejected: usize,
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
            truncated: row.get(at + 17)?,
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
        camera: row.get(at + 16)?,
    })
}

pub struct Db {
    conn: Mutex<Connection>,
    /// The fallback when the index file can't be opened: everything is gone on exit.
    in_memory: bool,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("cannot open database {}", path.display()))?;
        // A larger page cache and memory-mapped reads: `preload` reads a row per photo, and the
        // later columns lie behind each row's thumbnail (measured 2026-10-10 on a 143 MB index:
        // 522 lookups 12.5 → 8 ms).
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -32768; PRAGMA mmap_size = 268435456;",
        )?;
        Self::init(conn, false)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?, true)
    }

    fn init(conn: Connection, in_memory: bool) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            in_memory,
        })
    }

    /// Nothing written here survives the session (the index file could not be opened).
    pub fn is_in_memory(&self) -> bool {
        self.in_memory
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Adds columns introduced after an index was created, and clears what no longer counts.
fn migrate(conn: &Connection) -> Result<()> {
    add_columns(conn, "images", ADDED_IMAGE_COLUMNS)?;
    add_columns(conn, "files", ADDED_FILE_COLUMNS)?;
    // Up to 1.8.0 every deleted photo was a 0-star example for the prediction – wrong: a good
    // photo is often deleted only because there are too many alike. Those rows go (the
    // tables stay, never dropped).
    // Only when there is something: a DELETE takes the write lock at every start.
    let any = |table: &str| -> Result<bool> {
        Ok(conn.query_row(
            &format!("SELECT EXISTS (SELECT 1 FROM {table})"),
            [],
            |row| row.get(0),
        )?)
    };
    if any("feedback")? {
        let cleared = conn.execute("DELETE FROM feedback", [])?;
        log::info!("index: {cleared} deletions no longer count for the prediction");
    }
    if any("restored")? {
        conn.execute("DELETE FROM restored", [])?;
    }
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
mod tests;
