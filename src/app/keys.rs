//! The keyboard. `read_keys` turns one frame's input into a `KeyInput` (pure, tested);
//! `handle_keys` decides who gets the keys (cards, menus, help, an edit session) and carries
//! them out.

use eframe::egui::{self, Key};

use crate::metadata::{Label, Rating};
use crate::ui::icons::Panel;
use crate::ui::viewer;

use super::CernoApp;
use super::command::{Advance, Command, Source, ZoomKeys};
use super::layer::Layer;

/// `0` clears the stars, `1`–`5` set them.
const STAR_KEYS: [(Key, Rating); 6] = [
    (Key::Num0, Rating::Unrated),
    (Key::Num1, Rating::Stars(1)),
    (Key::Num2, Rating::Stars(2)),
    (Key::Num3, Rating::Stars(3)),
    (Key::Num4, Rating::Stars(4)),
    (Key::Num5, Rating::Stars(5)),
];

/// `6`–`9` set the colour label. Purple has no digit; it lives in the menu bar.
const LABEL_KEYS: [(Key, Label); 4] = [
    (Key::Num6, Label::Red),
    (Key::Num7, Label::Yellow),
    (Key::Num8, Label::Green),
    (Key::Num9, Label::Blue),
];

/// What the keys of one frame ask for.
#[cfg_attr(test, derive(Debug, Default, PartialEq))]
struct KeyInput {
    next: bool,
    prev: bool,
    /// Page Down / Page Up: one photo on, in the grid one screen.
    page_down: bool,
    page_up: bool,
    /// `↓`/`↑`: a row in the grid and the four-up view.
    down: bool,
    up: bool,
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
    /// `Shift+C`: the four-up view.
    quad: bool,
    keep_left: bool,
    keep_right: bool,
    /// `Enter`: in the grid, open the photo; on the description tab, the keyword field.
    play: bool,
    /// Plain `Space` (with repeats; on a video only a fresh press plays or pauses).
    space: bool,
    /// The video keys (fresh presses): `Space`, `Alt+←`/`Alt+→`, `,`/`.`, `↑`/`↓`.
    video: super::video::VideoKeys,
    /// `G`: every face over the photo. (The details panel's tabs are `Ctrl+Tab`, taken in
    /// `raw_input_hook`.)
    face_grid: bool,
    /// `E`: edit the photo in the remembered program (or choose one).
    edit_elsewhere: bool,
    /// `O`: the next check overlay (sharp edges, clipping, off).
    overlay: bool,
    /// `M`: only photos like this one, or all again.
    similar: bool,
    toggle_fullscreen: bool,
    escape: bool,
    toggle_toolbar: bool,
    toggle_filmstrip: bool,
    /// `F7`: the grid.
    toggle_grid: bool,
    help: bool,
    language: bool,
    /// `F10`: the menu bar.
    toggle_side_bar: bool,
    toggle_zoom: bool,
    /// `+`/`−`, with or without Ctrl.
    zoom_in: bool,
    zoom_out: bool,
    /// `Ctrl+0`: the whole photo.
    zoom_fit: bool,
    /// `Ctrl+1`: 100 %.
    zoom_actual: bool,
    open: bool,
    /// `Ctrl+U`: subfolders on or off.
    subfolders: bool,
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

/// `Ctrl` and a digit, by the digit's place on the keyboard (see [`digit_key`]).
fn ctrl_digit(events: &[egui::Event], digit: Key) -> bool {
    events.iter().any(|event| {
        matches!(event, egui::Event::Key { key, physical_key, pressed: true, repeat: false, modifiers }
            if modifiers.command_only() && physical_key.unwrap_or(*key) == digit)
    })
}

/// `Ctrl+Plus`, `Ctrl+Minus` and `Ctrl+0` zoom the photo: egui would otherwise take them
/// (and the key events) for the size of the whole interface.
pub(super) fn keep_zoom_keys(ctx: &egui::Context) {
    ctx.options_mut(|options| options.zoom_with_keyboard = false);
}

/// `key` pressed now without modifiers – not a repeat of a held key.
fn fresh(events: &[egui::Event], key: Key) -> bool {
    fresh_with(events, key, egui::Modifiers::NONE)
}

/// `key` pressed now with exactly `modifiers` – not a repeat of a held key.
fn fresh_with(events: &[egui::Event], key: Key, with: egui::Modifiers) -> bool {
    events.iter().any(|event| {
        matches!(event, egui::Event::Key { key: k, pressed: true, repeat: false, modifiers, .. }
            if *k == key && *modifiers == with)
    })
}

/// −1, 0 or +1 from a pair of keys.
fn axis(minus: bool, plus: bool) -> i8 {
    i8::from(plus) - i8::from(minus)
}

/// One frame's keys. Digits count by their place on the keyboard (see [`digit_key`]).
fn read_keys(i: &egui::InputState) -> KeyInput {
    let plain = i.modifiers.is_none();
    let rate_and_next = shifted_digit(&i.events);
    let rating = digit_key(&i.events, &STAR_KEYS, false);
    let label = digit_key(&i.events, &LABEL_KEYS, false);
    KeyInput {
        // Without Ctrl: Ctrl+Left/Right turn the photo instead. Plain Space is read on its own
        // (`space`): on a video it plays and pauses; Shift+Space always moves on.
        // Alt+Left/Right jump in a video.
        next: !i.modifiers.command
            && !i.modifiers.alt
            && (i.key_pressed(Key::ArrowRight)
                || (i.modifiers.shift_only() && i.key_pressed(Key::Space))),
        prev: !i.modifiers.command
            && !i.modifiers.alt
            && [Key::ArrowLeft, Key::Backspace]
                .iter()
                .any(|k| i.key_pressed(*k)),
        page_down: !i.modifiers.command && i.key_pressed(Key::PageDown),
        page_up: !i.modifiers.command && i.key_pressed(Key::PageUp),
        down: !i.modifiers.command && i.key_pressed(Key::ArrowDown),
        up: !i.modifiers.command && i.key_pressed(Key::ArrowUp),
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
        quad: i.modifiers.shift_only() && i.key_pressed(Key::C),
        keep_left: plain && i.key_pressed(Key::A),
        keep_right: plain && i.key_pressed(Key::D),
        play: plain && i.key_pressed(Key::Enter),
        space: plain && i.key_pressed(Key::Space),
        video: super::video::VideoKeys {
            toggle: fresh(&i.events, Key::Space),
            jump: axis(
                fresh_with(&i.events, Key::ArrowLeft, egui::Modifiers::ALT),
                fresh_with(&i.events, Key::ArrowRight, egui::Modifiers::ALT),
            ),
            step: axis(fresh(&i.events, Key::Comma), fresh(&i.events, Key::Period)),
            volume: axis(
                plain && i.key_pressed(Key::ArrowDown),
                plain && i.key_pressed(Key::ArrowUp),
            ),
        },
        face_grid: plain && i.key_pressed(Key::G),
        edit_elsewhere: plain && i.key_pressed(Key::E),
        // Ctrl+O opens a folder.
        overlay: plain && i.key_pressed(Key::O),
        similar: plain && i.key_pressed(Key::M),
        // F and F11 full screen; T (`toggle_toolbar`) is the filter bar.
        toggle_fullscreen: i.key_pressed(Key::F11) || (plain && i.key_pressed(Key::F)),
        escape: i.key_pressed(Key::Escape),
        toggle_toolbar: plain && i.key_pressed(Key::T),
        toggle_filmstrip: i.key_pressed(Key::F6),
        toggle_grid: i.key_pressed(Key::F7),
        help: i.key_pressed(Key::F1)
            || i.key_pressed(Key::Questionmark)
            || (plain && i.key_pressed(Key::H)),
        language: i.modifiers.command && i.key_pressed(Key::L),
        toggle_side_bar: i.key_pressed(Key::F10),
        toggle_zoom: plain && i.key_pressed(Key::Z),
        // German layouts type "=" for Shift+0, which is "rate 0 and next" here.
        zoom_in: !i.modifiers.alt
            && rate_and_next.is_none()
            && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)),
        // AZERTY types "-" on the 6 key, which is the red label here.
        zoom_out: !i.modifiers.alt && label.is_none() && i.key_pressed(Key::Minus),
        zoom_fit: ctrl_digit(&i.events, Key::Num0),
        zoom_actual: ctrl_digit(&i.events, Key::Num1),
        open: i.modifiers.command && i.key_pressed(Key::O),
        subfolders: i.modifiers.command_only() && i.key_pressed(Key::U),
        straighten: plain && i.key_pressed(Key::S),
        crop: plain && i.key_pressed(Key::R),
        rotate_cw: i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::ArrowRight),
        rotate_ccw: i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::ArrowLeft),
        undo: i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::Z),
    }
}

impl KeyInput {
    /// The commands of one frame's keys, in the order they are carried out: the grid and the
    /// edits, the video keys, moving, marks, compare mode, the bars, the overlay, the zoom,
    /// full screen and `Esc` last. `tabs` are the `Tab` presses (`true` with Shift).
    fn commands(&self, tabs: &[bool]) -> Vec<Command> {
        let mut out = Vec::new();
        let mut push = |on: bool, command: Command| {
            if on {
                out.push(command);
            }
        };
        push(self.toggle_grid, Command::ToggleGrid);
        push(self.straighten, Command::Straighten);
        push(self.crop, Command::Crop);
        push(self.rotate_ccw, Command::Rotate(false));
        push(self.rotate_cw, Command::Rotate(true));
        push(self.undo, Command::Undo);
        push(self.open, Command::Open);
        push(self.subfolders, Command::Subfolders);
        // Every frame: whether the current photo is a video is looked at here, as the zoom
        // keys' hint needs it.
        push(
            true,
            Command::Video {
                keys: self.video,
                space: self.space,
            },
        );
        push(self.next, Command::Next);
        push(self.prev, Command::Prev);
        push(self.page_down, Command::PageDown);
        push(self.page_up, Command::PageUp);
        push(self.down || self.up, Command::Row { down: self.down });
        push(self.first, Command::First);
        push(self.last, Command::Last);
        if let Some(stars) = self.rating {
            push(true, Command::SetRating(stars, Advance::Setting));
        }
        if let Some(stars) = self.rate_and_next {
            push(true, Command::SetRating(stars, Advance::Yes));
        }
        push(self.reject, Command::ToggleReject(Advance::Setting));
        push(
            self.reject_and_next,
            Command::SetRating(Rating::Rejected, Advance::Yes),
        );
        if let Some(label) = self.label {
            push(true, Command::ToggleLabel(label, Advance::Setting));
        }
        if let Some(label) = self.label_and_next {
            push(true, Command::SetLabel(Some(label), Advance::Yes));
        }
        push(self.compare, Command::Compare);
        push(self.quad, Command::Quad);
        push(self.keep_left, Command::KeepLeft);
        push(self.edit_elsewhere, Command::EditElsewhere);
        push(self.similar, Command::Similar);
        push(self.keep_right, Command::KeepRight);
        push(self.play, Command::Enter);
        push(self.face_grid, Command::FaceGrid);
        push(self.delete, Command::DeleteCurrent);
        for &shift in tabs {
            push(
                true,
                if shift {
                    Command::AllPanels
                } else {
                    Command::TogglePanel(Panel::Right)
                },
            );
        }
        push(self.toggle_toolbar, Command::TogglePanel(Panel::Top));
        push(self.toggle_filmstrip, Command::TogglePanel(Panel::Bottom));
        push(self.toggle_side_bar, Command::TogglePanel(Panel::Left));
        push(self.overlay, Command::NextOverlay);
        let zoom = ZoomKeys {
            toggle: self.toggle_zoom,
            zoom_in: self.zoom_in,
            zoom_out: self.zoom_out,
            fit: self.zoom_fit,
            actual: self.zoom_actual,
        };
        push(zoom != ZoomKeys::default(), Command::Zoom(zoom));
        push(self.toggle_fullscreen, Command::Fullscreen);
        push(self.escape, Command::Escape);
        out
    }
}

/// Keys `raw_input_hook` took out of egui's input before egui could move the focus with them;
/// `handle_keys` reads them.
#[derive(Default)]
pub(super) struct Pressed {
    /// `Tab` presses (`true` = with Shift).
    pub(super) tabs: Vec<bool>,
    /// `Ctrl+Tab` presses (`true` = with Shift: backwards): the details panel's tabs.
    pub(super) details_cycles: Vec<bool>,
}

impl CernoApp {
    /// Carries out this frame's keys (see the help page for all).
    pub(super) fn handle_keys(&mut self, ctx: &egui::Context, frames: &[viewer::Frame]) {
        if let Some(path) =
            ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
        {
            self.open(ctx, &path);
        }
        let tabs = std::mem::take(&mut self.pressed.tabs);
        // The faces grid closes with `G` too – read here, before the grid draws: it would see
        // the press that opened it and close in the same frame.
        if self.viewer.faces.grid_open
            && ctx.input(|i| i.modifiers.is_none() && i.key_pressed(Key::G))
        {
            self.viewer.faces.grid_open = false;
            return;
        }
        // The models card, a confirmation and the faces grid read their own keys.
        if self.modal_open() {
            self.pressed.details_cycles.clear();
            return;
        }
        // `Ctrl+Tab` also leaves a field of the description tab for the next tab.
        for backwards in std::mem::take(&mut self.pressed.details_cycles) {
            self.cycle_details_tab(backwards);
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
        // An open list reads its own arrows, Enter, letters and Esc (`palette`).
        if self.layer.is_list() {
            return;
        }
        // The help page is modal: only closing it, switching its page (←/→) and the language
        // work.
        if let Layer::Help(page) = self.layer {
            if keys.help || keys.escape {
                self.layer = Layer::None;
            } else if keys.next {
                self.layer = Layer::Help(page.next());
            } else if keys.prev {
                self.layer = Layer::Help(page.prev());
            }
            return;
        }
        // Also on the start screen, which only shows the first keys.
        if keys.help {
            self.open_help();
            return;
        }
        if self.edits.session.is_some() {
            self.handle_edit_keys(ctx);
            return;
        }
        self.execute_all(ctx, &keys.commands(&tabs), Source::Keys, frames);
    }
}

/// What can be open when `Esc` comes.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Escapable {
    pub(super) countdown: bool,
    pub(super) zoomed: bool,
    pub(super) grid: bool,
    pub(super) quad: bool,
    pub(super) compare: bool,
    pub(super) fullscreen: bool,
}

/// What `Esc` ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Escape {
    Deletions,
    Zoom,
    Quad,
    Compare,
    Grid,
    Fullscreen,
    Notice,
}

/// The first of: a deletion counting down (brought back), the zoom – not while the grid hides
/// the photo, its zoom is left for when it shows again –, the four-up view, compare mode, the
/// grid, full screen; else the notice goes.
pub(super) fn escape_target(open: Escapable) -> Escape {
    if open.countdown {
        Escape::Deletions
    } else if open.zoomed && !open.grid {
        Escape::Zoom
    } else if open.quad {
        Escape::Quad
    } else if open.compare {
        Escape::Compare
    } else if open.grid {
        Escape::Grid
    } else if open.fullscreen {
        Escape::Fullscreen
    } else {
        Escape::Notice
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::video::VideoKeys;

    /// The keys of one frame are carried out in a fixed order: the grid and the edits, the
    /// video keys (every frame), moving, marks, compare mode, the bars, the overlay, the zoom,
    /// full screen, `Esc`.
    #[test]
    fn commands_come_in_a_fixed_order() {
        let keys = KeyInput {
            escape: true,
            toggle_zoom: true,
            overlay: true,
            toggle_toolbar: true,
            delete: true,
            compare: true,
            rating: Some(Rating::Stars(3)),
            next: true,
            undo: true,
            toggle_grid: true,
            ..KeyInput::default()
        };
        let video = Command::Video {
            keys: VideoKeys::default(),
            space: false,
        };
        assert_eq!(
            keys.commands(&[true]),
            vec![
                Command::ToggleGrid,
                Command::Undo,
                video,
                Command::Next,
                Command::SetRating(Rating::Stars(3), Advance::Setting),
                Command::Compare,
                Command::DeleteCurrent,
                Command::AllPanels,
                Command::TogglePanel(Panel::Top),
                Command::NextOverlay,
                Command::Zoom(ZoomKeys {
                    toggle: true,
                    ..ZoomKeys::default()
                }),
                Command::Escape,
            ]
        );
        assert_eq!(KeyInput::default().commands(&[]), vec![video]);
    }

    /// `↓` and `↑` together are one step down, as before.
    #[test]
    fn both_rows_at_once_step_down() {
        let keys = KeyInput {
            down: true,
            up: true,
            ..KeyInput::default()
        };
        assert!(keys.commands(&[]).contains(&Command::Row { down: true }));
    }

    /// `Esc` ends one thing at a time, in this order; the grid keeps a zoom it hides.
    #[test]
    fn escape_ends_one_thing_at_a_time() {
        let all = Escapable {
            countdown: true,
            zoomed: true,
            grid: false,
            quad: true,
            compare: true,
            fullscreen: true,
        };
        assert_eq!(escape_target(all), Escape::Deletions);
        let open = Escapable {
            countdown: false,
            ..all
        };
        assert_eq!(escape_target(open), Escape::Zoom);
        assert_eq!(
            escape_target(Escapable {
                grid: true,
                quad: false,
                compare: false,
                ..open
            }),
            Escape::Grid
        );
        let open = Escapable {
            zoomed: false,
            ..open
        };
        assert_eq!(escape_target(open), Escape::Quad);
        let open = Escapable {
            quad: false,
            ..open
        };
        assert_eq!(escape_target(open), Escape::Compare);
        let open = Escapable {
            compare: false,
            ..open
        };
        assert_eq!(escape_target(open), Escape::Fullscreen);
        assert_eq!(escape_target(Escapable::default()), Escape::Notice);
    }
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
        keep_zoom_keys(&ctx);
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

    /// One key cap of the English help as key presses: `Ctrl+←`, `1 – 5`, `Shift+…` (every
    /// digit and `X` with Shift). The mouse gives none, and neither does `Tab` – it is taken in
    /// `raw_input_hook`, before `read_keys`.
    fn presses(cap: &str) -> Vec<(Key, Modifiers)> {
        let (modifiers, name) = [
            ("Ctrl+", Modifiers::COMMAND),
            ("Shift+", Modifiers::SHIFT),
            ("Alt+", Modifiers::ALT),
        ]
        .into_iter()
        .find_map(|(prefix, modifiers)| cap.strip_prefix(prefix).map(|rest| (modifiers, rest)))
        .unwrap_or((Modifiers::NONE, cap));
        let digits = |from: u8, to: u8| -> Vec<Key> {
            (from..=to)
                .filter_map(|digit| Key::from_name(&digit.to_string()))
                .collect()
        };
        let keys = match name {
            "Mouse wheel" | "Drag" | "Double-click" | "Tab" => Vec::new(),
            "…" => [digits(0, 9), vec![Key::X]].concat(),
            "1 – 5" => digits(1, 5),
            "6 – 9" => digits(6, 9),
            "→" => vec![Key::ArrowRight],
            "←" => vec![Key::ArrowLeft],
            "↑" => vec![Key::ArrowUp],
            "↓" => vec![Key::ArrowDown],
            "PgDn" => vec![Key::PageDown],
            "PgUp" => vec![Key::PageUp],
            "Del" => vec![Key::Delete],
            other => vec![Key::from_name(other).unwrap_or_else(|| panic!("no key {other}"))],
        };
        keys.into_iter().map(|key| (key, modifiers)).collect()
    }

    /// Every key the help names does something: `read_keys` answers each of them.
    #[test]
    fn every_key_in_the_help_is_read() {
        let t = crate::i18n::Lang::En.texts();
        let rows = [
            &t.help_browse[..],
            &t.help_rate,
            &t.help_cull,
            &t.help_video,
            &t.help_view,
            &t.help_panels,
            &t.help_edit,
            &t.help_more,
            &t.welcome_keys,
        ];
        let mut checked = 0;
        for (caps, _) in rows.into_iter().flatten() {
            for cap in caps.split(", ") {
                for (pressed, modifiers) in presses(cap) {
                    let input = read(vec![key(pressed, pressed, modifiers)], modifiers);
                    assert_ne!(
                        input,
                        KeyInput::default(),
                        "{cap} ({pressed:?}) does nothing"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 50, "only {checked} keys");
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

    /// Plain Space plays and pauses a video (a held key not again) and moves on elsewhere;
    /// Shift+Space always moves on. Alt+←/→, `,`/`.` and ↑/↓ are the video's; Enter is not.
    #[test]
    fn space_and_the_video_keys() {
        let plain = Modifiers::NONE;
        let keys = read(vec![key(Key::Space, Key::Space, plain)], plain);
        assert!(keys.space && keys.video.toggle && !keys.next);
        let shift = Modifiers::SHIFT;
        let keys = read(vec![key(Key::Space, Key::Space, shift)], shift);
        assert!(keys.next && !keys.space && !keys.video.toggle);
        // Held down: egui marks the second press of a key still down as a repeat.
        let ctx = egui::Context::default();
        let frame = |events: Vec<Event>| {
            let mut keys = None;
            let mut output = ctx.run_ui(
                RawInput {
                    events,
                    ..Default::default()
                },
                |ui| keys = Some(ui.ctx().input(read_keys)),
            );
            output.textures_delta.clear();
            keys.expect("one frame")
        };
        assert!(frame(vec![key(Key::Space, Key::Space, plain)]).video.toggle);
        let held = frame(vec![key(Key::Space, Key::Space, plain)]);
        assert!(
            held.space && !held.video.toggle,
            "a held Space does not toggle"
        );
        assert!(
            !read(vec![key(Key::Enter, Key::Enter, plain)], plain)
                .video
                .toggle,
            "Enter does not play"
        );
        let alt = Modifiers::ALT;
        let back = read(vec![key(Key::ArrowLeft, Key::ArrowLeft, alt)], alt);
        assert!(
            back.video.jump == -1 && !back.prev,
            "Alt+← jumps, it does not step"
        );
        let on = read(vec![key(Key::ArrowRight, Key::ArrowRight, alt)], alt);
        assert!(on.video.jump == 1 && !on.next);
        assert_eq!(read(vec![key(Key::L, Key::L, plain)], plain).video.jump, 0);
        assert_eq!(
            read(vec![key(Key::Comma, Key::Comma, plain)], plain)
                .video
                .step,
            -1
        );
        assert_eq!(
            read(vec![key(Key::Period, Key::Period, plain)], plain)
                .video
                .step,
            1
        );
        let up = read(vec![key(Key::ArrowUp, Key::ArrowUp, plain)], plain);
        assert_eq!(up.video.volume, 1);
        assert!(up.up, "the grid keeps its rows");
        let ctrl = Modifiers::COMMAND;
        assert_eq!(
            read(vec![key(Key::L, Key::L, ctrl)], ctrl).video.jump,
            0,
            "Ctrl+L: language"
        );
    }

    #[test]
    fn e_edits_elsewhere_only_without_modifiers() {
        let plain = Modifiers::NONE;
        assert!(read(vec![key(Key::E, Key::E, plain)], plain).edit_elsewhere);
        let ctrl = Modifiers::COMMAND;
        assert!(!read(vec![key(Key::E, Key::E, ctrl)], ctrl).edit_elsewhere);
    }

    #[test]
    fn o_switches_the_overlay_and_ctrl_o_opens_a_folder() {
        let plain = Modifiers::NONE;
        let keys = read(vec![key(Key::O, Key::O, plain)], plain);
        assert!(keys.overlay && !keys.open);
        let ctrl = Modifiers::COMMAND;
        let keys = read(vec![key(Key::O, Key::O, ctrl)], ctrl);
        assert!(keys.open && !keys.overlay);
    }

    /// `Ctrl+U` switches the subfolders; `Ctrl+0` fits, `Ctrl+1` is 100 % and `Ctrl+Plus` /
    /// `Ctrl+Minus` zoom – none of them rates, and egui's interface zoom doesn't take them.
    #[test]
    fn ctrl_keys_for_subfolders_and_zoom() {
        let (plain, ctrl) = (Modifiers::NONE, Modifiers::COMMAND);
        assert!(read(vec![key(Key::U, Key::U, ctrl)], ctrl).subfolders);
        assert!(!read(vec![key(Key::U, Key::U, plain)], plain).subfolders);
        let fit = read(vec![key(Key::Num0, Key::Num0, ctrl)], ctrl);
        assert!(fit.zoom_fit && !fit.zoom_actual);
        assert_eq!(fit.rating, None);
        // On AZERTY the 1 key types "&": its place counts.
        let actual = read(vec![key(Key::Num1, Key::Quote, ctrl)], ctrl);
        assert!(actual.zoom_actual && !actual.zoom_fit);
        assert_eq!(actual.rating, None);
        assert!(read(vec![key(Key::Plus, Key::Plus, ctrl)], ctrl).zoom_in);
        assert!(read(vec![key(Key::Minus, Key::Minus, ctrl)], ctrl).zoom_out);
        assert!(!read(vec![key(Key::Num1, Key::Num1, plain)], plain).zoom_actual);
    }

    /// `M` filters similar photos; `Ctrl+M` and `Ctrl+K` do nothing since 1.12. `F10` is the
    /// menu bar, with any modifier like `F6`.
    #[test]
    fn m_filters_similar_photos_and_f10_is_the_menu_bar() {
        let plain = Modifiers::NONE;
        let keys = read(vec![key(Key::M, Key::M, plain)], plain);
        assert!(keys.similar);
        let ctrl = Modifiers::COMMAND;
        assert_eq!(
            read(vec![key(Key::M, Key::M, ctrl)], ctrl),
            KeyInput::default()
        );
        assert_eq!(
            read(vec![key(Key::K, Key::K, ctrl)], ctrl),
            KeyInput::default()
        );
        assert!(read(vec![key(Key::F10, Key::F10, plain)], plain).toggle_side_bar);
        let keys = read(vec![key(Key::F10, Key::F10, plain)], plain);
        assert!(
            keys.commands(&[])
                .contains(&Command::TogglePanel(Panel::Left))
        );
    }

    #[test]
    fn f7_the_grid_and_page_keys_on_their_own() {
        let plain = Modifiers::NONE;
        assert!(read(vec![key(Key::F7, Key::F7, plain)], plain).toggle_grid);
        let keys = read(vec![key(Key::PageDown, Key::PageDown, plain)], plain);
        assert!(
            keys.page_down && !keys.next,
            "a page is its own step in the grid"
        );
        let keys = read(vec![key(Key::ArrowDown, Key::ArrowDown, plain)], plain);
        assert!(keys.down && !keys.next && !keys.up);
        let ctrl = Modifiers::COMMAND;
        assert!(!read(vec![key(Key::ArrowUp, Key::ArrowUp, ctrl)], ctrl).up);
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

    /// `C` compares two photos, `Shift+C` shows four.
    #[test]
    fn shift_c_is_the_four_up_view() {
        let plain = Modifiers::NONE;
        let shift = Modifiers::SHIFT;
        let four = read(vec![key(Key::C, Key::C, shift)], shift);
        assert!(four.quad && !four.compare);
        let two = read(vec![key(Key::C, Key::C, plain)], plain);
        assert!(two.compare && !two.quad);
    }

    /// `G` alone shows every face; with Shift it is nothing any more.
    #[test]
    fn g_is_the_face_grid() {
        let plain = Modifiers::NONE;
        let shift = Modifiers::SHIFT;
        assert!(read(vec![key(Key::G, Key::G, plain)], plain).face_grid);
        assert!(!read(vec![key(Key::G, Key::G, shift)], shift).face_grid);
    }
}
