//! The photo area: one photo or two side by side (compare mode), mouse zoom and pan.

use eframe::egui::{self, CursorIcon, PointerButton, Rect, Sense, pos2, vec2};

use crate::i18n;
use crate::library;
use crate::loader::{LoadedImage, Lookup};
use crate::metadata::Rating;
use crate::theme::tokens;
use crate::ui::{bars, viewer};

use super::CernoApp;
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

    /// `A`: the left photo wins, the right one is rejected and the next photo moves in.
    pub(super) fn keep_left(&mut self, ctx: &egui::Context) {
        if self.pinned.is_none() {
            return;
        }
        let Some(right) = self.view.get(self.current).cloned() else {
            return;
        };
        let next = self.neighbour(self.current, &[&right]);
        self.rate(ctx, right, Rating::Rejected, false);
        self.rebuild_view(ctx, next);
    }

    /// `D`: the right photo wins and moves to the left, the left one is rejected.
    pub(super) fn keep_right(&mut self, ctx: &egui::Context) {
        let (Some(left), Some(right)) = (self.pinned.clone(), self.view.get(self.current).cloned())
        else {
            return;
        };
        let next = self.neighbour(self.current, &[&left, &right]);
        self.rate(ctx, left, Rating::Rejected, false);
        self.pinned = Some(right);
        self.rebuild_view(ctx, next);
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
                Lookup::Failed(message) => bars::centred_message(
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
                    bars::centred_message(ui, slot.area, i18n::t().loading, tokens::MUTED);
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
        bars::compare_label(
            ui,
            slot.area,
            side,
            &library::file_name_lossy(&path),
            self.rating_of(&path, Some(image)),
            &(t.keeps_this)(key),
        );
        let scores = self.board.get(&path).map(|k| k.scores);
        let percentiles = self.percentiles().clone();
        bars::compare_scores(
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
