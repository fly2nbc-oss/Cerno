//! `Shift+T`: the whole filter set aside for a moment – every photo of the folder shows – and
//! back exactly as it was with the same key or a click on the info bar's mark (ticket CER-60,
//! the user's decisions). Setting a filter meanwhile discards it (`options_changed`), and so
//! does another folder (`open`, the user's answer of 2026-11-09: it lapses, every photo of
//! the new folder shows). While aside it is what is saved (`save_options`), so a restart in
//! the same folder brings it back; once it lapses, the filter of now is saved.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui;

use crate::i18n;
use crate::name_list::Matched;
use crate::view::ViewOptions;

use super::CernoApp;
use super::notice::Notice;

/// The filter set aside, with what it needs beside the options: the photo "similar photos" is
/// about, Top N's pick (it comes back as it was, not picked again) and the file list in effect.
pub(super) struct FilterAside {
    /// Every option as it was; the sort is taken from the moment it comes back.
    pub(super) options: ViewOptions,
    similar_to: Option<(PathBuf, Arc<[f32]>)>,
    top_pick: HashSet<PathBuf>,
    top_pick_for: Option<ViewOptions>,
    name_list: Option<Matched>,
    /// The photo shown when the filter went aside: shown again when the photo shown at the
    /// return is one the filter hides.
    shown: Option<PathBuf>,
}

impl FilterAside {
    /// Cerno moved a photo: it stays among the best, on the list, the one to come back to.
    pub(super) fn follow(&mut self, from: &Path, to: &Path) {
        if self.top_pick.remove(from) {
            self.top_pick.insert(to.to_path_buf());
        }
        if let Some(list) = &mut self.name_list
            && list.paths.remove(from)
        {
            list.paths.insert(to.to_path_buf());
        }
        if self.shown.as_deref() == Some(from) {
            self.shown = Some(to.to_path_buf());
        }
    }
}

impl CernoApp {
    /// `Shift+T` and the info bar's mark: the filter aside, or back. Without a filter only a
    /// hint says so.
    pub(super) fn toggle_filter_aside(&mut self, ctx: &egui::Context) {
        if self.browse.filter_aside.is_some() {
            let shown = self.view.get(self.current).cloned();
            let before = self.put_filter_back();
            self.sync_library();
            let view = self.build_view();
            // The photo on screen stays where the filter shows it, else the one shown when the
            // filter went aside.
            let keep = shown.filter(|p| view.contains(p)).or(before);
            self.set_view(ctx, view, keep);
        } else if self.options.is_filtered() {
            self.browse.filter_aside = Some(FilterAside {
                options: self.options.set_filters_aside(),
                similar_to: self.browse.similar_to.take(),
                top_pick: std::mem::take(&mut self.browse.top_pick),
                top_pick_for: self.browse.top_pick_for.take(),
                name_list: self.name_list.take_applied(),
                shown: self.view.get(self.current).cloned(),
            });
            // The 🗑 box went aside too: the deleted photos leave what is looked at.
            self.sync_library();
            self.rebuild_view(ctx, None);
            self.notice = Some(Notice::hint((i18n::t().filter_aside_set)(
                &i18n::with_shift("T"),
            )));
        } else {
            self.notice = Some(Notice::hint((i18n::t().filter_aside_none)(
                &i18n::with_shift("T"),
            )));
        }
    }

    /// The filter set aside back in place – options, the similar photo, Top N's pick, the file
    /// list – without building the view; the photo shown when it went aside.
    pub(super) fn put_filter_back(&mut self) -> Option<PathBuf> {
        let aside = self.browse.filter_aside.take()?;
        self.options.restore_filters(aside.options);
        self.browse.similar_to = aside.similar_to;
        self.browse.top_pick = aside.top_pick;
        self.browse.top_pick_for = aside.top_pick_for;
        self.name_list.put_applied(aside.name_list);
        aside.shown
    }

    /// The options as they are saved: while the filter is aside, the one set aside with the
    /// sort of now.
    pub(super) fn saved_options(&self) -> ViewOptions {
        let mut options = self.options;
        if let Some(aside) = &self.browse.filter_aside {
            options.restore_filters(aside.options);
        }
        options
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Key, Modifiers};

    use super::super::harness::Harness;
    use crate::name_list::Matched;
    use crate::view::{FilterKind, SortKey};

    const SHIFT: Modifiers = Modifiers::SHIFT;

    /// The view's file names, in order.
    fn shown(h: &Harness) -> Vec<String> {
        h.app
            .view
            .iter()
            .map(|p| crate::library::file_name_lossy(p))
            .collect()
    }

    fn current(h: &Harness) -> String {
        crate::library::file_name_lossy(h.app.view.get(h.app.current).expect("a photo"))
    }

    /// Four photos, the second with 4 stars, and the 4★ box ticked: only that one shows.
    fn four_star_filter() -> Harness {
        let mut h = Harness::new(4);
        h.settle();
        h.press(Key::ArrowRight, Modifiers::NONE);
        h.press(Key::Num4, Modifiers::NONE);
        let ctx = h.ctx.clone();
        h.app
            .change_options(&ctx, |o| o.toggle_filter(FilterKind::Stars(4)));
        assert_eq!(shown(&h), ["IMG_2.jpg"]);
        h
    }

    /// `Shift+T` shows every photo with the photo on screen kept, and again brings back exactly
    /// that filter; a photo the filter hides gives way to the one shown when it went aside.
    #[test]
    fn shift_t_sets_the_filter_aside_and_brings_it_back() {
        let mut h = four_star_filter();
        let filtered = h.app.options;
        h.press(Key::T, SHIFT);
        assert_eq!(h.app.view.len(), 4, "every photo");
        assert_eq!(current(&h), "IMG_2.jpg", "the photo stays");
        assert!(!h.app.options.is_filtered());
        assert!(h.app.browse.filter_aside.is_some());
        assert!(!h.app.bars.toolbar, "plain T's filter bar stays as it was");

        // Back from a photo the filter shows: it stays.
        h.press(Key::T, SHIFT);
        assert_eq!(h.app.options, filtered);
        assert_eq!(shown(&h), ["IMG_2.jpg"]);
        assert!(h.app.browse.filter_aside.is_none());

        // Back from a series neighbour the filter hides: the photo shown when it went aside.
        h.press(Key::T, SHIFT);
        h.press(Key::ArrowRight, Modifiers::NONE);
        assert_eq!(current(&h), "IMG_3.jpg");
        h.press(Key::T, SHIFT);
        assert_eq!(current(&h), "IMG_2.jpg");

        // A photo the filter shows, chosen while it was aside, stays on screen.
        h.press(Key::T, SHIFT);
        h.press(Key::ArrowRight, Modifiers::NONE);
        h.press(Key::Num4, Modifiers::NONE);
        assert_eq!(current(&h), "IMG_3.jpg");
        h.press(Key::T, SHIFT);
        assert_eq!(shown(&h), ["IMG_2.jpg", "IMG_3.jpg"]);
        assert_eq!(current(&h), "IMG_3.jpg");
    }

    /// Top N comes back with the photos it had picked – not picked again (here a new pick
    /// would find nothing: the harness has no scores) –, the file list with its photos.
    #[test]
    fn top_n_and_the_file_list_come_back_as_they_were() {
        let mut h = Harness::new(4);
        h.settle();
        let ctx = h.ctx.clone();
        let all = std::sync::Arc::clone(&h.app.folder.all);
        let path = |n: usize| all[n].clone();

        // Without scores no photo is among the best in the view (`Facts::top` needs the
        // index), so Top N is looked at through its pick.
        let picked: std::collections::HashSet<_> = [path(0), path(2)].into();
        h.app.options.top = Some(10);
        h.app.browse.top_pick = picked.clone();
        h.app.browse.top_pick_for = h.app.options.top_key();
        h.app.rebuild_view(&ctx, None);
        h.press(Key::T, SHIFT);
        assert_eq!(h.app.view.len(), 4);
        assert!(h.app.browse.top_pick.is_empty());
        // A mark meanwhile would change a new pick.
        h.press(Key::Num5, Modifiers::NONE);
        h.press(Key::T, SHIFT);
        assert_eq!(h.app.options.top, Some(10));
        assert_eq!(h.app.browse.top_pick, picked);
        assert_eq!(h.app.browse.top_pick_for, h.app.options.top_key());

        h.app.change_options(&ctx, |o| o.clear_filters());
        let listed = Matched {
            paths: [path(1), path(2)].into(),
            found: 2,
            total: 2,
            ..Matched::default()
        };
        h.app.name_list.put_applied(Some(listed));
        h.app.change_options(&ctx, |o| o.name_list = true);
        assert_eq!(shown(&h), ["IMG_2.jpg", "IMG_3.jpg"]);
        h.press(Key::T, SHIFT);
        assert_eq!(h.app.view.len(), 4);
        assert!(!h.app.is_listed(&path(1)));
        h.press(Key::T, SHIFT);
        assert_eq!(shown(&h), ["IMG_2.jpg", "IMG_3.jpg"], "the list is back");
    }

    /// A filter set while one is aside discards it – "Show all" then shows everything, and no
    /// older filter comes back; another sort keeps it. `Shift+T` without a filter only hints.
    #[test]
    fn a_new_filter_discards_the_one_aside() {
        let mut h = four_star_filter();
        let ctx = h.ctx.clone();
        h.press(Key::T, SHIFT);
        h.app.change_options(&ctx, |o| o.sort = SortKey::Rating);
        assert!(h.app.browse.filter_aside.is_some(), "the sort is no filter");

        h.app
            .change_options(&ctx, |o| o.toggle_filter(FilterKind::Rejected));
        assert!(h.app.browse.filter_aside.is_none());
        h.app.change_options(&ctx, |o| o.clear_filters());
        h.app.notice = None;
        h.press(Key::T, SHIFT);
        assert!(!h.app.options.is_filtered(), "nothing comes back");
        assert_eq!(h.app.view.len(), 4);
        assert!(h.app.notice.is_some(), "a hint says why");
    }

    /// The aside is never saved: what is saved meanwhile – another sort – keeps the filter that
    /// went aside, so the next start shows it.
    #[test]
    fn the_saved_filter_is_the_one_aside() {
        let mut h = four_star_filter();
        let ctx = h.ctx.clone();
        let saved = h.app.db.setting("filter");
        assert!(saved.is_some());
        h.press(Key::T, SHIFT);
        h.app.change_options(&ctx, |o| o.sort = SortKey::Rating);
        assert_eq!(h.app.db.setting("filter"), saved);
        assert_eq!(h.app.db.setting("sort").as_deref(), Some("rating"));
    }

    /// Setting the filter aside says so in a hint naming the key back.
    #[test]
    fn setting_the_filter_aside_says_so() {
        let mut h = four_star_filter();
        h.press(Key::T, SHIFT);
        let notice = h.app.notice.as_ref().expect("a hint");
        assert!(notice.text.contains("Filter off"), "{}", notice.text);
        assert!(!notice.error);
    }

    /// Another folder: the filter set aside lapses (the user's decision of 2026-11-09) – every
    /// photo shows, `Shift+T` has nothing to bring back, and no filter is saved any more.
    #[test]
    fn another_folder_lets_the_filter_aside_lapse() {
        let mut h = four_star_filter();
        let ctx = h.ctx.clone();
        h.press(Key::T, SHIFT);
        let dir = h.app.folder.dir.clone().expect("the folder");
        h.app.open(&ctx, &dir);
        h.settle();
        assert!(h.app.browse.filter_aside.is_none());
        assert!(!h.app.options.is_filtered());
        assert_eq!(shown(&h).len(), 4);
        let saved = h.app.db.setting("filter").unwrap_or_default();
        assert!(!saved.contains('4'), "no 4★ box saved: {saved:?}");
        h.press(Key::T, SHIFT);
        assert!(h.app.browse.filter_aside.is_none(), "nothing to bring back");
    }
}
