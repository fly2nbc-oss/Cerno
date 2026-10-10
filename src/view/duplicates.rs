//! Exact duplicates: which file is the original, which ones are copies of it.

use super::*;

/// Photos with the same fingerprint: one is the original (`pick_original`), every other one
/// points at it.
pub(super) fn duplicate_originals(
    all: &[PathBuf],
    fingerprint: impl Fn(&Path) -> Option<u64>,
    marked: impl Fn(&Path) -> bool,
) -> HashMap<PathBuf, PathBuf> {
    let mut groups: HashMap<u64, Vec<&PathBuf>> = HashMap::new();
    for path in all {
        if let Some(fp) = fingerprint(path) {
            groups.entry(fp).or_default().push(path);
        }
    }
    let mut copies = HashMap::new();
    for paths in groups.into_values() {
        if paths.len() < 2 {
            continue;
        }
        let original = pick_original(&paths, &marked);
        for dup in paths.into_iter().filter(|path| *path != original) {
            copies.insert(dup.clone(), original.clone());
        }
    }
    copies
}

/// Which of several identical files is the original:
/// 1. the one whose name the others only extend – `IMG_1.jpg` for `IMG_1 - Kopie.jpg` or
///    `IMG_1 (1).jpg`, which sort *before* it (a space comes before the dot);
/// 2. otherwise the only one with stars, a rejection or a colour label;
/// 3. otherwise the first in folder order.
pub(super) fn pick_original<'a>(
    paths: &[&'a PathBuf],
    marked: &impl Fn(&Path) -> bool,
) -> &'a PathBuf {
    let stem = |path: &Path| -> Vec<char> {
        path.file_stem()
            .map(|s| {
                s.to_string_lossy()
                    .chars()
                    .map(crate::library::sort_key)
                    .collect()
            })
            .unwrap_or_default()
    };
    let stems: Vec<Vec<char>> = paths.iter().map(|path| stem(path)).collect();
    let shortest: Vec<usize> = (0..paths.len())
        .filter(|&i| {
            (0..paths.len()).all(|j| {
                j == i || (stems[j].len() > stems[i].len() && stems[j].starts_with(&stems[i]))
            })
        })
        .collect();
    if let [only] = shortest[..] {
        return paths[only];
    }
    let with_marks: Vec<usize> = (0..paths.len()).filter(|&i| marked(paths[i])).collect();
    if let [only] = with_marks[..] {
        return paths[only];
    }
    paths[0]
}
