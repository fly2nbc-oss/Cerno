//! Renaming from the app's side (`F2` the current photo, `Shift+F2` every photo shown): the
//! card, the job for the transfer worker and what follows the new names. The names come from
//! `rename` (pure); a RAW riding along takes the same new name, sidecars go along.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{self, Rect};

use crate::i18n;
use crate::library;
use crate::metadata;
use crate::rename::{self, Invalid, Item, Names, Pattern, Planned};
use crate::transfer::Outcome as TransferOutcome;
use crate::ui::rename as card_ui;

use super::CernoApp;
use super::gate::Change;
use super::layer::Layer;
use super::notice::Notice;
use super::undo::Entry;

/// The open rename card.
pub(super) struct Card {
    /// Every photo shown (`Shift+F2`) rather than the current one.
    all: bool,
    /// One photo, or every photo shown in capture order (the Event pattern counts so).
    items: Vec<Item>,
    /// The file names of each folder involved, read when the card opened.
    names: HashMap<PathBuf, Names>,
    state: card_ui::State,
    /// The plan for the pattern and text it was made for (several photos only).
    planned: Option<(usize, String, Vec<Planned>)>,
}

/// The Event pattern's text to start with: the folder's name.
fn folder_text(dir: Option<&Path>) -> String {
    dir.and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn invalid_text(invalid: Invalid) -> String {
    let t = i18n::t();
    match invalid {
        Invalid::Empty => t.rename_empty.to_owned(),
        Invalid::Character(c) => (t.rename_bad_char)(c),
        Invalid::Reserved => t.rename_reserved.to_owned(),
        Invalid::Ending => t.rename_ending.to_owned(),
        Invalid::TooLong => t.rename_too_long.to_owned(),
        Invalid::Taken => t.rename_taken.to_owned(),
    }
}

impl CernoApp {
    /// `F2` / `Shift+F2`: the card for the current photo or for every photo shown.
    pub(super) fn open_rename(&mut self, all: bool) {
        let t = i18n::t();
        if self.view.is_empty() {
            self.notice = Some(Notice::hint(t.no_match));
            return;
        }
        if self.transfers.is_busy() {
            self.notice = Some(Notice::hint(t.transfer_busy));
            return;
        }
        let current = self.view.get(self.current).cloned();
        let paths: Vec<PathBuf> = if all {
            self.view
                .iter()
                .filter(|path| !self.is_deleted(path))
                .cloned()
                .collect()
        } else {
            current.clone().into_iter().collect()
        };
        let path = if all { None } else { current.as_deref() };
        if !self.allowed(Change::Rename, path) {
            return;
        }
        if paths.is_empty() {
            self.notice = Some(Notice::hint(t.no_match));
            return;
        }
        let mut items: Vec<(Option<i64>, Item)> = paths
            .into_iter()
            .map(|path| {
                let taken = self.facts(&path).and_then(|facts| facts.taken_ms);
                let item = Item {
                    rider: self.folder.pairs.companion(&path).map(Path::to_path_buf),
                    clock: taken.map(metadata::clock_of),
                    path,
                };
                (taken, item)
            })
            .collect();
        // Capture order for the counter; photos without a time keep the view's order, last.
        items.sort_by_key(|(taken, _)| (taken.is_none(), *taken));
        let items: Vec<Item> = items.into_iter().map(|(_, item)| item).collect();
        let mut names = HashMap::new();
        for item in &items {
            if let Some(dir) = item.path.parent() {
                names
                    .entry(dir.to_path_buf())
                    .or_insert_with(|| Names::in_dir(dir));
            }
        }
        let text = folder_text(self.folder.dir.as_deref().or(items[0].path.parent()));
        // Without a capture time no pattern applies: the field starts with the name it has.
        let edited = !all && items[0].clock.is_none();
        let state = card_ui::State {
            pattern: 0,
            name: if edited {
                rename::stem_of(&items[0].path)
            } else {
                String::new()
            },
            edited,
            text,
        };
        self.layer = Layer::Rename(Card {
            all,
            items,
            names,
            state,
            planned: None,
        });
    }

    /// The card, while open; Rename hands the new names to the worker.
    pub(super) fn draw_rename_card(&mut self, ctx: &egui::Context, window: Rect) {
        let Layer::Rename(card) = &mut self.layer else {
            return;
        };
        let t = i18n::t();
        let text = card.state.text.clone();
        let examples: Vec<Option<String>> = Pattern::ALL
            .iter()
            .map(|pattern| {
                let first = card.items.first()?;
                first.clock?;
                let planned =
                    rename::plan(std::slice::from_ref(first), *pattern, &text, &card.names);
                Some(match &planned[0].to {
                    Some(to) => rename::stem_of(to),
                    None => rename::stem_of(&first.path),
                })
            })
            .collect();
        let patterns: Vec<card_ui::PatternRow> = t
            .rename_patterns
            .iter()
            .zip(&examples)
            .map(|(label, example)| card_ui::PatternRow {
                label,
                example: example.clone(),
            })
            .collect();
        let pattern = Pattern::ALL[card.state.pattern.min(Pattern::ALL.len() - 1)];
        // Several photos: planned once per pattern and text, listed as *old → new*.
        if card.all
            && card
                .planned
                .as_ref()
                .is_none_or(|(p, t, _)| *p != card.state.pattern || *t != text)
        {
            let planned = rename::plan(&card.items, pattern, &text, &card.names);
            card.planned = Some((card.state.pattern, text.clone(), planned));
        }
        let planned = card
            .planned
            .as_ref()
            .filter(|_| card.all)
            .map_or(&[][..], |(_, _, p)| p.as_slice());
        let rows: Vec<(String, String, bool)> = planned
            .iter()
            .zip(&card.items)
            .map(|(planned, item)| {
                let old = library::file_name_lossy(&planned.path);
                match &planned.to {
                    Some(to) => (old, library::file_name_lossy(to), true),
                    None if item.clock.is_none() => (old, t.rename_keeps.to_owned(), false),
                    None => (old, t.rename_unchanged.to_owned(), false),
                }
            })
            .collect();
        let (title, body, blocked, renames) = if card.all {
            let renames: Vec<(PathBuf, PathBuf)> = planned
                .iter()
                .filter_map(|p| Some((p.path.clone(), p.to.clone()?)))
                .collect();
            let blocked = renames.is_empty().then(|| t.rename_nothing.to_owned());
            (
                (t.rename_all_title)(card.items.len()),
                card_ui::Body::All {
                    count: (t.rename_count)(renames.len(), card.items.len()),
                    rows: &rows,
                    text: pattern.takes_text(),
                },
                blocked,
                renames,
            )
        } else {
            if !card.state.edited
                && let Some(Some(example)) = examples.get(card.state.pattern)
            {
                card.state.name = example.clone();
            }
            let item = &card.items[0];
            let folder = item
                .path
                .parent()
                .and_then(|dir| card.names.get(dir))
                .cloned()
                .unwrap_or_default();
            let checked = rename::rename_one(item, card.state.name.trim_end(), &folder);
            let (error, renames) = match checked {
                Ok(to) => (
                    None,
                    to.map(|to| (item.path.clone(), to)).into_iter().collect(),
                ),
                Err(invalid) => (Some(invalid_text(invalid)), Vec::new()),
            };
            let ext = item.path.extension().and_then(|e| e.to_str()).unwrap_or("");
            (
                t.rename_title.to_owned(),
                card_ui::Body::One {
                    ext,
                    error: error.clone(),
                },
                error,
                renames,
            )
        };
        let view = card_ui::View {
            title,
            patterns,
            body,
            blocked,
        };
        let out = card_ui::show(ctx, window, &mut card.state, &view);
        if out.apply {
            self.layer = Layer::None;
            if !renames.is_empty() {
                self.start_rename(ctx, renames, true);
            }
        } else if out.close {
            self.layer = Layer::None;
        }
    }

    /// Hands `renames` (photo → new path) to the worker; a RAW riding along takes the same new
    /// name. `record`: `Ctrl+Z` can take it back (not when this is the taking back).
    pub(super) fn start_rename(
        &mut self,
        ctx: &egui::Context,
        renames: Vec<(PathBuf, PathBuf)>,
        record: bool,
    ) {
        if self.transfers.is_busy() {
            self.notice = Some(Notice::hint(i18n::t().transfer_busy));
            return;
        }
        let riders: HashMap<PathBuf, PathBuf> = renames
            .iter()
            .filter_map(|(from, _)| {
                Some((
                    from.clone(),
                    self.folder.pairs.companion(from)?.to_path_buf(),
                ))
            })
            .collect();
        let involved: Vec<PathBuf> = renames
            .iter()
            .map(|(from, _)| from.clone())
            .chain(riders.values().cloned())
            .collect();
        // Marks not written yet are tried once more first, and a playing video lets go of its
        // file (Windows refuses to rename an open one); the job waits for both.
        self.retry_before(&involved);
        self.stop_video();
        self.renaming_records = record;
        self.transfers.push_rename(renames, riders);
        self.poll_transfer(ctx);
    }

    /// The worker is through: what knows the photos by path follows their new names, and the
    /// view stays on the photo it showed.
    pub(super) fn finish_rename(&mut self, ctx: &egui::Context, outcome: &TransferOutcome) {
        let t = i18n::t();
        let (name, err) = outcome
            .failed
            .first()
            .map(|(path, err)| (library::file_name_lossy(path), err.clone()))
            .unwrap_or_default();
        let text = (t.rename_done)(outcome.done.len(), outcome.skipped.len(), &name, &err);
        self.notice = Some(if outcome.failed.is_empty() {
            Notice::hint(text)
        } else {
            Notice::error(text)
        });
        if outcome.done.is_empty() {
            return;
        }
        let current = self.view.get(self.current).cloned();
        self.retarget_moved(outcome);
        let moved: HashMap<&PathBuf, &PathBuf> = outcome
            .done
            .iter()
            .chain(&outcome.riders)
            .map(|(from, to)| (from, to))
            .collect();
        let follow = |path: &PathBuf| {
            moved
                .get(path)
                .map_or_else(|| path.clone(), |to| (*to).clone())
        };
        for (from, to) in &outcome.done {
            if let Some(rating) = self.marks.ratings.remove(from) {
                self.marks.ratings.insert(to.clone(), rating);
            }
            if let Some(label) = self.marks.labels.remove(from) {
                self.marks.labels.insert(to.clone(), label);
            }
            if let Some(description) = self.marks.descriptions.remove(from) {
                self.marks.descriptions.insert(to.clone(), description);
            }
            if let Some(rider) = self.folder.pairs.companion(from).map(Path::to_path_buf) {
                self.folder.pairs.forget(from);
                self.folder.pairs.insert(to.clone(), follow(&rider));
            }
        }
        for (from, to) in outcome.done.iter().chain(&outcome.riders) {
            self.board.rename(from, to);
        }
        self.viewer.pinned = self.viewer.pinned.as_ref().map(follow);
        let mut all: Vec<PathBuf> = self.folder.all.iter().map(follow).collect();
        if let Some(dir) = &self.folder.dir {
            library::sort(dir, &mut all);
        }
        self.folder.all = Arc::new(all);
        self.scan_raw_marks();
        self.sync_library();
        if self.renaming_records {
            let pairs: Vec<(PathBuf, PathBuf)> = outcome
                .done
                .iter()
                .map(|(from, to)| (to.clone(), from.clone()))
                .collect();
            self.marks.journal.push(Entry::Rename { pairs });
        }
        self.rebuild_view(ctx, current.as_ref().map(follow));
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use eframe::egui::{Key, Modifiers};

    use super::super::harness::Harness;

    const NONE: Modifiers = Modifiers::NONE;

    /// 2026-08-23 19:15:46 as Cerno's milliseconds; `+ s` seconds.
    fn at(s: i64) -> i64 {
        20_688 * 86_400_000 + ((19 * 60 + 15) * 60 + 46 + s) * 1000
    }

    /// Frames until the worker is through.
    fn finish(h: &mut Harness) {
        let until = Instant::now() + Duration::from_secs(10);
        while h.app.transfers.is_busy() && Instant::now() < until {
            h.frame(Vec::new());
            std::thread::sleep(Duration::from_millis(5));
        }
        h.frame(Vec::new());
    }

    fn dir(h: &Harness) -> PathBuf {
        h.app.folder.dir.clone().expect("a folder")
    }

    /// `F2` opens the card on the standard pattern, `Enter` renames, the view stays on the
    /// photo under its new name; `Ctrl+Z` gives the old name back.
    #[test]
    fn f2_renames_the_photo_and_ctrl_z_takes_it_back() {
        let mut h = Harness::new(3);
        h.settle();
        let first = h.app.view.first().cloned().expect("a photo");
        h.app.board.put_taken(&first, at(0));
        h.press(Key::F2, NONE);
        assert_eq!(h.layer(), "rename");
        h.settle();
        h.press(Key::Enter, NONE);
        assert_eq!(h.layer(), "none");
        finish(&mut h);
        let renamed = dir(&h).join("2026_08_23__191546.jpg");
        assert!(renamed.exists() && !first.exists());
        assert_eq!(h.app.view.get(h.app.current), Some(&renamed));
        assert!(h.app.folder.all.contains(&renamed));
        assert!(h.app.board.get(&renamed).is_some(), "what is known follows");
        h.press(Key::Z, Modifiers::COMMAND);
        finish(&mut h);
        assert!(first.exists() && !renamed.exists(), "Ctrl+Z: the old name");
        assert_eq!(h.app.view.get(h.app.current), Some(&first));
    }

    /// `Shift+F2` with `Alt+3`: every photo shown gets the Android pattern; two of the same
    /// second take `_1`, one without a capture time keeps its name.
    #[test]
    fn shift_f2_renames_every_photo_shown() {
        let mut h = Harness::new(3);
        h.settle();
        let paths: Vec<PathBuf> = (0..3).filter_map(|i| h.app.view.get(i).cloned()).collect();
        h.app.board.put_taken(&paths[0], at(0));
        h.app.board.put_taken(&paths[1], at(0));
        h.press(Key::F2, Modifiers::SHIFT);
        assert_eq!(h.layer(), "rename");
        h.settle();
        h.press(Key::Num3, Modifiers::ALT);
        h.press(Key::Enter, NONE);
        finish(&mut h);
        let dir = dir(&h);
        assert!(dir.join("20260823_191546.jpg").exists());
        assert!(dir.join("20260823_191546_1.jpg").exists());
        assert!(paths[2].exists(), "no capture time: kept");
        assert!(!paths[0].exists() && !paths[1].exists());
    }
}
