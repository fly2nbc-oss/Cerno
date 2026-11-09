//! Keywords across the folder (CER-68): the keyword filter (*Visible photos ▸ By keyword …* –
//! its card, the pick in effect (`ViewOptions::keywords`, `Facts::tagged`), the filter bar's
//! chip) and the vocabulary the description tab suggests from. A photo's keywords are this
//! session's change (`Marks::descriptions`), else what the analysis read into the board. The
//! pick is never saved; it goes when another folder opens.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Rect};

use crate::i18n;
use crate::keywords::{Pick, Vocabulary};
use crate::ui::keywords as card_ui;

use super::CernoApp;
use super::layer::Layer;

/// The vocabulary is made again at most this often while the analysis brings new keywords
/// (a change of this session's own is taken at once).
const VOCABULARY_REFRESH: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(super) struct Keywords {
    /// The pick in effect while `ViewOptions::keywords` is on.
    applied: Option<Pick>,
    /// Every keyword of the index with its number of files, read once per folder when the
    /// vocabulary is first needed (the suggestions after the folder's own).
    index: Option<Arc<Vec<(String, usize)>>>,
    /// The vocabulary, with the board's version and `edits` it was made from, and when.
    vocabulary: Option<Made>,
    /// Bumped by every comment or keyword change of this session.
    edits: u64,
}

struct Made {
    board: u64,
    edits: u64,
    at: Instant,
    vocabulary: Arc<Vocabulary>,
}

impl Keywords {
    /// Another folder: the pick and the vocabulary were about the last one.
    pub(super) fn forget(&mut self) {
        self.applied = None;
        self.index = None;
        self.vocabulary = None;
    }

    /// The filter went off ("Show all", the chip): the pick goes with it.
    pub(super) fn drop_applied(&mut self) {
        self.applied = None;
    }

    /// `Shift+T`: the pick goes aside with the filter, and comes back with it.
    pub(super) fn take_applied(&mut self) -> Option<Pick> {
        self.applied.take()
    }

    pub(super) fn put_applied(&mut self, applied: Option<Pick>) {
        self.applied = applied;
    }

    /// A comment or keyword changed in this session.
    pub(super) fn edited(&mut self) {
        self.edits += 1;
    }

    /// The filter bar's chip while the filter is on: its label and tooltip.
    pub(super) fn chip(&self) -> Option<(String, String)> {
        let pick = self.applied.as_ref()?;
        let t = i18n::t();
        let mut names: Vec<&str> = pick.keywords.iter().map(String::as_str).collect();
        if pick.none {
            names.insert(0, t.keywords_none);
        }
        let first = names.first()?;
        let label = match names.len() {
            1 => (*first).to_owned(),
            n => format!("{first} +{}", n - 1),
        };
        Some((
            (t.keywords_chip)(&label),
            (t.keywords_chip_tooltip)(&names.join(", ")),
        ))
    }
}

impl CernoApp {
    /// The keywords of `path` as Cerno knows them now – this session's change, else what the
    /// analysis read – handed to `f`; `None` while not read yet.
    pub(super) fn with_keywords<R>(
        &self,
        path: &Path,
        f: impl FnOnce(&[String]) -> R,
    ) -> Option<R> {
        if let Some(description) = self.marks.descriptions.get(path) {
            return Some(f(&description.keywords));
        }
        self.board.keywords(path).map(|keywords| f(&keywords))
    }

    /// Whether the keyword filter in effect lets `path` through.
    pub(super) fn is_tagged(&self, path: &Path) -> bool {
        self.options.keywords
            && self.keywords.applied.as_ref().is_some_and(|pick| {
                self.with_keywords(path, |keywords| pick.accepts(keywords))
                    .unwrap_or(false)
            })
    }

    /// The folder's keywords, then the rest of the index's: what the description tab suggests.
    pub(super) fn vocabulary(&mut self) -> Arc<Vocabulary> {
        let board = self.board.version();
        let edits = self.keywords.edits;
        if let Some(made) = &self.keywords.vocabulary
            && made.edits == edits
            && (made.board == board || made.at.elapsed() < VOCABULARY_REFRESH)
        {
            return Arc::clone(&made.vocabulary);
        }
        let index = match &self.keywords.index {
            Some(index) => Arc::clone(index),
            None => {
                let counts = self.db.keyword_counts().unwrap_or_else(|err| {
                    log::warn!("keywords of the index: {err:#}");
                    Vec::new()
                });
                let counts = Arc::new(counts);
                self.keywords.index = Some(Arc::clone(&counts));
                counts
            }
        };
        let lists: Vec<Vec<String>> = self
            .folder
            .all
            .iter()
            .filter_map(|path| self.with_keywords(path, <[String]>::to_vec))
            .collect();
        let vocabulary = Arc::new(Vocabulary::build(lists.iter().map(Vec::as_slice), &index));
        self.keywords.vocabulary = Some(Made {
            board,
            edits,
            at: Instant::now(),
            vocabulary: Arc::clone(&vocabulary),
        });
        vocabulary
    }

    /// *Visible photos ▸ By keyword …*: the card, with the pick in effect ticked.
    pub(super) fn open_keywords(&mut self) {
        let vocabulary = self.vocabulary();
        let (mut untagged, mut unread) = (0, 0);
        for path in self.folder.all.iter() {
            match self.with_keywords(path, <[String]>::is_empty) {
                Some(true) => untagged += 1,
                Some(false) => {}
                None => unread += 1,
            }
        }
        self.layer = Layer::Keywords(card_ui::Card {
            entries: vocabulary.in_folder().cloned().collect(),
            untagged,
            unread,
            pick: self
                .keywords
                .applied
                .clone()
                .filter(|_| self.options.keywords)
                .unwrap_or_default(),
            was_on: self.options.keywords,
            ..card_ui::Card::default()
        });
    }

    /// The card, while open: Apply shows the photos with one of the ticked keywords; nothing
    /// ticked ends the filter.
    pub(super) fn draw_keywords_card(&mut self, ctx: &egui::Context, window: Rect) {
        let Layer::Keywords(card) = &mut self.layer else {
            return;
        };
        let out = card_ui::show(ctx, window, card);
        if out.apply {
            let Layer::Keywords(card) = std::mem::take(&mut self.layer) else {
                return;
            };
            if card.pick.is_empty() {
                self.change_options(ctx, |o| o.keywords = false);
            } else {
                self.keywords.applied = Some(card.pick);
                self.change_options(ctx, |o| o.keywords = true);
            }
        } else if out.close {
            self.layer = Layer::None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::metadata::Description;

    use super::super::harness::Harness;
    use super::*;

    fn words(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    /// The filter shows the photos with a picked keyword – what the analysis read, or this
    /// session's change over it – and the chip names it; another folder ends it.
    #[test]
    fn the_keyword_filter_shows_the_photos_with_a_picked_word() {
        let mut h = Harness::new(4);
        let ctx = h.ctx.clone();
        let path = |i: usize| h.app.folder.all[i].clone();
        let (a, b, c, d): (PathBuf, PathBuf, PathBuf, PathBuf) =
            (path(0), path(1), path(2), path(3));
        h.app.board.set_keywords(&a, words(&["Familie", "Strand"]));
        h.app.board.set_keywords(&b, words(&["familie"]));
        h.app.board.set_keywords(&c, Vec::new());
        // `d` not read yet; this session gives `c` a keyword.
        h.app.marks.descriptions.insert(
            c.clone(),
            Description {
                comment: String::new(),
                keywords: words(&["Strand"]),
            },
        );
        h.app.open_keywords();
        let Layer::Keywords(card) = &mut h.app.layer else {
            panic!("the card is open");
        };
        let counts: Vec<(&str, usize)> = card
            .entries
            .iter()
            .map(|e| (e.text.as_str(), e.folder))
            .collect();
        assert_eq!(counts, [("Familie", 2), ("Strand", 2)]);
        assert_eq!((card.untagged, card.unread), (0, 1));
        // The card ticks the folder's spelling; "familie" on `b` is the same word.
        card.toggle(card_ui::Row::Word(0));
        h.app.keywords.applied = match std::mem::take(&mut h.app.layer) {
            Layer::Keywords(card) => Some(card.pick),
            _ => None,
        };
        h.app.change_options(&ctx, |o| o.keywords = true);
        assert_eq!(&h.app.view.paths[..], &[a.clone(), b.clone()]);
        let (label, tooltip) = h.app.keywords.chip().expect("a chip");
        assert!(label.contains("Familie"), "{label}");
        assert!(tooltip.contains("Familie"), "{tooltip}");

        // Without keywords: `c` has one now (this session), `d` is not read – neither shows.
        h.app.keywords.applied = Some(Pick {
            keywords: Vec::new(),
            none: true,
        });
        h.app.change_options(&ctx, |o| o.keywords = true);
        assert!(h.app.view.paths.is_empty(), "{:?}", h.app.view.paths);
        h.app.board.set_keywords(&d, Vec::new());
        h.app.change_options(&ctx, |o| o.keywords = true);
        assert_eq!(&h.app.view.paths[..], std::slice::from_ref(&d));

        // "Show all" ends it, and the pick with it.
        h.app.change_options(&ctx, |o| o.clear_filters());
        assert!(h.app.keywords.applied.is_none());
        assert_eq!(h.app.view.len(), 4);
    }

    /// A change of this session counts for the suggestions at once; the vocabulary keeps the
    /// folder's spelling.
    #[test]
    fn the_vocabulary_follows_this_sessions_changes() {
        let mut h = Harness::new(2);
        let a = h.app.folder.all[0].clone();
        h.app.board.set_keywords(&a, words(&["Bodensee"]));
        assert_eq!(h.app.vocabulary().spelling("bodensee"), "Bodensee");
        h.app.marks.descriptions.insert(
            a,
            Description {
                comment: String::new(),
                keywords: words(&["Konstanz"]),
            },
        );
        h.app.keywords.edited();
        let vocabulary = h.app.vocabulary();
        assert_eq!(vocabulary.spelling("KONSTANZ"), "Konstanz");
        assert_eq!(
            vocabulary.suggest("bo", &[], 5).len(),
            0,
            "no longer in the folder"
        );
    }
}
