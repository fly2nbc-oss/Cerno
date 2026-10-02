//! Straighten, crop, quarter turns and `Ctrl+Z`: the session, its keys and pointer, and
//! handing the work to the writer.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{self, Event, Key, MouseWheelUnit, Pos2, Rect};

use crate::edit::{self, Ratio};
use crate::i18n;
use crate::loader::Lookup;
use crate::ui::{edit as edit_ui, viewer};

use super::CernoApp;
use super::gate::{Blocked, Change};
use super::notice::Notice;

pub(super) struct EditSession {
    /// The photo being edited; the session ends if another one becomes current.
    pub(super) path: PathBuf,
    pub(super) zoom: viewer::Zoom,
    kind: EditKind,
}

enum EditKind {
    Straighten {
        radians: f64,
    },
    Crop {
        ratio: Ratio,
        landscape: bool,
        image_size: [u32; 2],
        crop: edit::Crop,
        gesture: Option<CropGesture>,
    },
}

/// A drag on the crop, from where the button went down.
enum CropGesture {
    Move {
        start: Pos2,
        crop: edit::Crop,
    },
    /// `grab` is the corner minus the pointer, in image pixels: the corner keeps its distance
    /// to the pointer instead of jumping to it.
    Resize {
        corner: edit::Corner,
        anchor: (f64, f64),
        grab: (f64, f64),
    },
    Draw {
        anchor: Pos2,
        before: edit::Crop,
    },
}

impl CropGesture {
    fn kind(&self) -> edit_ui::Gesture {
        match self {
            Self::Move { .. } => edit_ui::Gesture::Move,
            Self::Resize { corner, .. } => edit_ui::Gesture::Resize(*corner),
            Self::Draw { .. } => edit_ui::Gesture::Draw,
        }
    }
}

/// A drawn frame shorter than this (in points) was a click: the frame before it comes back.
const DRAW_MIN: f32 = 6.0;

enum PixelJob {
    Rotate(f64),
    Crop(edit::PixelRect),
}

struct EditKeyInput {
    enter: bool,
    escape: bool,
    left: bool,
    right: bool,
    shift: bool,
    command: bool,
    flip: bool,
    cycle: bool,
    wheel: f64,
}

/// Wheel down rotates clockwise. One notch is `FINE_STEP`; Shift uses the finer step.
fn wheel_rotation(events: &[Event]) -> f64 {
    let mut turns = 0.0;
    for event in events {
        if let Event::MouseWheel {
            unit,
            delta,
            modifiers,
            ..
        } = event
        {
            let amount = -(delta.x + delta.y);
            let notches = match unit {
                MouseWheelUnit::Line | MouseWheelUnit::Page => amount,
                MouseWheelUnit::Point => amount / 50.0,
            };
            let step = if modifiers.shift {
                edit::FINER_STEP
            } else {
                edit::FINE_STEP
            };
            turns += f64::from(notches) * step;
        }
    }
    turns
}

fn refit_crop(ratio: Ratio, landscape: bool, image_size: [u32; 2], crop: &mut edit::Crop) {
    let aspect = edit::ratio_aspect(ratio, image_size[0], image_size[1], landscape);
    *crop = edit::Crop::max_centered(f64::from(image_size[0]), f64::from(image_size[1]), aspect);
}

fn crop_caption(ratio: Ratio, landscape: bool) -> String {
    let texts = i18n::t();
    let name = match ratio {
        Ratio::Original => texts.ratio_original,
        Ratio::ThreeTwo => "3:2",
        Ratio::FourThree => "4:3",
        Ratio::SixteenNine => "16:9",
        Ratio::Square => "1:1",
    };
    let side = if landscape {
        texts.crop_landscape
    } else {
        texts.crop_portrait
    };
    format!("{name} · {side}")
}

impl CernoApp {
    /// JPEG, one photo on screen, and nothing already being written.
    fn can_edit(&mut self) -> bool {
        if self.pinned.is_some() {
            return false;
        }
        let Some(path) = self.view.get(self.current).cloned() else {
            return false;
        };
        // Also refuses anything but JPEG (`Blocked::NotJpeg`).
        if !self.allowed(Change::Edit, Some(&path)) {
            return false;
        }
        matches!(self.loader.get(self.current), Lookup::Ready(_))
    }

    pub(super) fn begin_straighten(&mut self) {
        if matches!(
            self.edit,
            Some(EditSession {
                kind: EditKind::Straighten { .. },
                ..
            })
        ) {
            self.cancel_edit();
            return;
        }
        if !self.can_edit() {
            return;
        }
        let zoom = self
            .edit
            .take()
            .map(|session| session.zoom)
            .unwrap_or(self.zoom);
        self.zoom = viewer::Zoom::default();
        self.edit = Some(EditSession {
            path: self.view[self.current].clone(),
            zoom,
            kind: EditKind::Straighten { radians: 0.0 },
        });
    }

    pub(super) fn begin_crop(&mut self) {
        if matches!(
            self.edit,
            Some(EditSession {
                kind: EditKind::Crop { .. },
                ..
            })
        ) {
            self.cancel_edit();
            return;
        }
        if !self.can_edit() {
            return;
        }
        let Lookup::Ready(image) = self.loader.get(self.current) else {
            return;
        };
        let image_size = image.original_size;
        let landscape = image_size[0] >= image_size[1];
        let aspect = edit::ratio_aspect(Ratio::Original, image_size[0], image_size[1], landscape);
        let crop =
            edit::Crop::max_centered(f64::from(image_size[0]), f64::from(image_size[1]), aspect);
        let zoom = self
            .edit
            .take()
            .map(|session| session.zoom)
            .unwrap_or(self.zoom);
        self.zoom = viewer::Zoom::default();
        self.edit = Some(EditSession {
            path: self.view[self.current].clone(),
            zoom,
            kind: EditKind::Crop {
                ratio: Ratio::Original,
                landscape,
                image_size,
                crop,
                gesture: None,
            },
        });
    }

    pub(super) fn cancel_edit(&mut self) {
        if let Some(session) = self.edit.take() {
            self.zoom = session.zoom;
        }
    }

    fn confirm_edit(&mut self) {
        // Nothing may still be in the writer for this photo: the render reads the file now.
        if self.edit_busy {
            self.notice = Some(Notice::hint(Blocked::Writing.hint()));
            return;
        }
        let Some(session) = self.edit.take() else {
            return;
        };
        self.zoom = session.zoom;
        if self.view.get(self.current) != Some(&session.path) {
            self.notice = Some(Notice::hint(i18n::t().edit_cancelled));
            return;
        }
        let path = session.path;
        match session.kind {
            EditKind::Straighten { radians } if radians.abs() < 1e-8 => {}
            EditKind::Straighten { radians } => {
                self.spawn_edit(path, PixelJob::Rotate(radians));
            }
            EditKind::Crop {
                image_size, crop, ..
            } => {
                if crop.covers_image(f64::from(image_size[0]), f64::from(image_size[1])) {
                    return;
                }
                let rect = edit::PixelRect::from_crop(crop, image_size[0], image_size[1]);
                if rect.is_entire(image_size[0], image_size[1]) {
                    return;
                }
                let short = rect.w.min(rect.h);
                if short < edit::MIN_CROP_SIDE && short < image_size[0].min(image_size[1]) {
                    return;
                }
                self.spawn_edit(path, PixelJob::Crop(rect));
            }
        }
    }

    fn spawn_edit(&mut self, path: PathBuf, job: PixelJob) {
        self.edit_busy = true;
        self.notice = Some(Notice::working(i18n::t().edit_writing));
        let channel = self.writer.channel();
        let files = Arc::clone(&self.files);
        let thread = std::thread::Builder::new()
            .name("cerno-edit".into())
            .spawn(move || {
                // A panic would never report back, and `edit_busy` would block every edit.
                let result = crate::decode::catch_panic(|| match job {
                    PixelJob::Rotate(radians) => edit::render_rotation(&path, radians, &files),
                    PixelJob::Crop(rect) => edit::render_crop(&path, rect, &files),
                });
                match result {
                    Ok(jpeg) => channel.replace_pixels(path, jpeg),
                    Err(err) => channel.fail(path, format!("{err:#}")),
                }
            });
        match thread {
            Ok(thread) => self.edit_thread = Some(thread),
            Err(err) => {
                self.edit_busy = false;
                self.notice = Some(Notice::error((i18n::t().edit_failed)(&err.to_string())));
            }
        }
    }

    /// In a straighten or crop session it waits for `Enter` or `Esc`: the render would read
    /// the file before or after the turn, and one of the two would be lost.
    pub(super) fn rotate_quarter(&mut self, clockwise: bool) {
        if self.pinned.is_some() {
            return;
        }
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        if !self.allowed(Change::Rewrite, Some(&path)) {
            return;
        }
        self.edit_busy = true;
        self.writer.rotate_quarter(path, clockwise);
    }

    /// `Ctrl+Z`: the first original of the current photo goes back into the file (the first
    /// straighten, crop or quarter turn keeps it in `.originals`, for good).
    pub(super) fn undo_edit(&mut self) {
        if self.pinned.is_some() {
            return;
        }
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        if !self.allowed(Change::Rewrite, Some(&path)) {
            return;
        }
        match crate::originals::original(&self.db, &path) {
            Ok(Some(_)) => {
                self.edit_busy = true;
                self.writer.restore(path);
            }
            Ok(None) => self.notice = Some(Notice::hint(i18n::t().undo_nothing)),
            Err(err) => self.notice = Some(Notice::error(format!("{err:#}"))),
        }
    }

    pub(super) fn poll_edits(&mut self) {
        if let Some(thread) = self.edit_thread.take_if(|thread| thread.is_finished()) {
            let _ = thread.join();
        }
        let writing = i18n::t().edit_writing;
        for outcome in self.writer.poll_edits() {
            if outcome.elsewhere {
                self.reload_saved(&outcome.path);
                continue;
            }
            self.edit_busy = false;
            match outcome.error {
                Some(err) => self.notice = Some(Notice::error((i18n::t().edit_failed)(&err))),
                None => {
                    self.refresh_edited(&outcome.path);
                    if outcome.restored {
                        self.notice = Some(Notice::hint(i18n::t().undo_done));
                    } else if outcome.reencoded {
                        self.notice = Some(Notice::hint(i18n::t().edit_reencoded));
                    } else if self.notice.as_ref().is_some_and(|n| n.text == writing) {
                        self.notice = None;
                    }
                }
            }
        }
    }

    pub(super) fn refresh_edited(&mut self, path: &Path) {
        if let Some(index) = self.view.iter().position(|candidate| candidate == path) {
            self.loader.invalidate(index);
        }
        self.thumbs.invalidate(path);
        self.analyzer.revisit(path);
    }

    pub(super) fn handle_edit_keys(&mut self, ctx: &egui::Context) {
        let input = ctx.input(|input| EditKeyInput {
            enter: input.key_pressed(Key::Enter),
            escape: input.key_pressed(Key::Escape),
            left: input.key_pressed(Key::ArrowLeft),
            right: input.key_pressed(Key::ArrowRight),
            shift: input.modifiers.shift,
            command: input.modifiers.command,
            flip: input.modifiers.is_none() && input.key_pressed(Key::X),
            cycle: input.modifiers.is_none() && input.key_pressed(Key::A),
            wheel: wheel_rotation(&input.events),
        });
        if input.escape {
            self.cancel_edit();
            return;
        }
        if input.enter {
            self.confirm_edit();
            return;
        }
        if input.command && !input.shift && input.left {
            self.rotate_quarter(false);
        }
        if input.command && !input.shift && input.right {
            self.rotate_quarter(true);
        }
        let Some(session) = self.edit.as_mut() else {
            return;
        };
        match &mut session.kind {
            EditKind::Straighten { radians } if !input.command => {
                let step = if input.shift {
                    edit::FINER_STEP
                } else {
                    edit::FINE_STEP
                };
                let mut delta = input.wheel;
                if input.left {
                    delta -= step;
                }
                if input.right {
                    delta += step;
                }
                *radians = edit::clamp_angle(*radians + delta);
            }
            EditKind::Crop {
                ratio,
                landscape,
                image_size,
                crop,
                gesture,
            } => {
                if input.flip {
                    *landscape = !*landscape;
                    refit_crop(*ratio, *landscape, *image_size, crop);
                    *gesture = None;
                }
                if input.cycle {
                    *ratio = ratio.next();
                    refit_crop(*ratio, *landscape, *image_size, crop);
                    *gesture = None;
                }
            }
            EditKind::Straighten { .. } => {}
        }
    }

    pub(super) fn handle_edit_pointer(&mut self, ui: &egui::Ui, frame: &viewer::Frame) {
        let Some(EditSession {
            kind: EditKind::Crop { .. },
            ..
        }) = self.edit.as_ref()
        else {
            return;
        };
        let response = edit_ui::pointer_area(ui, frame.area);
        let photo = self.zoom.image_rect(frame);
        let (image_size, crop, active) = match &self.edit {
            Some(EditSession {
                kind:
                    EditKind::Crop {
                        image_size,
                        crop,
                        gesture,
                        ..
                    },
                ..
            }) => (*image_size, *crop, gesture.as_ref().map(CropGesture::kind)),
            _ => return,
        };
        let movable = crop.can_move(f64::from(image_size[0]), f64::from(image_size[1]));
        let screen = edit_ui::crop_to_screen(photo, image_size, crop);
        let under = |pos| edit_ui::gesture(edit_ui::hit_test(screen, pos), movable);
        if let Some(pos) = response.hover_pos().filter(|pos| frame.area.contains(*pos)) {
            let shown = active.unwrap_or_else(|| under(pos));
            ui.ctx().set_cursor_icon(edit_ui::cursor(shown));
        }
        if let Some(pos) = edit_ui::drag_start(ui, &response) {
            let gesture = match under(pos) {
                edit_ui::Gesture::Resize(corner) => {
                    let (cx, cy) = corner.point(crop);
                    let (px, py) = edit_ui::screen_to_image(photo, image_size, pos);
                    CropGesture::Resize {
                        corner,
                        anchor: corner.anchor(crop),
                        grab: (cx - px, cy - py),
                    }
                }
                edit_ui::Gesture::Move => CropGesture::Move { start: pos, crop },
                edit_ui::Gesture::Draw => CropGesture::Draw {
                    anchor: pos,
                    before: crop,
                },
            };
            if let Some(EditSession {
                kind: EditKind::Crop { gesture: slot, .. },
                ..
            }) = self.edit.as_mut()
            {
                *slot = Some(gesture);
            }
        }
        if response.dragged()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.drag_crop(photo, image_size, pos);
        }
        if response.drag_stopped()
            && let Some(EditSession {
                kind: EditKind::Crop { gesture, crop, .. },
                ..
            }) = self.edit.as_mut()
        {
            if let Some(CropGesture::Draw { before, anchor }) = gesture
                && !response
                    .interact_pointer_pos()
                    .is_some_and(|pos| pos.distance(*anchor) >= DRAW_MIN)
            {
                *crop = *before;
            }
            *gesture = None;
        }
    }

    fn drag_crop(&mut self, photo: Rect, image_size: [u32; 2], pos: Pos2) {
        let Some(EditSession {
            kind:
                EditKind::Crop {
                    crop,
                    gesture,
                    landscape,
                    ratio,
                    ..
                },
            ..
        }) = self.edit.as_mut()
        else {
            return;
        };
        let image = (f64::from(image_size[0]), f64::from(image_size[1]));
        let aspect = edit::ratio_aspect(*ratio, image_size[0], image_size[1], *landscape);
        let floor = edit::min_side(image_size[0], image_size[1]);
        match *gesture {
            Some(CropGesture::Move {
                start,
                crop: origin,
            }) => {
                let (dx, dy) = edit_ui::screen_delta(photo, image_size, pos - start);
                *crop = origin.translate(dx, dy, image.0, image.1);
            }
            Some(CropGesture::Resize { anchor, grab, .. }) => {
                let (px, py) = edit_ui::screen_to_image(photo, image_size, pos);
                let pointer = (px + grab.0, py + grab.1);
                *crop = edit::resize_from_anchor(anchor, pointer, aspect, image, floor);
            }
            // Nothing yet while it could still be a click.
            Some(CropGesture::Draw { anchor, before }) if pos.distance(anchor) < DRAW_MIN => {
                *crop = before;
            }
            // From the margin beside the photo the frame starts at the photo's nearest point.
            Some(CropGesture::Draw { anchor, .. }) => {
                let (ax, ay) = edit_ui::screen_to_image(photo, image_size, anchor);
                let anchor = (ax.clamp(0.0, image.0), ay.clamp(0.0, image.1));
                let pointer = edit_ui::screen_to_image(photo, image_size, pos);
                *crop = edit::resize_from_anchor(anchor, pointer, aspect, image, floor);
            }
            None => {}
        }
    }

    /// The angle the photo is drawn at while straightening.
    pub(super) fn straighten_angle(&self) -> Option<f64> {
        match &self.edit {
            Some(EditSession {
                kind: EditKind::Straighten { radians },
                ..
            }) => Some(*radians),
            _ => None,
        }
    }

    /// Grid or crop frame over the photo, with the banner that names the keys.
    pub(super) fn draw_edit_overlay(&self, ui: &egui::Ui, frame: &viewer::Frame) {
        let Some(session) = &self.edit else {
            return;
        };
        let photo = self.zoom.image_rect(frame);
        match &session.kind {
            EditKind::Straighten { radians } => {
                let painter = ui.painter().with_clip_rect(photo);
                edit_ui::grid(&painter, photo);
                edit_ui::banner(
                    ui.painter(),
                    frame.area,
                    &format!("{:+.2}°", radians.to_degrees()),
                    i18n::t().edit_hint_straighten,
                );
            }
            EditKind::Crop {
                ratio,
                landscape,
                image_size,
                crop,
                ..
            } => {
                let screen = edit_ui::crop_to_screen(photo, *image_size, *crop);
                edit_ui::crop_frame(ui.painter(), photo, screen);
                edit_ui::banner(
                    ui.painter(),
                    frame.area,
                    &crop_caption(*ratio, *landscape),
                    i18n::t().edit_hint_crop,
                );
            }
        }
    }
}
