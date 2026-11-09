use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::FileStamp;

/// File extensions per format, lower case.
const EXTENSIONS: &[(Format, &[&str])] = &[
    (Format::Jpeg, &["jpg", "jpeg", "jpe", "jfif"]),
    (Format::Heif, &["heic", "heif", "hif"]),
    (Format::Png, &["png"]),
    (Format::Tiff, &["tif", "tiff"]),
    (Format::WebP, &["webp"]),
    (Format::Bmp, &["bmp"]),
    (Format::Gif, &["gif"]),
    (Format::Dng, &["dng"]),
    (
        Format::Raw,
        &[
            "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw",
            "3fr", "erf", "mrw", "x3f",
        ],
    ),
    (
        Format::Video,
        &[
            "mp4", "mov", "m4v", "avi", "mkv", "mts", "m2ts", "3gp", "webm", "wmv", "mpg", "mpeg",
        ],
    ),
];

/// What Cerno can show. Only JPEG can be straightened, cropped and turned; RAW (except DNG),
/// BMP and videos keep their marks in an XMP sidecar (`IMG_1.xmp`) instead of the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Jpeg,
    Heif,
    Png,
    Tiff,
    WebP,
    Bmp,
    /// The first frame.
    Gif,
    /// Adobe's open RAW: shown by its embedded preview, marks written into it like a JPEG.
    Dng,
    /// A camera RAW, shown by the largest JPEG preview it carries.
    Raw,
    /// Shown by one frame (`video::poster`), played inside Cerno (`playback`).
    Video,
}

impl Format {
    /// Marks go into an XMP sidecar: proprietary RAW is not rewritten, BMP holds no metadata,
    /// and a star must not mean rewriting a video of several gigabytes.
    pub fn marks_in_sidecar(self) -> bool {
        matches!(self, Self::Raw | Self::Bmp | Self::Video)
    }

    /// Shown through an embedded JPEG preview.
    pub fn is_raw(self) -> bool {
        matches!(self, Self::Raw | Self::Dng)
    }
}

pub fn format_of(path: &Path) -> Option<Format> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    EXTENSIONS
        .iter()
        .find(|(_, list)| list.contains(&ext.as_str()))
        .map(|(format, _)| *format)
}

/// The images of one folder, in natural filename order (relative path, when subfolders are
/// included).
pub struct Library {
    pub dir: PathBuf,
    pub paths: Arc<Vec<PathBuf>>,
    /// Size and modification time of `paths[i]` at `stamps[i]`, from the listing itself – on
    /// Windows it carries them, so the index preload needs no `fs::metadata` per photo
    /// (CER-71: 2.6 s for 50 000 files). `None` where the metadata could not be read, and
    /// everywhere but on Windows (`collect`). A list, not a map: hashing 100 000 paths cost
    /// the scan more than it saved.
    pub stamps: Vec<Option<FileStamp>>,
}

impl Library {
    /// Opens a folder, or the folder containing an image. Returns the library and the index to
    /// start at (the given image, otherwise 0). `subfolders` walks nested folders, skipping
    /// hidden ones (a name starting with `.`) and never following a directory symlink.
    ///
    /// Paths are made absolute first: `cerno IMG_0042.jpg` from inside the folder must select
    /// that photo (a bare name would become `./IMG_0042.jpg` and match nothing), and the index
    /// keys files by their full path.
    ///
    /// Photos in an `.originals` folder are never shown: opening that folder, or a photo in it,
    /// opens the folder it belongs to, with nothing selected.
    pub fn open(path: &Path, subfolders: bool) -> io::Result<(Library, usize)> {
        let path = std::path::absolute(path)?;
        let path = &outside_originals(&path).unwrap_or(path);
        let (dir, selected) = if path.is_dir() {
            (path.to_path_buf(), None)
        } else {
            let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
            (
                parent.unwrap_or(Path::new(".")).to_path_buf(),
                Some(path.to_path_buf()),
            )
        };

        let mut found = Vec::new();
        collect(&dir, subfolders, &mut found)?;
        let (paths, stamps) = sorted(&dir, found).into_iter().unzip();
        let paths: Vec<PathBuf> = paths;

        let index = selected
            .and_then(|selected| paths.iter().position(|p| p == &selected))
            .unwrap_or(0);
        Ok((
            Library {
                dir,
                paths: Arc::new(paths),
                stamps,
            },
            index,
        ))
    }
}

/// The library's order: natural, by the path below `dir` (a photo put back joins its place).
pub fn sort(dir: &Path, paths: &mut Vec<PathBuf>) {
    let found = paths.drain(..).map(|path| (path, ())).collect();
    paths.extend(sorted(dir, found).into_iter().map(|(path, ())| path));
}

/// `sort` for paths that carry something along.
fn sorted<T>(dir: &Path, found: Vec<(PathBuf, T)>) -> Vec<(PathBuf, T)> {
    // Each key once, not twice per comparison (a year folder has tens of thousands of files).
    let mut keyed: Vec<(String, (PathBuf, T))> = found
        .into_iter()
        .map(|item| (relative_key(dir, &item.0), item))
        .collect();
    keyed.sort_by(|(a, _), (b, _)| natural_cmp(a, b).then_with(|| a.cmp(b)));
    keyed.into_iter().map(|(_, item)| item).collect()
}

/// The listing's stamps (`Library::stamps`) for `wanted` – the library's paths in their order
/// with some left out (RAW + JPG: the RAW rides along), so one walk over both finds them
/// without hashing a path. A path it cannot place gets `None`.
pub fn stamps_for(
    paths: &[PathBuf],
    stamps: &[Option<FileStamp>],
    wanted: &[PathBuf],
) -> Vec<Option<FileStamp>> {
    let mut listed = paths.iter().zip(stamps);
    wanted
        .iter()
        .map(|want| {
            listed
                .by_ref()
                .find(|(path, _)| *path == want)
                .and_then(|(_, stamp)| *stamp)
        })
        .collect()
}

/// The folders `Library::open` walks: `start`, and with `subfolders` every folder below it
/// that is not hidden (no directory symlinks) – where their `.originals` may lie.
pub fn folders(start: &Path, subfolders: bool) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![start.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if subfolders && let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if !is_hidden(&path) && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    stack.push(path);
                }
            }
        }
        found.push(dir);
    }
    found
}

/// `100CANON/IMG_0001.JPG` when the photo is in a subfolder of `root`, otherwise the file name.
pub fn display_name(root: &Path, path: &Path) -> String {
    let relative = relative_key(root, path);
    if relative.contains('/') {
        relative
    } else {
        file_name_lossy(path)
    }
}

fn relative_key(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|rel| {
            rel.components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_else(|_| file_name_lossy(path))
}

/// The folder an `.originals` folder in `path` belongs to, when there is one.
fn outside_originals(path: &Path) -> Option<PathBuf> {
    let mut outside = PathBuf::new();
    for part in path.components() {
        if crate::originals::is_inside(Path::new(part.as_os_str())) {
            return Some(outside);
        }
        outside.push(part);
    }
    None
}

/// Hidden folders – `.originals` among them – are never walked.
fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

/// Walks `start` with an explicit stack. A failure on the folder the user opened is reported;
/// a failure deeper down is skipped. Directory symlinks are not followed.
fn collect(
    start: &Path,
    subfolders: bool,
    out: &mut Vec<(PathBuf, Option<FileStamp>)>,
) -> io::Result<()> {
    let mut stack = vec![(start.to_path_buf(), true)];
    while let Some((dir, is_root)) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if is_root => return Err(err),
            Err(_) => continue,
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if is_hidden(&path) {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            // `file_type` does not follow symlinks, so a linked folder is not walked.
            if kind.is_symlink() {
                if path.is_file() && format_of(&path).is_some() {
                    // The link's own metadata is not the photo's.
                    let stamp = FileStamp::of(&path).ok();
                    out.push((path, stamp));
                }
                continue;
            }
            if kind.is_dir() && subfolders {
                stack.push((path, false));
            } else if kind.is_file() && format_of(&path).is_some() {
                // Windows lists size and time with the name; elsewhere `metadata` is one more
                // call per file (a round trip on a network share) whether the preload needs it
                // or not – there the preload asks for itself, as before CER-71.
                let stamp = if cfg!(windows) {
                    entry
                        .metadata()
                        .ok()
                        .and_then(|meta| FileStamp::from_metadata(&meta).ok())
                } else {
                    None
                };
                out.push((path, stamp));
            }
        }
    }
    Ok(())
}

pub fn file_name_lossy(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Case- and accent-insensitive comparison that orders digit runs by value: `IMG_2` < `IMG_10`,
/// `Über` next to `uber`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (na, nb) = (take_digits(&mut a), take_digits(&mut b));
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta
                    .len()
                    .cmp(&tb.len())
                    .then_with(|| ta.cmp(tb))
                    .then_with(|| na.len().cmp(&nb.len()));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = sort_key(x).cmp(&sort_key(y));
                if ord != Ordering::Equal {
                    return ord;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// Lower-case base letter of common Latin-1 letters; plain code point order would put `Ü`
/// after `z`.
pub(crate) fn sort_key(c: char) -> char {
    match c.to_lowercase().next().unwrap_or(c) {
        'à'..='å' => 'a',
        'ç' => 'c',
        'è'..='ë' => 'e',
        'ì'..='ï' => 'i',
        'ñ' => 'n',
        'ò'..='ö' | 'ø' => 'o',
        'ù'..='ü' => 'u',
        'ý' | 'ÿ' => 'y',
        'ß' => 's',
        other => other,
    }
}

fn take_digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut digits = String::new();
    while let Some(c) = chars.next_if(char::is_ascii_digit) {
        digits.push(c);
    }
    digits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_path_selects_its_photo_and_everything_is_absolute() {
        // `cargo test` runs in the crate root.
        let (library, index) = Library::open(Path::new("tests/fixtures/tiny.jpg"), false).unwrap();
        assert!(library.dir.is_absolute());
        assert!(library.paths.iter().all(|p| p.is_absolute()));
        assert!(library.paths[index].ends_with("tiny.jpg"));
        let (from_dir, _) = Library::open(Path::new("tests/fixtures"), false).unwrap();
        assert_eq!(from_dir.paths, library.paths);
    }

    #[test]
    fn on_windows_the_listing_stamps_every_photo() {
        let (library, _) = Library::open(Path::new("tests/fixtures"), true).unwrap();
        assert!(!library.paths.is_empty());
        assert_eq!(library.stamps.len(), library.paths.len());
        for (path, stamp) in library.paths.iter().zip(&library.stamps) {
            let expected = if cfg!(windows) {
                FileStamp::of(path).ok()
            } else {
                None
            };
            assert_eq!(*stamp, expected, "{}", path.display());
        }
    }

    #[test]
    fn stamps_follow_the_paths_that_are_kept() {
        let stamp = |size| Some(FileStamp { size, mtime_ns: 1 });
        let paths: Vec<PathBuf> = ["a.jpg", "a.cr3", "b.jpg", "c.jpg"]
            .map(PathBuf::from)
            .into();
        let stamps = [stamp(1), stamp(2), None, stamp(4)];
        let wanted: Vec<PathBuf> = ["a.jpg", "b.jpg", "c.jpg"].map(PathBuf::from).into();
        assert_eq!(
            stamps_for(&paths, &stamps, &wanted),
            [stamp(1), None, stamp(4)]
        );
        // Out of order the rest goes without (the preload then asks the file).
        let reversed: Vec<PathBuf> = ["c.jpg", "a.jpg"].map(PathBuf::from).into();
        assert_eq!(stamps_for(&paths, &stamps, &reversed), [stamp(4), None]);
    }

    #[test]
    fn natural_order() {
        let mut names = vec![
            "IMG_10.jpg",
            "img_2.jpg",
            "IMG_1.jpg",
            "IMG_02.jpg",
            "a.jpg",
        ];
        names.sort_by(|a, b| natural_cmp(a, b).then_with(|| a.cmp(b)));
        assert_eq!(
            names,
            [
                "a.jpg",
                "IMG_1.jpg",
                "img_2.jpg",
                "IMG_02.jpg",
                "IMG_10.jpg"
            ]
        );
    }

    #[test]
    fn umlauts_sort_with_their_base_letter() {
        let mut names = vec!["Zebra.jpg", "Über.jpg", "windows.jpg", "Apfel.jpg"];
        names.sort_by(|a, b| natural_cmp(a, b).then_with(|| a.cmp(b)));
        assert_eq!(names, ["Apfel.jpg", "Über.jpg", "windows.jpg", "Zebra.jpg"]);
    }

    #[test]
    fn subfolders_are_optional_and_skip_hidden_dirs() {
        let root = std::env::temp_dir().join(format!("cerno-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("100CANON")).unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        std::fs::create_dir_all(root.join("100CANON").join("sub")).unwrap();
        for name in [
            "IMG_10.jpg",
            "100CANON/IMG_2.jpg",
            "100CANON/IMG_10.jpg",
            "100CANON/sub/a.jpg",
            ".hidden/secret.jpg",
            "note.txt",
        ] {
            let path = root.join(name);
            std::fs::write(&path, b"x").unwrap();
        }

        let (flat, _) = Library::open(&root, false).unwrap();
        assert_eq!(
            flat.paths
                .iter()
                .map(|p| file_name_lossy(p))
                .collect::<Vec<_>>(),
            ["IMG_10.jpg"]
        );

        let (deep, _) = Library::open(&root, true).unwrap();
        let names: Vec<_> = deep
            .paths
            .iter()
            .map(|p| display_name(&deep.dir, p))
            .collect();
        assert_eq!(
            names,
            [
                "100CANON/IMG_2.jpg",
                "100CANON/IMG_10.jpg",
                "100CANON/sub/a.jpg",
                "IMG_10.jpg",
            ]
        );
        assert_eq!(display_name(&deep.dir, &deep.paths[3]), "IMG_10.jpg");

        let (from_file, index) = Library::open(&root.join("100CANON/IMG_2.jpg"), true).unwrap();
        assert_eq!(index, 0);
        assert_eq!(
            display_name(&from_file.dir, &from_file.paths[0]),
            "IMG_2.jpg"
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    /// Kept originals and set-aside photos are never shown – not when their folder or one of
    /// them is opened directly either.
    #[test]
    fn originals_are_never_shown() {
        let root = std::env::temp_dir().join(format!("cerno-lib-orig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let originals = root.join(".originals");
        std::fs::create_dir_all(&originals).unwrap();
        std::fs::write(root.join("IMG_1.jpg"), b"x").unwrap();
        std::fs::write(originals.join("IMG_1.jpg"), b"x").unwrap();
        std::fs::write(originals.join("IMG_2.jpg"), b"x").unwrap();

        for opened in [root.clone(), originals.clone(), originals.join("IMG_2.jpg")] {
            for subfolders in [false, true] {
                let (library, index) = Library::open(&opened, subfolders).unwrap();
                assert_eq!(library.dir, root, "{}", opened.display());
                assert_eq!(*library.paths, [root.join("IMG_1.jpg")]);
                assert_eq!(index, 0);
            }
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn formats_by_extension() {
        assert_eq!(format_of(Path::new("x/P1.JPG")), Some(Format::Jpeg));
        assert_eq!(format_of(Path::new("x/P1.heic")), Some(Format::Heif));
        assert_eq!(format_of(Path::new("x/P1.PNG")), Some(Format::Png));
        assert_eq!(format_of(Path::new("x/IMG_1.CR3")), Some(Format::Raw));
        assert_eq!(format_of(Path::new("x/IMG_1.dng")), Some(Format::Dng));
        assert_eq!(format_of(Path::new("x/clip.MOV")), Some(Format::Video));
        assert_eq!(
            format_of(Path::new("x/IMG_1.xmp")),
            None,
            "sidecars are not photos"
        );
        assert_eq!(format_of(Path::new("x/noext")), None);
        assert!(Format::Raw.marks_in_sidecar() && Format::Video.marks_in_sidecar());
        assert!(!Format::Dng.marks_in_sidecar() && !Format::Png.marks_in_sidecar());
    }

    /// CER-71: how long `Library::open` takes on real folders, read-only (it only lists
    /// directories). `CERNO_SCAN=<dir>[;<dir>…]`, five runs each with subfolders off and on –
    /// the first run of a folder not read for a while comes closest to a cold start.
    /// `CERNO_SCAN=D:/Pictures/2026 cargo test --release -- --ignored --nocapture folder_scan_costs`
    #[test]
    #[ignore = "measures real folders named by CERNO_SCAN"]
    fn folder_scan_costs() {
        let Ok(dirs) = std::env::var("CERNO_SCAN") else {
            return;
        };
        for dir in dirs.split(';').filter(|dir| !dir.is_empty()) {
            for subfolders in [false, true] {
                let mut runs = Vec::new();
                let mut files = 0;
                for _ in 0..5 {
                    let started = std::time::Instant::now();
                    let (library, _) = Library::open(Path::new(dir), subfolders).unwrap();
                    runs.push(started.elapsed().as_secs_f64() * 1000.0);
                    files = library.paths.len();
                }
                let shown: Vec<String> = runs.iter().map(|ms| format!("{ms:.0}")).collect();
                println!(
                    "{dir} subfolders {}: {files} files, ms {}",
                    if subfolders { "on" } else { "off" },
                    shown.join(" / ")
                );
            }
        }
    }
}
