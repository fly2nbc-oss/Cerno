//! RAW + JPG pairs as one photo: in a folder, a camera RAW (or DNG) and a JPEG of the same name
//! (`IMG_1.CR3` + `IMG_1.JPG`) are shown as the JPEG alone, the RAW riding along – marks are
//! written to both, copy, move, delete and put back take both, edits change the JPEG only.
//! Only an unmistakable pair counts: the same folder, the same name (case aside), exactly one
//! JPEG and exactly one RAW. Pure, so it is tested here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::library::{self, Format};

/// The RAW riding along with each JPEG shown in its place.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Pairs {
    companions: HashMap<PathBuf, PathBuf>,
}

impl Pairs {
    /// The RAW that rides along with `primary`, the JPEG on screen.
    pub fn companion(&self, primary: &Path) -> Option<&Path> {
        self.companions.get(primary).map(PathBuf::as_path)
    }

    /// The JPEG a RAW rides along with.
    pub fn primary_of(&self, companion: &Path) -> Option<&Path> {
        self.companions
            .iter()
            .find(|(_, raw)| raw.as_path() == companion)
            .map(|(jpeg, _)| jpeg.as_path())
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.companions.is_empty()
    }

    /// The pair is gone (moved away, deleted).
    pub fn forget(&mut self, primary: &Path) {
        self.companions.remove(primary);
    }

    pub fn insert(&mut self, primary: PathBuf, companion: PathBuf) {
        self.companions.insert(primary, companion);
    }

    /// Adds the pairs found later (a RAW put back beside its JPEG).
    pub fn extend(&mut self, other: Pairs) {
        self.companions.extend(other.companions);
    }
}

/// Takes the RAW of every pair out of `paths` (their order stays) and returns the pairs.
/// `name_of` gives the name a file is paired by – its own, or for a deleted photo the one it
/// had in its folder.
pub fn pair_up(paths: Vec<PathBuf>, name_of: impl Fn(&Path) -> PathBuf) -> (Vec<PathBuf>, Pairs) {
    #[derive(Default)]
    struct Twins {
        jpegs: Vec<usize>,
        raws: Vec<usize>,
    }
    let mut groups: HashMap<(PathBuf, String), Twins> = HashMap::new();
    for (i, path) in paths.iter().enumerate() {
        let format = library::format_of(path);
        let is_jpeg = format == Some(Format::Jpeg);
        let is_raw = format.is_some_and(Format::is_raw);
        if !is_jpeg && !is_raw {
            continue;
        }
        let name = name_of(path);
        let Some(stem) = name.file_stem() else {
            continue;
        };
        let key = (
            name.parent().map(Path::to_path_buf).unwrap_or_default(),
            stem.to_string_lossy().to_lowercase(),
        );
        let twins = groups.entry(key).or_default();
        if is_jpeg {
            twins.jpegs.push(i);
        } else {
            twins.raws.push(i);
        }
    }
    let mut pairs = Pairs::default();
    let mut riding = vec![false; paths.len()];
    for twins in groups.values() {
        if let ([jpeg], [raw]) = (twins.jpegs.as_slice(), twins.raws.as_slice()) {
            pairs.insert(paths[*jpeg].clone(), paths[*raw].clone());
            riding[*raw] = true;
        }
    }
    let shown = paths
        .into_iter()
        .zip(riding)
        .filter_map(|(path, rides)| (!rides).then_some(path))
        .collect();
    (shown, pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(|n| Path::new("/shoot").join(n)).collect()
    }

    fn own(path: &Path) -> PathBuf {
        path.to_path_buf()
    }

    fn names(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|p| {
                p.strip_prefix("/shoot")
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    /// A RAW and a JPEG of one name are one photo, shown as the JPEG; case doesn't matter.
    #[test]
    fn a_raw_rides_along_with_its_jpeg() {
        let all = paths(&[
            "IMG_1.CR3",
            "img_1.jpg",
            "IMG_2.ARW",
            "IMG_3.jpg",
            "IMG_4.dng",
            "IMG_4.JPEG",
        ]);
        let (shown, pairs) = pair_up(all, own);
        assert_eq!(
            names(&shown),
            ["img_1.jpg", "IMG_2.ARW", "IMG_3.jpg", "IMG_4.JPEG"]
        );
        assert_eq!(
            pairs.companion(Path::new("/shoot/img_1.jpg")),
            Some(Path::new("/shoot/IMG_1.CR3"))
        );
        assert_eq!(
            pairs.companion(Path::new("/shoot/IMG_4.JPEG")),
            Some(Path::new("/shoot/IMG_4.dng"))
        );
        assert_eq!(
            pairs.primary_of(Path::new("/shoot/IMG_1.CR3")),
            Some(Path::new("/shoot/img_1.jpg"))
        );
        assert!(pairs.companion(Path::new("/shoot/IMG_3.jpg")).is_none());
    }

    /// Anything but exactly one JPEG and one RAW in one folder stays apart.
    #[test]
    fn only_an_unmistakable_pair_counts() {
        let all = paths(&[
            "A.JPG", "A.jpg", "A.CR2", // two JPEGs (a case-sensitive file system)
            "B.CR2", "B.DNG", "B.jpg", // two RAWs
            "x/C.jpg", "y/C.NEF", // other folders
            "D.MP4", "D.jpg", // a video is no RAW
        ]);
        let (shown, pairs) = pair_up(all.clone(), own);
        assert!(pairs.is_empty());
        assert_eq!(shown, all);
    }

    /// Deleted photos pair by the names they had: `IMG_1 (2).jpg` in `.originals` was
    /// `IMG_1.jpg`.
    #[test]
    fn deleted_photos_pair_by_their_old_names() {
        let aside = paths(&[".originals/IMG_1 (2).jpg", ".originals/IMG_1.CR3"]);
        let original = |p: &Path| {
            let name = p.file_name().unwrap().to_string_lossy().replace(" (2)", "");
            Path::new("/shoot").join(name)
        };
        let (shown, pairs) = pair_up(aside, original);
        assert_eq!(names(&shown), [".originals/IMG_1 (2).jpg"]);
        assert!(!pairs.is_empty());
    }
}
