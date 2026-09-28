//! Copies of originals, taken before Cerno changes a photo's pixels or orientation (straighten,
//! crop, quarter turn), so `Ctrl+Z` can put the photo back. They live in the app data folder –
//! never next to the photos – and are deleted after [`KEEP`], or with the photo when it goes
//! to the trash.
//!
//! The index only points at them. A row whose file is not directly in the backup folder – an
//! index someone else changed – is never read, written back or deleted ([`inside`]).

use std::path::{Component, Path, PathBuf};
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

/// The newest kept original of `path` (row id and file), for `Ctrl+Z`. Rows outside `dir` are
/// dropped on the way.
pub fn latest(db: &Db, dir: &Path, path: &Path) -> Result<Option<(i64, PathBuf)>> {
    let key = path.to_string_lossy();
    while let Some((id, copy)) = db.latest_backup(&key)? {
        let copy = PathBuf::from(copy);
        if inside(dir, &copy) {
            return Ok(Some((id, copy)));
        }
        log::warn!(
            "kept original outside the backup folder ignored: {}",
            copy.display()
        );
        db.drop_backup(id)?;
    }
    Ok(None)
}

/// Deletes the originals older than [`KEEP`]. Returns how many went.
pub fn prune(db: &Db, dir: &Path) -> Result<usize> {
    let before = now_ms() - KEEP.as_millis() as i64;
    let mut gone = 0;
    for file in db.take_old_backups(before)? {
        let file = PathBuf::from(file);
        if !inside(dir, &file) {
            log::warn!(
                "kept original outside the backup folder ignored: {}",
                file.display()
            );
        } else if remove(&file) {
            gone += 1;
        }
    }
    Ok(gone)
}

/// The photo went to the trash: its kept originals go too, instead of staying for up to 30
/// days where nobody expects copies of it. A copy that can't be deleted keeps its row, so
/// [`prune`] tries again. Returns how many went.
pub fn discard(db: &Db, dir: &Path, path: &Path) -> Result<usize> {
    let mut gone = 0;
    for (id, copy) in db.backups_of(&path.to_string_lossy())? {
        let copy = PathBuf::from(copy);
        if !inside(dir, &copy) {
            log::warn!(
                "kept original outside the backup folder ignored: {}",
                copy.display()
            );
        } else if remove(&copy) {
            gone += 1;
        } else {
            continue;
        }
        db.drop_backup(id)?;
    }
    Ok(gone)
}

/// `keep` puts every copy directly into `dir`; a path anywhere else is not ours.
fn inside(dir: &Path, copy: &Path) -> bool {
    copy.parent() == Some(dir)
        && matches!(copy.components().next_back(), Some(Component::Normal(_)))
}

/// Whether the file is gone now (also when it already was).
fn remove(file: &Path) -> bool {
    match std::fs::remove_file(file) {
        Ok(()) => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
        Err(err) => {
            log::warn!("kept original {}: {err}", file.display());
            false
        }
    }
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
        assert_eq!(prune(&db, &backups).expect("prune"), 0);
        db.drop_backup(id).expect("drop");
        let old = now_ms() - KEEP.as_millis() as i64 - 86_400_000;
        db.push_backup(&photo.to_string_lossy(), &copy.to_string_lossy(), old)
            .expect("old row");
        assert_eq!(prune(&db, &backups).expect("prune"), 1);
        assert!(!copy.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn only_files_directly_in_the_folder_are_ours() {
        let dir = Path::new("/data/backups");
        assert!(inside(dir, &dir.join("1-00ff-IMG_1.jpg")));
        assert!(!inside(dir, &dir.join("..").join("IMG_1.jpg")));
        assert!(!inside(dir, &dir.join("..")));
        assert!(!inside(dir, &dir.join("sub").join("IMG_1.jpg")));
        assert!(!inside(dir, dir));
        assert!(!inside(dir, Path::new("/data/IMG_1.jpg")));
    }

    /// Rows an index editor pointed at other files: `Ctrl+Z` skips and drops them, pruning and
    /// deleting the photo leave those files alone.
    #[test]
    fn rows_outside_the_backup_folder_are_ignored() {
        let db = Db::open_in_memory().expect("db");
        let root =
            std::env::temp_dir().join(format!("cerno-backup-outside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dir");
        let photo = root.join("photo.jpg");
        std::fs::write(&photo, b"original").expect("photo");
        let victim = root.join("victim.txt");
        std::fs::write(&victim, b"not a backup").expect("victim");
        let backups = root.join("backups");
        let key = photo.to_string_lossy();
        let copy = keep(&db, &backups, &photo).expect("backup");

        // Newer than the real copy, so Ctrl+Z would pick them first.
        let sneaky = backups.join("..").join("victim.txt");
        for (file, at) in [(&victim, now_ms() + 1_000), (&sneaky, now_ms() + 2_000)] {
            db.push_backup(&key, &file.to_string_lossy(), at)
                .expect("row");
        }
        let found = latest(&db, &backups, &photo).expect("latest");
        assert_eq!(found.map(|(_, file)| file), Some(copy.clone()));
        assert_eq!(db.backups_of(&key).expect("rows").len(), 1, "dropped");

        let old = now_ms() - KEEP.as_millis() as i64 - 86_400_000;
        db.push_backup(&key, &victim.to_string_lossy(), old)
            .expect("old row");
        assert_eq!(prune(&db, &backups).expect("prune"), 0);

        db.push_backup(&key, &victim.to_string_lossy(), now_ms())
            .expect("row");
        assert_eq!(discard(&db, &backups, &photo).expect("discard"), 1);
        assert!(!copy.exists());
        assert!(db.backups_of(&key).expect("rows").is_empty());
        assert_eq!(std::fs::read(&victim).expect("victim"), b"not a backup");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A photo that went to the trash takes its kept originals along, and only its own.
    #[test]
    fn a_deleted_photo_takes_its_copies_along() {
        let db = Db::open_in_memory().expect("db");
        let root =
            std::env::temp_dir().join(format!("cerno-backup-discard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dir");
        let backups = root.join("backups");
        let (gone, kept) = (root.join("gone.jpg"), root.join("kept.jpg"));
        std::fs::write(&gone, b"gone").expect("photo");
        std::fs::write(&kept, b"kept").expect("photo");
        let copies = [
            keep(&db, &backups, &gone).expect("backup"),
            keep(&db, &backups, &gone).expect("backup"),
        ];
        let other = keep(&db, &backups, &kept).expect("backup");

        assert_eq!(discard(&db, &backups, &gone).expect("discard"), 2);
        assert!(copies.iter().all(|copy| !copy.exists()));
        assert!(
            db.backups_of(&gone.to_string_lossy())
                .expect("rows")
                .is_empty()
        );
        assert!(other.exists());
        assert_eq!(
            db.backups_of(&kept.to_string_lossy()).expect("rows").len(),
            1
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
