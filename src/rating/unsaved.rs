//! Marks that could not be written – a file locked by another program, read-only, a network
//! drive or card briefly gone. They are kept with their error and tried again (ticket CER-57):
//! after 5 s, 30 s and 2 min, then whenever the app asks (another folder, the card's *Try
//! again*, before a copy, move or delete, before exit). Nothing here touches a file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::metadata::{Description, Label, Rating};

/// When a failed write is tried again: after the first failure, the second, the third. After
/// that only when the app asks.
pub(super) const BACKOFF: [Duration; 3] = [
    Duration::from_secs(5),
    Duration::from_secs(30),
    Duration::from_secs(120),
];

/// What is to be written into one file; `None` leaves that part alone.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Marks {
    pub(super) rating: Option<Rating>,
    /// `Some(None)` clears the label.
    pub(super) label: Option<Option<Label>>,
    pub(super) description: Option<Description>,
}

impl Marks {
    /// These marks over `older` ones: what is set here wins, the rest comes from `older`.
    pub(super) fn over(self, older: Self) -> Self {
        Self {
            rating: self.rating.or(older.rating),
            label: self.label.or(older.label),
            description: self.description.or(older.description),
        }
    }
}

/// One file whose marks are not written.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Failed {
    pub(super) marks: Marks,
    /// Why the last attempt failed.
    pub(super) error: String,
    /// Failed attempts since the last time the app asked (it starts the backoff over).
    pub(super) failures: usize,
    /// When it is tried again on its own; `None` once the backoff has run out.
    pub(super) next_try: Option<Instant>,
}

/// A file whose marks are not written, for the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsavedMark {
    pub path: PathBuf,
    pub error: String,
}

/// Every file with marks not written, by path.
#[derive(Debug, Default)]
pub(super) struct Unsaved {
    entries: HashMap<PathBuf, Failed>,
}

impl Unsaved {
    /// A write of `marks` into `path` failed with `error`, after `failures` earlier failed
    /// attempts. Marks still kept for the file are older: these win over them.
    pub(super) fn fail(
        &mut self,
        path: PathBuf,
        marks: Marks,
        error: String,
        failures: usize,
        now: Instant,
    ) {
        let marks = match self.entries.remove(&path) {
            Some(older) => marks.over(older.marks),
            None => marks,
        };
        self.entries.insert(
            path,
            Failed {
                marks,
                error,
                failures: failures + 1,
                next_try: BACKOFF.get(failures).map(|wait| now + *wait),
            },
        );
    }

    /// The kept marks of `path`, taken out to be written (or merged into a newer mark).
    pub(super) fn take(&mut self, path: &Path) -> Option<Failed> {
        self.entries.remove(path)
    }

    /// The files whose next attempt is due at `now`.
    pub(super) fn due(&self, now: Instant) -> Vec<PathBuf> {
        let mut due: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|(_, failed)| failed.next_try.is_some_and(|at| at <= now))
            .map(|(path, _)| path.clone())
            .collect();
        due.sort();
        due
    }

    /// Every file, or those of `paths` that have marks kept.
    pub(super) fn matching(&self, paths: Option<&[PathBuf]>) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = match paths {
            None => self.entries.keys().cloned().collect(),
            Some(paths) => paths
                .iter()
                .filter(|path| self.entries.contains_key(*path))
                .cloned()
                .collect(),
        };
        found.sort();
        found.dedup();
        found
    }

    /// How long until the next attempt is due on its own; `None` when none is.
    pub(super) fn next_wake(&self, now: Instant) -> Option<Duration> {
        self.entries
            .values()
            .filter_map(|failed| failed.next_try)
            .min()
            .map(|at| at.saturating_duration_since(now))
    }

    /// Cerno moved the file: its marks go with it (into marks kept for `to`, if any – those
    /// are older).
    pub(super) fn moved(&mut self, from: &Path, to: PathBuf) {
        let Some(failed) = self.entries.remove(from) else {
            return;
        };
        let failed = match self.entries.remove(&to) {
            Some(older) => Failed {
                marks: failed.marks.over(older.marks),
                ..failed
            },
            None => failed,
        };
        self.entries.insert(to, failed);
    }

    /// Cerno set the file aside (deleted): its kept marks are dropped. `true` when there were.
    pub(super) fn forget(&mut self, path: &Path) -> bool {
        self.entries.remove(path).is_some()
    }

    /// Every file with its last error, by name.
    pub(super) fn report(&self) -> Vec<UnsavedMark> {
        let mut report: Vec<UnsavedMark> = self
            .entries
            .iter()
            .map(|(path, failed)| UnsavedMark {
                path: path.clone(),
                error: failed.error.clone(),
            })
            .collect();
        report.sort_by(|a, b| a.path.cmp(&b.path));
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stars(n: u8) -> Marks {
        Marks {
            rating: Some(Rating::Stars(n)),
            ..Marks::default()
        }
    }

    #[test]
    fn newer_marks_win_and_the_rest_stays() {
        let older = Marks {
            rating: Some(Rating::Stars(2)),
            label: Some(Some(Label::Red)),
            description: None,
        };
        let newer = Marks {
            rating: Some(Rating::Rejected),
            label: Some(None),
            description: Some(Description::default()),
        };
        assert_eq!(newer.clone().over(older.clone()), newer);
        assert_eq!(stars(4).over(older).label, Some(Some(Label::Red)));
    }

    /// 5 s, 30 s, 2 min – then the file waits for the app to ask.
    #[test]
    fn retries_back_off_and_then_wait() {
        let now = Instant::now();
        let path = PathBuf::from("IMG_1.jpg");
        let mut unsaved = Unsaved::default();
        unsaved.fail(path.clone(), stars(3), "locked".into(), 0, now);
        assert!(unsaved.due(now).is_empty());
        assert_eq!(unsaved.next_wake(now), Some(Duration::from_secs(5)));
        let mut at = now;
        for (failures, wait) in BACKOFF.iter().enumerate() {
            at += *wait;
            assert_eq!(unsaved.due(at), std::slice::from_ref(&path));
            let failed = unsaved.take(&path).unwrap();
            assert_eq!(failed.failures, failures + 1);
            assert_eq!(failed.marks, stars(3), "the same marks again");
            unsaved.fail(
                path.clone(),
                failed.marks,
                "still locked".into(),
                failed.failures,
                at,
            );
        }
        assert_eq!(unsaved.next_wake(at), None, "the backoff has run out");
        assert!(unsaved.due(at + Duration::from_secs(3600)).is_empty());
        assert_eq!(
            unsaved.report(),
            [UnsavedMark {
                path: path.clone(),
                error: "still locked".into()
            }]
        );
        // Asked by the app: tried now, and a failure starts the backoff over.
        assert_eq!(unsaved.matching(None), std::slice::from_ref(&path));
        let failed = unsaved.take(&path).unwrap();
        unsaved.fail(path.clone(), failed.marks, "locked".into(), 0, at);
        assert_eq!(unsaved.next_wake(at), Some(BACKOFF[0]));
    }

    /// A second failure for a file merges: the newer marks win.
    #[test]
    fn a_newer_failure_merges_into_the_kept_marks() {
        let now = Instant::now();
        let path = PathBuf::from("IMG_1.jpg");
        let mut unsaved = Unsaved::default();
        let red = Marks {
            label: Some(Some(Label::Red)),
            ..Marks::default()
        };
        unsaved.fail(path.clone(), red, "locked".into(), 0, now);
        unsaved.fail(path.clone(), stars(5), "read-only".into(), 0, now);
        let failed = unsaved.take(&path).unwrap();
        assert_eq!(failed.marks.rating, Some(Rating::Stars(5)));
        assert_eq!(failed.marks.label, Some(Some(Label::Red)));
        assert_eq!(failed.error, "read-only");
        assert!(unsaved.report().is_empty());
    }

    #[test]
    fn marks_follow_a_move_and_go_with_a_deletion() {
        let now = Instant::now();
        let (a, b) = (PathBuf::from("a/IMG_1.jpg"), PathBuf::from("b/IMG_1.jpg"));
        let mut unsaved = Unsaved::default();
        unsaved.fail(a.clone(), stars(4), "locked".into(), 0, now);
        unsaved.moved(&PathBuf::from("a/other.jpg"), b.clone());
        assert_eq!(
            unsaved.matching(None),
            std::slice::from_ref(&a),
            "nothing kept for it"
        );
        unsaved.moved(&a, b.clone());
        assert_eq!(unsaved.matching(None), std::slice::from_ref(&b));
        assert_eq!(
            unsaved.matching(Some(&[a.clone(), b.clone()])),
            std::slice::from_ref(&b)
        );
        assert!(!unsaved.forget(&a));
        assert!(unsaved.forget(&b));
        assert!(unsaved.report().is_empty());
    }
}
