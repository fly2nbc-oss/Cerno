//! The photo area: one photo, two side by side (compare mode), four (the four-up view) or one
//! photo twice – whole and at 100 % (the split view) –, mouse zoom and pan.

use std::path::{Path, PathBuf};

use eframe::egui::{self, CursorIcon, PointerButton, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::analysis::aesthetic;
use crate::i18n;
use crate::library;
use crate::loader::{LoadedImage, Lookup};
use crate::metadata::Rating;
use crate::overlay;
use crate::theme::tokens;
use crate::ui::{navigator, overlays, viewer};
use crate::view;

use super::CernoApp;
use super::command::Command;
use super::gate::Change;
use super::notice::Notice;

/// Gap between the photos in compare mode and the four-up view.
const COMPARE_GUTTER: f32 = 4.0;

/// What the photo area shows and how; nothing of it is saved.
pub(super) struct Viewer {
    /// Compare mode: the photo pinned on the left. The current photo is shown on the right.
    pub(super) pinned: Option<PathBuf>,
    /// The four-up view (`Shift+C`): the view index of the first of its four photos.
    pub(super) quad: Option<usize>,
    /// The split view (`Shift+Z`, ticket CER-49): the current photo whole on the left and at
    /// `zoom` on the right.
    pub(super) split: bool,
    /// The photo the split view's 100 % side was last aimed at (its eyes or sharpest spot):
    /// another current photo is aimed at once it is known where to look.
    pub(super) split_aim: Option<PathBuf>,
    pub(super) zoom: viewer::Zoom,
    /// The check overlay over the photos (`O`).
    pub(super) overlay: overlay::Mode,
    /// The grid (`F7`) instead of the single photo.
    pub(super) grid: bool,
    /// Its cell size, an index into `grid::STEPS`.
    pub(super) grid_step: usize,
    /// The photo the grid last scrolled to: another current photo scrolls it into view.
    pub(super) grid_shown: Option<usize>,
    /// Columns and cells per page of the last drawn grid, for `↑`/`↓` and Page Up / Down.
    pub(super) grid_columns: usize,
    pub(super) grid_page: usize,
    /// The grid shows the scene view (`F8`): every scene from a new row under its header.
    pub(super) scenes: bool,
    /// The scenes and the order the view was built in.
    pub(super) scene_state: super::scenes::SceneState,
    /// The current photo's faces and the faces grid (`G`).
    pub(super) faces: super::faces::Faces,
}

impl Default for Viewer {
    fn default() -> Self {
        Self {
            pinned: None,
            quad: None,
            split: false,
            split_aim: None,
            zoom: viewer::Zoom::default(),
            overlay: overlay::Mode::Off,
            grid: false,
            grid_step: crate::ui::grid::DEFAULT_STEP,
            grid_shown: None,
            grid_columns: 1,
            grid_page: 1,
            scenes: false,
            scene_state: super::scenes::SceneState::default(),
            faces: super::faces::Faces::default(),
        }
    }
}

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
    /// One of the four-up view.
    Quad,
    /// The split view's left side: the whole photo, never zoomed.
    Whole,
    /// The split view's right side: the same photo at 100 % (the shared zoom).
    Detail,
}

impl Slot {
    /// The slot's zoom keys and mouse act on the shared zoom: every one but the split view's
    /// whole photo.
    pub(super) fn zooms(&self) -> bool {
        self.side != Side::Whole
    }
}

impl CernoApp {
    /// `C`: pin the current photo on the left and show the next one on the right – or leave
    /// compare mode.
    pub(super) fn toggle_compare(&mut self, ctx: &egui::Context) {
        if self.viewer.pinned.take().is_some() {
            self.loader.set_shown(Vec::new());
            return;
        }
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        if library::format_of(&path) == Some(library::Format::Video) {
            self.notice = Some(Notice::hint(i18n::t().video_no_compare));
            return;
        }
        if self.view.len() < 2 {
            self.notice = Some(Notice::hint(i18n::t().compare_needs_two));
            return;
        }
        let right = self.neighbour(self.current, &[&path]);
        // From the four-up view: the framed photo is pinned. The split view makes way.
        self.viewer.quad = None;
        self.end_split();
        self.viewer.pinned = Some(path);
        self.rebuild_view(ctx, right);
    }

    /// `Shift+C`: four photos at once from the current one, the frame on the current one – or
    /// the single photo again. From compare mode it starts at the right photo.
    pub(super) fn toggle_quad(&mut self) {
        if self.viewer.quad.take().is_some() {
            self.loader.set_shown(Vec::new());
            return;
        }
        if self.view.len() < 2 {
            self.notice = Some(Notice::hint(i18n::t().compare_needs_two));
            return;
        }
        self.viewer.pinned = None;
        self.end_split();
        if self.viewer.grid {
            self.set_grid(false);
        }
        self.viewer.quad = Some(self.current.min(self.view.len().saturating_sub(view::QUAD)));
        self.sync_quad();
    }

    /// The four-up window follows the current photo a row at a time, and the loader keeps the
    /// photos on screen.
    pub(super) fn sync_quad(&mut self) {
        if let Some(start) = self.viewer.quad {
            self.viewer.quad = (self.view.len() >= 2)
                .then(|| view::quad_start(start, self.current, self.view.len()));
        }
        self.loader.set_shown(self.others_on_screen());
    }

    /// The photos on screen besides the current one: the pinned one, or the four-up view's.
    pub(super) fn others_on_screen(&self) -> Vec<usize> {
        if let Some(start) = self.viewer.quad {
            let end = (start + view::QUAD).min(self.view.len());
            return (start..end).filter(|&i| i != self.current).collect();
        }
        self.pinned_index()
            .filter(|&pinned| pinned != self.current)
            .into_iter()
            .collect()
    }

    /// `Shift+Z`: the current photo whole on the left and at 100 % on the right, where Cerno
    /// measured the sharpness – or the single photo again (ticket CER-49). Compare mode and
    /// the four-up view make way; not for a video, not during a straighten or crop session.
    pub(super) fn toggle_split(&mut self) {
        if self.viewer.split {
            self.end_split();
            return;
        }
        if self.edits.session.is_some() {
            return;
        }
        let Some(path) = self.view.get(self.current) else {
            return;
        };
        if library::format_of(path) == Some(library::Format::Video) {
            self.notice = Some(Notice::hint(i18n::t().video_no_zoom));
            return;
        }
        if self.viewer.pinned.take().is_some() || self.viewer.quad.take().is_some() {
            self.loader.set_shown(Vec::new());
        }
        self.viewer.split = true;
        self.viewer.split_aim = None;
        self.viewer.zoom = viewer::Zoom {
            scale: Some(1.0),
            ..viewer::Zoom::default()
        };
    }

    /// The single photo again, whole – the split view's 100 % is not kept.
    pub(super) fn end_split(&mut self) {
        if self.viewer.split {
            self.viewer.split = false;
            self.viewer.split_aim = None;
            self.viewer.zoom = viewer::Zoom::default();
        }
    }

    /// Aims the split view's 100 % side at the current photo once it is known where to look:
    /// between the eyes of its largest face, else its sharpest spot (`LoadedImage::focus`),
    /// else its middle. The zoom's scale stays. False while the faces are still being looked
    /// up – the side waits rather than jump under the eye.
    fn aim_split(
        &mut self,
        ctx: &egui::Context,
        frame: &viewer::Frame,
        image: &LoadedImage,
    ) -> bool {
        let Some(path) = self.view.get(self.current).cloned() else {
            return false;
        };
        if self.viewer.split_aim.as_ref() == Some(&path) {
            return true;
        }
        let Some(eyes) = self.split_eyes(ctx) else {
            return false;
        };
        let [x, y] = eyes.or(image.focus).unwrap_or([0.5, 0.5]);
        self.viewer.zoom.centre_on(frame, vec2(x, y));
        self.viewer.split_aim = Some(path);
        true
    }

    /// `A`: the left photo wins – the right one is rejected, compare mode ends on the left one.
    /// `Shift+A` (`next`): the left one stays pinned and the next photo comes in on the right.
    pub(super) fn keep_left(&mut self, ctx: &egui::Context, next: bool) {
        if let Some((left, right)) = self.compared() {
            self.choose(ctx, left, right, next);
        }
    }

    /// `D`: the right photo wins – the left one is rejected, compare mode ends on the right one.
    /// `Shift+D` (`next`): the right one is pinned on the left, the next photo comes in.
    pub(super) fn keep_right(&mut self, ctx: &egui::Context, next: bool) {
        if let Some((left, right)) = self.compared() {
            self.choose(ctx, right, left, next);
        }
    }

    /// The left (pinned) and right photo while comparing.
    fn compared(&self) -> Option<(PathBuf, PathBuf)> {
        Some((
            self.viewer.pinned.clone()?,
            self.view.get(self.current).cloned()?,
        ))
    }

    /// Rejects `loser`, leaves compare mode and shows `winner` alone. A photo that takes no
    /// mark right now (it is being moved) keeps the comparison open.
    ///
    /// With `next` (`Shift+A` / `Shift+D`, ticket CER-64) the winner stays – or goes – on the
    /// left and the next photo of the view after the right one comes in on the right, at the
    /// same zoom: the pinned photo stays pinned throughout, so the loader keeps it and the
    /// shared zoom is not reset. At the end of the view the comparison ends with a hint and
    /// the winner shows alone.
    fn choose(&mut self, ctx: &egui::Context, winner: PathBuf, loser: PathBuf, next: bool) {
        if !self.allowed(Change::Mark, Some(&loser)) {
            return;
        }
        if !next {
            self.viewer.pinned = None;
            self.rate(ctx, loser, Rating::Rejected, false);
            self.rebuild_view(ctx, Some(winner));
            return;
        }
        // Before the mark: it may move the loser (capture-time order puts rejected photos last
        // in their series) or take it out of the view.
        let contender = self.next_contender(&winner, &loser);
        self.viewer.pinned = Some(winner.clone());
        self.rate(ctx, loser, Rating::Rejected, false);
        if contender.is_none() {
            self.viewer.pinned = None;
            self.notice = Some(Notice::hint(i18n::t().compare_last));
        }
        self.rebuild_view(ctx, contender.or(Some(winner)));
    }

    /// The photo that comes in on the right after `Shift+A` / `Shift+D`: the first one after
    /// the right photo in the view that is neither of the two compared nor already rejected –
    /// a rejected photo is out of the running, and in capture-time order it would come round
    /// again at the end of its series. Never one before: the comparison ends at the end.
    fn next_contender(&self, winner: &Path, loser: &Path) -> Option<PathBuf> {
        let after = self.view.get(self.current + 1..).unwrap_or_default();
        after
            .iter()
            .find(|path| {
                path.as_path() != winner
                    && path.as_path() != loser
                    && self.rating_of(path, None) != Rating::Rejected
            })
            .cloned()
    }

    /// `O` and View ▸ Overlay: sharp edges, clipped highlights and shadows, or nothing. A hint
    /// says what the colours mean.
    pub(super) fn set_overlay(&mut self, mode: overlay::Mode) {
        self.viewer.overlay = mode;
        self.loader.set_overlay(mode);
        let t = i18n::t();
        self.notice = Some(Notice::hint(match mode {
            overlay::Mode::Off => t.overlay_hint_off,
            overlay::Mode::Sharpness => t.overlay_hint_sharpness,
            overlay::Mode::Exposure => t.overlay_hint_exposure,
        }));
    }

    /// `F7`: the grid instead of the single photo, or back. While it shows only its cursor
    /// photo is decoded, and the thumbnail cache keeps what it shows. Not during a straighten
    /// or crop session.
    pub(super) fn set_grid(&mut self, on: bool) {
        if on && (self.edits.session.is_some() || self.view.is_empty()) {
            return;
        }
        // The grid replaces the four-up view and the split view, like every other photo on
        // screen.
        if on && self.viewer.quad.take().is_some() {
            self.loader.set_shown(Vec::new());
        }
        if on {
            self.end_split();
        }
        self.viewer.grid = on;
        // The grid itself; the scene view sets this again (`set_scenes`).
        self.viewer.scenes = false;
        self.viewer.grid_shown = None;
        self.loader.set_prefetch(!on);
        if !on {
            self.thumbs.set_visible(0);
        }
    }

    /// The info bar's view switcher: the photo, the grid, the scenes or the faces of the photo
    /// – what `Esc`, `F7`, `F8` and `G` do.
    pub(super) fn show_view(&mut self, view: crate::ui::info_bar::View) {
        use crate::ui::info_bar::View;
        match view {
            View::Photo => {
                self.viewer.faces.grid_open = false;
                if self.viewer.grid {
                    self.set_grid(false);
                }
            }
            View::Grid => {
                self.viewer.faces.grid_open = false;
                self.set_grid(true);
            }
            View::Scenes => {
                self.viewer.faces.grid_open = false;
                self.set_scenes(true);
            }
            View::Faces => self.viewer.faces.grid_open = true,
        }
    }

    /// `+`/`−` or Ctrl + wheel in the grid: the cell size, a step at a time.
    pub(super) fn resize_grid(&mut self, steps: i32) {
        let last = crate::ui::grid::STEPS.len() as i32 - 1;
        self.viewer.grid_step = (self.viewer.grid_step as i32 + steps).clamp(0, last) as usize;
        // The cursor stays in view at the new size.
        self.viewer.grid_shown = None;
    }

    /// Photo slots on screen: one, pinned left + current right in compare mode, or the four-up
    /// view in two rows – the current photo's slot last, where the zoom keys look.
    pub(super) fn slots(&self, area: Rect) -> Vec<Slot> {
        if let Some(start) = self.viewer.quad {
            let end = (start + view::QUAD).min(self.view.len());
            let size = vec2(
                (area.width() - COMPARE_GUTTER) / 2.0,
                (area.height() - COMPARE_GUTTER) / 2.0,
            );
            let mut slots: Vec<Slot> = (start..end)
                .map(|index| {
                    let place = index - start;
                    let offset = vec2(
                        (place % 2) as f32 * (size.x + COMPARE_GUTTER),
                        (place / 2) as f32 * (size.y + COMPARE_GUTTER),
                    );
                    Slot {
                        index,
                        area: Rect::from_min_size(area.min + offset, size),
                        side: Side::Quad,
                    }
                })
                .collect();
            slots.sort_by_key(|slot| slot.index == self.current);
            return slots;
        }
        let half = (area.width() - COMPARE_GUTTER) / 2.0;
        let left = Rect::from_min_size(area.min, vec2(half, area.height()));
        let right = Rect::from_min_max(pos2(area.max.x - half, area.min.y), area.max);
        // The split view: the same photo twice – a video shows alone.
        if self.viewer.split
            && self
                .view
                .get(self.current)
                .is_some_and(|p| library::format_of(p) != Some(library::Format::Video))
        {
            return vec![
                Slot {
                    index: self.current,
                    area: left,
                    side: Side::Whole,
                },
                Slot {
                    index: self.current,
                    area: right,
                    side: Side::Detail,
                },
            ];
        }
        match self.pinned_index() {
            Some(pinned) if pinned != self.current => {
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
            Lookup::Ready(image) => Some(frame_for(slot.area, &image, ctx.pixels_per_point())),
            _ => None,
        }
    }

    /// The photo areas on screen, for the decode size: one, or the two halves of compare mode
    /// and the split view.
    pub(super) fn photo_areas(&self, area: Rect) -> Vec<Rect> {
        self.slots(area).iter().map(|slot| slot.area).collect()
    }

    /// A click on one of the four-up view's photos: the frame moves there.
    fn frame_slot(&mut self, ctx: &egui::Context, index: usize) {
        if index != self.current {
            let direction = if index > self.current { 1 } else { -1 };
            self.go_to(ctx, index, direction);
        }
    }

    /// The slot shows a video (never zoomed, its own bar instead of the mouse zoom).
    pub(super) fn slot_is_video(&self, slot: &Slot) -> bool {
        self.view
            .get(slot.index)
            .is_some_and(|p| library::format_of(p) == Some(library::Format::Video))
    }

    /// Whether a video's play button sits at the bottom of the photo area (not in the grid).
    pub(super) fn video_on_screen(&self, area: Rect) -> bool {
        !self.viewer.grid
            && self.slots(area).iter().any(|slot| {
                self.view
                    .get(slot.index)
                    .is_some_and(|p| library::format_of(p) == Some(library::Format::Video))
            })
    }

    /// Mouse on a photo: double-click toggles 100 %, wheel zooms, drag pans; in the four-up view
    /// a click puts the frame on it. The photos on screen share one zoom, so they stay aligned.
    fn handle_mouse(&mut self, ui: &egui::Ui, frame: &viewer::Frame, slot: &Slot) {
        let id = ui.scope_id().with(("photo", slot.side as u8, slot.index));
        let response = ui.interact(frame.area, id, Sense::click_and_drag());
        if slot.side == Side::Quad && response.clicked() {
            self.frame_slot(ui.ctx(), slot.index);
        }
        if response.double_clicked() {
            self.viewer
                .zoom
                .toggle(frame, response.interact_pointer_pos());
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0
                && let Some(pos) = response.hover_pos()
            {
                self.viewer.zoom.zoom_by(frame, (scroll / 200.0).exp(), pos);
            }
        }
        if self.viewer.zoom.is_zoomed() {
            if response.dragged_by(PointerButton::Primary) {
                self.viewer.zoom.pan(frame, response.drag_delta());
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
        // The browse arrows: the single photo only – also while it loads or is zoomed – never
        // over a straighten or crop session, a menu or a card. Drawn after the photo, so they
        // take the click.
        let covered = self.layer.is_open() || self.modal_open();
        if let [slot] = slots
            && slot.side == Side::Single
            && self.edits.session.is_none()
            && !covered
        {
            let prev = slot.index > 0;
            let next = slot.index + 1 < self.view.len();
            if let Some(step) = overlays::browse_arrows(ui, slot.area, prev, next) {
                let index = slot.index.saturating_add_signed(step);
                self.click(
                    ui.ctx(),
                    Command::GoTo {
                        index,
                        direction: step,
                    },
                );
            }
            self.draw_navigator(ui, slot);
        }
        self.draw_split_part(ui, slots);
    }

    /// The split view: on the whole photo, the part the 100 % side shows, framed in the
    /// accent like the navigator's – once that side is aimed at this photo.
    fn draw_split_part(&self, ui: &egui::Ui, slots: &[Slot]) {
        let [whole, detail] = slots else {
            return;
        };
        if whole.side != Side::Whole
            || self.viewer.split_aim.as_ref() != self.view.get(self.current)
            || self.edits.session.is_some()
        {
            return;
        }
        let Lookup::Ready(image) = self.loader.get(whole.index) else {
            return;
        };
        let ppp = ui.ctx().pixels_per_point();
        let detail = frame_for(detail.area, &image, ppp);
        let Some(part) = viewer::visible_part(&detail, &self.viewer.zoom) else {
            return;
        };
        let photo = viewer::Zoom::default().image_rect(&frame_for(whole.area, &image, ppp));
        let at = |p: egui::Pos2| photo.min + p.to_vec2() * photo.size();
        ui.painter().with_clip_rect(photo).rect_stroke(
            Rect::from_min_max(at(part.min), at(part.max)),
            0.0,
            Stroke::new(2.0, tokens::ACCENT),
            StrokeKind::Inside,
        );
    }

    /// The navigator while the single photo is zoomed beyond the view (ticket CER-55, U2):
    /// never for a video, which has no zoom. Its thumbnail – mipmapped, so it shrinks cleanly –
    /// else the display image.
    fn draw_navigator(&mut self, ui: &egui::Ui, slot: &Slot) {
        if !self.viewer.zoom.is_zoomed() || self.slot_is_video(slot) {
            return;
        }
        let Some(frame) = self.frame_of(ui.ctx(), slot) else {
            return;
        };
        let Some(visible) = viewer::visible_part(&frame, &self.viewer.zoom) else {
            return;
        };
        let thumbnail = self
            .view
            .get(slot.index)
            .and_then(|path| self.thumbs.get_or_request(path))
            .map(|texture| texture.id());
        let texture = thumbnail.or_else(|| match self.loader.get(slot.index) {
            Lookup::Ready(image) => Some(image.texture.id()),
            _ => None,
        });
        let Some(texture) = texture else {
            return;
        };
        let aspect = frame.image_size[0] as f32 / frame.image_size[1].max(1) as f32;
        if let Some(point) = navigator::navigator(ui, slot.area, texture, aspect, visible) {
            self.viewer.zoom.centre_on(&frame, point);
        }
    }

    /// One loaded photo: its pointer input (unless a menu or card is open over it), the
    /// pixels, then the edit overlay or the compare labels.
    fn draw_photo(&mut self, ui: &egui::Ui, slot: &Slot, image: &LoadedImage) {
        let frame = frame_for(slot.area, image, ui.ctx().pixels_per_point());
        // The split view's 100 % side waits until it knows where to look.
        if slot.side == Side::Detail && !self.aim_split(ui.ctx(), &frame, image) {
            overlays::centred_message(ui, slot.area, i18n::t().loading, tokens::MUTED);
            return;
        }
        let covered = self.layer.is_open() || self.modal_open();
        let editing = self.edits.session.is_some();
        let is_video = self.slot_is_video(slot);
        // The whole photo of the split view is never zoomed.
        let zoom = if slot.zooms() {
            self.viewer.zoom
        } else {
            viewer::Zoom::default()
        };
        if !covered {
            if editing && slot.side == Side::Single {
                self.handle_edit_pointer(ui, &frame);
            } else if !editing && !is_video && slot.zooms() {
                self.handle_mouse(ui, &frame, slot);
            } else if !editing && slot.side == Side::Quad {
                // A video in the four-up view has no zoom, but a click frames it.
                let id = ui.scope_id().with(("photo", slot.side as u8, slot.index));
                if ui.interact(frame.area, id, Sense::click()).clicked() {
                    self.frame_slot(ui.ctx(), slot.index);
                }
            }
        }
        let full = self.loader.full(slot.index);
        // Not over a straighten or crop session: the frame and grid need the photo alone.
        let (overlay, full_overlay) = if self.viewer.overlay == overlay::Mode::Off || editing {
            (None, None)
        } else {
            (
                self.loader.overlay(slot.index),
                self.loader.full_overlay(slot.index),
            )
        };
        let needs_full = viewer::draw(
            ui.painter(),
            &frame,
            &zoom,
            image,
            full.as_deref(),
            viewer::Overlay {
                display: overlay.as_ref(),
                full: full_overlay.as_deref().map(Vec::as_slice),
            },
            self.straighten_angle(),
        );
        if slot.side == Side::Single {
            self.draw_edit_overlay(ui, &frame);
        }
        if is_video && let Some(path) = self.view.get(slot.index).cloned() {
            self.draw_video_slot(ui, slot, &path);
        }
        if needs_full && full.is_none() {
            self.loader.request_full(slot.index);
        }
        if self.startup.first_photo.is_none() {
            self.startup.first_photo = Some(std::time::Instant::now());
            log::info!(
                "start-up: first photo drawn after {} ms",
                self.startup.started.elapsed().as_millis()
            );
        }
        if matches!(slot.side, Side::Left | Side::Right | Side::Quad) {
            self.draw_compare_labels(ui, slot, image);
        }
        // The four-up view's frame: the current photo, which marks and keys act on.
        if slot.side == Side::Quad && slot.index == self.current {
            ui.painter().rect_stroke(
                slot.area,
                0.0,
                Stroke::new(2.0, tokens::ACCENT),
                StrokeKind::Inside,
            );
        }
    }

    /// A video: its playing frame and bar, else the poster's play button – in the single view
    /// (compare mode shows the poster only).
    fn draw_video_slot(&mut self, ui: &egui::Ui, slot: &Slot, path: &Path) {
        if self.draw_video(ui, slot.area, path) || self.current_video() != Some(path) {
            return;
        }
        // A click on the play button plays it, like `Enter` or `Space`.
        if overlays::video_badge(ui, slot.area, slot.index).clicked() {
            self.toggle_video(ui.ctx());
        }
    }

    /// Name, stars and scores of each side, and which key keeps it.
    fn draw_compare_labels(&mut self, ui: &egui::Ui, slot: &Slot, image: &LoadedImage) {
        let t = i18n::t();
        let Some(path) = self.view.get(slot.index).cloned() else {
            return;
        };
        // The four-up view: the place in the view instead of a side, no key to keep it.
        let (side, hint) = match slot.side {
            Side::Left => (t.compare_left.to_owned(), (t.keeps_this)("A")),
            Side::Quad => ((slot.index + 1).to_string(), String::new()),
            _ => (t.compare_right.to_owned(), (t.keeps_this)("D")),
        };
        overlays::compare_label(
            ui,
            slot.area,
            &side,
            &library::file_name_lossy(&path),
            self.rating_of(&path, Some(image)),
            &hint,
        );
        let scores = self.board.get(&path).map(|k| k.scores);
        let percentiles = self.percentiles();
        overlays::compare_scores(
            ui,
            slot.area,
            scores.and_then(|s| aesthetic::combined(s.aesthetic, s.aesthetic25)),
            scores.and_then(|s| percentiles.subject(&s)),
        );
    }
}

/// The geometry of `image` drawn in `area`.
fn frame_for(area: Rect, image: &LoadedImage, pixels_per_point: f32) -> viewer::Frame {
    let [w, h] = image.texture.size();
    viewer::Frame {
        area,
        image_size: image.original_size,
        display_size: [w as u32, h as u32],
        pixels_per_point,
    }
}
