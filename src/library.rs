use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const JPEG_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jpe", "jfif"];
const HEIF_EXTENSIONS: &[&str] = &["heic", "heif", "hif"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Jpeg,
    Heif,
}

pub fn format_of(path: &Path) -> Option<Format> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if JPEG_EXTENSIONS.contains(&ext.as_str()) {
        Some(Format::Jpeg)
    } else if HEIF_EXTENSIONS.contains(&ext.as_str()) {
        Some(Format::Heif)
    } else {
        None
    }
}

/// The images of one folder, in natural filename order (relative path, when subfolders are
/// included).
pub struct Library {
    pub dir: PathBuf,
    pub paths: Arc<Vec<PathBuf>>,
}

impl Library {
    /// Opens a folder, or the folder containing an image. Returns the library and the index to
    /// start at (the given image, otherwise 0). `subfolders` walks nested folders, skipping
    /// hidden ones (a name starting with `.`) and never following a directory symlink.
    pub fn open(path: &Path, subfolders: bool) -> io::Result<(Library, usize)> {
        let (dir, selected) = if path.is_dir() {
            (path.to_path_buf(), None)
        } else {
            let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
            (
                parent.unwrap_or(Path::new(".")).to_path_buf(),
                Some(path.to_path_buf()),
            )
        };

        let mut paths = Vec::new();
        collect(&dir, subfolders, &mut paths)?;
        paths.sort_by(|a, b| {
            let (a, b) = (relative_key(&dir, a), relative_key(&dir, b));
            natural_cmp(&a, &b).then_with(|| a.cmp(&b))
        });

        let index = selected
            .and_then(|selected| paths.iter().position(|p| p == &selected))
            .unwrap_or(0);
        Ok((
            Library {
                dir,
                paths: Arc::new(paths),
            },
            index,
        ))
    }
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

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

/// Walks `start` with an explicit stack. A failure on the folder the user opened is reported;
/// a failure deeper down is skipped. Directory symlinks are not followed.
fn collect(start: &Path, subfolders: bool, out: &mut Vec<PathBuf>) -> io::Result<()> {
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
                    out.push(path);
                }
                continue;
            }
            if kind.is_dir() && subfolders {
                stack.push((path, false));
            } else if kind.is_file() && format_of(&path).is_some() {
                out.push(path);
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
fn sort_key(c: char) -> char {
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

    #[test]
    fn formats_by_extension() {
        assert_eq!(format_of(Path::new("x/P1.JPG")), Some(Format::Jpeg));
        assert_eq!(format_of(Path::new("x/P1.heic")), Some(Format::Heif));
        assert_eq!(format_of(Path::new("x/P1.png")), None);
        assert_eq!(format_of(Path::new("x/noext")), None);
    }
}
