//! The menu bar on the left (since 1.11, ticket CER-35, the user's decisions F1–F3 of
//! 2026-10-10): the menu as groups that fold, beside the photo like the details panel on the
//! other side. Nothing in it has another home – the stars are in the info bar, sort and
//! filters in the filter bar, the panels' buttons in the info bar.
//!
//! The bar takes no key unless it has the keyboard (`Ctrl+K`, `Ctrl+M`, `E`): then ↑/↓ move,
//! Enter runs a row or folds a group, → unfolds a group or steps to the next segment, ← steps
//! back or folds the group (from a row inside it too), a letter jumps, Esc gives the keyboard
//! back. Its rows never take egui's focus (`Sense::CLICK`), so `Space` can't click one again.

use std::collections::BTreeSet;

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    Color32, CursorIcon, Event, FontId, Id, Key, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, pos2, vec2,
};

use crate::theme::{text, tokens};
use crate::ui::icons;
use crate::ui::palette::{self, Mark, Row};

/// The same in every language and state, so nothing moves under the pointer; a label that
/// doesn't fit is cut short and shows whole in its tooltip.
pub const WIDTH: f32 = 256.0;
const ROW: f32 = 28.0;
const HEADER: f32 = 30.0;
/// A segment of a row of icons (colours, turns), and the height of a row of text segments.
const SEGMENT: f32 = 24.0;
/// Room above a group's title, between two groups.
const GAP: f32 = 6.0;

/// The group open at the first start: what acts on the photo shown.
pub const FIRST_OPEN: &str = "photo";

/// One line of the bar.
pub enum Item<A> {
    /// A command, a switch (box) or a choice (tick), as in a list.
    Row(Row<A>),
    /// A few small choices side by side: the colours, the turns, the overlay's modes.
    Segments(Segments<A>),
    /// A row that opens a list beside the bar (*Edit elsewhere*, *Language*): it runs its
    /// action, and the app opens the list at the row (`Output::row`).
    List(Row<A>),
}

/// What a segment shows.
pub enum Look {
    Text(String),
    /// A colour label's dot.
    Swatch(Color32),
    /// An empty ring: no colour label.
    NoColour,
    /// A quarter turn, clockwise or not.
    Turn(bool),
}

pub struct Segment<A> {
    pub action: A,
    pub look: Look,
    /// Its name and key.
    pub tooltip: String,
    /// The current value (framed in the accent).
    pub on: bool,
}

pub struct Segments<A> {
    pub label: String,
    pub segments: Vec<Segment<A>>,
    /// Why none can run right now (greyed out, the reason as tooltip).
    pub disabled: Option<&'static str>,
}

impl<A> Segments<A> {
    /// Text segments take a line of their own under the label; icons sit beside it.
    fn text(&self) -> bool {
        self.segments
            .iter()
            .any(|s| matches!(s.look, Look::Text(_)))
    }

    /// The segment the keyboard lands on: the current value, else the first.
    fn start(&self) -> usize {
        self.segments.iter().position(|s| s.on).unwrap_or(0)
    }
}

impl<A> Item<A> {
    fn label(&self) -> &str {
        match self {
            Self::Row(row) | Self::List(row) => &row.label,
            Self::Segments(segments) => &segments.label,
        }
    }

    fn height(&self) -> f32 {
        match self {
            Self::Segments(segments) if segments.text() => ROW + SEGMENT + 4.0,
            _ => ROW,
        }
    }
}

/// A group that folds: its title, and its lines while it is open.
pub struct Section<A> {
    /// Saved with the open groups.
    pub id: &'static str,
    pub title: String,
    pub items: Vec<Item<A>>,
}

pub struct Bar<A> {
    /// Above the groups (*Open folder*), always shown.
    pub top: Vec<Item<A>>,
    pub sections: Vec<Section<A>>,
}

/// A line of the bar as it shows: an item above the groups, a group's title, an item of an
/// open group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    Top(usize),
    Title(usize),
    Item(usize, usize),
}

impl<A> Bar<A> {
    fn lines(&self, open: &BTreeSet<String>) -> Vec<Line> {
        let mut lines: Vec<Line> = (0..self.top.len()).map(Line::Top).collect();
        for (s, section) in self.sections.iter().enumerate() {
            lines.push(Line::Title(s));
            if open.contains(section.id) {
                lines.extend((0..section.items.len()).map(|i| Line::Item(s, i)));
            }
        }
        lines
    }

    fn item(&self, line: Line) -> Option<&Item<A>> {
        match line {
            Line::Top(i) => self.top.get(i),
            Line::Item(s, i) => self.sections.get(s)?.items.get(i),
            Line::Title(_) => None,
        }
    }

    fn label(&self, line: Line) -> &str {
        match line {
            Line::Title(s) => self.sections.get(s).map_or("", |section| &section.title),
            _ => self.item(line).map_or("", Item::label),
        }
    }
}

/// Which groups are open and where the keyboard is.
#[derive(Debug)]
pub struct State {
    open: BTreeSet<String>,
    /// The bar has the keyboard: its keys don't reach the photo.
    pub focus: bool,
    cursor: Option<Line>,
    /// The segment of a row of segments the cursor is on.
    segment: usize,
    /// The keyboard moved the cursor: scroll its line into view.
    follow: bool,
    /// Where the keyboard goes once the bar is drawn: a group, and one of its items.
    wanted: Option<(&'static str, Option<usize>)>,
    /// The keyboard came this frame: the key that brought it (`E` types an "e") is not the
    /// bar's, or it would jump to a row starting with it.
    fresh: bool,
}

impl Default for State {
    fn default() -> Self {
        Self::restore(None)
    }
}

impl State {
    /// The open groups saved last time (`saved`); the first time *This photo* is open.
    pub fn restore(saved: Option<&str>) -> Self {
        let open = match saved {
            Some(text) => text
                .split(',')
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .collect(),
            None => BTreeSet::from([FIRST_OPEN.to_owned()]),
        };
        Self {
            open,
            focus: false,
            cursor: None,
            segment: 0,
            follow: false,
            wanted: None,
            fresh: false,
        }
    }

    /// The open groups, for the settings.
    pub fn saved(&self) -> String {
        self.open
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }

    /// `Ctrl+K`: the keyboard goes to the bar, where its cursor was (or on the first line).
    pub fn take_keyboard(&mut self) {
        self.focus = true;
        self.follow = true;
        self.fresh = true;
    }

    /// `Ctrl+M`, `E`: the keyboard goes to the bar, on a group – unfolded – or one of its
    /// items.
    pub fn take_keyboard_at(&mut self, section: &'static str, item: Option<usize>) {
        self.take_keyboard();
        self.open.insert(section.to_owned());
        self.wanted = Some((section, item));
    }

    pub fn release(&mut self) {
        self.focus = false;
    }

    fn fold(&mut self, id: &str, open: bool) {
        if open {
            self.open.insert(id.to_owned());
        } else {
            self.open.remove(id);
        }
    }
}

#[derive(Debug)]
pub struct Output<A> {
    pub run: Option<A>,
    /// Where the line that ran is: a list opens beside it.
    pub row: Option<Rect>,
    /// Where the keyboard's line is (E opens the programs' list beside it).
    pub cursor: Option<Rect>,
    /// What ran keeps the keyboard in the bar: a switch, a choice, a segment, a list.
    pub keep: bool,
    /// The keyboard goes back to the photo: Esc, or a click beside the bar.
    pub leave: bool,
    /// A group folded or unfolded: its state is saved.
    pub folded: bool,
}

impl<A> Default for Output<A> {
    fn default() -> Self {
        Self {
            run: None,
            row: None,
            cursor: None,
            keep: false,
            leave: false,
            folded: false,
        }
    }
}

/// Draws the bar in `rect` and reads its keys while `listening` (it has the keyboard and no
/// list beside it is open).
pub fn show<A: Copy>(
    ui: &mut Ui,
    rect: Rect,
    state: &mut State,
    bar: &Bar<A>,
    listening: bool,
) -> Output<A> {
    let mut out = Output::default();
    let listening = listening && !std::mem::take(&mut state.fresh);
    if let Some((id, item)) = state.wanted.take()
        && let Some(s) = bar.sections.iter().position(|section| section.id == id)
    {
        state.cursor = Some(match item {
            Some(i) if i < bar.sections[s].items.len() => Line::Item(s, i),
            _ if !bar.sections[s].items.is_empty() => Line::Item(s, 0),
            _ => Line::Title(s),
        });
        state.segment = state
            .cursor
            .and_then(|line| match bar.item(line) {
                Some(Item::Segments(segments)) => Some(segments.start()),
                _ => None,
            })
            .unwrap_or(0);
    }
    let mut lines = bar.lines(&state.open);
    if !state.cursor.is_some_and(|line| lines.contains(&line)) {
        state.cursor = state.focus.then(|| lines.first().copied()).flatten();
        state.segment = 0;
    }
    // Which line runs with the keyboard; its rect is known once it is drawn.
    let mut ran_by_key = None;
    if listening {
        keyboard(ui, state, bar, &mut lines, &mut out, &mut ran_by_key);
        let pressed_beside = ui.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|pos| !rect.contains(pos))
        });
        if pressed_beside {
            out.leave = true;
        }
    }

    ui.painter().rect_filled(rect, 0.0, tokens::SURFACE);
    ui.painter().vline(
        rect.right() - 0.5,
        rect.y_range(),
        Stroke::new(1.0, tokens::LINE),
    );
    let mut panel = ui.new_child(UiBuilder::new().max_rect(rect).id_salt("side-bar"));
    let id = Id::new("side-bar");
    ScrollArea::vertical()
        .auto_shrink(false)
        .id_salt("side-bar-scroll")
        .show(&mut panel, |ui| {
            ui.set_width(rect.width());
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(4.0);
            for &line in &lines {
                if matches!(line, Line::Title(s) if s > 0 || !bar.top.is_empty()) {
                    ui.add_space(GAP);
                }
                let height = match line {
                    Line::Title(_) => HEADER,
                    _ => bar.item(line).map_or(ROW, Item::height),
                };
                let (slot, _) =
                    ui.allocate_exact_size(vec2(rect.width() - 1.0, height), Sense::hover());
                let area = slot.shrink2(vec2(4.0, 0.0));
                let lit = state.focus && state.cursor == Some(line);
                if lit {
                    out.cursor = Some(area);
                    if state.follow {
                        ui.scroll_to_rect(slot, None);
                    }
                }
                if ran_by_key == Some(line) {
                    out.row = Some(area);
                }
                let line_id = id.with(line_key(line));
                match line {
                    Line::Title(s) => {
                        let section = &bar.sections[s];
                        let open = state.open.contains(section.id);
                        if title(ui, area, line_id, &section.title, open, lit) {
                            state.fold(section.id, !open);
                            out.folded = true;
                            state.cursor = state.focus.then_some(line);
                        }
                    }
                    _ => {
                        let Some(item) = bar.item(line) else {
                            continue;
                        };
                        let segment = lit.then_some(state.segment);
                        if let Some(clicked) = draw_item(ui, area, line_id, item, lit, segment) {
                            out.row = Some(area);
                            match clicked {
                                Clicked::Row => run(item, None, &mut out),
                                Clicked::Segment(k) => run(item, Some(k), &mut out),
                            }
                            if state.focus {
                                state.cursor = Some(line);
                                if let Clicked::Segment(k) = clicked {
                                    state.segment = k;
                                }
                            }
                        }
                    }
                }
            }
            ui.add_space(8.0);
        });
    state.follow = false;
    if state.focus {
        // The bar has the keyboard: an accent line along its edge.
        ui.painter().vline(
            rect.right() - 1.0,
            rect.y_range(),
            Stroke::new(2.0, tokens::ACCENT),
        );
    }
    out
}

/// A line's part of the id, stable while groups fold.
fn line_key(line: Line) -> (u8, usize, usize) {
    match line {
        Line::Top(i) => (0, i, 0),
        Line::Title(s) => (1, s, 0),
        Line::Item(s, i) => (2, s, i),
    }
}

/// What a click hit.
#[derive(Clone, Copy)]
enum Clicked {
    Row,
    Segment(usize),
}

/// Runs an item – a row, or segment `segment` of a row of them. A greyed-out one does
/// nothing (its tooltip says why).
fn run<A: Copy>(item: &Item<A>, segment: Option<usize>, out: &mut Output<A>) {
    match item {
        Item::Row(row) | Item::List(row) => {
            if row.disabled.is_some() {
                return;
            }
            out.run = Some(row.action);
            out.keep = matches!(item, Item::List(_)) || row.mark != Mark::None;
        }
        Item::Segments(segments) => {
            if segments.disabled.is_some() {
                return;
            }
            if let Some(s) = segments.segments.get(segment.unwrap_or(0)) {
                out.run = Some(s.action);
                out.keep = true;
            }
        }
    }
}

/// Keys without modifiers, in the order they were pressed, taken out of the input so the
/// photo never sees them.
fn keyboard<A: Copy>(
    ui: &Ui,
    state: &mut State,
    bar: &Bar<A>,
    lines: &mut Vec<Line>,
    out: &mut Output<A>,
    ran: &mut Option<Line>,
) {
    let (keys, letters) = ui.ctx().input_mut(|i| {
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
    if !keys.is_empty() || !letters.is_empty() {
        state.follow = true;
    }
    for key in keys {
        let Some(line) = state.cursor else {
            state.cursor = lines.first().copied();
            continue;
        };
        let item = bar.item(line);
        match key {
            Key::Escape => out.leave = true,
            Key::ArrowDown | Key::ArrowUp => {
                let at = lines.iter().position(|l| *l == line).unwrap_or(0);
                let len = lines.len().max(1);
                let next = if key == Key::ArrowDown {
                    (at + 1) % len
                } else {
                    (at + len - 1) % len
                };
                state.cursor = lines.get(next).copied();
                state.segment = state
                    .cursor
                    .and_then(|l| match bar.item(l) {
                        Some(Item::Segments(segments)) => Some(segments.start()),
                        _ => None,
                    })
                    .unwrap_or(0);
            }
            Key::Enter => match (line, item) {
                (Line::Title(s), _) => {
                    let id = bar.sections[s].id;
                    let open = state.open.contains(id);
                    state.fold(id, !open);
                    out.folded = true;
                }
                (_, Some(item)) => {
                    run(item, Some(state.segment), out);
                    *ran = Some(line);
                }
                _ => {}
            },
            Key::ArrowRight => match (line, item) {
                (Line::Title(s), _) => {
                    let id = bar.sections[s].id;
                    if !state.open.contains(id) {
                        state.fold(id, true);
                        out.folded = true;
                    }
                }
                (_, Some(Item::Segments(segments))) => {
                    state.segment =
                        (state.segment + 1).min(segments.segments.len().saturating_sub(1));
                }
                (_, Some(item @ Item::List(_))) => {
                    run(item, None, out);
                    *ran = Some(line);
                }
                _ => {}
            },
            Key::ArrowLeft => match (line, item) {
                (_, Some(Item::Segments(_))) if state.segment > 0 => state.segment -= 1,
                (Line::Item(s, _), _) => {
                    // Back on the title, and the group folds.
                    state.cursor = Some(Line::Title(s));
                    state.fold(bar.sections[s].id, false);
                    out.folded = true;
                }
                (Line::Title(s), _) => {
                    let id = bar.sections[s].id;
                    if state.open.contains(id) {
                        state.fold(id, false);
                        out.folded = true;
                    }
                }
                _ => {}
            },
            _ => {}
        }
        *lines = bar.lines(&state.open);
    }
    for letter in letters {
        let labels: Vec<&str> = lines.iter().map(|&line| bar.label(line)).collect();
        let at = state
            .cursor
            .and_then(|c| lines.iter().position(|l| *l == c));
        if let Some(i) = palette::jump(&labels, at, letter) {
            state.cursor = lines.get(i).copied();
            state.segment = 0;
        }
    }
}

/// A group's title: muted capitals behind a fold triangle. Returns whether it was clicked.
fn title(ui: &mut Ui, area: Rect, id: Id, label: &str, open: bool, lit: bool) -> bool {
    let response = ui
        .interact(area, id, Sense::CLICK)
        .on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    if response.hovered() || lit {
        painter.rect_filled(area, 5.0, tokens::ACCENT_SUBTLE);
    }
    let y = area.center().y + 2.0;
    icons::chevron(painter, pos2(area.left() + 14.0, y), open, tokens::MUTED);
    let mut job = LayoutJob::simple_singleline(
        label.to_uppercase(),
        FontId::proportional(text::SMALL),
        tokens::MUTED,
    );
    job.wrap = TextWrapping::truncate_at_width(area.width() - 36.0);
    let galley = painter.layout_job(job);
    let size = galley.size();
    painter.galley(
        pos2(area.left() + 26.0, y - size.y / 2.0),
        galley,
        tokens::MUTED,
    );
    response.clicked()
}

/// Draws an item; returns what a click hit.
fn draw_item<A>(
    ui: &mut Ui,
    area: Rect,
    id: Id,
    item: &Item<A>,
    lit: bool,
    segment: Option<usize>,
) -> Option<Clicked> {
    match item {
        Item::Row(row) => palette::bar_row(ui, area, id, row, false, lit)
            .clicked
            .then_some(Clicked::Row),
        Item::List(row) => palette::bar_row(ui, area, id, row, true, lit)
            .clicked
            .then_some(Clicked::Row),
        Item::Segments(segments) => draw_segments(ui, area, id, segments, lit, segment),
    }
}

/// A label and its segments – beside it for icons, under it for text.
fn draw_segments<A>(
    ui: &mut Ui,
    area: Rect,
    id: Id,
    segments: &Segments<A>,
    lit: bool,
    cursor: Option<usize>,
) -> Option<Clicked> {
    let disabled = segments.disabled.is_some();
    let colour = if disabled {
        tokens::MUTED
    } else {
        tokens::TEXT
    };
    let painter = ui.painter().clone();
    if lit {
        painter.rect_filled(area, 5.0, tokens::ACCENT_SUBTLE);
    }
    let label_y = area.top() + ROW / 2.0;
    let text = segments.text();
    let n = segments.segments.len();
    let rects: Vec<Rect> = if text {
        let texts: Vec<f32> = segments
            .segments
            .iter()
            .map(|segment| match &segment.look {
                Look::Text(label) => text_width(&painter, label, text::SMALL) + SEGMENT_PAD,
                _ => SEGMENT,
            })
            .collect();
        text_rects(area, &texts)
    } else {
        icon_rects(area, n)
    };
    let label_right = if text {
        area.right() - 10.0
    } else {
        rects.first().map_or(area.right(), |r| r.left()) - 6.0
    };
    let mut job = LayoutJob::simple_singleline(
        segments.label.clone(),
        FontId::proportional(text::BODY),
        colour,
    );
    job.wrap = TextWrapping::truncate_at_width((label_right - area.left() - LABEL_LEFT).max(20.0));
    let galley = painter.layout_job(job);
    let size = galley.size();
    painter.galley(
        pos2(area.left() + LABEL_LEFT, label_y - size.y / 2.0),
        galley,
        colour,
    );
    if let Some(reason) = segments.disabled {
        let label_area = Rect::from_min_max(area.min, pos2(label_right, area.top() + ROW));
        ui.interact(label_area, id.with("label"), Sense::hover())
            .on_hover_text(reason);
    }

    let mut clicked = None;
    for (k, (segment, rect)) in segments.segments.iter().zip(&rects).enumerate() {
        let response = ui.interact(*rect, id.with(("segment", k)), Sense::CLICK);
        let response = match segments.disabled {
            Some(reason) => response.on_hover_text(reason),
            None => response
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(segment.tooltip.as_str()),
        };
        let hovered = response.hovered() && !disabled;
        if response.clicked() && !disabled {
            clicked = Some(Clicked::Segment(k));
        }
        if hovered || cursor == Some(k) {
            painter.rect_filled(*rect, 4.0, tokens::ACCENT_SUBTLE);
        }
        if text {
            let stroke = if segment.on {
                tokens::ACCENT
            } else {
                tokens::LINE
            };
            painter.rect_stroke(*rect, 4.0, Stroke::new(1.0, stroke), StrokeKind::Inside);
        } else if segment.on {
            painter.rect_stroke(
                *rect,
                4.0,
                Stroke::new(1.2, tokens::ACCENT),
                StrokeKind::Inside,
            );
        }
        let c = rect.center();
        let ink = if disabled {
            tokens::MUTED
        } else if segment.on || hovered {
            tokens::ACCENT_STRONG
        } else {
            tokens::TEXT
        };
        match &segment.look {
            Look::Text(label) => {
                let mut job = LayoutJob::simple_singleline(
                    label.clone(),
                    FontId::proportional(text::SMALL),
                    ink,
                );
                job.wrap = TextWrapping::truncate_at_width(rect.width() - SEGMENT_PAD / 2.0);
                let galley = painter.layout_job(job);
                let size = galley.size();
                painter.galley(c - size / 2.0, galley, ink);
            }
            Look::Swatch(swatch) => {
                let fill = if disabled {
                    swatch.gamma_multiply(0.4)
                } else {
                    *swatch
                };
                painter.circle_filled(c, 5.5, fill);
            }
            Look::NoColour => {
                painter.circle_stroke(c, 5.0, Stroke::new(1.2, tokens::MUTED));
            }
            Look::Turn(clockwise) => icons::turn(&painter, c, *clockwise, ink),
        }
    }
    clicked
}

/// Room around a text segment's label.
const SEGMENT_PAD: f32 = 10.0;

/// Where a label starts in its row: after the tick or icon column.
const LABEL_LEFT: f32 = 30.0;

fn text_width(painter: &eframe::egui::Painter, text: &str, size: f32) -> f32 {
    painter
        .layout_no_wrap(text.to_owned(), FontId::proportional(size), Color32::WHITE)
        .size()
        .x
}

/// Icon segments (colours, turns) right-aligned beside the label, each `SEGMENT` wide.
fn icon_rects(area: Rect, n: usize) -> Vec<Rect> {
    let right = area.right() - 6.0;
    let y = area.top() + ROW / 2.0;
    (0..n)
        .map(|k| {
            let x = right - (n - k) as f32 * SEGMENT;
            Rect::from_min_size(pos2(x, y - SEGMENT / 2.0), vec2(SEGMENT, SEGMENT))
        })
        .collect()
}

/// Text segments on the line under the label, sharing it by their labels' widths: each gets
/// its own and an equal part of what is left, so a long word ("Desactivada") isn't cut while
/// a short one has room to spare.
fn text_rects(area: Rect, widths: &[f32]) -> Vec<Rect> {
    let left = area.left() + LABEL_LEFT;
    let room = area.right() - 6.0 - left;
    let sum: f32 = widths.iter().sum();
    let spare = (room - sum) / widths.len().max(1) as f32;
    let mut x = left;
    widths
        .iter()
        .map(|w| {
            let w = if spare >= 0.0 {
                w + spare
            } else {
                w * room / sum.max(1.0)
            };
            let rect = Rect::from_min_size(pos2(x, area.top() + ROW), vec2(w, SEGMENT));
            x += w;
            rect.shrink2(vec2(1.0, 0.0))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, Modifiers, RawInput};

    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn bar() -> Bar<u8> {
        Bar {
            top: vec![Item::Row(Row::new(1, "Ordner öffnen", None))],
            sections: vec![
                Section {
                    id: "photo",
                    title: "Dieses Foto".into(),
                    items: vec![
                        Item::Segments(Segments {
                            label: "Farbe".into(),
                            segments: (10..13)
                                .map(|action| Segment {
                                    action,
                                    look: Look::NoColour,
                                    tooltip: String::new(),
                                    on: action == 11,
                                })
                                .collect(),
                            disabled: None,
                        }),
                        Item::Row(Row::new(2, "Ablehnen", Some("X".into())).toggle(false)),
                        Item::Row(Row::new(3, "Vergleichen", Some("C".into()))),
                        Item::List(Row::new(4, "Extern bearbeiten", Some("E".into()))),
                    ],
                },
                Section {
                    id: "view",
                    title: "Ansicht".into(),
                    items: vec![Item::Row(Row::new(5, "Raster", Some("F7".into())))],
                },
            ],
        }
    }

    const WINDOW: Rect = Rect {
        min: pos2(0.0, 0.0),
        max: pos2(900.0, 700.0),
    };

    fn area() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH, 700.0))
    }

    /// One frame of the bar; `consumed` says which of the events nobody took.
    fn frame(state: &mut State, bar: &Bar<u8>, events: Vec<Event>) -> (Output<u8>, usize) {
        let ctx = Context::default();
        let mut result = None;
        let mut left = 0;
        let listening = state.focus;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(WINDOW),
                events,
                ..Default::default()
            },
            |ui| {
                result = Some(show(ui, area(), state, bar, listening));
                left = ui.input(|i| i.events.len());
            },
        );
        output.textures_delta.clear();
        (result.unwrap(), left)
    }

    #[test]
    fn without_the_keyboard_the_bar_takes_no_key() {
        let mut state = State::default();
        let (out, left) = frame(
            &mut state,
            &bar(),
            vec![
                key(Key::ArrowDown),
                key(Key::Enter),
                Event::Text("x".into()),
            ],
        );
        assert!(out.run.is_none());
        assert_eq!(left, 3, "every key stays for the photo");
    }

    #[test]
    fn arrows_and_enter_run_a_row_and_escape_gives_the_keyboard_back() {
        let bar = bar();
        let mut state = State::default();
        state.take_keyboard();
        frame(&mut state, &bar, Vec::new());
        // Open folder, This photo, Colour, Reject: Enter toggles it and the bar keeps the
        // keyboard (a switch).
        let (out, left) = frame(
            &mut state,
            &bar,
            vec![
                key(Key::ArrowDown),
                key(Key::ArrowDown),
                key(Key::ArrowDown),
                key(Key::Enter),
            ],
        );
        assert_eq!(left, 0, "the bar took its keys");
        assert_eq!(out.run, Some(2));
        assert!(out.keep);
        // Compare: a plain command, the keyboard goes back.
        let (out, _) = frame(&mut state, &bar, vec![key(Key::ArrowDown), key(Key::Enter)]);
        assert_eq!(out.run, Some(3));
        assert!(!out.keep);
        let (out, _) = frame(&mut state, &bar, vec![key(Key::Escape)]);
        assert!(out.leave);
    }

    #[test]
    fn segments_step_with_left_and_right() {
        let bar = bar();
        let mut state = State::default();
        state.take_keyboard_at("photo", Some(0));
        frame(&mut state, &bar, Vec::new());
        // It lands on the current value (the middle one); → and Enter take the last.
        let (out, _) = frame(
            &mut state,
            &bar,
            vec![key(Key::ArrowRight), key(Key::Enter)],
        );
        assert_eq!(out.run, Some(12));
        assert!(out.keep, "a segment keeps the keyboard");
        // ← twice to the first, a third ← folds the group and lands on its title.
        let (out, _) = frame(
            &mut state,
            &bar,
            vec![key(Key::ArrowLeft), key(Key::ArrowLeft), key(Key::Enter)],
        );
        assert_eq!(out.run, Some(10));
        let (out, _) = frame(&mut state, &bar, vec![key(Key::ArrowLeft)]);
        assert!(out.folded);
        assert!(!state.open.contains("photo"));
        assert_eq!(state.cursor, Some(Line::Title(0)));
        // → unfolds it again.
        frame(&mut state, &bar, vec![key(Key::ArrowRight)]);
        assert!(state.open.contains("photo"));
    }

    #[test]
    fn a_list_row_keeps_the_keyboard_and_says_where_it_is() {
        let bar = bar();
        let mut state = State::default();
        state.take_keyboard_at("photo", Some(3));
        frame(&mut state, &bar, Vec::new());
        let (out, _) = frame(&mut state, &bar, vec![key(Key::ArrowRight)]);
        assert_eq!(out.run, Some(4));
        assert!(out.keep);
        assert!(out.row.is_some(), "the list opens beside its row");
    }

    #[test]
    fn letters_jump_and_titles_fold() {
        let bar = bar();
        let mut state = State::default();
        state.take_keyboard();
        // The key that brought the keyboard (`E` opens the programs' list) jumps nowhere.
        let (_, left) = frame(&mut state, &bar, vec![Event::Text("a".into())]);
        assert_eq!(state.cursor, Some(Line::Top(0)), "still on the first line");
        assert_eq!(left, 1);
        frame(&mut state, &bar, vec![Event::Text("a".into())]);
        assert_eq!(state.cursor, Some(Line::Item(0, 1)), "Ablehnen");
        frame(&mut state, &bar, vec![Event::Text("a".into())]);
        assert_eq!(state.cursor, Some(Line::Title(1)), "Ansicht");
        let (out, _) = frame(&mut state, &bar, vec![key(Key::Enter)]);
        assert!(out.folded && state.open.contains("view"));
        assert_eq!(state.saved(), "photo,view");
        assert_eq!(State::restore(Some("view")).saved(), "view");
    }

    #[test]
    fn a_click_runs_what_the_key_runs() {
        let bar = bar();
        let mut state = State::default();
        // Lines: 4 + 28 Open folder, 6 + 30 This photo, 28 Colour, then Reject.
        let reject = pos2(100.0, 4.0 + ROW + GAP + HEADER + ROW + ROW / 2.0);
        let ctx = Context::default();
        let mut runs = Vec::new();
        for events in [
            vec![Event::PointerMoved(reject)],
            vec![Event::PointerButton {
                pos: reject,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
            vec![Event::PointerButton {
                pos: reject,
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        ] {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(WINDOW),
                    events,
                    ..Default::default()
                },
                |ui| runs.push(show(ui, area(), &mut state, &bar, false).run),
            );
            output.textures_delta.clear();
        }
        assert_eq!(runs.last().copied().flatten(), Some(2));
        assert!(!state.focus, "a click never takes the keyboard");
        assert!(
            ctx.memory(|m| m.focused().is_none()),
            "nor egui's focus: Space would click the row again"
        );
    }

    /// The short labels of the rows with segments fit in every language: the colour row's
    /// beside its six dots, the overlay's three under it. egui's own font stands in for Segoe
    /// UI (about as wide on the screenshots), with 5 % to spare.
    #[test]
    fn the_segment_labels_fit_in_every_language() {
        let ctx = Context::default();
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            let painter = ui.painter();
            let row = Rect::from_min_size(pos2(4.0, 0.0), vec2(WIDTH - 9.0, ROW + SEGMENT));
            let dots = icon_rects(row, 6);
            let room = dots[0].left() - 6.0 - row.left() - LABEL_LEFT;
            for lang in crate::i18n::Lang::ALL {
                let t = lang.texts();
                let colour = text_width(painter, t.bar_colour, text::BODY) * 1.05;
                assert!(
                    colour <= room,
                    "{lang:?}: {} {colour} > {room}",
                    t.bar_colour
                );
                let labels = [
                    t.overlay_off,
                    t.bar_overlay_sharpness,
                    t.bar_overlay_exposure,
                ];
                let widths: Vec<f32> = labels
                    .iter()
                    .map(|label| text_width(painter, label, text::SMALL) * 1.05 + SEGMENT_PAD)
                    .collect();
                for (label, rect) in labels.iter().zip(text_rects(row, &widths)) {
                    let width = text_width(painter, label, text::SMALL) * 1.05;
                    assert!(
                        width <= rect.width() - SEGMENT_PAD / 2.0,
                        "{lang:?}: {label} {width} > {}",
                        rect.width()
                    );
                }
            }
        });
        output.textures_delta.clear();
    }

    #[test]
    fn a_greyed_out_row_runs_nothing() {
        let bar = Bar {
            top: vec![Item::Row(
                Row::new(1u8, "Verschieben", None).disabled(Some("läuft schon")),
            )],
            sections: Vec::new(),
        };
        let mut state = State::default();
        state.take_keyboard();
        frame(&mut state, &bar, Vec::new());
        let (out, _) = frame(&mut state, &bar, vec![key(Key::Enter)]);
        assert!(out.run.is_none());
    }
}
