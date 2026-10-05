//! The programs Cerno downloads itself: ExifTool on Windows, which writes the marks. Pinned
//! like the models – one version, its size and SHA-256 – and unpacked into the data folder
//! (`exiftool::download_dir`), where `exiftool::locate` finds it. Linux takes ExifTool from the
//! system's packages.

use std::fs::File;
use std::io::{self, Read, Seek};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context as _, Result, bail};

use crate::download::{self, RemoteFile};

/// The official Windows package (its own Perl inside), from SourceForge, where every version
/// keeps its address – exiftool.org only has the newest. A new ExifTool comes with a new Cerno
/// version that changes these values.
pub const EXIFTOOL: RemoteFile = RemoteFile {
    name: "exiftool-13.59_64.zip",
    url: "https://sourceforge.net/projects/exiftool/files/exiftool-13.59_64.zip/download",
    mirror: Some("https://downloads.sourceforge.net/project/exiftool/exiftool-13.59_64.zip"),
    bytes: 11_183_675,
    sha256: "44b512b25af500724ba579d0a53c8fc5851628b692dd5e5d94ae4a15c2cba9ec",
};

/// The program in the package: with `(-k)` in its name it waits for a key before it ends.
const PACKED_EXE: &str = "exiftool(-k).exe";

/// What an archive may hold at most: 13.59 has 629 entries, 34.5 MB, the largest 3.4 MB.
#[derive(Debug, Clone, Copy)]
struct Limits {
    entries: usize,
    entry_bytes: u64,
    total_bytes: u64,
}

const LIMITS: Limits = Limits {
    entries: 2000,
    entry_bytes: 64 << 20,
    total_bytes: 200 << 20,
};

/// Downloads ExifTool, checks it and unpacks it into `dest` (`exiftool::download_dir`): first
/// into a folder of its own beside it, renamed into place only when complete, so a half
/// unpacked ExifTool is never found. The zip goes afterwards. Returns the program's path.
pub fn install_exiftool(
    dest: &Path,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<PathBuf> {
    let parent = dest.parent().context("no folder for ExifTool")?;
    std::fs::create_dir_all(parent)?;
    remove_leftovers(parent);
    download::download_file(parent, &EXIFTOOL, progress, cancelled)?;
    let zip = parent.join(EXIFTOOL.name);
    let unpacking = parent.join(format!("exiftool.tmp-{}", std::process::id()));
    let unpacked = File::open(&zip)
        .context("cannot open the ExifTool download")
        .and_then(|file| unpack(file, &unpacking, LIMITS))
        .and_then(|()| {
            std::fs::rename(
                unpacking.join(PACKED_EXE),
                unpacking.join(crate::exiftool::NAME),
            )
            .context("the ExifTool package has no exiftool(-k).exe")
        })
        .and_then(|()| check_program(&unpacking));
    if let Err(err) = unpacked {
        let _ = std::fs::remove_dir_all(&unpacking);
        return Err(err);
    }
    // An older or broken one in its place goes; only this fixed folder, never a path from
    // anywhere else.
    if dest.exists() {
        std::fs::remove_dir_all(dest).context("cannot remove the old ExifTool")?;
    }
    std::fs::rename(&unpacking, dest).context("cannot put ExifTool in place")?;
    let _ = std::fs::remove_file(&zip);
    Ok(dest.join(crate::exiftool::NAME))
}

/// Folders an interrupted unpacking left beside the tools (`exiftool.tmp-<pid>`).
fn remove_leftovers(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let leftover = entry
            .file_name()
            .to_string_lossy()
            .starts_with("exiftool.tmp-");
        if leftover && entry.path().is_dir() {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// The unpacked program and its Perl folder are there, inside `dir`.
fn check_program(dir: &Path) -> Result<()> {
    let exe = dir.join(crate::exiftool::NAME).canonicalize()?;
    let dir = dir.canonicalize()?;
    if !exe.starts_with(&dir) || !dir.join("exiftool_files").is_dir() {
        bail!("the ExifTool package is not as expected");
    }
    Ok(())
}

/// Unpacks `archive` into `dest`, without its one top folder (`exiftool-13.59_64/`). Every
/// name must stay inside `dest`: no `..`, no absolute path, no drive, no `:` (an alternate data
/// stream on Windows), no link. The bytes are counted while they are read – a size the archive
/// states is not trusted.
fn unpack(archive: impl Read + Seek, dest: &Path, limits: Limits) -> Result<()> {
    let mut archive = zip::ZipArchive::new(archive).context("the download is no zip archive")?;
    if archive.len() > limits.entries {
        bail!("the archive has {} entries", archive.len());
    }
    std::fs::create_dir_all(dest)?;
    let mut top: Option<PathBuf> = None;
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        let path = entry
            .enclosed_name()
            .filter(|path| plain(path))
            .with_context(|| format!("unsafe name in the archive: {name}"))?;
        if entry.is_symlink() {
            bail!("link in the archive: {name}");
        }
        let mut components = path.components();
        let first = PathBuf::from(components.next().context("empty name")?.as_os_str());
        if top.get_or_insert_with(|| first.clone()) != &first {
            bail!("more than one top folder in the archive: {name}");
        }
        let inner = components.as_path();
        if inner.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(inner);
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = File::create_new(&target)
            .with_context(|| format!("cannot create {}", target.display()))?;
        let written = io::copy(&mut (&mut entry).take(limits.entry_bytes + 1), &mut out)?;
        total += written;
        if written > limits.entry_bytes || total > limits.total_bytes {
            bail!("the archive unpacks to more than expected ({name})");
        }
        if written != entry.size() {
            bail!("{name}: {written} bytes instead of {}", entry.size());
        }
    }
    Ok(())
}

/// Only plain names: no root, drive, `.` or `..`, and no `:` anywhere.
fn plain(path: &Path) -> bool {
    path.components().all(|component| match component {
        Component::Normal(part) => !part.to_string_lossy().contains(':'),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cerno-tools-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// A zip in memory with these files (`/` at the end: a folder).
    fn archive(entries: &[(&str, &[u8])]) -> Cursor<Vec<u8>> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        for (name, bytes) in entries {
            if name.ends_with('/') {
                zip.add_directory(*name, options).unwrap();
            } else {
                zip.start_file(*name, options).unwrap();
                zip.write_all(bytes).unwrap();
            }
        }
        Cursor::new(zip.finish().unwrap().into_inner())
    }

    const SMALL: Limits = Limits {
        entries: 10,
        entry_bytes: 100,
        total_bytes: 150,
    };

    #[test]
    fn the_package_is_unpacked_without_its_top_folder() {
        let dest = temp_dir("good");
        let zip = archive(&[
            ("exiftool-13.59_64/", b""),
            ("exiftool-13.59_64/exiftool(-k).exe", b"exe"),
            ("exiftool-13.59_64/exiftool_files/", b""),
            ("exiftool-13.59_64/exiftool_files/lib/Image.pm", b"perl"),
        ]);
        unpack(zip, &dest, SMALL).unwrap();
        assert_eq!(std::fs::read(dest.join(PACKED_EXE)).unwrap(), b"exe");
        let module = dest.join("exiftool_files").join("lib").join("Image.pm");
        assert_eq!(std::fs::read(module).unwrap(), b"perl");
        std::fs::rename(dest.join(PACKED_EXE), dest.join(crate::exiftool::NAME)).unwrap();
        check_program(&dest).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }

    /// Nothing may land outside the folder, and nothing unexpected inside it.
    #[test]
    fn an_unsafe_archive_is_refused() {
        let cases: [&[(&str, &[u8])]; 4] = [
            &[("top/../../evil.txt", b"x")],
            &[("top/file.txt:stream", b"x")],
            &[("top/a.txt", b"x"), ("other/b.txt", b"x")],
            // Bigger than one entry may be.
            &[("top/big.bin", &[0u8; 101])],
        ];
        for (i, entries) in cases.iter().enumerate() {
            let dest = temp_dir(&format!("bad{i}"));
            assert!(
                unpack(archive(entries), &dest, SMALL).is_err(),
                "{entries:?}"
            );
            let parent = dest.parent().unwrap();
            assert!(!parent.join("evil.txt").exists());
            let _ = std::fs::remove_dir_all(&dest);
        }
    }

    /// The zip writer strips a leading `/` and a drive itself, so those names are checked here.
    #[test]
    fn only_plain_names_count() {
        assert!(plain(Path::new("top/exiftool_files/lib/Image.pm")));
        assert!(!plain(Path::new("/top/abs.txt")));
        assert!(!plain(Path::new("top/../evil.txt")));
        assert!(!plain(Path::new("./top/a.txt")));
        assert!(!plain(Path::new("top/a.txt:stream")));
        if cfg!(windows) {
            assert!(!plain(Path::new(r"C:\top\a.txt")));
            assert!(!plain(Path::new(r"\\server\share\a.txt")));
        }
    }

    #[test]
    fn too_many_or_too_big_entries_are_refused() {
        let many: Vec<(String, Vec<u8>)> = (0..11)
            .map(|i| (format!("top/{i}.txt"), b"x".to_vec()))
            .collect();
        let many: Vec<(&str, &[u8])> = many
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
            .collect();
        let dest = temp_dir("many");
        let err = unpack(archive(&many), &dest, SMALL).unwrap_err();
        assert!(err.to_string().contains("entries"), "{err}");
        let _ = std::fs::remove_dir_all(&dest);

        // Each one allowed, together too much.
        let dest = temp_dir("total");
        let zip = archive(&[("top/a.bin", &[0u8; 80]), ("top/b.bin", &[0u8; 80])]);
        let err = unpack(zip, &dest, SMALL).unwrap_err();
        assert!(err.to_string().contains("more than expected"), "{err}");
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn a_link_in_the_archive_is_refused() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.add_symlink("top/link", "../../outside", SimpleFileOptions::default())
            .unwrap();
        let zip = Cursor::new(zip.finish().unwrap().into_inner());
        let dest = temp_dir("link");
        let err = unpack(zip, &dest, SMALL).unwrap_err();
        assert!(err.to_string().contains("link"), "{err}");
        let _ = std::fs::remove_dir_all(&dest);
    }

    /// Pinned like the models: a versioned file over HTTPS, 64 hex digits.
    #[test]
    fn exiftool_is_pinned() {
        for url in std::iter::once(EXIFTOOL.url).chain(EXIFTOOL.mirror) {
            assert!(url.starts_with("https://"), "{url}");
            assert!(url.contains(EXIFTOOL.name), "{url}");
            assert!(!url.contains("latest"), "{url}");
        }
        assert_eq!(EXIFTOOL.sha256.len(), 64);
        assert!(EXIFTOOL.sha256.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// Against SourceForge: `CERNO_TEST_DOWNLOAD=exiftool cargo test -- --ignored
    /// the_exiftool_package` downloads the package into a temp folder, unpacks it there and
    /// asks the program for its version. The data folder is never touched.
    #[test]
    #[ignore = "network; set CERNO_TEST_DOWNLOAD=exiftool"]
    fn the_exiftool_package_installs() {
        if std::env::var("CERNO_TEST_DOWNLOAD").as_deref() != Ok("exiftool") {
            return;
        }
        let root = temp_dir("install");
        let dest = root.join("exiftool");
        let exe = install_exiftool(&dest, &mut |_| {}, &|| false).unwrap();
        assert!(!root.join(EXIFTOOL.name).exists(), "the zip goes");
        if cfg!(windows) {
            let mut command = std::process::Command::new(&exe);
            command.arg("-ver");
            crate::process::hide_window(&mut command);
            let output = command.output().unwrap();
            let version = String::from_utf8_lossy(&output.stdout);
            assert_eq!(version.trim(), "13.59");
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
