//! The keyboard. `read_keys` turns one frame's input into a `KeyInput` (pure, tested);
//! `handle_keys` decides who gets the keys (cards, menus, help, an edit session) and carries
//! them out.

use std::time::Instant;

use eframe::egui::{self, Key, ViewportCommand};

use crate::metadata::{Label, Rating};
use crate::ui::icons::Panel;
use crate::ui::{palette, viewer};

use super::CernoApp;

/// `0` clears the stars, `1`–`5` set them.
const STAR_KEYS: [(Key, Rating); 6] = [
    (Key::Num0, Rating::Unrated),
    (Key::Num1, Rating::Stars(1)),
    (Key::Num2, Rating::Stars(2)),
    (Key::Num3, Rating::Stars(3)),
    (Key::Num4, Rating::Stars(4)),
    (Key::Num5, Rating::Stars(5)),
];

/// `6`–`9` set the colour label. Purple has no digit; it lives in the command palette.
const LABEL_KEYS: [(Key, Label); 4] = [
    (Key::Num6, Label::Red),
    (Key::Num7, Label::Yellow),
    (Key::Num8, Label::Green),
    (Key::Num9, Label::Blue),
];

/// Zoom step for `+`/`-`.
const ZOOM_STEP: f32 = 1.25;

/// What the keys of one frame ask for.
struct KeyInput {
    next: bool,
    prev: bool,
    first: bool,
    last: bool,
    rating: Option<Rating>,
    /// `Shift+0…5`: rate and move on.
    rate_and_next: Option<Rating>,
    /// `X` (toggles), `Shift+X` rejects and moves on.
    reject: bool,
    reject_and_next: bool,
    label: Option<Label>,
    /// `Shift+6…9`: set the colour and move on.
    label_and_next: Option<Label>,
    delete: bool,
    compare: bool,
    keep_left: bool,
    keep_right: bool,
    /// `Enter`: play the current video in the system's player.
    play: bool,
    /// `B`: the description tab (comment and keywords).
    describe: bool,
    /// `E`: edit the photo in the remembered program (or choose one).
    edit_elsewhere: bool,
    toggle_fullscreen: bool,
    escape: bool,
    toggle_toolbar: bool,
    toggle_filmstrip: bool,
    cycle_details: bool,
    help: bool,
    language: bool,
    palette: bool,
    /// `Ctrl+M`: the action menu in the filter bar.
    actions: bool,
    toggle_zoom: bool,
    zoom_in: bool,
    zoom_out: bool,
    open: bool,
    is_fullscreen: bool,
    straighten: bool,
    crop: bool,
    rotate_cw: bool,
    rotate_ccw: bool,
    /// `Ctrl+Z`: the kept original comes back.
    undo: bool,
}

/// A digit key, found by its place on the keyboard: the typed character depends on the layout
/// – with Shift (`!`, `"`, `§`, `=` …) and on AZERTY even without (`&`, `é`, `"`, `'`, `-` …).
/// Without a physical key (headless tests) the logical one counts. `shift`: only with Shift
/// alone, otherwise only without any modifier. Key repeats don't count.
fn digit_key<T: Copy>(events: &[egui::Event], keys: &[(Key, T)], shift: bool) -> Option<T> {
    events.iter().find_map(|event| match event {
        egui::Event::Key {
            key,
            physical_key,
            pressed: true,
            repeat: false,
            modifiers,
        } if (shift && modifiers.shift_only()) || (!shift && modifiers.is_none()) => {
            let place = physical_key.unwrap_or(*key);
            keys.iter()
                .find(|(k, _)| *k == place)
                .map(|(_, value)| *value)
        }
        _ => None,
    })
}

/// `Shift+0…5`: rate and move on.
fn shifted_digit(events: &[egui::Event]) -> Option<Rating> {
    digit_key(events, &STAR_KEYS, true)
}

fn shifted_label(events: &[egui::Event]) -> Option<Label> {
    digit_key(events, &LABEL_KEYS, true)
}

/// One frame's keys. Digits count by their place on the keyboard (see [`digit_key`]).
fn read_keys(i: &egui::InputState) -> KeyInput {
    let plain = i.modifiers.is_none();
    let rate_and_next = shifted_digit(&i.events);
    let rating = digit_key(&i.events, &STAR_KEYS, false);
    let label = digit_key(&i.events, &LABEL_KEYS, false);
    KeyInput {
        // Without Ctrl: Ctrl+Left/Right turn the photo instead.
        next: !i.modifiers.command
            && [Key::ArrowRight, Key::Space, Key::PageDown]
                .iter()
                .any(|k| i.key_pressed(*k)),
        prev: !i.modifiers.command
            && [Key::ArrowLeft, Key::Backspace, Key::PageUp]
                .iter()
                .any(|k| i.key_pressed(*k)),
        first: i.key_pressed(Key::Home),
        last: i.key_pressed(Key::End),
        rating,
        rate_and_next,
        reject: plain && i.key_pressed(Key::X),
        reject_and_next: i.modifiers.shift_only() && i.key_pressed(Key::X),
        label,
        label_and_next: shifted_label(&i.events),
        delete: plain && i.key_pressed(Key::Delete),
        compare: plain && i.key_pressed(Key::C),
        keep_left: plain && i.key_pressed(Key::A),
        keep_right: plain && i.key_pressed(Key::D),
        play: plain && i.key_pressed(Key::Enter),
        describe: plain && i.key_pressed(Key::B),
        edit_elsewhere: plain && i.key_pressed(Key::E),
        // F and F11 full screen; T (`toggle_toolbar`) is the filter bar.
        toggle_fullscreen: i.key_pressed(Key::F11) || (plain && i.key_pressed(Key::F)),
        escape: i.key_pressed(Key::Escape),
        toggle_toolbar: plain && i.key_pressed(Key::T),
        toggle_filmstrip: i.key_pressed(Key::F6),
        cycle_details: plain && i.key_pressed(Key::I),
        help: i.key_pressed(Key::F1)
            || i.key_pressed(Key::Questionmark)
            || (plain && i.key_pressed(Key::H)),
        language: i.modifiers.command && i.key_pressed(Key::L),
        palette: i.modifiers.command && i.key_pressed(Key::K),
        actions: i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::M),
        toggle_zoom: plain && i.key_pressed(Key::Z),
        // German layouts type "=" for Shift+0, which is "rate 0 and next" here.
        zoom_in: !i.modifiers.command
            && rate_and_next.is_none()
            && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)),
        // AZERTY types "-" on the 6 key, which is the red label here.
        zoom_out: !i.modifiers.command && label.is_none() && i.key_pressed(Key::Minus),
        open: i.modifiers.command && i.key_pressed(Key::O),
        is_fullscreen: i.viewport().fullscreen.unwrap_or(false),
        straighten: plain && i.key_pressed(Key::S),
        crop: plain && i.key_pressed(Key::R),
        rotate_cw: i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::ArrowRight),
        rotate_ccw: i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::ArrowLeft),
        undo: i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::Z),
    }
}

impl CernoApp {
    /// Carries out this frame's keys (see the help page for all).
    pub(super) fn handle_keys(&mut self, ctx: &egui::Context, frames: &[viewer::Frame]) {
        if let Some(path) =
            ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
        {
            self.open(ctx, &path);
        }
        let tabs = std::mem::take(&mut self.tab_presses);
        // The models card and a confirmation read their own Enter and Esc.
        if self.modal_open() {
            return;
        }
        // A comment or keyword is being typed: the keys belong to its field (`Esc` leaves it).
        // Otherwise `X` would reject, digits rate and `Space` move on while typing.
        if ctx.egui_wants_keyboard_input() {
            return;
        }

        let keys = ctx.input(read_keys);

        if keys.language {
            self.switch_language(ctx);
        }
        if keys.actions {
            if self.action_menu.is_some() {
                self.close_action_menu();
            } else {
                self.open_action_menu();
            }
            return;
        }
        // Both menus read their own arrows, Enter, letters and Esc (see `palette`).
        if self.action_menu.is_some() {
            if keys.palette {
                self.close_action_menu();
                self.palette = Some(palette::State::default());
            }
            return;
        }
        if self.palette.is_some() {
            if keys.palette {
                self.palette = None;
            }
            return;
        }
        if keys.palette {
            self.help_open = false;
            self.palette = Some(palette::State::default());
            return;
        }
        // The help page is modal: only closing it (and switching the language) works.
        if self.help_open {
            if keys.help || keys.escape {
                self.help_open = false;
            }
            return;
        }
        // Also on the start screen, which only shows the first keys.
        if keys.help {
            self.help_open = true;
            return;
        }
        if self.edit.is_some() {
            self.handle_edit_keys(ctx);
            return;
        }
        self.act_on_keys(ctx, &keys, &tabs, frames);
    }

    /// The keys that reach the photo: nothing modal is open.
    fn act_on_keys(
        &mut self,
        ctx: &egui::Context,
        keys: &KeyInput,
        tabs: &[bool],
        frames: &[viewer::Frame],
    ) {
        if keys.straighten {
            self.begin_straighten();
        }
        if keys.crop {
            self.begin_crop();
        }
        if keys.rotate_ccw {
            self.rotate_quarter(false);
        }
        if keys.rotate_cw {
            self.rotate_quarter(true);
        }
        if keys.undo {
            self.undo_edit();
        }
        if keys.open {
            self.pick_folder(ctx);
        }
        if keys.next {
            self.go_to(ctx, self.current.saturating_add(1), 1);
        }
        if keys.prev {
            self.go_to(ctx, self.current.saturating_sub(1), -1);
        }
        if keys.first {
            self.go_to(ctx, 0, 1);
        }
        if keys.last {
            self.go_to(ctx, usize::MAX, -1);
        }
        if let Some(stars) = keys.rating {
            self.set_rating(ctx, stars, self.auto_advance);
        }
        if let Some(stars) = keys.rate_and_next {
            self.set_rating(ctx, stars, true);
        }
        if keys.reject {
            self.toggle_reject(ctx, self.auto_advance);
        }
        if keys.reject_and_next {
            self.set_rating(ctx, Rating::Rejected, true);
        }
        if let Some(label) = keys.label {
            self.toggle_label(ctx, label, self.auto_advance);
        }
        if let Some(label) = keys.label_and_next {
            self.set_label(ctx, Some(label), true);
        }
        if keys.compare {
            self.toggle_compare(ctx);
        }
        if keys.keep_left {
            self.keep_left(ctx);
        }
        if keys.edit_elsewhere {
            self.edit_elsewhere();
        }
        if keys.keep_right {
            self.keep_right(ctx);
        }
        if keys.play {
            self.play_video();
        }
        if keys.describe {
            self.open_description(ctx);
        }
        if keys.delete {
            self.delete_current(ctx);
        }
        for &shift in tabs {
            if shift {
                self.toggle_all_panels();
            } else {
                self.toggle_panel(Panel::Right);
            }
        }
        if keys.toggle_toolbar {
            self.toggle_panel(Panel::Top);
        }
        if keys.toggle_filmstrip {
            self.toggle_panel(Panel::Bottom);
        }
        if keys.cycle_details {
            self.toggle_explanations();
        }
        // Zoom keys act on the photo under the mouse, otherwise on the current (right) one.
        let pointer = ctx.pointer_hover_pos();
        let hovered = frames
            .iter()
            .find(|f| pointer.is_some_and(|p| f.area.contains(p)))
            .or(frames.last());
        if let Some(frame) = hovered {
            let pointer = pointer.filter(|p| frame.area.contains(*p));
            let anchor = pointer.unwrap_or(frame.area.center());
            if keys.toggle_zoom {
                self.zoom.toggle(frame, pointer);
            }
            if keys.zoom_in {
                self.zoom.zoom_by(frame, ZOOM_STEP, anchor);
            }
            if keys.zoom_out {
                self.zoom.zoom_by(frame, 1.0 / ZOOM_STEP, anchor);
            }
        }
        if keys.toggle_fullscreen {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!keys.is_fullscreen));
        }
        if keys.escape {
            if self.deletions.countdown(Instant::now()).is_some() {
                self.undo_deletions(ctx);
            } else if self.zoom.is_zoomed() {
                self.zoom.scale = None;
            } else if self.pinned.is_some() {
                self.toggle_compare(ctx);
            } else if keys.is_fullscreen {
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
            } else {
                self.notice = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, Modifiers, RawInput};

    fn key(physical: Key, logical: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key: logical,
            physical_key: Some(physical),
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// The keys of one headless frame, read the way `handle_keys` reads them.
    fn read(events: Vec<Event>, modifiers: Modifiers) -> KeyInput {
        let ctx = egui::Context::default();
        let mut keys = None;
        let mut all = vec![Event::ModifiersChanged(modifiers)];
        all.extend(events);
        let mut output = ctx.run_ui(
            RawInput {
                events: all,
                ..Default::default()
            },
            |ui| keys = Some(ui.ctx().input(read_keys)),
        );
        output.textures_delta.clear();
        keys.expect("one frame")
    }

    #[test]
    fn azerty_six_is_red_and_does_not_zoom_out() {
        let plain = Modifiers::NONE;
        let keys = read(vec![key(Key::Num6, Key::Minus, plain)], plain);
        assert_eq!(keys.label, Some(Label::Red));
        assert!(!keys.zoom_out);
        // The minus key itself (numpad) still zooms out.
        let keys = read(vec![key(Key::Minus, Key::Minus, plain)], plain);
        assert!(keys.zoom_out);
        assert_eq!(keys.label, None);
    }

    #[test]
    fn german_shift_zero_rates_and_does_not_zoom_in() {
        let shift = Modifiers::SHIFT;
        let keys = read(vec![key(Key::Num0, Key::Equals, shift)], shift);
        assert_eq!(keys.rate_and_next, Some(Rating::Unrated));
        assert!(!keys.zoom_in);
    }

    #[test]
    fn ctrl_arrows_turn_the_photo_and_do_not_step() {
        let ctrl = Modifiers::COMMAND;
        let keys = read(vec![key(Key::ArrowRight, Key::ArrowRight, ctrl)], ctrl);
        assert!(keys.rotate_cw && !keys.next);
        let plain = Modifiers::NONE;
        let keys = read(vec![key(Key::ArrowRight, Key::ArrowRight, plain)], plain);
        assert!(keys.next && !keys.rotate_cw);
        let keys = read(vec![key(Key::Z, Key::Z, ctrl)], ctrl);
        assert!(keys.undo && !keys.toggle_zoom);
    }

    #[test]
    fn e_edits_elsewhere_only_without_modifiers() {
        let plain = Modifiers::NONE;
        assert!(read(vec![key(Key::E, Key::E, plain)], plain).edit_elsewhere);
        let ctrl = Modifiers::COMMAND;
        assert!(!read(vec![key(Key::E, Key::E, ctrl)], ctrl).edit_elsewhere);
    }

    #[test]
    fn shift_digits_rate_by_physical_key() {
        // German layout: Shift+3 types "§", Shift+0 types "=".
        let shift = Modifiers::SHIFT;
        assert_eq!(
            shifted_digit(&[key(Key::Num3, Key::Num3, shift)]),
            Some(Rating::Stars(3))
        );
        assert_eq!(
            shifted_digit(&[key(Key::Num0, Key::Equals, shift)]),
            Some(Rating::Unrated)
        );
        assert_eq!(
            shifted_digit(&[key(Key::Num3, Key::Num3, Modifiers::NONE)]),
            None
        );
        assert_eq!(shifted_digit(&[key(Key::Num7, Key::Slash, shift)]), None);
    }

    #[test]
    fn plain_digits_count_by_their_place_on_the_keyboard() {
        let plain = Modifiers::NONE;
        let stars = |event| digit_key(&[event], &STAR_KEYS, false);
        let label = |event| digit_key(&[event], &LABEL_KEYS, false);
        // French AZERTY: the 4 key types "'", the 6 key "-", the 3 key '"'.
        assert_eq!(
            stars(key(Key::Num4, Key::Quote, plain)),
            Some(Rating::Stars(4))
        );
        assert_eq!(
            stars(key(Key::Num3, Key::Quote, plain)),
            Some(Rating::Stars(3))
        );
        assert_eq!(label(key(Key::Num6, Key::Minus, plain)), Some(Label::Red));
        // QWERTZ / QWERTY and headless events without a physical key.
        assert_eq!(
            stars(key(Key::Num2, Key::Num2, plain)),
            Some(Rating::Stars(2))
        );
        let logical_only = Event::Key {
            key: Key::Num5,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: plain,
        };
        assert_eq!(stars(logical_only), Some(Rating::Stars(5)));
        // Shift, Ctrl and repeats are not plain digits.
        assert_eq!(stars(key(Key::Num4, Key::Num4, Modifiers::SHIFT)), None);
        assert_eq!(stars(key(Key::Num4, Key::Num4, Modifiers::COMMAND)), None);
        let repeat = Event::Key {
            key: Key::Num4,
            physical_key: Some(Key::Num4),
            pressed: true,
            repeat: true,
            modifiers: plain,
        };
        assert_eq!(stars(repeat), None);
    }
}
