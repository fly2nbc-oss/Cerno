//! Crop and straighten (`R`, one session since 1.16), quarter turns and `Ctrl+Z`: the
//! session, its keys and pointer, and handing the work to the writer.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use eframe::egui::{self, Event, Key, MouseWheelUnit, Pos2, Rect};

use crate::edit::{self, Ratio};
use crate::i18n;
use crate::loader::Lookup;
use crate::ui::{edit as edit_ui, viewer};

use super::CernoApp;
use super::gate::{Blocked, Change};
use super::notice::Notice;
use super::undo::Entry;

/// What `Ctrl+Z` undoes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Undo {
    /// The photos waiting to be deleted come back (like `Esc`).
    Deletion,
    /// The deleted photo shown goes back into its folder.
    Restore,
    /// The session's newest mark or edit (`undo::Journal`), on whichever photo it was.
    Journal,
    /// The kept original replaces the edited photo – an edit of an earlier session.
    Original,
}

/// A deletion that still counts down comes first: the photo just deleted is hidden already, so
/// the one on screen is its neighbour – `Ctrl+Z` must bring the deleted one back, never put the
/// neighbour's original over its edit. A deleted photo on screen comes back next, then the
/// session's marks and edits go back newest first, then an original kept in an earlier one.
fn undo_target(counting_down: bool, current_deleted: bool, journal: bool) -> Undo {
    if counting_down {
        Undo::Deletion
    } else if current_deleted {
        Undo::Restore
    } else if journal {
        Undo::Journal
    } else {
        Undo::Original
    }
}

/// Crop and straighten, quarter turns.
#[derive(Default)]
pub(super) struct Edits {
    /// Crop and straighten, while it is open. The saved zoom comes back on Enter or Esc.
    pub(super) session: Option<EditSession>,
    /// Encodes a confirmed edit. Joined on exit so the write is not lost.
    pub(super) thread: Option<std::thread::JoinHandle<()>>,
    /// A quarter turn or a re-encode is still in the writer.
    pub(super) busy: bool,
}

pub(super) struct EditSession {
    /// The photo being edited; the session ends if another one becomes current.
    pub(super) path: PathBuf,
    pub(super) zoom: viewer::Zoom,
    tool: Tool,
}

#[cfg(test)]
impl EditSession {
    /// A session on `path` (as if 400×300), without the checks `begin_edit` makes (tests).
    pub(super) fn editing(path: PathBuf) -> Self {
        Self {
            path,
            zoom: viewer::Zoom::default(),
            tool: Tool::new([400, 300]),
        }
    }
}

/// The angle and the frame together: the photo turns under the frame (scaled so the original
/// frame stays full), the frame lies over the turned photo, and `Enter` writes both at once –
/// one new JPEG instead of two.
struct Tool {
    /// Fine angle, ±45°.
    radians: f64,
    ratio: Ratio,
    landscape: bool,
    image_size: [u32; 2],
    /// In pixels of the turned photo, which keeps the original's size.
    crop: edit::Crop,
    gesture: Option<CropGesture>,
}

impl Tool {
    fn new(image_size: [u32; 2]) -> Self {
        let landscape = image_size[0] >= image_size[1];
        let aspect = edit::ratio_aspect(Ratio::Original, image_size[0], image_size[1], landscape);
        Self {
            radians: 0.0,
            ratio: Ratio::Original,
            landscape,
            image_size,
            crop: edit::Crop::max_centered(
                f64::from(image_size[0]),
                f64::from(image_size[1]),
                aspect,
            ),
            gesture: None,
        }
    }

    /// The part of the photo to keep, or `None` for all of it (or a frame too small to write).
    fn rect(&self) -> Option<edit::PixelRect> {
        let [w, h] = self.image_size;
        if self.crop.covers_image(f64::from(w), f64::from(h)) {
            return None;
        }
        let rect = edit::PixelRect::from_crop(self.crop, w, h);
        let short = rect.w.min(rect.h);
        (!rect.is_entire(w, h) && (short >= edit::MIN_CROP_SIDE || short >= w.min(h)))
            .then_some(rect)
    }

    fn turned(&self) -> Option<f64> {
        (self.radians.abs() >= 1e-8).then_some(self.radians)
    }
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

struct EditKeyInput {
    enter: bool,
    escape: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    /// `+` and `−`: the crop frame larger or smaller.
    grow: bool,
    shrink: bool,
    shift: bool,
    command: bool,
    flip: bool,
    cycle: bool,
    /// Wheel, `,` and `.`: the fine angle.
    turn: f64,
}

/// One keyboard step of the crop frame: 1 % of the photo's long side, with Shift one pixel
/// (finer, like Shift in straighten).
fn crop_step(image_size: [u32; 2], fine: bool) -> f64 {
    if fine {
        1.0
    } else {
        f64::from(image_size[0].max(image_size[1])) / 100.0
    }
}

/// `+` / `−` while cropping, held down too. By the key typed – and with Shift by where the key
/// sits: Shift turns them into `*` and `_` on German layouts, which egui has no name for, so it
/// reports the position instead (`]` and `/` on a US keyboard). Measured with posted keys on a
/// German layout on 2026-10-05.
fn size_keys(events: &[Event]) -> (bool, bool) {
    let (mut grow, mut shrink) = (false, false);
    for event in events {
        if let Event::Key {
            key,
            physical_key,
            pressed: true,
            modifiers,
            ..
        } = event
        {
            if modifiers.alt || modifiers.command {
                continue;
            }
            let shifted = |place: Key| modifiers.shift && *physical_key == Some(place);
            grow |= matches!(key, Key::Plus | Key::Equals) || shifted(Key::CloseBracket);
            shrink |= *key == Key::Minus || shifted(Key::Slash);
        }
    }
    (grow, shrink)
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

/// `,` turns anticlockwise, `.` clockwise, by one step (Shift finer). By the key's place too:
/// Shift turns them into `;` and `:` on German layouts.
fn turn_keys(events: &[Event]) -> f64 {
    let mut turn = 0.0;
    for event in events {
        if let Event::Key {
            key,
            physical_key,
            pressed: true,
            modifiers,
            ..
        } = event
        {
            if modifiers.alt || modifiers.command {
                continue;
            }
            let place = physical_key.unwrap_or(*key);
            let step = if modifiers.shift {
                edit::FINER_STEP
            } else {
                edit::FINE_STEP
            };
            if place == Key::Comma || *key == Key::Comma {
                turn -= step;
            } else if place == Key::Period || *key == Key::Period {
                turn += step;
            }
        }
    }
    turn
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
        if self.viewer.pinned.is_some() {
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

    /// `R`: crop and straighten the photo shown; again ends it, as `Esc` does.
    pub(super) fn begin_edit(&mut self) {
        if self.edits.session.is_some() {
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
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        let zoom = self.viewer.zoom;
        self.viewer.zoom = viewer::Zoom::default();
        self.edits.session = Some(EditSession {
            path,
            zoom,
            tool: Tool::new(image_size),
        });
    }

    pub(super) fn cancel_edit(&mut self) {
        if let Some(session) = self.edits.session.take() {
            self.viewer.zoom = session.zoom;
        }
    }

    fn confirm_edit(&mut self) {
        // Nothing may still be in the writer for this photo: the render reads the file now.
        if self.edits.busy {
            self.notice = Some(Notice::hint(Blocked::Writing.hint()));
            return;
        }
        let Some(session) = self.edits.session.take() else {
            return;
        };
        self.viewer.zoom = session.zoom;
        if self.view.get(self.current) != Some(&session.path) {
            self.notice = Some(Notice::hint(i18n::t().edit_cancelled));
            return;
        }
        let (radians, rect) = (session.tool.turned(), session.tool.rect());
        if radians.is_none() && rect.is_none() {
            return;
        }
        self.spawn_edit(session.path, radians, rect);
    }

    fn spawn_edit(&mut self, path: PathBuf, radians: Option<f64>, rect: Option<edit::PixelRect>) {
        self.edits.busy = true;
        self.notice = Some(Notice::working(i18n::t().edit_writing));
        let channel = self.writer.channel();
        let files = Arc::clone(&self.files);
        let thread = std::thread::Builder::new()
            .name("cerno-edit".into())
            .spawn(move || {
                // A panic would never report back, and `edit_busy` would block every edit.
                let result =
                    crate::decode::catch_panic(|| edit::render_edit(&path, radians, rect, &files));
                match result {
                    Ok(jpeg) => channel.replace_pixels(path, jpeg),
                    Err(err) => channel.fail(path, format!("{err:#}")),
                }
            });
        match thread {
            Ok(thread) => self.edits.thread = Some(thread),
            Err(err) => {
                self.edits.busy = false;
                self.notice = Some(Notice::error((i18n::t().edit_failed)(&err.to_string())));
            }
        }
    }

    /// In a crop and straighten session it waits for `Enter` or `Esc`: the render would read
    /// the file before or after the turn, and one of the two would be lost.
    pub(super) fn rotate_quarter(&mut self, clockwise: bool) {
        if self.viewer.pinned.is_some() {
            return;
        }
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        if !self.allowed(Change::Rewrite, Some(&path)) {
            return;
        }
        self.edits.busy = true;
        self.writer.rotate_quarter(path, clockwise);
    }

    /// `Ctrl+Z`: the first original of the current photo goes back into the file (the first
    /// straighten, crop or quarter turn keeps it in `.originals`, for good). On a deleted photo
    /// it undoes the deletion: the photo goes back into its folder.
    /// `Ctrl+Z` and the menu's *Undo*: a pending deletion, else a deleted photo, else an edit.
    pub(super) fn undo(&mut self, ctx: &egui::Context) {
        let counting_down = self.deletions.countdown(Instant::now()).is_some();
        let deleted = self
            .view
            .get(self.current)
            .is_some_and(|path| self.is_deleted(path));
        match undo_target(
            counting_down,
            deleted,
            self.marks.journal.newest().is_some(),
        ) {
            Undo::Deletion => self.undo_deletions(ctx),
            Undo::Restore => self.restore_current(),
            Undo::Journal => self.undo_newest(ctx),
            Undo::Original => self.undo_edit(),
        }
    }

    fn undo_edit(&mut self) {
        if self.viewer.pinned.is_some() {
            return;
        }
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        if !self.allowed(Change::Rewrite, Some(&path)) {
            return;
        }
        self.restore_original(path);
    }

    /// The first original of `path` goes back into the file, when one is kept.
    pub(super) fn restore_original(&mut self, path: PathBuf) {
        match crate::originals::original(&self.db, &path) {
            Ok(Some(_)) => {
                self.edits.busy = true;
                self.writer.restore(path);
            }
            Ok(None) => self.notice = Some(Notice::hint(i18n::t().undo_nothing)),
            Err(err) => self.notice = Some(Notice::error(format!("{err:#}"))),
        }
    }

    pub(super) fn poll_edits(&mut self) {
        if let Some(thread) = self.edits.thread.take_if(|thread| thread.is_finished()) {
            let _ = thread.join();
        }
        let writing = i18n::t().edit_writing;
        for outcome in self.writer.poll_edits() {
            if outcome.elsewhere {
                self.reload_saved(&outcome.path);
                continue;
            }
            self.edits.busy = false;
            match outcome.error {
                Some(err) => self.notice = Some(Notice::error((i18n::t().edit_failed)(&err))),
                None => {
                    self.refresh_edited(&outcome.path);
                    if outcome.restored {
                        // The first original is back: every edit of the session went with it.
                        self.marks.journal.drop_edits(&outcome.path);
                        self.notice = Some(Notice::hint(i18n::t().undo_done));
                        continue;
                    }
                    self.marks.journal.push(Entry::Edit {
                        path: outcome.path.clone(),
                    });
                    if outcome.reencoded {
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
        self.forget_faces(path);
    }

    pub(super) fn handle_edit_keys(&mut self, ctx: &egui::Context) {
        let input = ctx.input(|input| {
            let (grow, shrink) = size_keys(&input.events);
            EditKeyInput {
                grow,
                shrink,
                enter: input.key_pressed(Key::Enter),
                escape: input.key_pressed(Key::Escape),
                left: input.key_pressed(Key::ArrowLeft),
                right: input.key_pressed(Key::ArrowRight),
                up: input.key_pressed(Key::ArrowUp),
                down: input.key_pressed(Key::ArrowDown),
                shift: input.modifiers.shift,
                command: input.modifiers.command,
                flip: input.modifiers.is_none() && input.key_pressed(Key::X),
                cycle: input.modifiers.is_none() && input.key_pressed(Key::A),
                turn: wheel_rotation(&input.events) + turn_keys(&input.events),
            }
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
        let Some(session) = self.edits.session.as_mut() else {
            return;
        };
        let Tool {
            radians,
            ratio,
            landscape,
            image_size,
            crop,
            gesture,
        } = &mut session.tool;
        if !input.command && input.turn != 0.0 {
            *radians = edit::clamp_angle(*radians + input.turn);
        }
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
        // The keyboard instead of the mouse: arrows slide the frame, + and − resize it about
        // its centre; Ctrl+arrows stay the quarter turns.
        if !input.command && gesture.is_none() {
            let step = crop_step(*image_size, input.shift);
            let (w, h) = (f64::from(image_size[0]), f64::from(image_size[1]));
            let dx = f64::from(i8::from(input.right) - i8::from(input.left)) * step;
            let dy = f64::from(i8::from(input.down) - i8::from(input.up)) * step;
            if dx != 0.0 || dy != 0.0 {
                *crop = crop.translate(dx, dy, w, h);
            }
            let resize = f64::from(i8::from(input.grow) - i8::from(input.shrink)) * step;
            if resize != 0.0 {
                *crop = crop.resize_about_centre(
                    resize,
                    w,
                    h,
                    edit::min_side(image_size[0], image_size[1]),
                );
            }
        }
    }

    pub(super) fn handle_edit_pointer(&mut self, ui: &egui::Ui, frame: &viewer::Frame) {
        let Some(session) = self.edits.session.as_ref() else {
            return;
        };
        let (image_size, crop, active) = (
            session.tool.image_size,
            session.tool.crop,
            session.tool.gesture.as_ref().map(CropGesture::kind),
        );
        let response = edit_ui::pointer_area(ui, frame.area);
        let photo = self.viewer.zoom.image_rect(frame);
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
            if let Some(session) = self.edits.session.as_mut() {
                session.tool.gesture = Some(gesture);
            }
        }
        if response.dragged()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.drag_crop(photo, image_size, pos);
        }
        if response.drag_stopped()
            && let Some(EditSession {
                tool: Tool { gesture, crop, .. },
                ..
            }) = self.edits.session.as_mut()
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
            tool:
                Tool {
                    crop,
                    gesture,
                    landscape,
                    ratio,
                    ..
                },
            ..
        }) = self.edits.session.as_mut()
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

    /// The angle the photo is drawn at while it is being straightened.
    pub(super) fn straighten_angle(&self) -> Option<f64> {
        self.edits
            .session
            .as_ref()
            .and_then(|session| session.tool.turned())
    }

    /// The crop frame over the turned photo, a fine grid inside it while the photo is turned,
    /// and the banner with the angle, the ratio and the keys.
    pub(super) fn draw_edit_overlay(&self, ui: &egui::Ui, frame: &viewer::Frame) {
        let Some(session) = &self.edits.session else {
            return;
        };
        let tool = &session.tool;
        let photo = self.viewer.zoom.image_rect(frame);
        let screen = edit_ui::crop_to_screen(photo, tool.image_size, tool.crop);
        if tool.turned().is_some() {
            edit_ui::grid(&ui.painter().with_clip_rect(screen), screen);
        }
        edit_ui::crop_frame(ui.painter(), photo, screen);
        edit_ui::banner(
            ui.painter(),
            frame.area,
            &format!(
                "{:+.2}° · {}",
                tool.radians.to_degrees(),
                crop_caption(tool.ratio, tool.landscape)
            ),
            i18n::t().edit_hint_crop,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key, physical: Option<Key>, modifiers: egui::Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: physical,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn ctrl_z_brings_a_pending_deletion_back_first() {
        assert_eq!(undo_target(true, false, true), Undo::Deletion);
        assert_eq!(undo_target(true, true, true), Undo::Deletion);
        assert_eq!(undo_target(false, true, true), Undo::Restore);
        assert_eq!(undo_target(false, false, true), Undo::Journal);
        assert_eq!(undo_target(false, false, false), Undo::Original);
    }

    /// `+` and `−` by the key typed, or by the key itself where Shift types another sign;
    /// with Ctrl or Alt they are not the crop's.
    #[test]
    fn plus_and_minus_size_the_crop_frame() {
        let (plain, shift) = (egui::Modifiers::NONE, egui::Modifiers::SHIFT);
        assert_eq!(size_keys(&[press(Key::Plus, None, plain)]), (true, false));
        assert_eq!(
            size_keys(&[press(Key::Equals, Some(Key::Equals), shift)]),
            (true, false)
        );
        // German: Shift+`+` and Shift+`−` arrive by their place, `]` and `/` on US keys.
        assert_eq!(
            size_keys(&[press(Key::CloseBracket, Some(Key::CloseBracket), shift)]),
            (true, false)
        );
        assert_eq!(
            size_keys(&[press(Key::Slash, Some(Key::Slash), shift)]),
            (false, true)
        );
        assert_eq!(
            size_keys(&[press(Key::CloseBracket, Some(Key::CloseBracket), plain)]),
            (false, false),
            "a plain `]` is no plus"
        );
        assert_eq!(size_keys(&[press(Key::Minus, None, plain)]), (false, true));
        let ctrl = press(Key::Plus, None, egui::Modifiers::COMMAND);
        assert_eq!(size_keys(&[ctrl]), (false, false));
        assert_eq!(crop_step([4000, 3000], false), 40.0);
        assert_eq!(crop_step([4000, 3000], true), 1.0);
    }

    /// `,` and `.` turn by one step, with Shift a finer one – also where Shift types `;`
    /// (German); with Ctrl or Alt they don't turn.
    #[test]
    fn comma_and_period_turn_the_photo() {
        let (plain, shift) = (egui::Modifiers::NONE, egui::Modifiers::SHIFT);
        assert_eq!(
            turn_keys(&[press(Key::Period, None, plain)]),
            edit::FINE_STEP
        );
        assert_eq!(
            turn_keys(&[press(Key::Comma, None, plain)]),
            -edit::FINE_STEP
        );
        assert_eq!(
            turn_keys(&[press(Key::Semicolon, Some(Key::Comma), shift)]),
            -edit::FINER_STEP
        );
        let ctrl = press(Key::Period, None, egui::Modifiers::COMMAND);
        assert_eq!(turn_keys(&[ctrl]), 0.0);
    }

    /// Nothing to write until the photo is turned or the frame is smaller than the photo.
    #[test]
    fn the_tool_writes_only_what_changed() {
        let mut tool = Tool::new([4000, 3000]);
        assert_eq!((tool.turned(), tool.rect()), (None, None));
        tool.radians = edit::FINE_STEP;
        assert!(tool.turned().is_some() && tool.rect().is_none());
        tool.crop = tool.crop.resize_about_centre(-400.0, 4000.0, 3000.0, 32.0);
        assert!(tool.rect().is_some());
    }
}
