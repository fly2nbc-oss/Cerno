//! Opening a folder, building the view (sort, filter, pending deletions) and moving
//! through it.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, ViewportCommand};

use crate::analysis::manifest::Pack;
use crate::i18n;
use crate::library::{self, Library};
use crate::ui::viewer;
use crate::view::{self, Facts, FilterKind, Percentiles, View, ViewOptions};

use super::layer::Layer;
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
            // where the models are downloaded – all of them while CLIP is missing, else V2.5.
            if !library.paths.is_empty() {
                self.offer_models();
            }
        }
        // RAW + JPG of one name: the RAW rides along with the JPEG (`pairs`).
        let (paths, pairs) = if self.pair_mode {
            crate::pairs::pair_up(library.paths.to_vec(), Path::to_path_buf)
        } else {
            (library.paths.to_vec(), crate::pairs::Pairs::default())
        };
        // A photo that was opened directly stays selected (a RAW by its JPEG); a folder starts
        // at the top of the (possibly sorted) view.
        let start = path
            .is_file()
            .then(|| library.paths.get(index).cloned())
            .flatten()
            .map(|opened| {
                pairs
                    .primary_of(&opened)
                    .map_or(opened.clone(), Path::to_path_buf)
            });
        let paths = Arc::new(paths);
        self.all_index = index_of(&paths);
        self.pairs = pairs;
        self.all = paths;
        self.library = Arc::clone(&self.all);
        self.dir = Some(library.dir);
        self.load_camera_offsets();
        self.scan_raw_marks();
        // Found in the background; the 🗑 box shows them once they are known.
        self.scan_deleted();
        self.pinned = None;
        self.quad = None;
        // "Similar photos" was about a photo of the previous folder, Top N picked from it, the
        // deleted photos lay beside it.
        self.options.similar = false;
        self.options.filter.set(FilterKind::Deleted, false);
        self.options.name_list = false;
        self.name_list.forget();
        self.layer.close_if(Layer::is_name_list);
        self.similar_to = None;
        self.options.top = None;
        self.top_pick.clear();
        self.top_pick_for = None;
        // `Ctrl+Z` takes back what was done since the folder opened.
        self.marks.journal.clear();
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

    /// Subfolders on or off (`Ctrl+U`, the settings menu); the current folder opens again.
    pub(super) fn toggle_subfolders(&mut self, ctx: &egui::Context) {
        self.subfolders = !self.subfolders;
        self.db.put_flag("subfolders", self.subfolders);
        if let Some(dir) = self.dir.clone() {
            self.open(ctx, &dir);
        }
        // An error or "no photos here" from opening says more.
        if self.notice.is_none() {
            let t = i18n::t();
            self.notice = Some(Notice::hint(if self.subfolders {
                t.subfolders_on
            } else {
                t.subfolders_off
            }));
        }
    }

    /// The hint once, while CLIP is missing: one download fetches every missing model.
    fn offer_models(&mut self) {
        // ExifTool first: without it no star is saved. The models' hint comes next time.
        if self.offer_exiftool_once() {
            return;
        }
        let t = i18n::t();
        let missing = self.analyzer.status().missing();
        let size = i18n::size(missing.iter().map(|pack| pack.bytes()).sum());
        let shown = |key| self.db.setting(key).as_deref() == Some("1");
        if missing.contains(&Pack::Clip) && !shown(CLIP_OFFER_SHOWN) {
            self.notice = Some(Notice::hint((t.aesthetics_offer)(&size)));
            self.db.put_setting(CLIP_OFFER_SHOWN, "1");
        }
    }

    /// Re-applies sorting, filtering and pending deletions, staying on `keep` (or the current
    /// photo) if it is still shown.
    pub(super) fn rebuild_view(&mut self, ctx: &egui::Context, keep: Option<PathBuf>) {
        let view = self.build_view();
        self.set_view(ctx, view, keep);
    }

    pub(super) fn build_view(&self) -> View {
        view::build(
            &self.library,
            self.options,
            |p| self.facts(p),
            &self.marks.ratings,
            &self.marks.labels,
            |p| self.deletions.is_hidden(p),
        )
    }

    pub(super) fn set_view(&mut self, ctx: &egui::Context, view: View, keep: Option<PathBuf>) {
        let before = self.view.get(self.current).cloned();
        let keep = keep.or_else(|| before.clone());
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
        if self.view.get(self.current) != before.as_ref() {
            self.start_whole();
        }
        self.view_version = self.board.version();
        self.view_built = Instant::now();
        if let Some(start) = self.quad {
            self.quad = (self.view.len() >= 2)
                .then(|| view::quad_start(start, self.current, self.view.len()));
        }
        self.loader.set_library(
            Arc::clone(&self.view.paths),
            self.current,
            self.others_on_screen(),
        );
        self.sync_analyzer();
        self.update_title(ctx);
        // A copy or move finished, a filter changed …: the geometry belongs to the other photo.
        if let Some(session) = &self.edits.session
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

    /// The deleted photos are about this folder: the 🗑 box is never saved.
    pub(super) fn save_options(&self) {
        let mut filter = self.options.filter;
        filter.set(FilterKind::Deleted, false);
        self.db.put_setting("sort", self.options.sort.id());
        self.db.put_setting("filter", &filter.id());
        self.db
            .put_flag("hide_rejected", self.options.hide_rejected);
        self.db.put_setting("media", self.options.media.id());
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
        self.options_changed(ctx);
    }

    /// New sort or filter: saved, and the view is built again. "Similar photos" switched off
    /// (also by "Show all") forgets the photo it was about; Top N picks again when a filter
    /// changed – another sort only shows the same photos in another order.
    pub(super) fn options_changed(&mut self, ctx: &egui::Context) {
        if !self.options.similar {
            self.similar_to = None;
        }
        if !self.options.name_list {
            self.name_list.drop_applied();
        }
        self.save_options();
        if self.top_pick_for != self.options.top_key() {
            self.pick_top();
        }
        // The 🗑 box adds the deleted photos to what is looked at, or takes them away.
        self.sync_library();
        self.rebuild_view(ctx, None);
    }

    /// "Refresh order": the view built again with the scores known now – and Top N picked
    /// again, the only time besides a filter change.
    pub(super) fn refresh_order(&mut self, ctx: &egui::Context) {
        self.pick_top();
        self.rebuild_view(ctx, None);
    }

    fn pick_top(&mut self) {
        let picked = match self.options.top {
            Some(n) => view::pick_top(
                &self.all,
                self.options,
                |p| self.facts(p),
                &self.marks.ratings,
                &self.marks.labels,
                |p| self.deletions.is_hidden(p),
                usize::from(n),
            ),
            None => HashSet::new(),
        };
        self.top_pick = picked;
        self.top_pick_for = self.options.top_key();
    }

    /// `M`: only the photos like the current one – in compare mode like the pinned one – or
    /// all again. It needs the photo's CLIP embedding; when no other photo is close enough,
    /// the filter stays off and a hint says so. Photos without an embedding yet stay out until
    /// "Refresh order" (the analysis brings them along with their scores).
    pub(super) fn toggle_similar(&mut self, ctx: &egui::Context) {
        if self.options.similar {
            self.change_options(ctx, |o| o.similar = false);
            return;
        }
        let t = i18n::t();
        let Some(path) = self
            .pinned
            .clone()
            .or_else(|| self.view.get(self.current).cloned())
        else {
            return;
        };
        let Some(reference) = self.analyzer.embedding_of(&path) else {
            self.notice = Some(Notice::hint(if self.analyzer.clip_model_missing() {
                t.similar_needs_model
            } else {
                t.similar_not_analysed
            }));
            return;
        };
        let any = self.all.iter().any(|other| {
            *other != path
                && self
                    .analyzer
                    .similarity(other, &reference)
                    .is_some_and(|s| s >= view::SIMILAR_MIN)
        });
        if !any {
            self.notice = Some(Notice::hint((t.similar_none)(view::SIMILAR_MIN * 100.0)));
            return;
        }
        self.similar_to = Some((path, reference));
        self.change_options(ctx, |o| o.similar = true);
    }

    /// The photo "similar photos" is about, while that filter is on, and how alike `path` is.
    pub(super) fn similarity_to_reference(&self, path: &Path) -> Option<f32> {
        let (_, reference) = self.similar_to.as_ref().filter(|_| self.options.similar)?;
        self.analyzer.similarity(path, reference)
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
            self.start_whole();
            self.loader.set_current(index);
            self.sync_quad();
            self.sync_analyzer();
            self.update_title(ctx);
        }
    }

    /// Another photo is current: in the single view it starts whole, not at the zoom the last
    /// one had (the user's wish of 2026-10-05). Compare mode and the four-up view keep their
    /// shared zoom – it is what lines the photos up.
    fn start_whole(&mut self) {
        if self.pinned.is_none() && self.quad.is_none() {
            self.zoom = viewer::Zoom::default();
        }
        // A face zoom asked for the last photo does not apply to this one.
        self.faces.zoom_to = None;
    }

    /// Tells the analysis where the user is (in full-folder terms) and pauses it briefly – not
    /// in the grid, which decodes no big photo and whose cursor moves fast.
    fn sync_analyzer(&self) {
        if let Some(&index) = self
            .view
            .get(self.current)
            .and_then(|p| self.all_index.get(p))
        {
            if self.grid {
                self.analyzer.set_current_quietly(index);
            } else {
                self.analyzer.set_current(index);
            }
        }
    }

    fn update_title(&self, ctx: &egui::Context) {
        let title = match self.view.get(self.current) {
            Some(path) => format!("{} – Cerno", self.photo_name(path)),
            None => "Cerno".to_owned(),
        };
        ctx.send_viewport_cmd(ViewportCommand::Title(title));
    }

    /// What sorting, filtering and the UI know about a photo. A deleted photo the analysis
    /// hasn't reached yet is known to be deleted, which is all the 🗑 box needs.
    fn facts(&self, path: &Path) -> Option<Facts> {
        let deleted = self.is_deleted(path);
        let listed = self.is_listed(path);
        let Some(known) = self.board.get(path) else {
            return (deleted || listed).then(|| Facts {
                deleted,
                listed,
                ..Facts::default()
            });
        };
        Some(Facts {
            rating: known.rating,
            label: known.label,
            // A camera whose clock was off counts with the time set right.
            taken_ms: known
                .taken_ms
                .map(|taken| taken + self.camera_time.offset(known.camera)),
            camera: known.camera,
            fingerprint: known.fingerprint,
            scores: known.scores,
            personal: self.analyzer.personal(path),
            similarity: self.similarity_to_reference(path),
            top: self.top_pick.contains(path),
            deleted,
            listed,
        })
    }

    /// The name the info bar and the title show: relative to the open folder, and for a
    /// deleted photo the one it had there.
    pub(super) fn photo_name(&self, path: &Path) -> String {
        let path = self.deleted.original_of(path).unwrap_or(path);
        self.dir
            .as_ref()
            .map(|dir| library::display_name(dir, path))
            .unwrap_or_else(|| library::file_name_lossy(path))
    }

    /// Whether the folder has a video (the filter bar's *Videos only*), looked up once per
    /// list – `format_of` allocates, and the bar asks in every frame. The `Weak` keeps the
    /// list's address from being reused by the next one.
    pub(super) fn has_videos(&mut self) -> bool {
        if !std::sync::Weak::ptr_eq(&self.videos_in.0, &Arc::downgrade(&self.all)) {
            let any = self
                .all
                .iter()
                .any(|p| library::format_of(p) == Some(library::Format::Video));
            self.videos_in = (Arc::downgrade(&self.all), any);
        }
        self.videos_in.1
    }

    /// Sharpness percentiles of the folder, recomputed when scores changed – shared, not
    /// copied for every frame.
    pub(super) fn percentiles(&mut self) -> Arc<Percentiles> {
        let version = self.board.version();
        if self.percentiles.0 != version {
            let board = &self.board;
            let scores: Vec<_> = self
                .all
                .iter()
                .filter_map(|p| board.get(p).map(|k| k.scores))
                .collect();
            self.percentiles = (version, Arc::new(Percentiles::from_scores(scores.iter())));
        }
        Arc::clone(&self.percentiles.1)
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
