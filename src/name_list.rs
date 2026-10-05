//! The file-name list filter: a list a client sends back – names or numbers, one per line or
//! separated by commas, semicolons or tabs – picks out exactly those photos. Forgiving about the
//! format: case, accents and the extension don't matter (`IMG_0345` finds `IMG_0345.JPG` and
//! `IMG_0345.CR3`), a bare number is the last group of digits of a name (`345` finds
//! `IMG_0345`, not `2026_08_24__132345`), and a line is split at spaces only when it doesn't
//! match as a whole – so names with spaces stay whole. Pure, so it is tested here.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// What a list found.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Matched {
    /// The photos the list names.
    pub paths: HashSet<PathBuf>,
    /// How many of its entries found a photo, out of how many.
    pub found: usize,
    pub total: usize,
    /// Entries without a photo, as typed.
    pub missing: Vec<String>,
    /// Entries that found photos in more than one folder (all of them are in `paths`).
    pub ambiguous: Vec<String>,
}

/// Lower case, without accents – like the folder's own name order.
fn key(text: &str) -> String {
    text.trim().chars().map(crate::library::sort_key).collect()
}

/// The digits a name ends with, as a number (`IMG_0345` → 345).
fn trailing_number(stem: &str) -> Option<u64> {
    let digits: String = stem
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

/// An entry without its extension, when it has one Cerno knows (`IMG_1.JPG` → `IMG_1`), and
/// with `/` between folders.
fn entry_key(entry: &str) -> String {
    let entry = entry.trim().replace('\\', "/");
    let path = Path::new(&entry);
    let without = if crate::library::format_of(path).is_some() {
        entry
            .rsplit_once('.')
            .map_or(entry.as_str(), |(stem, _)| stem)
            .to_owned()
    } else {
        entry.clone()
    };
    key(&without)
}

/// The entries of a pasted list, each once: split at line breaks, commas, semicolons and tabs,
/// quotes around an entry dropped.
fn entries(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.split(['\n', '\r', ',', ';', '\t'])
        .map(|entry| entry.trim().trim_matches(['"', '\'']).trim())
        .filter(|entry| !entry.is_empty())
        .filter(|entry| seen.insert(key(entry)))
        .map(str::to_owned)
        .collect()
}

struct Photo<'a> {
    path: &'a PathBuf,
    stem: String,
    relative: String,
    number: Option<u64>,
}

/// The photos of `photos` (below `root`) the list `text` names.
pub fn match_list(text: &str, photos: &[PathBuf], root: &Path) -> Matched {
    let indexed: Vec<Photo<'_>> = photos
        .iter()
        .map(|path| {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let relative = path
                .strip_prefix(root)
                .unwrap_or(path)
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");
            Photo {
                path,
                number: trailing_number(&stem),
                stem: key(&stem),
                relative: key(&relative),
            }
        })
        .collect();
    let mut by_stem: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut by_relative: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut by_number: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, photo) in indexed.iter().enumerate() {
        by_stem.entry(&photo.stem).or_default().push(i);
        by_relative.entry(&photo.relative).or_default().push(i);
        if let Some(n) = photo.number {
            by_number.entry(n).or_default().push(i);
        }
    }
    let find = |entry: &str| -> Vec<usize> {
        let wanted = entry_key(entry);
        if let Some(hits) = by_relative
            .get(wanted.as_str())
            .filter(|_| wanted.contains('/'))
        {
            return hits.clone();
        }
        if let Some(hits) = by_stem.get(wanted.as_str()) {
            return hits.clone();
        }
        if !wanted.is_empty() && wanted.chars().all(|c| c.is_ascii_digit()) {
            return wanted
                .parse::<u64>()
                .ok()
                .and_then(|n| by_number.get(&n).cloned())
                .unwrap_or_default();
        }
        Vec::new()
    };

    let mut matched = Matched::default();
    for entry in entries(text) {
        let mut hits = find(&entry);
        // "IMG_1 IMG_2" from a chat: the parts, each an entry of its own.
        let parts: Vec<&str> = entry.split_whitespace().collect();
        if hits.is_empty() && parts.len() > 1 {
            for part in parts {
                record(&mut matched, part, find(part), &indexed);
            }
            continue;
        }
        hits.dedup();
        record(&mut matched, &entry, hits, &indexed);
    }
    matched
}

fn record(matched: &mut Matched, entry: &str, hits: Vec<usize>, photos: &[Photo<'_>]) {
    matched.total += 1;
    if hits.is_empty() {
        matched.missing.push(entry.to_owned());
        return;
    }
    matched.found += 1;
    let folders: HashSet<Option<&Path>> = hits.iter().map(|&i| photos[i].path.parent()).collect();
    if folders.len() > 1 {
        matched.ambiguous.push(entry.to_owned());
    }
    matched
        .paths
        .extend(hits.iter().map(|&i| photos[i].path.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photos(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(|n| Path::new("/shoot").join(n)).collect()
    }

    fn names(matched: &Matched) -> Vec<String> {
        let mut names: Vec<String> = matched
            .paths
            .iter()
            .map(|p| {
                p.strip_prefix("/shoot")
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        names.sort();
        names
    }

    /// Any separator, any case, with or without the extension; a RAW and its JPEG both.
    #[test]
    fn a_list_finds_its_photos_however_it_is_written() {
        let all = photos(&[
            "IMG_0345.JPG",
            "IMG_0345.CR3",
            "IMG_0346.jpg",
            "Hochzeit Tanz.jpg",
            "img_0400.jpg",
        ]);
        let list = "img_0345\nIMG_0346.JPG; \"IMG_0400\"\t Hochzeit Tanz.jpg , nothing";
        let found = match_list(list, &all, Path::new("/shoot"));
        assert_eq!(
            names(&found),
            [
                "Hochzeit Tanz.jpg",
                "IMG_0345.CR3",
                "IMG_0345.JPG",
                "IMG_0346.jpg",
                "img_0400.jpg"
            ]
        );
        assert_eq!((found.found, found.total), (4, 5));
        assert_eq!(found.missing, ["nothing"]);
        assert!(
            found.ambiguous.is_empty(),
            "a RAW and its JPEG are one photo"
        );
    }

    /// A bare number is the last group of digits – not any digits inside a capture time.
    #[test]
    fn numbers_match_the_last_digits() {
        let all = photos(&["IMG_0345.jpg", "2026_08_24__132345.jpg", "DSC00345.ARW"]);
        let found = match_list("345", &all, Path::new("/shoot"));
        assert_eq!(names(&found), ["DSC00345.ARW", "IMG_0345.jpg"]);
        let found = match_list("0345, 132345", &all, Path::new("/shoot"));
        assert_eq!(found.found, 2);
        assert!(
            found
                .paths
                .contains(Path::new("/shoot/2026_08_24__132345.jpg"))
        );
    }

    /// A line of names from a chat is split at spaces only when it doesn't match whole.
    #[test]
    fn spaces_split_only_when_the_whole_line_finds_nothing() {
        let all = photos(&["IMG_1.jpg", "IMG_2.jpg", "Mein Foto.jpg"]);
        let found = match_list("IMG_1 IMG_2 IMG_9\nMein Foto", &all, Path::new("/shoot"));
        assert_eq!(names(&found), ["IMG_1.jpg", "IMG_2.jpg", "Mein Foto.jpg"]);
        assert_eq!((found.found, found.total), (3, 4));
        assert_eq!(found.missing, ["IMG_9"]);
    }

    /// The same name in two subfolders: both, and the list says so; a path picks one.
    #[test]
    fn a_name_in_two_folders_is_named() {
        let all = photos(&["a/IMG_1.jpg", "b/IMG_1.jpg", "b/IMG_2.jpg"]);
        let found = match_list("IMG_1\nIMG_1\nb/IMG_2", &all, Path::new("/shoot"));
        assert_eq!(names(&found), ["a/IMG_1.jpg", "b/IMG_1.jpg", "b/IMG_2.jpg"]);
        assert_eq!(found.ambiguous, ["IMG_1"]);
        assert_eq!(found.total, 2, "an entry twice counts once");
        let one = match_list("a/IMG_1.jpg", &all, Path::new("/shoot"));
        assert_eq!(names(&one), ["a/IMG_1.jpg"]);
        assert!(one.ambiguous.is_empty());
    }
}
