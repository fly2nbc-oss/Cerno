//! Keywords across the folder (CER-68): the keyword filter (*Visible photos ▸ By keyword …* –
//! its card, the pick in effect (`ViewOptions::keywords`, `Facts::tagged`), the filter bar's
//! chip) and the vocabulary the description tab suggests from. A photo's keywords are this
//! session's change (`Marks::descriptions`), else what the analysis read into the board. The
//! pick is never saved; it goes when another folder opens.

use std::path::Path;
use std::sync::{Arc, mpsc};
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
    /// vocabulary is first needed (the suggestions after the folder's own) – on a thread: an
    /// index of 11 000 well tagged files takes ~11 ms (release, measured 2026-11-09), too
    /// long for a frame.
    index: IndexWords,
    /// The vocabulary, with the board's version and `edits` it was made from, and when.
    vocabulary: Option<Made>,
    /// Bumped by every comment or keyword change of this session.
    edits: u64,
}

/// The index's keywords for the suggestions: not asked yet, being read, or read.
#[derive(Default)]
enum IndexWords {
    #[default]
    NotAsked,
    Reading(mpsc::Receiver<Vec<(String, usize)>>),
    Read(Arc<Vec<(String, usize)>>),
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
        self.index = IndexWords::NotAsked;
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
        let index = self.index_words();
        let board = self.board.version();
        let edits = self.keywords.edits;
        if let Some(made) = &self.keywords.vocabulary
            && made.edits == edits
            && (made.board == board || made.at.elapsed() < VOCABULARY_REFRESH)
        {
            return Arc::clone(&made.vocabulary);
        }
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

    /// The index's keywords as far as they are read: asked for on a thread the first time,
    /// none until they arrive – then the vocabulary is made again with them. No repaint is
    /// asked for: they only matter for the suggestions, which show while a keyword is typed.
    fn index_words(&mut self) -> Arc<Vec<(String, usize)>> {
        match &self.keywords.index {
            IndexWords::Read(index) => return Arc::clone(index),
            IndexWords::Reading(rx) => match rx.try_recv() {
                Ok(counts) => {
                    let counts = Arc::new(counts);
                    self.keywords.index = IndexWords::Read(Arc::clone(&counts));
                    self.keywords.vocabulary = None;
                    return counts;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.keywords.index = IndexWords::Read(Arc::default());
                }
            },
            IndexWords::NotAsked => {
                let db = Arc::clone(&self.db);
                let (tx, rx) = mpsc::channel();
                let spawned = std::thread::Builder::new()
                    .name("cerno-keywords".into())
                    .spawn(move || {
                        let counts = db.keyword_counts().unwrap_or_else(|err| {
                            log::warn!("keywords of the index: {err:#}");
                            Vec::new()
                        });
                        let _ = tx.send(counts);
                    });
                self.keywords.index = match spawned {
                    Ok(_) => IndexWords::Reading(rx),
                    Err(err) => {
                        log::warn!("keywords of the index: {err}");
                        IndexWords::Read(Arc::default())
                    }
                };
            }
        }
        Arc::default()
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
                // The pick lives outside `ViewOptions`: with the filter already on, `top_key`
                // would not see it change, and Top N would keep the photos of the last pick.
                self.browse.top_pick_for = None;
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

    /// Through the card: `Ctrl+Enter` applies what is ticked; applying with nothing ticked ends
    /// the filter in effect.
    #[test]
    fn the_card_applies_and_an_empty_pick_ends_the_filter() {
        use eframe::egui::{Key, Modifiers};

        let mut h = Harness::new(3);
        let a = h.app.folder.all[0].clone();
        h.app.board.set_keywords(&a, words(&["Familie"]));
        for path in &h.app.folder.all[1..] {
            h.app.board.set_keywords(path, Vec::new());
        }
        let ctrl = Modifiers::COMMAND | Modifiers::CTRL;
        h.app.open_keywords();
        if let Layer::Keywords(card) = &mut h.app.layer {
            card.toggle(card_ui::Row::Word(0));
        }
        h.press(Key::Enter, ctrl);
        assert_eq!(h.layer(), "none");
        assert!(h.app.options.keywords);
        assert_eq!(&h.app.view.paths[..], std::slice::from_ref(&a));

        h.app.open_keywords();
        if let Layer::Keywords(card) = &mut h.app.layer {
            assert!(
                card.pick.contains("familie"),
                "the pick in effect is ticked"
            );
            card.toggle(card_ui::Row::Word(0));
        }
        h.press(Key::Enter, ctrl);
        assert!(!h.app.options.keywords, "nothing ticked: the filter ends");
        assert!(h.app.keywords.applied.is_none());
        assert_eq!(h.app.view.len(), 3);
    }

    /// Top N with the keyword filter on: another pick from the card picks the best photos
    /// again among those with the new keyword – the pick lives outside `ViewOptions`, so
    /// `top_key` alone would keep the photos of the last one, and none of them has the new
    /// keyword.
    #[test]
    fn another_keyword_pick_picks_top_n_again() {
        use eframe::egui::{Key, Modifiers};

        use crate::metadata::Rating;

        const HOUR: i64 = 3_600_000;

        let mut h = Harness::new(4);
        let all = Arc::clone(&h.app.folder.all);
        for (i, path) in all.iter().enumerate() {
            let word = if i < 2 { "Familie" } else { "Strand" };
            h.app.board.set_keywords(path, words(&[word]));
            // On the board (an hour apart: no series) with stars, a value for Top N; the
            // harness has no scores.
            h.app.board.put_taken(path, HOUR * i as i64);
            h.app.marks.ratings.insert(path.clone(), Rating::Stars(4));
        }
        let ctx = h.ctx.clone();
        h.app.change_options(&ctx, |o| o.top = Some(10));
        let ctrl = Modifiers::COMMAND | Modifiers::CTRL;
        let pick = |h: &mut Harness, word: &str| {
            h.app.open_keywords();
            let Layer::Keywords(card) = &mut h.app.layer else {
                panic!("the card is open");
            };
            for (i, entry) in card.entries.clone().iter().enumerate() {
                if card.pick.contains(&entry.text) != (entry.text == word) {
                    card.toggle(card_ui::Row::Word(i));
                }
            }
            h.press(Key::Enter, ctrl);
            assert_eq!(h.layer(), "none");
        };
        pick(&mut h, "Familie");
        assert_eq!(&h.app.view.paths[..], &all[..2]);
        pick(&mut h, "Strand");
        assert!(h.app.options.keywords);
        assert_eq!(h.app.options.top, Some(10));
        assert_eq!(
            &h.app.view.paths[..],
            &all[2..],
            "picked again among Strand"
        );
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

    /// The index's keywords come from a thread: none at first, then – without the folder's
    /// own changing – the vocabulary is made again with them.
    #[test]
    fn the_index_keywords_arrive_from_a_thread() {
        use crate::db::FileStamp;

        let mut h = Harness::new(1);
        let a = h.app.folder.all[0].clone();
        h.app.board.set_keywords(&a, words(&["Bodensee"]));
        let elsewhere = "/elsewhere/IMG_9.jpg";
        let stamp = FileStamp {
            size: 1,
            mtime_ns: 1,
        };
        h.app
            .db
            .put_file(elsewhere, stamp, 9, crate::metadata::Rating::Unrated, None)
            .unwrap();
        h.app
            .db
            .put_keywords(elsewhere, &words(&["Konstanz"]))
            .unwrap();

        let first = h.app.vocabulary();
        assert_eq!(first.suggest("bo", &[], 5).len(), 1, "the folder's at once");
        let started = Instant::now();
        loop {
            if !h.app.vocabulary().suggest("ko", &[], 5).is_empty() {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "the index's keywords arrive"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(matches!(h.app.keywords.index, IndexWords::Read(_)));
    }

    /// In the filter mode (`Ctrl+F`) the keyword chip is a box while it shows, after the file
    /// list's place: `Space` opens the card, whose `Esc` closes only the card – the mode goes
    /// on. Without a keyword filter there is no chip and no box.
    #[test]
    fn the_keyword_chip_is_a_box_of_the_filter_mode() {
        use eframe::egui::{Key, Modifiers};

        use crate::ui::filter_bar::Slot;

        let mut h = Harness::new(3);
        let a = h.app.folder.all[0].clone();
        h.app.board.set_keywords(&a, words(&["Familie"]));
        for path in &h.app.folder.all[1..] {
            h.app.board.set_keywords(path, Vec::new());
        }
        h.settle();
        h.press(Key::F, Modifiers::COMMAND);
        assert!(!h.app.filter_slots().contains(&Slot::Keywords));
        h.press(Key::F, Modifiers::COMMAND);

        h.app.open_keywords();
        if let Layer::Keywords(card) = &mut h.app.layer {
            card.toggle(card_ui::Row::Word(0));
        }
        h.press(Key::Enter, Modifiers::COMMAND | Modifiers::CTRL);
        assert!(h.app.options.keywords);
        h.press(Key::F, Modifiers::COMMAND);
        let slots = h.app.filter_slots();
        assert_eq!(
            slots[slots.len() - 3..],
            [Slot::Similar, Slot::Keywords, Slot::Sort]
        );
        h.app.filter_mode.cursor = Some(Slot::Keywords);
        let caption = h.app.filter_mode_caption().expect("a caption");
        assert!(
            caption.contains("Familie") && !caption.contains('×'),
            "{caption}"
        );
        h.press(Key::Space, Modifiers::NONE);
        h.settle();
        assert_eq!(h.layer(), "keywords");
        assert!(
            h.app.options.keywords,
            "the box opens the card, nothing ends"
        );
        h.press(Key::Escape, Modifiers::NONE);
        h.settle();
        assert_eq!(h.layer(), "none");
        assert!(h.app.filter_mode.is_on(), "Esc closed only the card");
        assert!(h.app.options.keywords);
        assert_eq!(h.app.filter_mode.cursor, Some(Slot::Keywords));
    }
}
