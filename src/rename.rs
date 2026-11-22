//! Renaming photos (`F2` the current one, `Shift+F2` every photo on screen): the name patterns,
//! the check of a typed name, the plan for several photos and the order the renames run in.
//! Pure, so it is tested here; the files are renamed by the transfer worker (`transfer.rs`).
//!
//! A name taken by another file gets `_1`, `_2` … – as in the author's own archive
//! (`2026_08_23__191546_1.jpg`). The extension stays as it is. A RAW riding along with its
//! JPEG takes the same new name.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::metadata::Clock;

/// The patterns the card offers, in its order (`Alt+1` … `Alt+4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// `2026_08_23__191546` – the author's own.
    Standard,
    /// `2026-08-23 19.15.46` – the camera uploads of Dropbox and OneDrive.
    Cloud,
    /// `20260823_191546` – Samsung and other Android cameras.
    Android,
    /// `2026-08-23_Roadtrip_001` – the day, a text (the folder's name to start with) and a
    /// counter in capture order.
    Event,
}

impl Pattern {
    pub const ALL: [Pattern; 4] = [
        Pattern::Standard,
        Pattern::Cloud,
        Pattern::Android,
        Pattern::Event,
    ];

    /// The name (without extension) for a photo taken at `clock`. `text`, `number` (from 1)
    /// and `digits` count only for [`Pattern::Event`].
    pub fn stem(self, clock: Clock, text: &str, number: usize, digits: usize) -> String {
        let Clock {
            year,
            month,
            day,
            hour,
            minute,
            second,
        } = clock;
        match self {
            Self::Standard => {
                format!("{year:04}_{month:02}_{day:02}__{hour:02}{minute:02}{second:02}")
            }
            Self::Cloud => {
                format!("{year:04}-{month:02}-{day:02} {hour:02}.{minute:02}.{second:02}")
            }
            Self::Android => format!("{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}"),
            Self::Event => {
                let text = text.trim();
                if text.is_empty() {
                    format!("{year:04}-{month:02}-{day:02}_{number:0digits$}")
                } else {
                    format!("{year:04}-{month:02}-{day:02}_{text}_{number:0digits$}")
                }
            }
        }
    }

    /// The pattern takes a text (the Event pattern's middle part).
    pub fn takes_text(self) -> bool {
        self == Self::Event
    }
}

/// The counter's width: at least three digits, more for more photos.
pub fn counter_digits(count: usize) -> usize {
    count.to_string().len().max(3)
}

/// Why a typed name can't be a file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    Empty,
    /// One of `< > : " / \ | ? *` or a control character – Windows refuses them.
    Character(char),
    /// `CON`, `NUL`, `COM1` … – names Windows keeps for devices.
    Reserved,
    /// A dot or a space at the end – Windows drops it silently.
    Ending,
    TooLong,
    /// Another file has the name already.
    Taken,
}

/// Longer names hit Windows' path limit too easily.
const MAX_CHARS: usize = 200;

/// Whether `stem` (without extension) can be a file name on Windows and Linux.
pub fn check(stem: &str) -> Result<(), Invalid> {
    if stem.trim().is_empty() {
        return Err(Invalid::Empty);
    }
    if let Some(c) = stem
        .chars()
        .find(|c| c.is_control() || "<>:\"/\\|?*".contains(*c))
    {
        return Err(Invalid::Character(c));
    }
    if stem.ends_with('.') || stem.ends_with(' ') {
        return Err(Invalid::Ending);
    }
    if stem.chars().count() > MAX_CHARS {
        return Err(Invalid::TooLong);
    }
    let base = stem.split('.').next().unwrap_or(stem).trim().to_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&base.as_str())
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base[3..].chars().all(|c| ('1'..='9').contains(&c)));
    if reserved {
        return Err(Invalid::Reserved);
    }
    Ok(())
}

/// The file names of a folder, case aside (Windows does not tell `a.jpg` from `A.JPG`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Names(HashSet<String>);

impl Names {
    pub fn of(names: impl IntoIterator<Item = String>) -> Self {
        Self(names.into_iter().map(|name| name.to_lowercase()).collect())
    }

    /// The names of the files in `dir` – empty when it can't be read.
    pub fn in_dir(dir: &Path) -> Self {
        let names = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned());
        Self::of(names)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.0.contains(&name.to_lowercase())
    }

    pub fn insert(&mut self, name: &str) {
        self.0.insert(name.to_lowercase());
    }

    pub fn remove(&mut self, name: &str) {
        self.0.remove(&name.to_lowercase());
    }
}

/// `stem` with an extension (none: the stem alone).
pub fn with_ext(stem: &str, ext: &str) -> String {
    if ext.is_empty() {
        stem.to_owned()
    } else {
        format!("{stem}.{ext}")
    }
}

/// A file's extension as it is written, without the dot.
pub fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|ext| ext.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A file's name without its extension.
pub fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `stem`, or the first of `stem_1`, `stem_2` … whose names – with each of `exts` – are free.
pub fn unique(stem: &str, exts: &[String], names: &Names) -> String {
    let free = |candidate: &str| {
        exts.iter()
            .all(|ext| !names.contains(&with_ext(candidate, ext)))
    };
    if free(stem) {
        return stem.to_owned();
    }
    (1..)
        .map(|n| format!("{stem}_{n}"))
        .find(|candidate| free(candidate))
        .unwrap_or_else(|| stem.to_owned())
}

/// One photo to rename: its RAW riding along, and its capture time (camera offset added).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub path: PathBuf,
    pub rider: Option<PathBuf>,
    pub clock: Option<Clock>,
}

impl Item {
    /// The extensions its new name must find free: its own, its rider's.
    fn exts(&self) -> Vec<String> {
        let mut exts = vec![ext_of(&self.path)];
        exts.extend(self.rider.as_deref().map(ext_of));
        exts
    }

    fn names(&self) -> Vec<String> {
        let mut names = vec![file_name(&self.path)];
        names.extend(self.rider.as_deref().map(file_name));
        names
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Where a photo goes: `None` when it keeps its name (no capture time, or the name is the
/// same already).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub path: PathBuf,
    pub to: Option<PathBuf>,
}

/// The new names of `items` – given in capture order, which the counter follows. `names` are
/// the file names of each folder involved. A photo renamed frees its name (and its rider's)
/// for the others; a photo without a capture time keeps its own.
pub fn plan(
    items: &[Item],
    pattern: Pattern,
    text: &str,
    names: &HashMap<PathBuf, Names>,
) -> Vec<Planned> {
    let mut names = names.clone();
    for item in items.iter().filter(|item| item.clock.is_some()) {
        if let Some(dir) = names.get_mut(&parent(&item.path)) {
            for name in item.names() {
                dir.remove(&name);
            }
        }
    }
    let count = items.iter().filter(|item| item.clock.is_some()).count();
    let digits = counter_digits(count);
    let mut number = 0;
    items
        .iter()
        .map(|item| {
            let Some(clock) = item.clock else {
                return Planned {
                    path: item.path.clone(),
                    to: None,
                };
            };
            number += 1;
            let dir = parent(&item.path);
            let folder = names.entry(dir.clone()).or_default();
            let stem = unique(
                &pattern.stem(clock, text, number, digits),
                &item.exts(),
                folder,
            );
            let exts = item.exts();
            for ext in &exts {
                folder.insert(&with_ext(&stem, ext));
            }
            let to = dir.join(with_ext(&stem, &exts[0]));
            Planned {
                to: (to != item.path).then_some(to),
                path: item.path.clone(),
            }
        })
        .collect()
}

/// A typed name for one photo: where it goes, or why it can't. `names` are its folder's file
/// names; the photo's own (and its rider's) don't count as taken.
pub fn rename_one(item: &Item, stem: &str, names: &Names) -> Result<Option<PathBuf>, Invalid> {
    check(stem)?;
    let mut names = names.clone();
    for name in item.names() {
        names.remove(&name);
    }
    if item
        .exts()
        .iter()
        .any(|ext| names.contains(&with_ext(stem, ext)))
    {
        return Err(Invalid::Taken);
    }
    let to = parent(&item.path).join(with_ext(stem, &ext_of(&item.path)));
    Ok((to != item.path).then_some(to))
}

fn parent(path: &Path) -> PathBuf {
    path.parent().map(Path::to_path_buf).unwrap_or_default()
}

/// One rename the worker carries out: `from` → `to`, for the photo at `origin` of the list
/// [`order`] was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub origin: usize,
    pub from: PathBuf,
    pub to: PathBuf,
}

/// The order to rename in: a photo whose new name is another one's old name goes after that
/// one; a ring (`A` → `B`, `B` → `A`) goes through a hidden temporary name. Names compare case
/// aside; a change of case alone is a rename of its own.
pub fn order(renames: &[(PathBuf, PathBuf)]) -> Vec<Step> {
    let key = |path: &Path| path.to_string_lossy().to_lowercase();
    let mut pending: Vec<Step> = renames
        .iter()
        .enumerate()
        .map(|(origin, (from, to))| Step {
            origin,
            from: from.clone(),
            to: to.clone(),
        })
        .collect();
    let mut steps = Vec::new();
    let mut temps = 0;
    while !pending.is_empty() {
        let blocked = |step: &Step, pending: &[Step]| {
            pending
                .iter()
                .any(|other| other.origin != step.origin && key(&other.from) == key(&step.to))
        };
        if let Some(i) = pending.iter().position(|step| !blocked(step, &pending)) {
            steps.push(pending.remove(i));
            continue;
        }
        // A ring: the first steps aside under a name nobody wants.
        let first = &mut pending[0];
        temps += 1;
        let temp =
            parent(&first.from).join(format!(".cerno-rename-{temps}-{}", file_name(&first.from)));
        steps.push(Step {
            origin: first.origin,
            from: first.from.clone(),
            to: temp.clone(),
        });
        first.from = temp;
    }
    steps
}

/// Where a rider goes when its photo goes to `to`: the same name with its own extension.
pub fn rider_to(rider: &Path, to: &Path) -> PathBuf {
    to.with_file_name(with_ext(&stem_of(to), &ext_of(rider)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::clock_of;

    /// 2026-08-23 19:15:46.
    fn clock() -> Clock {
        Clock {
            year: 2026,
            month: 8,
            day: 23,
            hour: 19,
            minute: 15,
            second: 46,
        }
    }

    #[test]
    fn the_four_patterns() {
        assert_eq!(
            Pattern::Standard.stem(clock(), "", 0, 3),
            "2026_08_23__191546"
        );
        assert_eq!(
            Pattern::Cloud.stem(clock(), "", 0, 3),
            "2026-08-23 19.15.46"
        );
        assert_eq!(Pattern::Android.stem(clock(), "", 0, 3), "20260823_191546");
        assert_eq!(
            Pattern::Event.stem(clock(), " Roadtrip ", 7, 3),
            "2026-08-23_Roadtrip_007"
        );
        assert_eq!(Pattern::Event.stem(clock(), "", 12, 4), "2026-08-23_0012");
        assert_eq!(counter_digits(9), 3);
        assert_eq!(counter_digits(1234), 4);
    }

    /// The capture time's milliseconds as a wall clock, to the second.
    #[test]
    fn a_capture_time_becomes_its_clock() {
        // 2026-08-23 19:15:46.789 as Cerno's milliseconds (days since 1970 × 86 400 000 …).
        let ms = 20_688 * 86_400_000 + ((19 * 60 + 15) * 60 + 46) * 1000 + 789;
        assert_eq!(clock_of(ms), clock());
    }

    #[test]
    fn typed_names_are_checked() {
        assert_eq!(check("2026_08_23__191546"), Ok(()));
        assert_eq!(check("Strand mit Hund"), Ok(()));
        assert_eq!(check("  "), Err(Invalid::Empty));
        assert_eq!(check("a:b"), Err(Invalid::Character(':')));
        assert_eq!(check("a/b"), Err(Invalid::Character('/')));
        assert_eq!(check("name."), Err(Invalid::Ending));
        assert_eq!(check("name "), Err(Invalid::Ending));
        assert_eq!(check("con"), Err(Invalid::Reserved));
        assert_eq!(check("COM3"), Err(Invalid::Reserved));
        assert_eq!(check("lpt1.backup"), Err(Invalid::Reserved));
        assert_eq!(check("COM0"), Ok(()));
        assert_eq!(check("console"), Ok(()));
        assert_eq!(check(&"x".repeat(201)), Err(Invalid::TooLong));
    }

    /// A taken name gets `_1`, `_2` …, case aside; the rider's name must be free too.
    #[test]
    fn a_taken_name_gets_a_number() {
        let names = Names::of([
            "2026_08_23__191546.JPG".into(),
            "2026_08_23__191546_1.cr3".into(),
        ]);
        let jpg = vec!["jpg".to_owned()];
        assert_eq!(
            unique("2026_08_23__191546", &jpg, &names),
            "2026_08_23__191546_1"
        );
        let pair = vec!["jpg".to_owned(), "CR3".to_owned()];
        assert_eq!(
            unique("2026_08_23__191546", &pair, &names),
            "2026_08_23__191546_2"
        );
        assert_eq!(unique("free", &pair, &names), "free");
    }

    fn item(path: &str, second: Option<u32>) -> Item {
        Item {
            path: PathBuf::from(path),
            rider: None,
            clock: second.map(|second| Clock { second, ..clock() }),
        }
    }

    fn folder(names: &[&str]) -> HashMap<PathBuf, Names> {
        HashMap::from([(
            PathBuf::from("d"),
            Names::of(names.iter().map(|name| (*name).to_owned())),
        )])
    }

    /// Two photos of the same second: the second gets `_1`. One without a time keeps its name,
    /// one that has the name already stays.
    #[test]
    fn a_plan_for_several_photos() {
        let items = [
            item("d/IMG_1.jpg", Some(46)),
            item("d/IMG_2.jpg", Some(46)),
            item("d/scan.jpg", None),
            item("d/2026_08_23__191550.jpg", Some(50)),
        ];
        let names = folder(&[
            "IMG_1.jpg",
            "IMG_2.jpg",
            "scan.jpg",
            "2026_08_23__191550.jpg",
        ]);
        let planned = plan(&items, Pattern::Standard, "", &names);
        let file = |i: usize| planned[i].to.as_deref().map(file_name);
        assert_eq!(file(0).as_deref(), Some("2026_08_23__191546.jpg"));
        assert_eq!(file(1).as_deref(), Some("2026_08_23__191546_1.jpg"));
        assert_eq!(file(2), None, "no capture time");
        assert_eq!(file(3), None, "already named so");
    }

    /// The counter follows the order given (capture order) and is three digits wide; a name
    /// another photo of the batch gives up is free.
    #[test]
    fn the_event_pattern_counts() {
        let items = [item("d/b.jpg", Some(1)), item("d/a.jpg", Some(2))];
        let names = folder(&["a.jpg", "b.jpg", "2026-08-23_Trip_001.jpg"]);
        let planned = plan(&items, Pattern::Event, "Trip", &names);
        let file = |i: usize| planned[i].to.as_deref().map(file_name);
        assert_eq!(file(0).as_deref(), Some("2026-08-23_Trip_001_1.jpg"));
        assert_eq!(file(1).as_deref(), Some("2026-08-23_Trip_002.jpg"));
    }

    #[test]
    fn a_typed_name_for_one_photo() {
        let one = Item {
            path: PathBuf::from("d/IMG_1.JPG"),
            rider: Some(PathBuf::from("d/IMG_1.CR3")),
            clock: None,
        };
        let names = Names::of(["IMG_1.JPG".into(), "IMG_1.CR3".into(), "taken.cr3".into()]);
        assert_eq!(
            rename_one(&one, "Strand", &names),
            Ok(Some(PathBuf::from("d").join("Strand.JPG")))
        );
        assert_eq!(rename_one(&one, "IMG_1", &names), Ok(None), "the same name");
        assert_eq!(
            rename_one(&one, "img_1", &names).map(|to| to.is_some()),
            Ok(true),
            "case"
        );
        assert_eq!(
            rename_one(&one, "taken", &names),
            Err(Invalid::Taken),
            "the RAW's"
        );
        assert_eq!(rename_one(&one, "a?", &names), Err(Invalid::Character('?')));
        assert_eq!(
            rider_to(Path::new("d/IMG_1.CR3"), Path::new("d/Strand.JPG")),
            PathBuf::from("d").join("Strand.CR3")
        );
    }

    /// A chain runs from its free end; a ring goes through a temporary name.
    #[test]
    fn the_order_of_the_renames() {
        let p = |name: &str| PathBuf::from("d").join(name);
        // a → b, b → c: b must go first.
        let chain = order(&[(p("a"), p("b")), (p("b"), p("c"))]);
        assert_eq!(chain.iter().map(|s| s.origin).collect::<Vec<_>>(), [1, 0]);
        // a ↔ b: a aside, b → a, the temporary → b.
        let ring = order(&[(p("a"), p("b")), (p("b"), p("a"))]);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring[0].from, p("a"));
        assert!(file_name(&ring[0].to).starts_with(".cerno-rename-1-"));
        assert_eq!((ring[1].from.clone(), ring[1].to.clone()), (p("b"), p("a")));
        assert_eq!(
            (ring[2].from.clone(), ring[2].to.clone()),
            (ring[0].to.clone(), p("b"))
        );
        assert_eq!(ring[2].origin, 0);
        // A change of case alone is no conflict with itself.
        let case = order(&[(p("IMG.jpg"), p("img.jpg"))]);
        assert_eq!(case.len(), 1);
    }
}
