//! Copies of originals, taken before Cerno changes a photo's pixels or orientation (straighten,
//! crop, quarter turn), so `Ctrl+Z` can put the photo back. They live in the app data folder –
//! never next to the photos – and are deleted after [`KEEP`].

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};

use crate::db::Db;

/// How long an original is kept.
pub const KEEP: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// `%LOCALAPPDATA%\Cerno\data\backups` (Linux: `~/.local/share/cerno/backups`).
pub fn dir() -> Result<PathBuf> {
    Ok(crate::paths::data_dir()?.join("backups"))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Copies `path` into `dir` and records the copy. Called right before an edit; if it fails,
/// the edit does not happen.
pub fn keep(db: &Db, dir: &Path, path: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).context("cannot create the backup folder")?;
    let at = now_ms();
    let name = path
        .file_name()
        .map_or_else(|| "photo".to_owned(), |n| n.to_string_lossy().into_owned());
    // The path hash keeps same-named photos from different folders apart.
    let copy = dir.join(format!("{at}-{:016x}-{name}", path_hash(path)));
    std::fs::copy(path, &copy).context("cannot keep a copy of the original")?;
    db.push_backup(&path.to_string_lossy(), &copy.to_string_lossy(), at)?;
    Ok(copy)
}

/// Deletes the originals older than [`KEEP`]. Returns how many went.
pub fn prune(db: &Db) -> Result<usize> {
    let before = now_ms() - KEEP.as_millis() as i64;
    let files = db.take_old_backups(before)?;
    for file in &files {
        if let Err(err) = std::fs::remove_file(file)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!("backup {file}: {err}");
        }
    }
    Ok(files.len())
}

/// FNV-1a over the path – stable across Rust versions, unlike `DefaultHasher`.
fn path_hash(path: &Path) -> u64 {
    path.to_string_lossy()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_a_copy_and_prunes_old_ones() {
        let db = Db::open_in_memory().expect("db");
        let root = std::env::temp_dir().join(format!("cerno-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dir");
        let photo = root.join("Übersicht.jpg");
        std::fs::write(&photo, b"original").expect("photo");
        let backups = root.join("backups");

        let copy = keep(&db, &backups, &photo).expect("backup");
        assert_eq!(std::fs::read(&copy).expect("copy"), b"original");
        let (id, stored) = db
            .latest_backup(&photo.to_string_lossy())
            .expect("query")
            .expect("row");
        assert_eq!(PathBuf::from(stored), copy);

        // Young backups stay; one pretending to be 31 days old goes.
        assert_eq!(prune(&db).expect("prune"), 0);
        db.drop_backup(id).expect("drop");
        let old = now_ms() - KEEP.as_millis() as i64 - 86_400_000;
        db.push_backup(&photo.to_string_lossy(), &copy.to_string_lossy(), old)
            .expect("old row");
        assert_eq!(prune(&db).expect("prune"), 1);
        assert!(!copy.exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
