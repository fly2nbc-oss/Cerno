//! `Ctrl+Z` for what the session did to photos: stars, rejections and colour labels, and the
//! edits that kept an original (straighten, crop, quarter turns). Newest last, at most
//! `LIMIT`, never saved; another folder starts it empty. Comment and keywords are text fields
//! and stay out; taking a mark back is not recorded again.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use eframe::egui;

use crate::i18n::{self, Texts};
use crate::library;
use crate::metadata::{Label, Rating};

use super::CernoApp;
use super::gate::Change;
use super::notice::Notice;

/// A long culling session's worth; the oldest go first.
const LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Entry {
    /// Stars or a rejection set over `before`. In a RAW + JPG pair `raw` is what the RAW's
    /// sidecar said before – `None` when it had none known: then the RAW gets `before` too.
    Rating {
        path: PathBuf,
        before: Rating,
        after: Rating,
        raw: Option<Rating>,
    },
    /// A colour label set or cleared over `before`; `raw` as for a rating.
    Label {
        path: PathBuf,
        before: Option<Label>,
        raw: Option<Option<Label>>,
    },
    /// Straighten, crop or a quarter turn: undone by putting the first original back.
    Edit { path: PathBuf },
}

impl Entry {
    pub(super) fn path(&self) -> &Path {
        match self {
            Self::Rating { path, .. } | Self::Label { path, .. } | Self::Edit { path } => path,
        }
    }

    /// What the menu's row names.
    fn what(&self, t: &Texts) -> &'static str {
        match self {
            Self::Rating { before, after, .. }
                if *before == Rating::Rejected || *after == Rating::Rejected =>
            {
                t.undo_what_reject
            }
            Self::Rating { .. } => t.undo_what_stars,
            Self::Label { .. } => t.undo_what_colour,
            Self::Edit { .. } => t.undo_what_edit,
        }
    }

    /// What a photo the entry is about takes no part in right now.
    fn change(&self) -> Change {
        match self {
            Self::Edit { .. } => Change::Rewrite,
            _ => Change::Mark,
        }
    }
}

#[derive(Default)]
pub(super) struct Journal {
    entries: VecDeque<Entry>,
}

impl Journal {
    pub(super) fn push(&mut self, entry: Entry) {
        if self.entries.len() == LIMIT {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// The newest entry whose photo is still where it was – one deleted or moved since is
    /// passed over.
    pub(super) fn newest(&self) -> Option<&Entry> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.path().exists())
    }

    /// Takes the newest entry `newest` gave, and the passed-over ones above it.
    fn take_newest(&mut self) -> Option<Entry> {
        while let Some(entry) = self.entries.pop_back() {
            if entry.path().exists() {
                return Some(entry);
            }
        }
        None
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    /// The first original of `path` is back: the edits recorded for it went with it.
    pub(super) fn drop_edits(&mut self, path: &Path) {
        self.entries
            .retain(|entry| !matches!(entry, Entry::Edit { path: edited } if edited == path));
    }
}

/// `3 stars`, `Rejected`, `Unrated`.
pub(super) fn rating_word(t: &Texts, rating: Rating) -> String {
    match rating {
        Rating::Stars(n) => (t.filter_stars)(n),
        Rating::Rejected => t.rejected.to_owned(),
        Rating::Unrated => t.filter_unrated.to_owned(),
    }
}

/// `Red`, `No colour`.
pub(super) fn label_word(t: &Texts, label: Option<Label>) -> &'static str {
    label.map_or(t.label_none, i18n::label_name)
}

impl CernoApp {
    /// `Ctrl+Z` with an entry in the journal: its photo is shown first, then it gets back what
    /// it had. A photo that takes no such change right now keeps the entry; the hint says why.
    pub(super) fn undo_newest(&mut self, ctx: &egui::Context) {
        let Some(entry) = self.marks.journal.newest().cloned() else {
            return;
        };
        let path = entry.path().to_path_buf();
        self.show_photo(ctx, &path);
        if !self.allowed(entry.change(), Some(&path)) {
            return;
        }
        self.marks.journal.take_newest();
        let t = i18n::t();
        let name = library::file_name_lossy(&path);
        match entry {
            Entry::Rating { before, raw, .. } => {
                self.apply_rating(ctx, path.clone(), before, false);
                if let Some(raw) = raw.filter(|raw| *raw != before) {
                    self.rate_companion(&path, raw);
                }
                self.notice = Some(Notice::hint((t.undo_mark_done)(
                    &name,
                    &rating_word(t, before),
                )));
            }
            Entry::Label { before, raw, .. } => {
                self.apply_label(ctx, path.clone(), before, false);
                if let Some(raw) = raw.filter(|raw| *raw != before) {
                    self.label_companion(&path, raw);
                }
                self.notice = Some(Notice::hint((t.undo_mark_done)(
                    &name,
                    label_word(t, before),
                )));
            }
            Entry::Edit { .. } => self.restore_original(path),
        }
    }

    /// The photo comes on screen when the view has it; in compare mode the pinned one is there
    /// already.
    fn show_photo(&mut self, ctx: &egui::Context, path: &Path) {
        if self.pinned.as_deref() == Some(path) {
            return;
        }
        if let Some(index) = self.view.iter().position(|shown| shown == path) {
            self.go_to(ctx, index, 1);
        }
    }

    /// The menu's *Undo* row while the journal has an entry: what it takes back on which
    /// photo, and why it can't right now.
    pub(super) fn undo_row(&self) -> Option<(String, Option<&'static str>)> {
        let entry = self.marks.journal.newest()?;
        let t = i18n::t();
        let label = (t.undo_mark_row)(entry.what(t), &library::file_name_lossy(entry.path()));
        Some((label, self.menu_block(entry.change(), Some(entry.path()))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rating(path: &Path, before: Rating) -> Entry {
        Entry::Rating {
            path: path.to_path_buf(),
            before,
            after: Rating::Stars(5),
            raw: None,
        }
    }

    #[test]
    fn the_journal_keeps_the_newest_two_hundred() {
        let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let mut journal = Journal::default();
        for n in 0..=LIMIT {
            journal.push(rating(&here, Rating::Stars((n % 5 + 1) as u8)));
        }
        assert_eq!(journal.entries.len(), LIMIT);
        assert_eq!(journal.newest(), Some(&rating(&here, Rating::Stars(1))));
        assert_eq!(
            journal.entries.front(),
            Some(&rating(&here, Rating::Stars(2)))
        );
    }

    /// Newest first; a photo that is gone (deleted, moved) is passed over and dropped with the
    /// entry taken.
    #[test]
    fn gone_photos_are_passed_over() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let (here, gone) = (root.join("Cargo.toml"), root.join("no such photo.jpg"));
        let mut journal = Journal::default();
        journal.push(rating(&here, Rating::Unrated));
        journal.push(Entry::Edit { path: here.clone() });
        journal.push(rating(&gone, Rating::Stars(2)));
        assert_eq!(journal.newest(), Some(&Entry::Edit { path: here.clone() }));
        assert_eq!(
            journal.take_newest(),
            Some(Entry::Edit { path: here.clone() })
        );
        assert_eq!(journal.take_newest(), Some(rating(&here, Rating::Unrated)));
        assert_eq!(journal.take_newest(), None);
    }

    #[test]
    fn a_restored_original_takes_its_edits_along() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let (a, b) = (root.join("Cargo.toml"), root.join("build.rs"));
        let mut journal = Journal::default();
        journal.push(Entry::Edit { path: a.clone() });
        journal.push(rating(&a, Rating::Unrated));
        journal.push(Entry::Edit { path: b.clone() });
        journal.push(Entry::Edit { path: a.clone() });
        journal.drop_edits(&a);
        assert_eq!(
            Vec::from(journal.entries.clone()),
            vec![rating(&a, Rating::Unrated), Entry::Edit { path: b }]
        );
    }

    #[test]
    fn the_row_names_what_goes_back() {
        let t = crate::i18n::Lang::En.texts();
        let path = PathBuf::from("IMG_1.jpg");
        assert_eq!(rating(&path, Rating::Unrated).what(t), "stars");
        assert_eq!(rating(&path, Rating::Rejected).what(t), "rejection");
        let reject = Entry::Rating {
            path: path.clone(),
            before: Rating::Stars(3),
            after: Rating::Rejected,
            raw: None,
        };
        assert_eq!(reject.what(t), "rejection");
        let label = Entry::Label {
            path: path.clone(),
            before: None,
            raw: None,
        };
        assert_eq!(label.what(t), "colour");
        assert_eq!(
            (t.undo_mark_row)(label.what(t), "IMG_1.jpg"),
            "Undo colour – IMG_1.jpg"
        );
        assert_eq!(rating_word(t, Rating::Stars(3)), "3 stars");
        assert_eq!(label_word(t, None), "No colour");
    }
}
