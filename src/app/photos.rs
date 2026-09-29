//! The photo area: one photo or two side by side (compare mode), mouse zoom and pan.

use std::path::PathBuf;

use eframe::egui::{self, CursorIcon, PointerButton, Rect, Sense, pos2, vec2};

use crate::i18n;
use crate::library;
use crate::loader::{LoadedImage, Lookup};
use crate::metadata::Rating;
use crate::theme::tokens;
use crate::ui::{overlays, viewer};

use super::CernoApp;
use super::gate::Change;
use super::notice::Notice;

/// Gap between the two photos in compare mode.
const COMPARE_GUTTER: f32 = 4.0;

/// One photo slot on screen: which photo, where, and which side (compare mode).
#[derive(Clone, Copy)]
pub(super) struct Slot {
    index: usize,
    area: Rect,
    side: Side,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Single,
    Left,
    Right,
}

impl CernoApp {
    /// `Enter` on a video: it plays in the system's player. Nothing for a photo.
    pub(super) fn play_video(&mut self) {
        let Some(path) = self.view.get(self.current) else {
            return;
        };
        if library::format_of(path) != Some(library::Format::Video) {
            return;
        }
        if let Err(err) = crate::video::play(path) {
            self.notice = Some(Notice::error((i18n::t().video_play_failed)(&format!(
                "{err:#}"
            ))));
        }
    }

    /// `C`: pin the current photo on the left and show the next one on the right – or leave
    /// compare mode.
    pub(super) fn toggle_compare(&mut self, ctx: &egui::Context) {
        if self.pinned.take().is_some() {
            self.loader.set_pinned(None);
            return;
        }
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        if self.view.len() < 2 {
            self.notice = Some(Notice::hint(i18n::t().compare_needs_two));
            return;
        }
        let right = self.neighbour(self.current, &[&path]);
        self.pinned = Some(path);
        self.rebuild_view(ctx, right);
    }

    /// `A`: the left photo wins – the right one is rejected, compare mode ends on the left one.
    pub(super) fn keep_left(&mut self, ctx: &egui::Context) {
        if let Some((left, right)) = self.compared() {
            self.choose(ctx, left, right);
        }
    }

    /// `D`: the right photo wins – the left one is rejected, compare mode ends on the right one.
    pub(super) fn keep_right(&mut self, ctx: &egui::Context) {
        if let Some((left, right)) = self.compared() {
            self.choose(ctx, right, left);
        }
    }

    /// The left (pinned) and right photo while comparing.
    fn compared(&self) -> Option<(PathBuf, PathBuf)> {
        Some((self.pinned.clone()?, self.view.get(self.current).cloned()?))
    }

    /// Rejects `loser`, leaves compare mode and shows `winner` alone. A photo that takes no
    /// mark right now (it is being moved) keeps the comparison open.
    fn choose(&mut self, ctx: &egui::Context, winner: PathBuf, loser: PathBuf) {
        if !self.allowed(Change::Mark, Some(&loser)) {
            return;
        }
        self.pinned = None;
        self.rate(ctx, loser, Rating::Rejected, false);
        self.rebuild_view(ctx, Some(winner));
    }

    /// `F7`: the grid instead of the single photo, or back. While it shows only its cursor
    /// photo is decoded, and the thumbnail cache keeps what it shows. Not during a straighten
    /// or crop session.
    pub(super) fn set_grid(&mut self, on: bool) {
        if on && (self.edit.is_some() || self.view.is_empty()) {
            return;
        }
        self.grid = on;
        self.grid_shown = None;
        self.loader.set_prefetch(!on);
        if !on {
            self.thumbs.set_visible(0);
        }
    }

    /// `+`/`−` or Ctrl + wheel in the grid: the cell size, a step at a time.
    pub(super) fn resize_grid(&mut self, steps: i32) {
        let last = crate::ui::grid::STEPS.len() as i32 - 1;
        self.grid_step = (self.grid_step as i32 + steps).clamp(0, last) as usize;
        // The cursor stays in view at the new size.
        self.grid_shown = None;
    }

    /// Photo slots on screen: one, or pinned left + current right in compare mode.
    pub(super) fn slots(&self, area: Rect) -> Vec<Slot> {
        match self.pinned_index() {
            Some(pinned) if pinned != self.current => {
                let half = (area.width() - COMPARE_GUTTER) / 2.0;
                let left = Rect::from_min_size(area.min, vec2(half, area.height()));
                let right = Rect::from_min_max(pos2(area.max.x - half, area.min.y), area.max);
                vec![
                    Slot {
                        index: pinned,
                        area: left,
                        side: Side::Left,
                    },
                    Slot {
                        index: self.current,
                        area: right,
                        side: Side::Right,
                    },
                ]
            }
            _ => vec![Slot {
                index: self.current,
                area,
                side: Side::Single,
            }],
        }
    }

    pub(super) fn frame_of(&self, ctx: &egui::Context, slot: &Slot) -> Option<viewer::Frame> {
        match self.loader.get(slot.index) {
            Lookup::Ready(image) => Some(viewer::Frame {
                area: slot.area,
                image_size: image.original_size,
                pixels_per_point: ctx.pixels_per_point(),
            }),
            _ => None,
        }
    }

    /// Mouse on a photo: double-click toggles 100 %, wheel zooms, drag pans. Both photos in
    /// compare mode share one zoom, so they stay aligned.
    fn handle_mouse(&mut self, ui: &egui::Ui, frame: &viewer::Frame, side: Side) {
        let id = ui.id().with(("photo", side as u8));
        let response = ui.interact(frame.area, id, Sense::click_and_drag());
        if response.double_clicked() {
            self.zoom.toggle(frame, response.interact_pointer_pos());
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0
                && let Some(pos) = response.hover_pos()
            {
                self.zoom.zoom_by(frame, (scroll / 200.0).exp(), pos);
            }
        }
        if self.zoom.is_zoomed() {
            if response.dragged_by(PointerButton::Primary) {
                self.zoom.pan(frame, response.drag_delta());
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }
        }
    }

    pub(super) fn draw_photos(&mut self, ui: &egui::Ui, slots: &[Slot]) {
        for slot in slots {
            match self.loader.get(slot.index) {
                Lookup::Ready(image) => self.draw_photo(ui, slot, &image),
                Lookup::Failed(message) => overlays::centred_message(
                    ui,
                    slot.area,
                    &format!(
                        "{}
{message}",
                        i18n::t().cannot_show
                    ),
                    tokens::STATUS_ERROR,
                ),
                Lookup::Pending => {
                    overlays::centred_message(ui, slot.area, i18n::t().loading, tokens::MUTED);
                }
            }
        }
    }

    /// One loaded photo: its pointer input (unless a menu or card is open over it), the
    /// pixels, then the edit overlay or the compare labels.
    fn draw_photo(&mut self, ui: &egui::Ui, slot: &Slot, image: &LoadedImage) {
        let frame = viewer::Frame {
            area: slot.area,
            image_size: image.original_size,
            pixels_per_point: ui.ctx().pixels_per_point(),
        };
        let covered = self.help_open
            || self.palette.is_some()
            || self.action_menu.is_some()
            || self.modal_open();
        let editing = self.edit.is_some();
        if !covered {
            if editing && slot.side == Side::Single {
                self.handle_edit_pointer(ui, &frame);
            } else if !editing {
                self.handle_mouse(ui, &frame, slot.side);
            }
        }
        let full = self.loader.full(slot.index);
        let needs_full = viewer::draw(
            ui.painter(),
            &frame,
            &self.zoom,
            image,
            full.as_deref(),
            self.straighten_angle(),
        );
        if slot.side == Side::Single {
            self.draw_edit_overlay(ui, &frame);
        }
        if self
            .view
            .get(slot.index)
            .is_some_and(|p| library::format_of(p) == Some(library::Format::Video))
        {
            let note = self.no_ffmpeg.then_some(i18n::t().video_no_ffmpeg);
            overlays::video_badge(ui, slot.area, note);
        }
        if needs_full && full.is_none() {
            self.loader.request_full(slot.index);
        }
        if !self.logged_first_photo {
            self.logged_first_photo = true;
            log::info!(
                "start-up: first photo drawn after {} ms",
                self.started.elapsed().as_millis()
            );
        }
        if slot.side != Side::Single {
            self.draw_compare_labels(ui, slot, image);
        }
    }

    /// Name, stars and scores of each side, and which key keeps it.
    fn draw_compare_labels(&mut self, ui: &egui::Ui, slot: &Slot, image: &LoadedImage) {
        let t = i18n::t();
        let path = self.view[slot.index].clone();
        let (side, key) = if slot.side == Side::Left {
            (t.compare_left, "A")
        } else {
            (t.compare_right, "D")
        };
        overlays::compare_label(
            ui,
            slot.area,
            side,
            &library::file_name_lossy(&path),
            self.rating_of(&path, Some(image)),
            &(t.keeps_this)(key),
        );
        let scores = self.board.get(&path).map(|k| k.scores);
        let percentiles = self.percentiles().clone();
        overlays::compare_scores(
            ui,
            slot.area,
            [
                scores.and_then(|s| s.aesthetic),
                scores.and_then(|s| s.aesthetic25),
            ],
            self.analyzer.personal(&path),
            scores.and_then(|s| percentiles.subject(&s)),
        );
    }
}
