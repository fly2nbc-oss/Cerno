//! Originals are never deleted. Before a photo's first straighten, crop or quarter turn its
//! file is copied into a hidden `.originals` folder beside it – only that first original; later
//! edits leave it alone – and a photo deleted in Cerno moves there instead of the trash. Cerno
//! never shows that folder (`library`). The index (`backups` table) ties each photo to its
//! original, so `Ctrl+Z` can write it back; the copy stays.
//!
//! A row is only followed when its file lies directly in the photo's own `.originals` folder,
//! or in the data folder where Cerno 1.0 kept its copies (not moved yet, e.g. the drive was
//! missing at start): an index someone else changed never makes Cerno read, move or overwrite
//! other files ([`allowed`]). Nothing here deletes a file.

use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, bail};

use crate::db::Db;

/// The hidden folder beside the photos.
pub const FOLDER: &str = ".originals";

/// `<the photo's folder>/.originals`.
pub fn folder_of(photo: &Path) -> Option<PathBuf> {
    photo.parent().map(|dir| dir.join(FOLDER))
}

/// Whether `path` is an originals folder or lies inside one – such photos are never shown.
pub fn is_inside(path: &Path) -> bool {
    path.components().any(|part| match part {
        Component::Normal(name) => name
            .to_str()
            .is_some_and(|name| name.eq_ignore_ascii_case(FOLDER)),
        _ => false,
    })
}

/// Where Cerno 1.0 kept its copies: `data/backups`.
pub fn legacy_dir() -> Result<PathBuf> {
    Ok(crate::paths::data_dir()?.join("backups"))
}

/// A copy Cerno may use for `photo`: a plain file name directly in the photo's `.originals`
/// folder, or in the old data folder.
fn allowed(photo: &Path, legacy: Option<&Path>, copy: &Path) -> bool {
    if !matches!(copy.components().next_back(), Some(Component::Normal(_))) {
        return false;
    }
    let parent = copy.parent();
    parent.is_some() && (parent == folder_of(photo).as_deref() || parent == legacy)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Before an edit: copies the photo into `.originals` – unless its first original is kept
/// already. If this fails, the edit does not happen.
pub fn keep(db: &Db, photo: &Path) -> Result<()> {
    if original(db, photo)?.is_some() {
        return Ok(());
    }
    let dir = ensure_folder(photo)?;
    let name = photo.file_name().context("photo has no file name")?;
    let copy = copy_into(photo, &dir, name)?;
    db.push_backup(&photo.to_string_lossy(), &copy.to_string_lossy(), now_ms())?;
    log::info!("original kept: {}", copy.display());
    Ok(())
}

/// The first original kept for `photo`, for `Ctrl+Z`. Rows pointing anywhere but the two
/// allowed places are dropped on the way; a row whose file is missing is skipped but kept (the
/// drive may only be unplugged).
pub fn original(db: &Db, photo: &Path) -> Result<Option<PathBuf>> {
    let legacy = legacy_dir().ok();
    original_in(db, photo, legacy.as_deref())
}

fn original_in(db: &Db, photo: &Path, legacy: Option<&Path>) -> Result<Option<PathBuf>> {
    let mut found = None;
    // `backups_of` is newest first; the first original is the oldest one.
    for (id, copy) in db.backups_of(&photo.to_string_lossy())?.into_iter().rev() {
        let copy = PathBuf::from(copy);
        if !allowed(photo, legacy, &copy) {
            log::warn!(
                "kept original outside .originals ignored: {}",
                copy.display()
            );
            db.drop_backup(id)?;
        } else if found.is_none() && copy.is_file() {
            found = Some(copy);
        }
    }
    Ok(found)
}

/// Deleting in Cerno: the photo moves into `.originals` beside it instead of the trash. Its
/// index rows stay, so its first original is still found if it is ever brought back.
pub fn set_aside(photo: &Path) -> Result<PathBuf> {
    let dir = ensure_folder(photo)?;
    let name = photo.file_name().context("photo has no file name")?;
    let to = move_into(photo, &dir, name)?;
    log::info!("set aside: {} → {}", photo.display(), to.display());
    Ok(to)
}

/// Cerno moved the photo from `from` to `to`: its originals follow into `.originals` there,
/// so `Ctrl+Z` keeps working. Call before the index rows are retargeted. Returns how many
/// moved.
pub fn follow(db: &Db, from: &Path, to: &Path) -> Result<usize> {
    let legacy = legacy_dir().ok();
    follow_in(db, from, to, legacy.as_deref())
}

fn follow_in(db: &Db, from: &Path, to: &Path, legacy: Option<&Path>) -> Result<usize> {
    let target = folder_of(to).context("photo has no folder")?;
    let mut moved = 0;
    for (id, copy) in db.backups_of(&from.to_string_lossy())? {
        let copy = PathBuf::from(copy);
        // Old copies in the data folder are moved beside the photo at the next start.
        if !allowed(from, None, &copy) || copy.parent() == Some(target.as_path()) {
            if !allowed(from, legacy, &copy) {
                db.drop_backup(id)?;
            }
            continue;
        }
        if !copy.is_file() {
            continue;
        }
        let dir = ensure_folder(to)?;
        let name = copy.file_name().context("copy has no file name")?;
        let new = move_into(&copy, &dir, name)?;
        db.set_backup_file(id, &new.to_string_lossy())?;
        moved += 1;
    }
    Ok(moved)
}

/// Once per start: copies Cerno 1.0 kept in the data folder move beside their photos, under
/// the photo's name, the oldest (the first original) first. A photo whose folder is missing
/// keeps its copy there until a later start. Removes the old folder once it is empty.
pub fn migrate(db: &Db, legacy: &Path) -> Result<usize> {
    if !legacy.is_dir() {
        return Ok(0);
    }
    let mut moved = 0;
    for (id, photo, copy) in db.all_backups()? {
        let (photo, copy) = (PathBuf::from(photo), PathBuf::from(copy));
        if copy.parent() != Some(legacy) || !copy.is_file() {
            continue;
        }
        if !photo.parent().is_some_and(Path::is_dir) {
            continue;
        }
        let Some(name) = photo.file_name() else {
            continue;
        };
        let placed = ensure_folder(&photo).and_then(|dir| move_into(&copy, &dir, name));
        match placed {
            Ok(new) => {
                db.set_backup_file(id, &new.to_string_lossy())?;
                moved += 1;
            }
            Err(err) => log::warn!("kept original {}: {err:#}", copy.display()),
        }
    }
    // Only succeeds once nothing is left in it.
    let _ = std::fs::remove_dir(legacy);
    Ok(moved)
}

/// Creates `.originals` beside `photo`, hidden (the dot hides it on Linux, the attribute on
/// Windows).
fn ensure_folder(photo: &Path) -> Result<PathBuf> {
    let dir = folder_of(photo).context("photo has no folder")?;
    std::fs::create_dir_all(&dir).context("cannot create the .originals folder")?;
    hide(&dir);
    Ok(dir)
}

#[cfg(windows)]
fn hide(dir: &Path) {
    use std::os::windows::ffi::OsStrExt;
    const HIDDEN: u32 = 0x2;
    const INVALID: u32 = u32::MAX;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileAttributesW(name: *const u16) -> u32;
        fn SetFileAttributesW(name: *const u16, attributes: u32) -> i32;
    }
    let wide: Vec<u16> = dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives both calls.
    let hidden = unsafe {
        let current = GetFileAttributesW(wide.as_ptr());
        current != INVALID
            && (current & HIDDEN != 0 || SetFileAttributesW(wide.as_ptr(), current | HIDDEN) != 0)
    };
    if !hidden {
        log::warn!("cannot hide {}", dir.display());
    }
}

#[cfg(not(windows))]
fn hide(_dir: &Path) {}

/// `IMG_1.jpg`, then `IMG_1 (2).jpg`, `IMG_1 (3).jpg` …
fn numbered(name: &OsStr, n: u32) -> OsString {
    if n == 1 {
        return name.to_owned();
    }
    let path = Path::new(name);
    let stem = path.file_stem().unwrap_or(name).to_string_lossy();
    match path.extension() {
        Some(ext) => format!("{stem} ({n}).{}", ext.to_string_lossy()).into(),
        None => format!("{stem} ({n})").into(),
    }
}

const MAX_NAMES: u32 = 10_000;

/// Copies `from` into `dir` under the first free name – a new file each time, nothing already
/// there is ever written through. The copy keeps the modification time.
fn copy_into(from: &Path, dir: &Path, name: &OsStr) -> Result<PathBuf> {
    let mut source = File::open(from).context("cannot read the photo")?;
    let modified = source.metadata().and_then(|meta| meta.modified()).ok();
    for n in 1..=MAX_NAMES {
        let to = dir.join(numbered(name, n));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&to) {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err).context("cannot keep a copy of the original"),
        };
        let written = std::io::copy(&mut source, &mut file).and_then(|_| file.sync_all());
        if let Err(err) = written {
            drop(file);
            let _ = std::fs::remove_file(&to);
            return Err(err).context("cannot keep a copy of the original");
        }
        if let Some(time) = modified {
            let _ = file.set_modified(time);
        }
        return Ok(to);
    }
    bail!("no free name in {}", dir.display())
}

/// Moves `from` into `dir` under the first free name. Within one drive it is a rename (the
/// dates stay); across drives a copy, then the source goes – if it can't, the copy is taken
/// back and the move fails.
fn move_into(from: &Path, dir: &Path, name: &OsStr) -> Result<PathBuf> {
    for n in 1..=MAX_NAMES {
        let to = dir.join(numbered(name, n));
        // `rename` replaces an existing file on Linux; never let it.
        if to.exists() {
            continue;
        }
        match std::fs::rename(from, &to) {
            Ok(()) => return Ok(to),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) if err.kind() == ErrorKind::CrossesDevices => {
                let copy = copy_into(from, dir, name)?;
                if let Err(err) = std::fs::remove_file(from) {
                    let _ = std::fs::remove_file(&copy);
                    return Err(err).context("cannot move the file");
                }
                return Ok(copy);
            }
            Err(err) => return Err(err).context("cannot move the file"),
        }
    }
    bail!("no free name in {}", dir.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("cerno-originals-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dir");
        root
    }

    #[test]
    fn the_first_original_is_kept_once_and_stays() {
        let db = Db::open_in_memory().expect("db");
        let root = temp("keep");
        let photo = root.join("Übersicht.jpg");
        std::fs::write(&photo, b"original").expect("photo");

        keep(&db, &photo).expect("keep");
        let copy = root.join(FOLDER).join("Übersicht.jpg");
        assert_eq!(std::fs::read(&copy).expect("copy"), b"original");
        assert_eq!(
            original_in(&db, &photo, None).expect("query"),
            Some(copy.clone())
        );

        // A second edit keeps the first original, not the edited state.
        std::fs::write(&photo, b"edited").expect("edit");
        keep(&db, &photo).expect("keep again");
        assert_eq!(std::fs::read(&copy).expect("copy"), b"original");
        assert_eq!(
            std::fs::read_dir(root.join(FOLDER)).expect("dir").count(),
            1
        );
        assert_eq!(
            db.backups_of(&photo.to_string_lossy()).expect("rows").len(),
            1
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A photo deleted in Cerno lands beside its original instead of in the trash; a name that
    /// is taken gets a number, nothing is overwritten.
    #[test]
    fn a_deleted_photo_is_set_aside_next_to_its_original() {
        let db = Db::open_in_memory().expect("db");
        let root = temp("aside");
        let photo = root.join("IMG_1.jpg");
        std::fs::write(&photo, b"original").expect("photo");
        keep(&db, &photo).expect("keep");
        std::fs::write(&photo, b"edited").expect("edit");

        let to = set_aside(&photo).expect("set aside");
        assert!(!photo.exists());
        assert_eq!(to, root.join(FOLDER).join("IMG_1 (2).jpg"));
        assert_eq!(std::fs::read(&to).expect("moved"), b"edited");
        assert_eq!(
            std::fs::read(root.join(FOLDER).join("IMG_1.jpg")).expect("original"),
            b"original"
        );
        assert!(is_inside(&to));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn originals_follow_a_moved_photo() {
        let db = Db::open_in_memory().expect("db");
        let root = temp("follow");
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).expect("a");
        std::fs::create_dir_all(&b).expect("b");
        let (from, to) = (a.join("IMG_2.jpg"), b.join("IMG_2.jpg"));
        std::fs::write(&from, b"original").expect("photo");
        keep(&db, &from).expect("keep");
        std::fs::rename(&from, &to).expect("move photo");

        assert_eq!(follow_in(&db, &from, &to, None).expect("follow"), 1);
        db.retarget_path(&from.to_string_lossy(), &to.to_string_lossy())
            .expect("retarget");
        let moved = b.join(FOLDER).join("IMG_2.jpg");
        assert_eq!(
            original_in(&db, &to, None).expect("query"),
            Some(moved.clone())
        );
        assert_eq!(std::fs::read(&moved).expect("moved"), b"original");
        assert!(!a.join(FOLDER).join("IMG_2.jpg").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Copies from Cerno 1.0's data folder move beside their photos under the photo's name,
    /// the oldest first; a later copy of the same photo gets a number.
    #[test]
    fn old_copies_move_beside_their_photos() {
        let db = Db::open_in_memory().expect("db");
        let root = temp("migrate");
        let legacy = root.join("data").join("backups");
        std::fs::create_dir_all(&legacy).expect("legacy");
        let photo = root.join("photos").join("IMG_3.jpg");
        std::fs::create_dir_all(photo.parent().unwrap()).expect("photos");
        std::fs::write(&photo, b"edited twice").expect("photo");
        let key = photo.to_string_lossy();
        for (at, content) in [(1_000, "first"), (2_000, "second")] {
            let copy = legacy.join(format!("{at}-00ff-IMG_3.jpg"));
            std::fs::write(&copy, content).expect("copy");
            db.push_backup(&key, &copy.to_string_lossy(), at)
                .expect("row");
        }
        let gone = root.join("gone").join("IMG_4.jpg");
        let stays = legacy.join("3000-00ee-IMG_4.jpg");
        std::fs::write(&stays, b"folder missing").expect("copy");
        db.push_backup(&gone.to_string_lossy(), &stays.to_string_lossy(), 3_000)
            .expect("row");

        assert_eq!(migrate(&db, &legacy).expect("migrate"), 2);
        let folder = photo.parent().unwrap().join(FOLDER);
        assert_eq!(
            std::fs::read(folder.join("IMG_3.jpg")).expect("first"),
            b"first"
        );
        assert_eq!(
            std::fs::read(folder.join("IMG_3 (2).jpg")).expect("second"),
            b"second"
        );
        assert_eq!(
            original_in(&db, &photo, Some(&legacy)).expect("query"),
            Some(folder.join("IMG_3.jpg"))
        );
        assert!(stays.exists(), "its folder is missing: it waits");
        assert!(legacy.exists(), "not empty yet");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn only_files_directly_in_the_allowed_folders_are_ours() {
        let photo = Path::new("/photos/IMG_1.jpg");
        let own = Path::new("/photos/.originals");
        let legacy = Path::new("/data/backups");
        assert!(allowed(photo, Some(legacy), &own.join("IMG_1.jpg")));
        assert!(allowed(
            photo,
            Some(legacy),
            &legacy.join("1-00ff-IMG_1.jpg")
        ));
        assert!(!allowed(photo, None, &legacy.join("1-00ff-IMG_1.jpg")));
        assert!(!allowed(
            photo,
            Some(legacy),
            &own.join("..").join("IMG_1.jpg")
        ));
        assert!(!allowed(
            photo,
            Some(legacy),
            &own.join("sub").join("IMG_1.jpg")
        ));
        assert!(!allowed(
            photo,
            Some(legacy),
            Path::new("/photos/IMG_1.jpg")
        ));
        assert!(!allowed(
            photo,
            Some(legacy),
            Path::new("/other/.originals/IMG_1.jpg")
        ));
    }

    /// Rows an index editor pointed at other files: `Ctrl+Z` skips and drops them, and moving
    /// the photo leaves those files where they are.
    #[test]
    fn rows_outside_the_allowed_folders_are_ignored() {
        let db = Db::open_in_memory().expect("db");
        let root = temp("outside");
        let photo = root.join("photo.jpg");
        std::fs::write(&photo, b"original").expect("photo");
        let victim = root.join("victim.txt");
        std::fs::write(&victim, b"not a backup").expect("victim");
        let key = photo.to_string_lossy();
        let sneaky = root.join(FOLDER).join("..").join("victim.txt");
        // Older than any real copy, so they would count as the first original.
        for (file, at) in [(&victim, 1), (&sneaky, 2)] {
            db.push_backup(&key, &file.to_string_lossy(), at)
                .expect("row");
        }
        assert_eq!(original_in(&db, &photo, None).expect("query"), None);
        assert!(db.backups_of(&key).expect("rows").is_empty(), "dropped");

        keep(&db, &photo).expect("keep");
        db.push_backup(&key, &victim.to_string_lossy(), 3)
            .expect("row");
        let moved = root.join("moved.jpg");
        std::fs::rename(&photo, &moved).expect("move");
        follow_in(&db, &photo, &moved, None).expect("follow");
        assert_eq!(std::fs::read(&victim).expect("victim"), b"not a backup");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn paths_inside_an_originals_folder_are_recognised() {
        assert!(is_inside(Path::new("/p/.originals")));
        assert!(is_inside(Path::new("/p/.Originals/IMG_1.jpg")));
        assert!(!is_inside(Path::new("/p/originals/IMG_1.jpg")));
        assert!(!is_inside(Path::new("/p/.originals.jpg")));
        assert_eq!(
            numbered(OsStr::new("a.b.jpg"), 3),
            OsString::from("a.b (3).jpg")
        );
        assert_eq!(
            numbered(OsStr::new("README"), 2),
            OsString::from("README (2)")
        );
    }
}
