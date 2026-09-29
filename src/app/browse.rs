//! Opening a folder, building the view (sort, filter, pending deletions) and moving
//! through it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, ViewportCommand};

use crate::i18n;
use crate::library::{self, Library};
use crate::view::{self, Facts, Percentiles, View, ViewOptions};

use super::notice::Notice;
use super::{CLIP_OFFER_SHOWN, CernoApp};

pub(super) fn index_of(paths: &[PathBuf]) -> HashMap<PathBuf, usize> {
    paths
        .iter()
        .enumerate()
        .map(|(i, p)| (p.clone(), i))
        .collect()
}

impl CernoApp {
    pub(super) fn open(&mut self, ctx: &egui::Context, path: &Path) {
        let (library, index) = match Library::open(path, self.subfolders) {
            Ok(opened) => opened,
            Err(err) => {
                let path = path.display().to_string();
                self.notice = Some(Notice::error((i18n::t().cannot_open)(
                    &path,
                    &err.to_string(),
                )));
                return;
            }
        };
        // An error (e.g. the index could not be opened at start) stays until it is read.
        if !self.notice.as_ref().is_some_and(|notice| notice.error) {
            self.notice = library.paths.is_empty().then(|| {
                Notice::hint((i18n::t().no_photos_in)(&library.dir.display().to_string()))
            });
            // No dialog at start: once the first folder with photos is open, a quiet hint says
            // where the aesthetics model is downloaded.
            if !library.paths.is_empty()
                && self.analyzer.clip_model_missing()
                && self.db.setting(CLIP_OFFER_SHOWN).as_deref() != Some("1")
            {
                self.notice = Some(Notice::hint(i18n::t().aesthetics_offer));
                self.db.put_setting(CLIP_OFFER_SHOWN, "1");
            }
        }
        self.all_index = index_of(&library.paths);
        // A photo that was opened directly stays selected; a folder starts at the top of the
        // (possibly sorted) view.
        let start = path
            .is_file()
            .then(|| library.paths.get(index).cloned())
            .flatten();
        self.all = Arc::clone(&library.paths);
        self.dir = Some(library.dir);
        self.pinned = None;
        self.cancel_edit();
        self.thumbs.clear();
        // Only needed when the view depends on scores; the analysis fills the board anyway,
        // and on big folders the lookups would delay the first frame.
        if self.options.depends_on_scores() {
            self.analyzer.preload(&self.all);
        }
        self.analyzer.set_library(Arc::clone(&self.all), index);
        self.view = View::default();
        self.rebuild_view(ctx, start);
    }

    /// Re-applies sorting, filtering and pending deletions, staying on `keep` (or the current
    /// photo) if it is still shown.
    pub(super) fn rebuild_view(&mut self, ctx: &egui::Context, keep: Option<PathBuf>) {
        let view = self.build_view();
        self.set_view(ctx, view, keep);
    }

    fn build_view(&self) -> View {
        view::build(
            &self.all,
            self.options,
            |p| self.facts(p),
            &self.session_ratings,
            &self.session_labels,
            |p| self.deletions.is_hidden(p),
        )
    }

    fn set_view(&mut self, ctx: &egui::Context, view: View, keep: Option<PathBuf>) {
        let keep = keep.or_else(|| self.view.get(self.current).cloned());
        // Comparing needs the pinned photo plus at least one other.
        if self.pinned.as_ref().is_some_and(|p| !view.contains(p)) || view.len() < 2 {
            self.pinned = None;
        }
        let pinned = self
            .pinned
            .as_ref()
            .and_then(|p| view.iter().position(|q| q == p));
        let current = keep
            .and_then(|k| view.iter().position(|p| *p == k))
            .unwrap_or(0);
        self.current = view::skip_pinned(view.len(), current, pinned, 1).unwrap_or(current);
        self.view = view;
        self.view_version = self.board.version();
        self.view_built = Instant::now();
        self.loader
            .set_library(Arc::clone(&self.view.paths), self.current, pinned);
        self.sync_analyzer();
        self.update_title(ctx);
        // A copy or move finished, a filter changed …: the geometry belongs to the other photo.
        if let Some(session) = &self.edit
            && self.view.get(self.current) != Some(&session.path)
        {
            self.cancel_edit();
            self.notice = Some(Notice::hint(i18n::t().edit_cancelled));
        }
    }

    /// Name order without filters: new scores cannot move a photo, but they bring the
    /// fingerprints and capture times behind duplicate marks and series. Refresh them quietly,
    /// at most every 2 s. (With a score-dependent sort or filter the filter bar offers "Refresh
    /// order" instead – rebuilding there would move photos under the user.)
    ///
    /// With the same photos in the same order only the marks are swapped: a full rebuild
    /// would restart the loader (discarding decodes in flight) and pause the analysis.
    pub(super) fn refresh_marks(&mut self, ctx: &egui::Context) {
        const EVERY: Duration = Duration::from_secs(2);
        if self.options.depends_on_scores() || self.board.version() == self.view_version {
            return;
        }
        let since = self.view_built.elapsed();
        if since < EVERY {
            ctx.request_repaint_after(EVERY - since);
            return;
        }
        let version = self.board.version();
        let view = self.build_view();
        if *view.paths == *self.view.paths {
            self.view = view;
            self.view_version = version;
            self.view_built = Instant::now();
        } else {
            self.set_view(ctx, view, None);
        }
    }

    pub(super) fn pinned_index(&self) -> Option<usize> {
        let pinned = self.pinned.as_ref()?;
        self.view.iter().position(|p| p == pinned)
    }

    pub(super) fn save_options(&self) {
        self.db.put_setting("sort", self.options.sort.id());
        self.db.put_setting("filter", &self.options.filter.id());
    }

    pub(super) fn pick_folder(&mut self, ctx: &egui::Context) {
        let mut dialog = rfd::FileDialog::new().set_title(i18n::t().open_folder);
        if let Some(dir) = &self.dir {
            dialog = dialog.set_directory(dir);
        }
        if let Some(dir) = dialog.pick_folder() {
            self.open(ctx, &dir);
        }
    }

    pub(super) fn change_options(
        &mut self,
        ctx: &egui::Context,
        change: impl FnOnce(&mut ViewOptions),
    ) {
        change(&mut self.options);
        self.save_options();
        self.rebuild_view(ctx, None);
    }

    /// Moves to `index`, stepping over the pinned photo in `direction` in compare mode.
    pub(super) fn go_to(&mut self, ctx: &egui::Context, index: usize, direction: isize) {
        let Some(last) = self.view.len().checked_sub(1) else {
            return;
        };
        let Some(index) = view::skip_pinned(
            self.view.len(),
            index.min(last),
            self.pinned_index(),
            direction,
        ) else {
            return;
        };
        if index != self.current {
            self.current = index;
            self.loader.set_current(index);
            self.sync_analyzer();
            self.update_title(ctx);
        }
    }

    /// Tells the analysis where the user is (in full-folder terms) and pauses it briefly.
    fn sync_analyzer(&self) {
        if let Some(index) = self
            .view
            .get(self.current)
            .and_then(|p| self.all_index.get(p))
        {
            self.analyzer.set_current(*index);
        }
    }

    fn update_title(&self, ctx: &egui::Context) {
        let title = match self.view.get(self.current) {
            Some(path) => format!(
                "{} – Cerno",
                self.dir
                    .as_ref()
                    .map(|dir| library::display_name(dir, path))
                    .unwrap_or_else(|| library::file_name_lossy(path))
            ),
            None => "Cerno".to_owned(),
        };
        ctx.send_viewport_cmd(ViewportCommand::Title(title));
    }

    /// What sorting, filtering and the UI know about a photo.
    fn facts(&self, path: &Path) -> Option<Facts> {
        let known = self.board.get(path)?;
        Some(Facts {
            rating: known.rating,
            label: known.label,
            taken_ms: known.taken_ms,
            camera: known.camera,
            fingerprint: known.fingerprint,
            scores: known.scores,
            personal: self.analyzer.personal(path),
        })
    }

    pub(super) fn photo_name(&self, path: &Path) -> String {
        self.dir
            .as_ref()
            .map(|dir| library::display_name(dir, path))
            .unwrap_or_else(|| library::file_name_lossy(path))
    }

    /// Sharpness percentiles of the folder, recomputed when scores changed.
    pub(super) fn percentiles(&mut self) -> &Percentiles {
        let version = self.board.version();
        if self.percentiles.0 != version {
            let board = &self.board;
            let scores: Vec<_> = self
                .all
                .iter()
                .filter_map(|p| board.get(p).map(|k| k.scores))
                .collect();
            self.percentiles = (version, Percentiles::from_scores(scores.iter()));
        }
        &self.percentiles.1
    }

    /// The photo to show after the one at `index` disappears: the next one, otherwise the
    /// previous one – never the pinned photo or one in `exclude`.
    pub(super) fn neighbour(&self, index: usize, exclude: &[&PathBuf]) -> Option<PathBuf> {
        let usable = |p: &&PathBuf| !exclude.contains(p) && Some(*p) != self.pinned.as_ref();
        let after = self.view.get(index + 1..).unwrap_or_default();
        let before = self.view.get(..index).unwrap_or_default();
        after
            .iter()
            .find(usable)
            .or_else(|| before.iter().rev().find(usable))
            .cloned()
    }
}
