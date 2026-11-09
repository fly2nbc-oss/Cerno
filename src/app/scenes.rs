//! The scene view (`F8`) from the app's side: it is the grid (`Viewer::grid`) with its photos
//! grouped into scenes (`Viewer::scenes`), so everything the grid does – the cursor is the
//! current photo, only it is decoded, the filmstrip gives way – holds here too. While it shows,
//! the view is built in capture-time order whatever the chosen sort (which stays saved), and
//! the scenes are cut from that view once per build (`view::scenes`).

use std::ops::Range;
use std::path::PathBuf;
use std::sync::{Arc, Weak};

use eframe::egui::{self, Rect};

use crate::i18n;
use crate::metadata;
use crate::ui::scenes;
use crate::view::{self, Scene, ScenePhoto, SortKey, ViewOptions};

use super::CernoApp;
use super::command::Command;

/// The scenes of the view they were cut from, and which order the view was built in.
#[derive(Default)]
pub(super) struct SceneState {
    /// The view's list and build the scenes belong to.
    from: (Weak<Vec<PathBuf>>, u64),
    scenes: Arc<Vec<Scene>>,
    /// The view was built for the scene view (capture-time order).
    ordered: bool,
}

impl CernoApp {
    /// What the view is built with: in the scene view always capture-time order. The chosen
    /// sort is what is saved and what the filter bar shows.
    pub(super) fn view_options(&self) -> ViewOptions {
        ViewOptions {
            sort: if self.viewer.scenes {
                SortKey::Taken
            } else {
                self.options.sort
            },
            ..self.options
        }
    }

    /// `F8`, the view switcher: the scene view instead of the photo or the grid, or the photo
    /// again. Not during a straighten or crop session (as the grid).
    pub(super) fn set_scenes(&mut self, on: bool) {
        if on {
            self.set_grid(true);
            self.viewer.scenes = self.viewer.grid;
        } else {
            self.set_grid(false);
        }
    }

    /// The view in the order the scene view (or the others) needs: built again once the scene
    /// view opened or closed, staying on the current photo. Called before anything draws.
    pub(super) fn sync_scene_order(&mut self, ctx: &egui::Context) {
        if self.viewer.scene_state.ordered == self.viewer.scenes {
            return;
        }
        self.viewer.scene_state.ordered = self.viewer.scenes;
        if self.options.sort != SortKey::Taken && !self.folder.library.is_empty() {
            self.rebuild_view(ctx, None);
        }
    }

    /// The scenes of the view as it is built now – cut again only when it was built again, so
    /// nothing moves while the analysis brings new scores (`Refresh order` takes them in).
    pub(super) fn scenes(&mut self) -> Arc<Vec<Scene>> {
        let from = (Arc::downgrade(&self.view.paths), self.browse.view_version);
        let state = &self.viewer.scene_state;
        if Weak::ptr_eq(&state.from.0, &from.0) && state.from.1 == from.1 {
            return Arc::clone(&state.scenes);
        }
        let facts: Vec<_> = self.view.iter().map(|p| self.facts(p)).collect();
        let embeddings: Vec<_> = self
            .view
            .iter()
            .map(|p| self.analyzer.embedding_of(p))
            .collect();
        let photos: Vec<ScenePhoto<'_>> = facts
            .iter()
            .zip(&embeddings)
            .zip(self.view.series.iter())
            .map(|((facts, embedding), place)| ScenePhoto {
                taken: facts.and_then(|f| f.taken_ms),
                series: place.map(|p| p.id),
                camera: facts.and_then(|f| f.camera),
                embedding: embedding.as_deref(),
            })
            .collect();
        let scenes = Arc::new(view::scenes(&photos));
        self.viewer.scene_state.from = from;
        self.viewer.scene_state.scenes = Arc::clone(&scenes);
        scenes
    }

    /// Page Down / Page Up in the scene view: the next scene's first photo, or back.
    pub(super) fn scene_jump(&mut self, ctx: &egui::Context, down: bool) {
        let scenes = self.scenes();
        let target = view::scene_step(&scenes, self.current, down);
        self.go_to(ctx, target, if down { 1 } else { -1 });
    }

    /// `↓`/`↑` in the scene view: a row, into the next or previous scene too.
    pub(super) fn scene_row(&mut self, ctx: &egui::Context, down: bool) {
        let ranges = ranges(&self.scenes());
        let target = scenes::row_step(&ranges, self.viewer.grid_columns, self.current, down);
        self.go_to(ctx, target, if down { 1 } else { -1 });
    }

    /// The scene view instead of the photo: the grid's clicks, double clicks and sizes.
    pub(super) fn draw_scenes(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let paths = Arc::clone(&self.view.paths);
        let scenes = self.scenes();
        let ranges = ranges(&scenes);
        let follow = self.viewer.grid_shown != Some(self.current);
        self.viewer.grid_shown = Some(self.current);
        let percentiles = self.percentiles();
        let percentiles = percentiles.as_ref();
        let around = self.around_current();
        let shown = scenes::Shown {
            paths: &paths,
            scenes: &ranges,
            current: self.current,
            step: self.viewer.grid_step,
            follow,
        };
        let out = scenes::draw(
            ui,
            rect,
            &shown,
            &self.thumbs,
            |i| self.cell_info(i, percentiles, around),
            |s| {
                scenes
                    .get(s)
                    .map(|s| self.scene_header(s))
                    .unwrap_or_default()
            },
        );
        self.thumbs.set_visible(out.visible);
        self.viewer.grid_columns = out.columns;
        self.viewer.grid_page = out.page;
        if out.resize != 0 {
            self.click(&ctx, Command::ResizeGrid(out.resize));
        }
        if self.layer.is_help() || self.layer.is_list() {
            return;
        }
        if let Some(index) = out.opened {
            self.click(&ctx, Command::OpenFromGrid(index));
        } else if let Some(index) = out.clicked {
            self.click(&ctx, Command::PickInGrid(index));
        }
    }

    /// A scene's header line: its time, how many photos, its cameras.
    fn scene_header(&self, scene: &Scene) -> String {
        let t = i18n::t();
        let mut parts = vec![match scene.time {
            Some((start, end)) => time_range(start, end),
            None => t.scene_untimed.to_owned(),
        }];
        parts.push((t.photos_count)(scene.range.len()));
        let cameras: Vec<String> = scene
            .cameras
            .iter()
            .filter_map(|&id| self.board.camera_name(id))
            .collect();
        if !cameras.is_empty() {
            parts.push(cameras.join(", "));
        }
        parts.join(" · ")
    }
}

fn ranges(scenes: &[Scene]) -> Vec<Range<usize>> {
    scenes.iter().map(|s| s.range.clone()).collect()
}

/// `23.08.2026 19:16–19:24` in the language's date style; one time when both fall in one
/// minute, both dates across midnight.
fn time_range(start: i64, end: i64) -> String {
    let (from, to) = (metadata::format_millis(start), metadata::format_millis(end));
    if from == to {
        i18n::date(&from)
    } else if from.get(..10) == to.get(..10) {
        format!("{}–{}", i18n::date(&from), to.get(11..).unwrap_or_default())
    } else {
        format!("{} – {}", i18n::date(&from), i18n::date(&to))
    }
}

#[cfg(test)]
mod tests {
    use super::super::harness::Harness;
    use super::*;
    use crate::metadata::Rating;
    use eframe::egui::{Key, Modifiers};

    const NONE: Modifiers = Modifiers::NONE;
    const HOUR: i64 = 3_600_000;

    /// Six photos in name order: `IMG_1` taken last, `IMG_2` and `IMG_3` a series, `IMG_4` and
    /// `IMG_5` an hour apart, `IMG_6` without a time. No embeddings: series group alone.
    fn six() -> (Harness, Vec<PathBuf>) {
        let mut h = Harness::new(6);
        h.settle();
        let paths = h.app.folder.all.to_vec();
        let board = Arc::clone(&h.app.board);
        for (k, taken) in [5 * HOUR, 0, 1_000, HOUR, 2 * HOUR].into_iter().enumerate() {
            board.put_taken(&paths[k], taken);
        }
        let ctx = h.ctx.clone();
        h.app.rebuild_view(&ctx, None);
        assert_eq!(h.app.options.sort, SortKey::Name);
        (h, paths)
    }

    fn current_path(h: &Harness) -> PathBuf {
        h.app.view[h.app.current].clone()
    }

    /// `F8` shows the scenes in capture-time order on the same photo; `F8` again the photo, in
    /// the chosen order. The sort itself never changes.
    #[test]
    fn f8_opens_the_scenes_in_capture_time_order() {
        let (mut h, paths) = six();
        h.press(Key::F8, NONE);
        assert!(h.app.viewer.grid && h.app.viewer.scenes);
        let order: Vec<PathBuf> = h.app.view.to_vec();
        let expected: Vec<PathBuf> = [1, 2, 3, 4, 0, 5].map(|k| paths[k].clone()).into();
        assert_eq!(order, expected);
        assert_eq!(current_path(&h), paths[0], "the same photo");
        let scenes: Vec<Range<usize>> = ranges(&h.app.scenes());
        assert_eq!(scenes, [0..2, 2..3, 3..4, 4..5, 5..6]);
        h.settle();
        assert_eq!(h.app.options.sort, SortKey::Name, "not saved");

        h.press(Key::F8, NONE);
        assert!(!h.app.viewer.grid && !h.app.viewer.scenes);
        assert_eq!(*h.app.view.paths, paths, "name order again");
        assert_eq!(current_path(&h), paths[0]);
    }

    /// Page Down / Page Up jump from scene to scene, `↑`/`↓` go from row to row – every scene
    /// starts its own.
    #[test]
    fn page_keys_jump_scenes_arrows_rows() {
        let (mut h, paths) = six();
        h.press(Key::F8, NONE);
        h.press(Key::Home, NONE);
        assert_eq!(h.app.current, 0);
        h.press(Key::PageDown, NONE);
        assert_eq!(h.app.current, 2, "past the series");
        h.press(Key::PageDown, NONE);
        h.press(Key::PageDown, NONE);
        h.press(Key::PageDown, NONE);
        assert_eq!(
            current_path(&h),
            paths[5],
            "the photos without a time come last"
        );
        h.press(Key::PageDown, NONE);
        assert_eq!(h.app.current, 5, "the last scene: nothing moves");
        h.press(Key::PageUp, NONE);
        assert_eq!(h.app.current, 4);
        h.press(Key::Home, NONE);
        h.press(Key::ArrowRight, NONE);
        assert_eq!(h.app.current, 1, "→ one photo");
        h.press(Key::ArrowDown, NONE);
        assert_eq!(h.app.current, 2, "↓ the next scene's row");
        h.press(Key::ArrowUp, NONE);
        assert_eq!(
            h.app.current, 0,
            "↑ the row above, in the column ↓ ended in"
        );
    }

    /// `F7` and `F8` switch between grid and scenes; `Esc`, `Enter` and the view switcher's
    /// photo leave for the single photo.
    #[test]
    fn grid_and_scenes_switch_and_leave_like_the_grid() {
        let (mut h, paths) = six();
        h.press(Key::F7, NONE);
        assert!(h.app.viewer.grid && !h.app.viewer.scenes);
        h.press(Key::F8, NONE);
        assert!(h.app.viewer.grid && h.app.viewer.scenes);
        h.press(Key::F7, NONE);
        assert!(h.app.viewer.grid && !h.app.viewer.scenes, "F7: the grid");
        assert_eq!(*h.app.view.paths, paths);
        h.press(Key::F8, NONE);
        h.press(Key::Escape, NONE);
        assert!(!h.app.viewer.grid && !h.app.viewer.scenes, "Esc: the photo");
        h.press(Key::F8, NONE);
        h.press(Key::Enter, NONE);
        assert!(
            !h.app.viewer.grid && !h.app.viewer.scenes,
            "Enter: the photo"
        );
        h.app.show_view(crate::ui::info_bar::View::Scenes);
        h.settle();
        assert!(h.app.viewer.scenes);
        h.app.show_view(crate::ui::info_bar::View::Photo);
        h.settle();
        assert!(!h.app.viewer.grid && !h.app.viewer.scenes);
        assert_eq!(*h.app.view.paths, paths);
    }

    /// The cursor is the current photo: a mark lands on it and the scene view stays.
    #[test]
    fn a_mark_in_the_scenes_lands_on_the_cursor() {
        let (mut h, paths) = six();
        h.press(Key::F8, NONE);
        h.press(Key::Home, NONE);
        h.press(Key::Num3, NONE);
        assert_eq!(h.app.marks.ratings.get(&paths[1]), Some(&Rating::Stars(3)));
        assert!(h.app.viewer.scenes);
    }

    /// Tests run in English (`i18n::t` starts so): ISO dates.
    #[test]
    fn a_scene_names_its_time() {
        let day = 1_787_443_200_000; // 2026-08-23 00:00 (wall clock)
        let at = |h: i64, m: i64| day + h * 3_600_000 + m * 60_000;
        let date = |s: &str| i18n::date(s);
        assert_eq!(
            time_range(at(19, 16), at(19, 16) + 30_000),
            date("2026-08-23 19:16")
        );
        assert_eq!(
            time_range(at(19, 16), at(19, 24)),
            format!("{}–19:24", date("2026-08-23 19:16"))
        );
        assert_eq!(
            time_range(at(23, 58), at(24, 3)),
            format!(
                "{} – {}",
                date("2026-08-23 23:58"),
                date("2026-08-24 00:03")
            )
        );
    }
}
