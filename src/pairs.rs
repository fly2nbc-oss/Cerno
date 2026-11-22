//! RAW + JPG pairs as one photo: in a folder, a camera RAW (or DNG) and a JPEG of the same name
//! (`IMG_1.CR3` + `IMG_1.JPG`) are shown as the JPEG alone, the RAW riding along – marks are
//! written to both, copy, move, delete and put back take both, edits change the JPEG only.
//! Only an unmistakable pair counts: the same folder, the same name (case aside), exactly one
//! JPEG and exactly one RAW. Pure, so it is tested here.
//!
//! iPhone Live Photos (CER-73) pair the same way: a HEIC or JPEG and a short `.MOV` of the
//! same name are one photo, the clip riding along like a RAW – but the user's marks go into the
//! picture only (`Pairs::motion`). Whether a `.MOV` is such a clip, the caller tells
//! ([`Rules::motion`]; `quicktime::Movie::is_live_clip`): a real video is never taken in.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::library::{self, Format};

/// The RAW or Live Photo clip riding along with each photo shown in its place.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Pairs {
    companions: HashMap<PathBuf, PathBuf>,
    /// The photos whose companion is a Live Photo's clip, not a RAW.
    motion: HashSet<PathBuf>,
}

/// Which pairs to make: RAW + JPG (*RAW+JPG as one photo*) and Live Photos (*Live Photos as
/// one photo*, `motion` tells a clip from a video).
pub struct Rules<'a> {
    pub raw: bool,
    pub motion: Option<&'a dyn Fn(&Path) -> bool>,
}

impl Rules<'_> {
    pub fn any(&self) -> bool {
        self.raw || self.motion.is_some()
    }
}

impl Pairs {
    /// `primary`'s companion when it is a RAW (marks go into it too).
    pub fn raw(&self, primary: &Path) -> Option<&Path> {
        self.companion(primary)
            .filter(|_| !self.motion.contains(primary))
    }

    /// `primary`'s companion when it is a Live Photo's clip.
    pub fn motion(&self, primary: &Path) -> Option<&Path> {
        self.companion(primary)
            .filter(|_| self.motion.contains(primary))
    }

    pub fn insert_motion(&mut self, primary: PathBuf, clip: PathBuf) {
        self.motion.insert(primary.clone());
        self.companions.insert(primary, clip);
    }

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
        self.motion.remove(primary);
    }

    pub fn insert(&mut self, primary: PathBuf, companion: PathBuf) {
        self.motion.remove(&primary);
        self.companions.insert(primary, companion);
    }

    /// `primary` is now `to`, its companion `companion` (a rename): the pair keeps its kind.
    pub fn rename(&mut self, primary: &Path, to: PathBuf, companion: PathBuf) {
        let motion = self.motion.contains(primary);
        self.forget(primary);
        if motion {
            self.insert_motion(to, companion);
        } else {
            self.insert(to, companion);
        }
    }

    /// Adds the pairs found later (a RAW put back beside its JPEG).
    pub fn extend(&mut self, other: Pairs) {
        self.companions.extend(other.companions);
        self.motion.extend(other.motion);
    }
}

/// Takes the RAW or clip of every pair out of `paths` (their order stays) and returns the
/// pairs. `name_of` gives the name a file is paired by – its own, or for a deleted photo the one
/// it had in its folder. A photo has one companion: beside a RAW pair a clip stays apart.
pub fn pair_up(
    paths: Vec<PathBuf>,
    name_of: impl Fn(&Path) -> PathBuf,
    rules: &Rules,
) -> (Vec<PathBuf>, Pairs) {
    #[derive(Default)]
    struct Twins {
        jpegs: Vec<usize>,
        raws: Vec<usize>,
        heics: Vec<usize>,
        clips: Vec<usize>,
    }
    let mut groups: HashMap<(PathBuf, String), Twins> = HashMap::new();
    for (i, path) in paths.iter().enumerate() {
        let format = library::format_of(path);
        let is_jpeg = format == Some(Format::Jpeg);
        let is_raw = format.is_some_and(Format::is_raw);
        let is_heic = format == Some(Format::Heif);
        let is_mov = format == Some(Format::Video)
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("mov"));
        if !is_jpeg && !is_raw && !is_heic && !is_mov {
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
        } else if is_raw {
            twins.raws.push(i);
        } else if is_heic {
            twins.heics.push(i);
        } else {
            twins.clips.push(i);
        }
    }
    let mut pairs = Pairs::default();
    let mut riding = vec![false; paths.len()];
    for twins in groups.values() {
        if rules.raw
            && let ([jpeg], [raw]) = (twins.jpegs.as_slice(), twins.raws.as_slice())
        {
            pairs.insert(paths[*jpeg].clone(), paths[*raw].clone());
            riding[*raw] = true;
            continue;
        }
        // One picture (HEIC or JPEG) and one clip that is a Live Photo's.
        let pictures: Vec<usize> = twins.jpegs.iter().chain(&twins.heics).copied().collect();
        if let (Some(motion), [picture], [clip]) =
            (rules.motion, pictures.as_slice(), twins.clips.as_slice())
            && motion(&paths[*clip])
        {
            pairs.insert_motion(paths[*picture].clone(), paths[*clip].clone());
            riding[*clip] = true;
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

    const RAW: Rules = Rules {
        raw: true,
        motion: None,
    };

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
        let (shown, pairs) = pair_up(all, own, &RAW);
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
        let (shown, pairs) = pair_up(all.clone(), own, &RAW);
        assert!(pairs.is_empty());
        assert_eq!(shown, all);
    }

    /// A HEIC or JPEG and a `.MOV` of one name are one photo when the clip is a Live Photo's;
    /// a long video stays apart, and so does a clip beside a RAW pair. Marks go to the picture
    /// alone: the clip is no RAW.
    #[test]
    fn a_live_photo_is_one_photo() {
        let all = paths(&[
            "IMG_1.HEIC",
            "IMG_1.MOV",
            "IMG_2.JPG",
            "IMG_2.mov",
            "IMG_3.HEIC",
            "IMG_3.MOV", // a real video
            "IMG_4.JPG",
            "IMG_4.CR3",
            "IMG_4.MOV", // beside a RAW pair
            "IMG_5.HEIC",
            "IMG_5.MP4", // not a QuickTime clip
        ]);
        let live = |path: &Path| !path.to_string_lossy().contains("IMG_3");
        let rules = Rules {
            raw: true,
            motion: Some(&live),
        };
        let (shown, pairs) = pair_up(all, own, &rules);
        assert_eq!(
            names(&shown),
            [
                "IMG_1.HEIC",
                "IMG_2.JPG",
                "IMG_3.HEIC",
                "IMG_3.MOV",
                "IMG_4.JPG",
                "IMG_4.MOV",
                "IMG_5.HEIC",
                "IMG_5.MP4"
            ]
        );
        let heic = Path::new("/shoot/IMG_1.HEIC");
        assert_eq!(pairs.motion(heic), Some(Path::new("/shoot/IMG_1.MOV")));
        assert_eq!(pairs.raw(heic), None, "no marks into the clip");
        let jpeg = Path::new("/shoot/IMG_4.JPG");
        assert_eq!(pairs.raw(jpeg), Some(Path::new("/shoot/IMG_4.CR3")));
        assert_eq!(pairs.motion(jpeg), None);
        // Live Photos off: the clip shows on its own.
        let (shown, pairs) = pair_up(paths(&["IMG_1.HEIC", "IMG_1.MOV"]), own, &RAW);
        assert_eq!(shown.len(), 2);
        assert!(pairs.is_empty());
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
        let (shown, pairs) = pair_up(aside, original, &RAW);
        assert_eq!(names(&shown), [".originals/IMG_1 (2).jpg"]);
        assert!(!pairs.is_empty());
    }
}
