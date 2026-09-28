//! Menus: the burger menu (`Ctrl+K` and the button at the bottom right) and the action menu
//! under the filter bar's "Action" button (`Ctrl+M`). Groups with several values open a
//! submenu, everything else is a row; shortcuts sit on the right.
//!
//! Keyboard: ↑/↓ move, Enter runs (or opens a submenu), → opens and ← closes a submenu, a
//! letter jumps to the next row starting with it, Esc closes the submenu first, then the menu.

use eframe::egui::{
    Align2, Area, Color32, Context, CursorIcon, Event, FontId, Id, Key, Order, Rect, Sense, Stroke,
    StrokeKind, pos2, vec2,
};

use crate::theme::tokens;
use crate::ui::icons;

const WIDTH: f32 = 340.0;
/// The action menu under its button is narrower.
const ANCHORED_WIDTH: f32 = 240.0;
const ROW_HEIGHT: f32 = 32.0;

/// How a row shows its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// A plain action; running it closes the menu.
    None,
    /// A switch: a box, ticked when on. Clicking keeps the menu open.
    Toggle(bool),
    /// One of several values: a tick on the current one only. Clicking keeps the menu open.
    Choice(bool),
}

pub struct Row<A> {
    pub action: A,
    pub label: String,
    pub shortcut: Option<String>,
    pub mark: Mark,
    /// A colour dot in front of the label (colour labels).
    pub swatch: Option<Color32>,
}

impl<A> Row<A> {
    pub fn new(action: A, label: impl Into<String>, shortcut: Option<String>) -> Self {
        Self {
            action,
            label: label.into(),
            shortcut,
            mark: Mark::None,
            swatch: None,
        }
    }

    pub fn toggle(mut self, on: bool) -> Self {
        self.mark = Mark::Toggle(on);
        self
    }

    pub fn choice(mut self, current: bool) -> Self {
        self.mark = Mark::Choice(current);
        self
    }

    pub fn swatch(mut self, colour: Color32) -> Self {
        self.swatch = Some(colour);
        self
    }
}

pub struct Group<A> {
    pub label: String,
    pub shortcut: Option<String>,
    pub rows: Vec<Row<A>>,
}

impl<A> Group<A> {
    pub fn new(label: impl Into<String>, shortcut: Option<String>, rows: Vec<Row<A>>) -> Self {
        Self {
            label: label.into(),
            shortcut,
            rows,
        }
    }
}

pub enum Entry<A> {
    Row(Row<A>),
    Group(Group<A>),
}

impl<A> Entry<A> {
    fn label(&self) -> &str {
        match self {
            Self::Row(row) => &row.label,
            Self::Group(group) => &group.label,
        }
    }
}

/// Where the menu opens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// Above the menu button at the bottom right of the window.
    BottomRight,
    /// Under a button, right-aligned with it.
    Below(Rect),
}

/// Open submenu and the highlighted rows (keyboard or mouse).
#[derive(Debug, Default)]
pub struct State {
    open: Option<usize>,
    cursor: Option<usize>,
    sub_cursor: Option<usize>,
}

pub struct Output<A> {
    pub run: Option<A>,
    pub close: bool,
}

pub fn show<A: Copy>(
    ctx: &Context,
    window: Rect,
    state: &mut State,
    entries: &[Entry<A>],
    placement: Placement,
) -> Output<A> {
    let mut out = Output {
        run: None,
        close: false,
    };
    keyboard(ctx, state, entries, &mut out);

    let (menu, id) = match placement {
        Placement::BottomRight => {
            let height = entries.len() as f32 * ROW_HEIGHT + 8.0;
            let menu = Rect::from_min_size(
                pos2(
                    window.right() - WIDTH - 12.0,
                    (window.bottom() - height - 12.0).max(window.top() + 8.0),
                ),
                vec2(WIDTH, height.min(window.height() - 24.0)),
            );
            (menu, Id::new("burger"))
        }
        Placement::Below(anchor) => {
            let height = entries.len() as f32 * ROW_HEIGHT + 8.0;
            let left = (anchor.right() - ANCHORED_WIDTH)
                .min(window.right() - ANCHORED_WIDTH - 8.0)
                .max(window.left() + 8.0);
            let menu = Rect::from_min_size(
                pos2(left, anchor.bottom() + 4.0),
                vec2(ANCHORED_WIDTH, height),
            );
            (menu, Id::new("action-menu"))
        }
    };

    Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(window.min)
        .show(ctx, |ui| {
            let backdrop = ui.allocate_rect(window, Sense::click());
            ui.painter()
                .rect_filled(window, 0.0, Color32::from_black_alpha(80));
            paint_card(ui.painter(), menu);

            let mut submenu: Option<(usize, Rect)> = None;
            for (index, entry) in entries.iter().enumerate() {
                let row = Rect::from_min_size(
                    pos2(
                        menu.left() + 4.0,
                        menu.top() + 4.0 + index as f32 * ROW_HEIGHT,
                    ),
                    vec2(menu.width() - 8.0, ROW_HEIGHT),
                );
                let lit = state.cursor == Some(index) && state.open.is_none_or(|o| o == index);
                match entry {
                    Entry::Row(item) => {
                        let response =
                            row_button(ui, row, id.with(("row", index)), RowLook::of(item), lit);
                        if response.hovered {
                            state.cursor = Some(index);
                        }
                        if response.clicked {
                            run_row(item, &mut out);
                        }
                    }
                    Entry::Group(group) => {
                        let look = RowLook {
                            label: &group.label,
                            shortcut: group.shortcut.as_deref(),
                            mark: Mark::None,
                            swatch: None,
                            submenu: true,
                        };
                        let response = row_button(ui, row, id.with(("group", index)), look, lit);
                        if response.hovered && state.open.is_none() {
                            state.cursor = Some(index);
                        }
                        if response.clicked {
                            let open = state.open == Some(index);
                            state.open = if open { None } else { Some(index) };
                            state.cursor = Some(index);
                            state.sub_cursor = None;
                        }
                        if state.open == Some(index) {
                            submenu =
                                Some((index, submenu_rect(window, menu, row, group.rows.len())));
                        }
                    }
                }
            }

            if let Some((index, sub)) = submenu
                && let Entry::Group(group) = &entries[index]
            {
                paint_card(ui.painter(), sub);
                for (j, item) in group.rows.iter().enumerate() {
                    let row = Rect::from_min_size(
                        pos2(sub.left() + 4.0, sub.top() + 4.0 + j as f32 * ROW_HEIGHT),
                        vec2(sub.width() - 8.0, ROW_HEIGHT),
                    );
                    if row.bottom() > sub.bottom() {
                        break;
                    }
                    let lit = state.sub_cursor == Some(j);
                    let response =
                        row_button(ui, row, id.with(("sub", index, j)), RowLook::of(item), lit);
                    if response.hovered {
                        state.sub_cursor = Some(j);
                    }
                    if response.clicked {
                        run_row(item, &mut out);
                    }
                }
            }

            if backdrop.clicked() {
                let pos = ui.input(|i| i.pointer.interact_pos());
                let on_card = pos.is_some_and(|p| {
                    menu.contains(p) || submenu.is_some_and(|(_, rect)| rect.contains(p))
                });
                if !on_card {
                    out.close = true;
                }
            }
        });
    out
}

/// Runs a row: a plain action closes the menu, a switch or a choice keeps it open.
fn run_row<A: Copy>(row: &Row<A>, out: &mut Output<A>) {
    out.run = Some(row.action);
    if row.mark == Mark::None {
        out.close = true;
    }
}

/// Keys without modifiers, in the order they were pressed, taken out of the input so nothing
/// below the menu sees them. Typed letters jump.
fn keyboard<A: Copy>(ctx: &Context, state: &mut State, entries: &[Entry<A>], out: &mut Output<A>) {
    let (keys, letters) = ctx.input_mut(|i| {
        let mut keys = Vec::new();
        let mut letters = Vec::new();
        i.events.retain(|event| match event {
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if modifiers.is_none()
                && matches!(
                    key,
                    Key::ArrowUp
                        | Key::ArrowDown
                        | Key::ArrowLeft
                        | Key::ArrowRight
                        | Key::Enter
                        | Key::Escape
                ) =>
            {
                keys.push(*key);
                false
            }
            Event::Text(text) => {
                letters.extend(text.chars().filter(|c| c.is_alphanumeric()));
                false
            }
            _ => true,
        });
        (keys, letters)
    });

    for key in keys {
        let sub_rows = state.open.and_then(|index| match &entries[index] {
            Entry::Group(group) => Some(&group.rows),
            Entry::Row(_) => None,
        });
        match (key, sub_rows) {
            (Key::Escape, Some(_)) | (Key::ArrowLeft, Some(_)) => {
                state.open = None;
                state.sub_cursor = None;
            }
            (Key::Escape, None) => out.close = true,
            (Key::ArrowDown | Key::ArrowUp, Some(rows)) => {
                state.sub_cursor = step(state.sub_cursor, rows.len(), key == Key::ArrowDown);
            }
            (Key::ArrowDown | Key::ArrowUp, None) => {
                state.cursor = step(state.cursor, entries.len(), key == Key::ArrowDown);
            }
            (Key::Enter, Some(rows)) => {
                if let Some(row) = state.sub_cursor.and_then(|j| rows.get(j)) {
                    run_row(row, out);
                }
            }
            (Key::Enter | Key::ArrowRight, None) => match state.cursor.map(|i| (i, &entries[i])) {
                Some((index, Entry::Group(group))) => {
                    state.open = Some(index);
                    state.sub_cursor = group
                        .rows
                        .iter()
                        .position(|row| matches!(row.mark, Mark::Choice(true)))
                        .or(Some(0));
                }
                Some((_, Entry::Row(row))) if key == Key::Enter => run_row(row, out),
                _ => {}
            },
            _ => {}
        }
    }

    for letter in letters {
        match state.open.map(|index| &entries[index]) {
            Some(Entry::Group(group)) => {
                let labels: Vec<&str> = group.rows.iter().map(|r| r.label.as_str()).collect();
                if let Some(j) = jump(&labels, state.sub_cursor, letter) {
                    state.sub_cursor = Some(j);
                }
            }
            _ => {
                let labels: Vec<&str> = entries.iter().map(Entry::label).collect();
                if let Some(i) = jump(&labels, state.cursor, letter) {
                    state.cursor = Some(i);
                    state.open = None;
                }
            }
        }
    }
}

/// Next (or previous) row, wrapping; from nothing, Down starts at the top, Up at the bottom.
fn step(cursor: Option<usize>, len: usize, down: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match (cursor, down) {
        (None, true) => 0,
        (None, false) => len - 1,
        (Some(i), true) => (i + 1) % len,
        (Some(i), false) => (i + len - 1) % len,
    })
}

/// The next label after `cursor` that starts with `letter` (case- and accent-insensitive).
fn jump(labels: &[&str], cursor: Option<usize>, letter: char) -> Option<usize> {
    let wanted = crate::library::sort_key(letter);
    let start = cursor.map_or(0, |c| c + 1);
    (0..labels.len())
        .map(|k| (start + k) % labels.len())
        .find(|&i| {
            labels[i]
                .chars()
                .next()
                .is_some_and(|c| crate::library::sort_key(c) == wanted)
        })
}

/// Submenu to the left of its group, shifted so a long list stays inside the window.
fn submenu_rect(window: Rect, menu: Rect, row: Rect, rows: usize) -> Rect {
    let sub_h = rows as f32 * ROW_HEIGHT + 8.0;
    let max_h = (window.height() - 16.0).max(ROW_HEIGHT + 8.0);
    let h = sub_h.min(max_h);
    let y = (row.top() - 4.0)
        .min(window.bottom() - 8.0 - h)
        .max(window.top() + 8.0);
    let x = (menu.left() - WIDTH - 4.0).max(window.left() + 8.0);
    Rect::from_min_size(pos2(x, y), vec2(WIDTH, h))
}

fn paint_card(painter: &eframe::egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 8.0, tokens::SURFACE);
    painter.rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0, tokens::LINE),
        StrokeKind::Inside,
    );
}

struct RowLook<'a> {
    label: &'a str,
    shortcut: Option<&'a str>,
    mark: Mark,
    swatch: Option<Color32>,
    submenu: bool,
}

impl<'a> RowLook<'a> {
    fn of<A>(row: &'a Row<A>) -> Self {
        Self {
            label: &row.label,
            shortcut: row.shortcut.as_deref(),
            mark: row.mark,
            swatch: row.swatch,
            submenu: false,
        }
    }
}

struct RowResponse {
    clicked: bool,
    hovered: bool,
}

fn row_button(
    ui: &mut eframe::egui::Ui,
    row: Rect,
    id: Id,
    look: RowLook<'_>,
    lit: bool,
) -> RowResponse {
    let response = ui
        .interact(row, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    if response.hovered() || lit {
        painter.rect_filled(row, 5.0, tokens::ACCENT_SUBTLE);
    }
    let y = row.center().y;
    let c = pos2(row.left() + 16.0, y);
    let tick = |colour: Color32| {
        let stroke = Stroke::new(1.6, colour);
        painter.line_segment([c + vec2(-4.5, 0.0), c + vec2(-1.5, 3.5)], stroke);
        painter.line_segment([c + vec2(-1.5, 3.5), c + vec2(4.5, -4.0)], stroke);
    };
    match look.mark {
        Mark::Toggle(on) => {
            let square = Rect::from_center_size(c, vec2(13.0, 13.0));
            painter.rect_stroke(
                square,
                3.0,
                Stroke::new(1.0, if on { tokens::ACCENT } else { tokens::MUTED }),
                StrokeKind::Inside,
            );
            if on {
                tick(tokens::ACCENT_STRONG);
            }
        }
        Mark::Choice(true) => tick(tokens::ACCENT_STRONG),
        Mark::Choice(false) | Mark::None => {}
    }
    let mut x = row.left() + 30.0;
    if let Some(colour) = look.swatch {
        painter.circle_filled(pos2(x + 5.0, y), 5.0, colour);
        x += 16.0;
    }
    painter.text(
        pos2(x, y),
        Align2::LEFT_CENTER,
        look.label,
        FontId::proportional(13.5),
        tokens::TEXT,
    );
    let right_reserve = if look.submenu { 22.0 } else { 10.0 };
    if let Some(shortcut) = look.shortcut {
        painter.text(
            pos2(row.right() - right_reserve, y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::proportional(12.0),
            tokens::MUTED,
        );
    }
    if look.submenu {
        icons::chevron(painter, pos2(row.right() - 12.0, y), false, tokens::MUTED);
    }
    RowResponse {
        clicked: response.clicked(),
        hovered: response.hovered(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Modifiers, RawInput};

    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn entries() -> Vec<Entry<u8>> {
        vec![
            Entry::Row(Row::new(1, "Ordner öffnen", None)),
            Entry::Group(Group::new(
                "Sortieren",
                None,
                vec![
                    Row::new(2, "Name", None).choice(false),
                    Row::new(3, "Sterne", None).choice(true),
                ],
            )),
            Entry::Row(Row::new(4, "Hilfe", Some("H".into()))),
        ]
    }

    fn frame(state: &mut State, entries: &[Entry<u8>], events: Vec<Event>) -> Output<u8> {
        let ctx = Context::default();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let mut result = None;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(window),
                events,
                ..Default::default()
            },
            |ui| {
                result = Some(show(
                    ui.ctx(),
                    window,
                    state,
                    entries,
                    Placement::BottomRight,
                ))
            },
        );
        output.textures_delta.clear();
        result.expect("menu shown")
    }

    #[test]
    fn escape_closes() {
        let out = frame(&mut State::default(), &entries(), vec![key(Key::Escape)]);
        assert!(out.close);
        assert!(out.run.is_none());
    }

    #[test]
    fn arrows_and_enter_run_a_row() {
        let entries = entries();
        let mut state = State::default();
        // Down to "Ordner öffnen", Up wraps to "Hilfe".
        let out = frame(
            &mut state,
            &entries,
            vec![key(Key::ArrowDown), key(Key::ArrowUp), key(Key::Enter)],
        );
        assert_eq!(out.run, Some(4));
        assert!(out.close);
    }

    #[test]
    fn right_opens_a_submenu_at_the_current_choice() {
        let entries = entries();
        let mut state = State::default();
        let out = frame(
            &mut state,
            &entries,
            vec![
                key(Key::ArrowDown),
                key(Key::ArrowDown),
                key(Key::ArrowRight),
            ],
        );
        assert!(out.run.is_none());
        assert_eq!(state.open, Some(1));
        assert_eq!(state.sub_cursor, Some(1), "starts on the ticked value");
        // Up to "Name", Enter runs it and the menu stays open (a choice).
        let out = frame(
            &mut state,
            &entries,
            vec![key(Key::ArrowUp), key(Key::Enter)],
        );
        assert_eq!(out.run, Some(2));
        assert!(!out.close);
        // Esc closes the submenu only, a second Esc the menu.
        let out = frame(&mut state, &entries, vec![key(Key::Escape)]);
        assert!(!out.close);
        assert_eq!(state.open, None);
        let out = frame(&mut state, &entries, vec![key(Key::Escape)]);
        assert!(out.close);
    }

    #[test]
    fn letters_jump() {
        let entries = entries();
        let mut state = State::default();
        let out = frame(&mut state, &entries, vec![Event::Text("h".into())]);
        assert!(out.run.is_none());
        assert_eq!(state.cursor, Some(2));
        // "o" matches "Ordner öffnen" regardless of the accent on "ö" further in.
        frame(&mut state, &entries, vec![Event::Text("O".into())]);
        assert_eq!(state.cursor, Some(0));
    }

    #[test]
    fn step_wraps() {
        assert_eq!(step(None, 3, true), Some(0));
        assert_eq!(step(None, 3, false), Some(2));
        assert_eq!(step(Some(2), 3, true), Some(0));
        assert_eq!(step(Some(0), 3, false), Some(2));
        assert_eq!(step(None, 0, true), None);
    }
}
