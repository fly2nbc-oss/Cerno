//! One frame's layout – filter bar | photo(s) + details | filmstrip | info bar – and drawing the
//! bars around the photo.

use std::sync::Arc;

use eframe::egui::{self, OpenUrl, Rect, pos2, vec2};

use crate::i18n;
use crate::loader::Lookup;
use crate::theme::tokens;
use crate::ui::details::{self, DetailsMode};
use crate::ui::{filmstrip, filter_bar, help, info_bar, overlays, palette};

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
    /// hides everything, so it can be changed back.
    pub(super) fn layout(&self, window: Rect) -> Layout {
        let mut area = window;
        let toolbar =
            (!self.all.is_empty() && (self.show_toolbar || self.view.is_empty())).then(|| {
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
        let filmstrip = (info.is_some() && self.show_filmstrip).then(|| {
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
        } else {
            // Navigation may have changed the photos; lay them out again.
            let slots = self.slots(area);
            self.draw_photos(ui, &slots);
        }
    }

    pub(super) fn draw_filmstrip(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let paths = Arc::clone(&self.view.paths);
        let series = Arc::clone(&self.view.series);
        let duplicates = Arc::clone(&self.view.duplicate_of);
        let grouped = self.view.grouped;
        let pinned = self.pinned_index();
        let current_series = series
            .get(self.current)
            .and_then(|place| *place)
            .map(|p| p.id);
        // Percentiles need `&mut self`; take them before borrowing `self` in the closure.
        let percentiles = self.percentiles().clone();
        let strip = filmstrip::draw(ui, rect, &paths, self.current, &self.thumbs, grouped, |i| {
            let path = &paths[i];
            let known = self.board.get(path);
            let blurry = known
                .filter(|k| percentiles.is_blurry(&k.scores))
                .and_then(|k| percentiles.subject(&k.scores))
                .map(|(p, eyes)| (i18n::t().blurry_tooltip)(eyes, p * 100.0));
            let rating = match self.session_ratings.get(path) {
                Some(rating) => *rating,
                None => known.map(|k| k.rating).unwrap_or_default(),
            };
            let place = series.get(i).and_then(|place| *place);
            filmstrip::CellInfo {
                rating,
                blurry,
                pinned: pinned == Some(i),
                label: self.label_of(path, None),
                series_id: place.map(|p| p.id),
                in_current_series: place.is_some_and(|p| Some(p.id) == current_series),
                duplicate_of: duplicates
                    .get(i)
                    .and_then(|p| p.as_ref())
                    .map(|original| self.photo_name(original)),
            }
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
        let percentiles = self.percentiles().clone();
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
                [None, None]
            } else {
                [
                    scores.and_then(|s| s.aesthetic),
                    scores.and_then(|s| s.aesthetic25),
                ]
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
        };
        let out = info_bar::info_bar(ui, rect, &bar);
        if let Some(stars) = out.rating {
            self.set_rating(&ctx, stars, false);
        }
        if out.help {
            self.help_open = true;
        }
        if out.menu {
            self.help_open = false;
            self.palette = if self.palette.is_some() {
                None
            } else {
                Some(palette::State::default())
            };
        }
        if let Some(url) = out.open_map {
            ctx.open_url(OpenUrl::new_tab(url));
        }
        if let Some(rect) = details_rect {
            let status = self.analyzer.status();
            details::draw(
                ui,
                rect,
                &details::Details {
                    scores,
                    personal,
                    frame_percentile: scores.and_then(|s| percentiles.frame(&s)),
                    eyes_percentile: scores.and_then(|s| percentiles.eyes(&s)),
                    attributes: self.analyzer.attributes(&path),
                    histogram: image.as_deref().map(|i| &i.histogram),
                    status: &status,
                    file: image.as_deref().map(|i| (i.original_size, i.load_ms)),
                },
                &mut self.details_expanded,
            );
        }
    }

    /// The filter bar: sort, filters, analysis status and the "Action" button.
    pub(super) fn draw_toolbar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let status = self.analyzer.status();
        let info = filter_bar::ToolbarInfo {
            stale: self.options.depends_on_scores() && self.board.version() != self.view_version,
            status: &status,
            actions_open: self.action_menu.is_some(),
        };
        let mut options = self.options;
        let out = filter_bar::toolbar(ui, rect, &mut options, &info);
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
            self.save_options();
            self.rebuild_view(&ctx, None);
        }
        if out.refresh {
            self.rebuild_view(&ctx, None);
        }
        if out.download_model {
            self.ask(ConfirmAction::DownloadModel, false);
        }
    }
}
