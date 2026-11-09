//! The file-name list filter (*Filter ▸ By file list …*): the card to paste the list into, the
//! photos it names (`ViewOptions::name_list`, `Facts::listed`) and the filter bar's chip. Never
//! saved; gone when another folder opens; a photo Cerno moves stays on the list.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Rect};

use crate::name_list::{self, Matched};
use crate::ui::name_list as card_ui;

use super::CernoApp;
use super::layer::Layer;

#[derive(Default)]
pub(super) struct NameList {
    /// The list in effect while `ViewOptions::name_list` is on.
    applied: Option<Matched>,
    /// The last list, offered again when the card opens.
    text: String,
}

/// The card, while it is open (`Layer::NameList`).
pub(super) struct Card {
    text: String,
    /// What `text` finds, made again when it changes.
    preview: Matched,
    previewed: Option<String>,
}

impl NameList {
    /// Found and asked for, for the filter bar's chip.
    pub(super) fn counts(&self) -> Option<(usize, usize)> {
        self.applied.as_ref().map(|m| (m.found, m.total))
    }

    pub(super) fn contains(&self, path: &Path) -> bool {
        self.applied
            .as_ref()
            .is_some_and(|m| m.paths.contains(path))
    }

    /// Another folder: the list was about the last one.
    pub(super) fn forget(&mut self) {
        self.applied = None;
    }

    /// The filter went off ("Show all", the chip): the list goes with it.
    pub(super) fn drop_applied(&mut self) {
        self.applied = None;
    }

    /// `Shift+T`: the list goes aside with the filter, and comes back with it.
    pub(super) fn take_applied(&mut self) -> Option<Matched> {
        self.applied.take()
    }

    pub(super) fn put_applied(&mut self, applied: Option<Matched>) {
        self.applied = applied;
    }

    /// Cerno moved a listed photo: it stays on the list.
    pub(super) fn follow(&mut self, from: &Path, to: &Path) {
        if let Some(applied) = &mut self.applied
            && applied.paths.remove(from)
        {
            applied.paths.insert(to.to_path_buf());
        }
    }
}

impl CernoApp {
    /// *Filter ▸ By file list …*: the card, with the last list in it.
    pub(super) fn open_name_list(&mut self) {
        self.layer = Layer::NameList(Card {
            text: self.name_list.text.clone(),
            preview: Matched::default(),
            previewed: None,
        });
    }

    /// The card, while open: what the list finds is shown as it is typed; Apply shows just
    /// those photos.
    pub(super) fn draw_name_list_card(&mut self, ctx: &egui::Context, window: Rect) {
        let dir = self.folder.dir.clone().unwrap_or_default();
        let all = std::sync::Arc::clone(&self.folder.all);
        let Layer::NameList(card) = &mut self.layer else {
            return;
        };
        if card.previewed.as_ref() != Some(&card.text) {
            card.preview = name_list::match_list(&card.text, &all, &dir);
            card.previewed = Some(card.text.clone());
        }
        let preview = card_ui::Preview {
            found: card.preview.found,
            total: card.preview.total,
            missing: &card.preview.missing,
            ambiguous: &card.preview.ambiguous,
        };
        let out = card_ui::show(ctx, window, &mut card.text, &preview);
        if out.apply {
            let Layer::NameList(card) = std::mem::take(&mut self.layer) else {
                return;
            };
            self.name_list.text = card.text;
            self.name_list.applied = Some(card.preview);
            // The list lives outside `ViewOptions`: with the filter already on, `top_key` would
            // not see it change, and Top N would keep the photos of the last list.
            self.browse.top_pick_for = None;
            self.change_options(ctx, |o| o.name_list = true);
        } else if out.close {
            self.layer = Layer::None;
        }
    }

    /// Whether the list in effect names `path`.
    pub(super) fn is_listed(&self, path: &Path) -> bool {
        self.options.name_list && self.name_list.contains(path)
    }

    /// The photos of the list, for "x of y photos" and the moved ones.
    pub(super) fn listed_follow(&mut self, moved: &[(PathBuf, PathBuf)]) {
        for (from, to) in moved {
            self.name_list.follow(from, to);
        }
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Key, Modifiers};

    use crate::metadata::Rating;

    const HOUR: i64 = 3_600_000;

    use super::super::harness::Harness;
    use super::*;

    /// Top N with the file list on: another list from the card picks the best photos again
    /// among the new list's – the list lives outside `ViewOptions`, so `top_key` alone would
    /// keep the photos of the last list, and none of them is on the new one.
    #[test]
    fn another_list_picks_top_n_again() {
        let mut h = Harness::new(4);
        let all = std::sync::Arc::clone(&h.app.folder.all);
        for (i, path) in all.iter().enumerate() {
            // On the board (an hour apart: no series) with stars, a value for Top N; the
            // harness has no scores.
            h.app.board.put_taken(path, HOUR * i as i64);
            h.app.marks.ratings.insert(path.clone(), Rating::Stars(4));
        }
        let ctx = h.ctx.clone();
        h.app.change_options(&ctx, |o| o.top = Some(10));
        let apply = |h: &mut Harness, text: &str| {
            h.app.open_name_list();
            let Layer::NameList(card) = &mut h.app.layer else {
                panic!("the card is open");
            };
            card.text = text.to_owned();
            h.press(Key::Enter, Modifiers::COMMAND | Modifiers::CTRL);
            assert_eq!(h.layer(), "none");
        };
        apply(&mut h, "IMG_1, IMG_2");
        assert_eq!(&h.app.view.paths[..], &all[..2]);
        apply(&mut h, "IMG_3, IMG_4");
        assert!(h.app.options.name_list);
        assert_eq!(h.app.options.top, Some(10));
        assert_eq!(
            &h.app.view.paths[..],
            &all[2..],
            "picked again among the new list"
        );
    }
}
