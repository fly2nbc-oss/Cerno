//! Menu rows, and the small list that opens beside a row of the menu bar (`ui/side_bar.rs`):
//! the programs of *Edit elsewhere* and the languages. Shortcuts sit on the right. A row never
//! takes egui's keyboard focus (`Sense::CLICK`): `Space` would click it again.
//!
//! The list's keyboard: ↑/↓ move, Enter runs, a letter jumps to the next row starting with it,
//! Esc (or ←) closes the list.

use eframe::egui::{
    Align2, Area, Color32, Context, CursorIcon, Event, FontId, Id, Key, Order, Rect, Sense, Stroke,
    StrokeKind, pos2, vec2,
};

use eframe::egui::text::{LayoutJob, TextWrapping};

use crate::theme::{text, tokens};
use crate::ui::icons;

/// A list beside a row is as wide as its longest row needs, between these two (a program's
/// name, French or Italian ones are long).
const MIN_WIDTH: f32 = 240.0;
const MAX_WIDTH: f32 = 340.0;
const ROW_HEIGHT: f32 = 32.0;

/// How a row shows its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// A plain action; running it closes the list (and gives the keyboard back).
    None,
    /// A switch: a box, ticked when on. Running it keeps the list or bar.
    Toggle(bool),
    /// One of several values: a tick on the current one only. Running it keeps the list or
    /// bar.
    Choice(bool),
}

pub struct Row<A> {
    pub action: A,
    pub label: String,
    pub shortcut: Option<String>,
    pub mark: Mark,
    /// Why the row can't run right now (greyed out, the reason as its tooltip).
    pub disabled: Option<&'static str>,
    /// What the row does, in more words than its label (its tooltip while it can run).
    pub hint: Option<&'static str>,
}

impl<A> Row<A> {
    pub fn new(action: A, label: impl Into<String>, shortcut: Option<String>) -> Self {
        Self {
            action,
            label: label.into(),
            shortcut,
            mark: Mark::None,
            disabled: None,
            hint: None,
        }
    }

    pub fn hint(mut self, text: &'static str) -> Self {
        self.hint = Some(text);
        self
    }

    pub fn disabled(mut self, reason: Option<&'static str>) -> Self {
        self.disabled = reason;
        self
    }

    pub fn toggle(mut self, on: bool) -> Self {
        self.mark = Mark::Toggle(on);
        self
    }

    pub fn choice(mut self, current: bool) -> Self {
        self.mark = Mark::Choice(current);
        self
    }
}

/// Where the list opens: right of a row of the menu bar, its top at the row's (moved up when
/// the window ends first).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement(pub Rect);

/// The highlighted row (keyboard or mouse).
#[derive(Debug, Default)]
pub struct State {
    cursor: Option<usize>,
}

pub struct Output<A> {
    pub run: Option<A>,
    pub close: bool,
}

/// Where the keyboard lands in a list it opens: on the ticked choice, else on the first row
/// that can run.
fn start_row<A>(rows: &[Row<A>]) -> Option<usize> {
    rows.iter()
        .position(|row| row.mark == Mark::Choice(true))
        .or_else(|| rows.iter().position(|row| row.disabled.is_none()))
        .or(Some(0))
}

pub fn show<A: Copy>(
    ctx: &Context,
    window: Rect,
    state: &mut State,
    rows: &[Row<A>],
    placement: Placement,
) -> Output<A> {
    let mut out = Output {
        run: None,
        close: false,
    };
    // The list is rebuilt every frame and can shrink while it is open.
    state.cursor = state
        .cursor
        .filter(|&i| i < rows.len())
        .or_else(|| start_row(rows).filter(|&i| i < rows.len()));
    keyboard(ctx, state, rows, &mut out);

    let Placement(anchor) = placement;
    let height = (rows.len() as f32 * ROW_HEIGHT + 8.0).min(window.height() - 16.0);
    let width = list_width(ctx, rows);
    let left = (anchor.right() + 4.0)
        .min(window.right() - width - 8.0)
        .max(window.left() + 8.0);
    let top = (anchor.top() - 4.0)
        .min(window.bottom() - height - 8.0)
        .max(window.top() + 8.0);
    let card = Rect::from_min_size(pos2(left, top), vec2(width, height));
    let id = Id::new("row-list");

    Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(window.min)
        .show(ctx, |ui| {
            let backdrop = ui.allocate_rect(window, Sense::click());
            ui.painter()
                .rect_filled(window, 0.0, Color32::from_black_alpha(80));
            paint_card(ui.painter(), card);
            for (index, item) in rows.iter().enumerate() {
                let row = Rect::from_min_size(
                    pos2(
                        card.left() + 4.0,
                        card.top() + 4.0 + index as f32 * ROW_HEIGHT,
                    ),
                    vec2(card.width() - 8.0, ROW_HEIGHT),
                );
                if row.bottom() > card.bottom() {
                    break;
                }
                let lit = state.cursor == Some(index);
                let response = row_button(ui, row, id.with(("row", index)), item, false, lit);
                if response.hovered {
                    state.cursor = Some(index);
                }
                if response.clicked {
                    run_row(item, &mut out);
                }
            }
            if backdrop.clicked() {
                let pos = ui.input(|i| i.pointer.interact_pos());
                if !pos.is_some_and(|p| card.contains(p)) {
                    out.close = true;
                }
            }
        });
    out
}

/// Runs a row: a plain action closes the list, a switch or a choice keeps it open. A disabled
/// row does nothing; its tooltip says why.
fn run_row<A: Copy>(row: &Row<A>, out: &mut Output<A>) {
    if row.disabled.is_some() {
        return;
    }
    out.run = Some(row.action);
    if row.mark == Mark::None {
        out.close = true;
    }
}

/// Keys without modifiers, in the order they were pressed, taken out of the input so nothing
/// below the list sees them. Typed letters jump.
fn keyboard<A: Copy>(ctx: &Context, state: &mut State, rows: &[Row<A>], out: &mut Output<A>) {
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
                    Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::Enter | Key::Escape
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
        match key {
            Key::Escape | Key::ArrowLeft => out.close = true,
            Key::ArrowDown | Key::ArrowUp => {
                state.cursor = step(state.cursor, rows.len(), key == Key::ArrowDown);
            }
            Key::Enter => {
                if let Some(row) = state.cursor.and_then(|i| rows.get(i)) {
                    run_row(row, out);
                }
            }
            _ => {}
        }
    }
    for letter in letters {
        let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
        if let Some(i) = jump(&labels, state.cursor, letter) {
            state.cursor = Some(i);
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
pub(crate) fn jump(labels: &[&str], cursor: Option<usize>, letter: char) -> Option<usize> {
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

fn paint_card(painter: &eframe::egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 8.0, tokens::SURFACE);
    painter.rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0, tokens::LINE),
        StrokeKind::Inside,
    );
}

/// As wide as the longest row needs, between `MIN_WIDTH` and `MAX_WIDTH`. The row's label
/// starts 30 pt in and keeps 10 pt free on the right, inside a card with 4 pt on each side.
fn list_width<A>(ctx: &Context, rows: &[Row<A>]) -> f32 {
    let widest = rows
        .iter()
        .map(|row| {
            ctx.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(
                        row.label.clone(),
                        FontId::proportional(text::BODY),
                        tokens::TEXT,
                    )
                    .size()
                    .x
            })
        })
        .fold(0.0, f32::max);
    (widest + 30.0 + 10.0 + 8.0 + 4.0).clamp(MIN_WIDTH, MAX_WIDTH)
}

pub(crate) struct RowResponse {
    pub(crate) clicked: bool,
    pub(crate) hovered: bool,
}

/// A row of the menu bar, drawn like a row of a list: its box or tick, label and shortcut,
/// and `›` when it opens a list beside it.
pub(crate) fn bar_row<A>(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    id: Id,
    row: &Row<A>,
    opens_list: bool,
    lit: bool,
) -> RowResponse {
    row_button(ui, rect, id, row, opens_list, lit)
}

fn row_button<A>(
    ui: &mut eframe::egui::Ui,
    rect: Rect,
    id: Id,
    row: &Row<A>,
    opens_list: bool,
    lit: bool,
) -> RowResponse {
    // Never focusable: a focused row would be clicked again by `Space` ("next photo").
    let response = ui.interact(rect, id, Sense::CLICK);
    let response = if row.disabled.is_some() {
        response
    } else {
        response.on_hover_cursor(CursorIcon::PointingHand)
    };
    let text_colour = if row.disabled.is_some() {
        tokens::MUTED
    } else {
        tokens::TEXT
    };
    let painter = ui.painter();
    if response.hovered() || lit {
        painter.rect_filled(rect, 5.0, tokens::ACCENT_SUBTLE);
    }
    let y = rect.center().y;
    let c = pos2(rect.left() + 16.0, y);
    let tick = |colour: Color32| {
        let stroke = Stroke::new(1.6, colour);
        painter.line_segment([c + vec2(-4.5, 0.0), c + vec2(-1.5, 3.5)], stroke);
        painter.line_segment([c + vec2(-1.5, 3.5), c + vec2(4.5, -4.0)], stroke);
    };
    match row.mark {
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
    let x = rect.left() + 30.0;
    let right_reserve = if opens_list { 22.0 } else { 10.0 };
    let mut label_right = rect.right() - right_reserve;
    if let Some(shortcut) = &row.shortcut {
        let shown = painter.text(
            pos2(rect.right() - right_reserve, y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::proportional(text::SMALL),
            tokens::MUTED,
        );
        label_right = shown.left() - 12.0;
    }
    let mut job = LayoutJob::simple_singleline(
        row.label.clone(),
        FontId::proportional(text::BODY),
        text_colour,
    );
    job.wrap = TextWrapping::truncate_at_width((label_right - x).max(20.0));
    let galley = painter.layout_job(job);
    let size = galley.size();
    let elided = galley.elided;
    painter.galley(pos2(x, y - size.y / 2.0), galley, text_colour);
    if opens_list {
        icons::chevron(painter, pos2(rect.right() - 12.0, y), false, tokens::MUTED);
    }
    // Why it can't run, else what it does, else – cut short – the whole label.
    let tooltip = row
        .disabled
        .or(row.hint)
        .map(str::to_owned)
        .or_else(|| elided.then(|| row.label.clone()));
    let response = match tooltip {
        Some(text) => response.on_hover_text(text),
        None => response,
    };
    RowResponse {
        clicked: response.clicked(),
        hovered: response.hovered(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Modifiers, RawInput};

    /// A short list keeps its width; a long row (a program's name, French) widens it, up to
    /// `MAX_WIDTH`.
    #[test]
    fn a_list_grows_with_its_longest_row() {
        let ctx = Context::default();
        let mut widths = (0.0, 0.0, 0.0);
        let mut output = ctx.run_ui(RawInput::default(), |_| {
            let row = |label: &str| vec![Row::new(0u8, label, None)];
            widths = (
                list_width(&ctx, &row("Paint")),
                list_width(&ctx, &row("Supprimer les rejetées (1234 photos)")),
                list_width(&ctx, &row(&"x".repeat(200))),
            );
        });
        output.textures_delta.clear();
        assert_eq!(widths.0, MIN_WIDTH);
        assert!(widths.1 > MIN_WIDTH, "{widths:?}");
        assert_eq!(widths.2, MAX_WIDTH);
    }

    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn rows() -> Vec<Row<u8>> {
        vec![
            Row::new(1, "Deutsch", None).choice(false),
            Row::new(2, "English", None).choice(true),
            Row::new(3, "Français", None).choice(false),
        ]
    }

    fn frame(state: &mut State, rows: &[Row<u8>], events: Vec<Event>) -> Output<u8> {
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
                let row = Rect::from_min_size(pos2(4.0, 100.0), vec2(248.0, 28.0));
                result = Some(show(ui.ctx(), window, state, rows, Placement(row)))
            },
        );
        output.textures_delta.clear();
        result.expect("list shown")
    }

    #[test]
    fn escape_and_left_close() {
        for closing in [Key::Escape, Key::ArrowLeft] {
            let out = frame(&mut State::default(), &rows(), vec![key(closing)]);
            assert!(out.close);
            assert!(out.run.is_none());
        }
    }

    #[test]
    fn the_list_opens_on_the_current_choice_and_a_choice_keeps_it() {
        let rows = rows();
        let mut state = State::default();
        // On "English" (ticked); Down to "Français", Enter runs it and the list stays.
        let out = frame(
            &mut state,
            &rows,
            vec![key(Key::ArrowDown), key(Key::Enter)],
        );
        assert_eq!(out.run, Some(3));
        assert!(!out.close);
        // Down wraps to the top.
        let out = frame(
            &mut state,
            &rows,
            vec![key(Key::ArrowDown), key(Key::Enter)],
        );
        assert_eq!(out.run, Some(1));
    }

    #[test]
    fn letters_jump() {
        let rows = rows();
        let mut state = State::default();
        frame(&mut state, &rows, vec![Event::Text("f".into())]);
        assert_eq!(state.cursor, Some(2));
        // "d" finds "Deutsch" from the end, round the top.
        frame(&mut state, &rows, vec![Event::Text("D".into())]);
        assert_eq!(state.cursor, Some(0));
    }

    #[test]
    fn a_list_that_shrinks_while_open_does_not_panic() {
        let mut state = State { cursor: Some(2) };
        let short = vec![Row::new(1u8, "Paint", None)];
        let out = frame(&mut state, &short, vec![key(Key::Enter)]);
        assert_eq!(
            out.run,
            Some(1),
            "the cursor falls back onto a row that is there"
        );
    }

    #[test]
    fn a_disabled_row_does_not_run() {
        let rows = vec![
            Row::new(1, "Paint", None).disabled(Some("läuft schon")),
            Row::new(2, "Fotos", None),
        ];
        let mut state = State::default();
        assert_eq!(
            start_row(&rows),
            Some(1),
            "it opens on the first row that can run"
        );
        let out = frame(&mut state, &rows, vec![key(Key::ArrowUp), key(Key::Enter)]);
        assert!(out.run.is_none());
        assert!(!out.close, "the list stays open, the tooltip says why");
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
