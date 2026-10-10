//! One frame's layout – filter bar | photo(s) + details | filmstrip | info bar – and drawing the
//! bars around the photo.

use std::sync::Arc;

use eframe::egui::{self, Rect, pos2, vec2};

use crate::analysis::aesthetic;
use crate::i18n;
use crate::library;
use crate::loader::Lookup;
use crate::theme::tokens;
use crate::ui::details::{self, DetailsMode, DetailsTab};
use crate::ui::{cells, filmstrip, filter_bar, grid, help, info_bar, overlays, palette};
use crate::view;

use super::CernoApp;
use super::menu::ConfirmAction;

/// Where everything goes this frame. `None` for a part that is hidden.
pub(super) struct Layout {
    /// What is left for the photo(s).
    pub(super) area: Rect,
    pub(super) toolbar: Option<Rect>,
    pub(super) info: Option<Rect>,
    pub(super) filmstrip: Option<Rect>,
    pub(super) details: Option<Rect>,
}

impl CernoApp {
    /// The info bar always shows (once there is a photo); the filter bar also when a filter
    /// hides everything, so it can be changed back – and then as long as the pointer is on it,
    /// so it doesn't vanish under the click that resets the filter.
    pub(super) fn layout(&self, window: Rect) -> Layout {
        let mut area = window;
        let toolbar = (!self.all.is_empty()
            && (self.show_toolbar || self.view.is_empty() || self.toolbar_held))
            .then(|| {
                let r = Rect::from_min_size(
                    window.min,
                    vec2(window.width(), filter_bar::TOOLBAR_HEIGHT),
                );
                area.min.y = r.max.y;
                r
            });
        let info = (!self.view.is_empty()).then(|| {
            let r = Rect::from_min_max(
                pos2(window.min.x, window.max.y - info_bar::INFO_HEIGHT),
                window.max,
            );
            area.max.y = r.min.y;
            r
        });
        // The grid shows every photo already: its space goes to the grid.
        let filmstrip = (info.is_some() && self.show_filmstrip && !self.grid).then(|| {
            let r = Rect::from_min_max(
                pos2(window.min.x, area.max.y - filmstrip::HEIGHT),
                pos2(window.max.x, area.max.y),
            );
            area.max.y = r.min.y;
            r
        });
        let details = (info.is_some() && self.details != DetailsMode::Off).then(|| {
            let r = Rect::from_min_max(pos2(area.max.x - details::WIDTH, area.min.y), area.max);
            area.max.x = r.min.x;
            r
        });
        Layout {
            area,
            toolbar,
            info,
            filmstrip,
            details,
        }
    }

    /// The start screen, "nothing matches the filter", or the photo(s).
    pub(super) fn draw_centre(&mut self, ui: &mut egui::Ui, window: Rect, area: Rect) {
        let ctx = ui.ctx().clone();
        if self.all.is_empty() {
            // Start screen: one sentence, "Open folder" and the first keys (H shows all).
            let out = help::welcome(ui, window);
            if out.language {
                self.switch_language(&ctx);
            }
            if out.open_folder {
                self.pick_folder(&ctx);
            }
        } else if self.view.is_empty() {
            overlays::centred_message(ui, area, i18n::t().no_match, tokens::MUTED);
        } else if self.grid {
            self.draw_grid(ui, area);
        } else {
            // Navigation may have changed the photos; lay them out again.
            let slots = self.slots(area);
            self.draw_photos(ui, &slots);
        }
    }

    /// What the cell of view index `i` shows, for the filmstrip and the grid. `around` is the
    /// pinned photo and the current series, looked up once per frame.
    fn cell_info(
        &self,
        i: usize,
        percentiles: &view::Percentiles,
        around: (Option<usize>, Option<u32>),
    ) -> cells::CellInfo {
        let (pinned, current_series) = around;
        let path = &self.view.paths[i];
        let known = self.board.get(path);
        let blurry = known
            .filter(|k| percentiles.is_blurry(&k.scores))
            .and_then(|k| percentiles.subject(&k.scores))
            .map(|(p, eyes)| (i18n::t().blurry_tooltip)(eyes, p * 100.0));
        let rating = match self.session_ratings.get(path) {
            Some(rating) => *rating,
            None => known.map(|k| k.rating).unwrap_or_default(),
        };
        let place = self.view.series.get(i).and_then(|place| *place);
        cells::CellInfo {
            current: i == self.current,
            rating,
            blurry,
            pinned: pinned == Some(i),
            label: self.label_of(path, None),
            series_id: place.map(|p| p.id),
            in_current_series: place.is_some_and(|p| Some(p.id) == current_series),
            duplicate_of: self
                .view
                .duplicate_of
                .get(i)
                .and_then(|p| p.as_ref())
                .map(|original| self.photo_name(original)),
            video: library::format_of(path) == Some(library::Format::Video),
            deleted: self.is_deleted(path),
            pair: self.pair_note(path),
        }
    }

    /// The pinned photo and the current photo's series, for [`Self::cell_info`].
    fn around_current(&self) -> (Option<usize>, Option<u32>) {
        let series = self
            .view
            .series
            .get(self.current)
            .and_then(|place| *place)
            .map(|p| p.id);
        (self.pinned_index(), series)
    }

    pub(super) fn draw_filmstrip(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let paths = Arc::clone(&self.view.paths);
        let grouped = self.view.grouped;
        // Brought up to date first (`&mut self`), then only read while the cells are drawn.
        let percentiles = self.percentiles();
        let percentiles = percentiles.as_ref();
        let around = self.around_current();
        let strip = filmstrip::draw(ui, rect, &paths, self.current, &self.thumbs, grouped, |i| {
            self.cell_info(i, percentiles, around)
        });
        if let Some(index) = strip.clicked
            && self.edit.is_none()
        {
            self.go_to(&ctx, index, 1);
        }
        if strip.step != 0 && !self.help_open && self.edit.is_none() {
            let target = self.current.saturating_add_signed(strip.step);
            self.go_to(&ctx, target, strip.step.signum());
        }
    }

    /// The grid (`F7`) instead of the photo: a click moves the cursor (the current photo), a
    /// double click opens it, Ctrl + wheel changes the size.
    fn draw_grid(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let paths = Arc::clone(&self.view.paths);
        let grouped = self.view.grouped;
        let follow = self.grid_shown != Some(self.current);
        self.grid_shown = Some(self.current);
        let percentiles = self.percentiles();
        let percentiles = percentiles.as_ref();
        let around = self.around_current();
        let shown = grid::Shown {
            paths: &paths,
            current: self.current,
            grouped,
            step: self.grid_step,
            follow,
        };
        let out = grid::draw(ui, rect, &shown, &self.thumbs, |i| {
            self.cell_info(i, percentiles, around)
        });
        self.thumbs.set_visible(out.visible);
        self.grid_columns = out.columns;
        self.grid_page = out.page;
        if out.resize != 0 {
            self.resize_grid(out.resize);
        }
        let covered = self.help_open || self.palette.is_some() || self.action_menu.is_some();
        if covered {
            return;
        }
        if let Some(index) = out.opened {
            self.go_to(&ctx, index, 1);
            self.set_grid(false);
        } else if let Some(index) = out.clicked {
            self.go_to(&ctx, index, 1);
            // A click is no reason to scroll.
            self.grid_shown = Some(self.current);
        }
    }

    /// The info bar and, when it is open, the details panel – both about the current photo.
    pub(super) fn draw_info_bar(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        details_rect: Option<Rect>,
    ) {
        let ctx = ui.ctx().clone();
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        let image = match self.loader.get(self.current) {
            Lookup::Ready(image) => Some(image),
            _ => None,
        };
        let scores = self.board.get(&path).map(|k| k.scores);
        let percentiles = self.percentiles();
        let personal = self.analyzer.personal(&path);
        let name = self.photo_name(&path);
        let series = self
            .view
            .series
            .get(self.current)
            .and_then(|place| *place)
            .map(|place| (place.index, place.len));
        let duplicate_of = self
            .view
            .duplicate_of
            .get(self.current)
            .and_then(|p| p.as_ref())
            .map(|original| self.photo_name(original));
        let comparing = self.pinned.is_some();
        let bar = info_bar::InfoBar {
            name: &name,
            position: (self.current + 1, self.view.len()),
            image: image.as_deref(),
            rating: self.rating_of(&path, image.as_deref()),
            label: self.label_of(&path, image.as_deref()),
            series,
            duplicate_of,
            auto_advance: self.auto_advance,
            analysed: scores.is_some(),
            aesthetics: if comparing {
                None
            } else {
                scores.and_then(|s| aesthetic::combined(s.aesthetic, s.aesthetic25))
            },
            personal: if comparing { None } else { personal },
            sharpness: if comparing {
                None
            } else {
                scores.and_then(|s| percentiles.subject(&s))
            },
            blurry: !comparing && scores.is_some_and(|s| percentiles.is_blurry(&s)),
            saving: self.writer.status().pending > 0,
            zoom: self.zoom.scale.map(|s| s * 100.0),
            overlay: match self.overlay {
                crate::overlay::Mode::Off => None,
                crate::overlay::Mode::Sharpness => Some(i18n::t().overlay_fact_sharpness),
                crate::overlay::Mode::Exposure => Some(i18n::t().overlay_fact_exposure),
            },
            similarity: self.similarity_to_reference(&path),
            deleted: self.is_deleted(&path),
            raw_preview: library::format_of(&path).is_some_and(library::Format::is_raw),
            time_offset: self.time_offset_of(&path),
            pair: self.pair_note(&path).map(|(note, _)| note),
        };
        let out = info_bar::info_bar(ui, rect, &bar);
        if let Some(stars) = out.rating {
            self.set_rating(&ctx, stars, false);
        }
        if out.help {
            self.open_help();
        }
        if out.menu {
            self.help_open = false;
            self.palette = if self.palette.is_some() {
                None
            } else {
                Some(palette::State::default())
            };
        }
        let Some(rect) = details_rect else {
            // The panel is closed: a comment still being typed is written now.
            self.commit_comment();
            return;
        };
        let (tabs, body) = rect.split_top_bottom_at_y(rect.top() + details::TABS_HEIGHT);
        if let Some(tab) = details::tabs(ui, tabs, self.details_tab) {
            self.set_details_tab(tab);
        }
        if self.details_tab == DetailsTab::Description {
            self.draw_description(ui, body, &path, image.as_deref());
            return;
        }
        if self.details_tab == DetailsTab::Faces {
            let state = self.faces_of_current(&ctx);
            if let Some(face) = crate::ui::faces::tab(ui, body, &state.shown()) {
                self.zoom_to_face(face);
            }
            return;
        }
        let status = self.analyzer.status();
        let video = library::format_of(&path) == Some(library::Format::Video);
        let media = video.then(|| self.media_info(&ctx, &path)).flatten();
        let overlay = details::draw(
            ui,
            body,
            &details::Details {
                path: &path,
                scores,
                personal,
                frame_percentile: scores.and_then(|s| percentiles.frame(&s)),
                eyes_percentile: scores.and_then(|s| percentiles.eyes(&s)),
                attributes: self.analyzer.attributes(&path),
                histogram: image.as_deref().map(|i| &i.histogram),
                status: &status,
                file: image.as_deref().map(|i| (i.original_size, i.load_ms)),
                file_bytes: image.as_deref().map(|i| i.file_bytes),
                jpeg: image.as_deref().and_then(|i| i.jpeg),
                raw_preview: library::format_of(&path).is_some_and(library::Format::is_raw),
                position: image.as_deref().and_then(|i| i.camera.gps),
                media: media.as_ref(),
                video,
                overlay: self.overlay,
            },
            &mut self.attributes_open,
        );
        if let Some(mode) = overlay {
            self.set_overlay(mode);
        }
    }

    /// The filter bar: sort, filters, analysis status and the "Action" button.
    pub(super) fn draw_toolbar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let status = self.analyzer.status();
        let similar_to = self
            .similar_to
            .as_ref()
            .map(|(path, _)| self.photo_name(path));
        let has_videos = self.has_videos();
        let info = filter_bar::ToolbarInfo {
            stale: self.options.depends_on_scores() && self.board.version() != self.view_version,
            status: &status,
            actions_open: self.action_menu.is_some(),
            similar_to: similar_to.as_deref(),
            shown: self.view.len(),
            // Photos waiting to be deleted have left the view already; they don't count. The
            // deleted ones do while the 🗑 box shows them.
            total: self
                .library
                .iter()
                .filter(|p| !self.deletions.is_hidden(p))
                .count(),
            has_videos,
            has_deleted: self.has_deleted(),
            name_list: self.name_list.counts(),
        };
        let mut options = self.options;
        let out = filter_bar::toolbar(ui, rect, &mut options, &info);
        self.toolbar_held = !self.show_toolbar && ui.rect_contains_pointer(rect);
        self.action_anchor = out.actions_anchor;
        if out.toggle_actions {
            if self.action_menu.is_some() {
                self.close_action_menu();
            } else {
                self.open_action_menu();
            }
        }
        if out.options_changed {
            self.options = options;
            self.options_changed(&ctx);
        }
        if out.toggle_similar {
            self.toggle_similar(&ctx);
        }
        if out.refresh {
            self.refresh_order(&ctx);
        }
        if out.download_model {
            self.ask(ConfirmAction::DownloadModel, false);
        }
    }
}
