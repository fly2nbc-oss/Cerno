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

/// The images of one folder, in natural filename order.
pub struct Library {
    pub dir: PathBuf,
    pub paths: Arc<Vec<PathBuf>>,
}

impl Library {
    /// Opens a folder, or the folder containing an image. Returns the library and the index to
    /// start at (the given image, otherwise 0).
    pub fn open(path: &Path) -> io::Result<(Library, usize)> {
        let (dir, selected) = if path.is_dir() {
            (path.to_path_buf(), None)
        } else {
            let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
            (
                parent.unwrap_or(Path::new(".")).to_path_buf(),
                path.file_name(),
            )
        };

        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
            .map(|entry| entry.path())
            .filter(|p| format_of(p).is_some())
            .collect();
        paths.sort_by(|a, b| {
            let (a, b) = (file_name_lossy(a), file_name_lossy(b));
            natural_cmp(&a, &b).then_with(|| a.cmp(&b))
        });

        let index = selected
            .and_then(|name| paths.iter().position(|p| p.file_name() == Some(name)))
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
    fn formats_by_extension() {
        assert_eq!(format_of(Path::new("x/P1.JPG")), Some(Format::Jpeg));
        assert_eq!(format_of(Path::new("x/P1.heic")), Some(Format::Heif));
        assert_eq!(format_of(Path::new("x/P1.png")), None);
        assert_eq!(format_of(Path::new("x/noext")), None);
    }
}
