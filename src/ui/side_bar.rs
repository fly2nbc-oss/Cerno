//! The menu bar on the left (since 1.11, ticket CER-35, the user's decisions F1–F3 of
//! 2026-10-10): the menu as groups that fold, beside the photo like the details panel on the
//! other side. Nothing in it has another home – the stars are in the info bar, sort and
//! filters in the filter bar, the panels' buttons in the info bar.
//!
//! It is for the mouse (since 1.12, the user's decision of 2026-10-10 – the bar is for
//! beginners; `F10` shows and hides it like `T`, `Tab` and `F6` the other bars): it takes no
//! key. Its rows never take egui's focus (`Sense::CLICK`), so `Space` can't click one again.

use std::collections::BTreeSet;

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    Color32, CursorIcon, FontId, Id, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui, UiBuilder,
    pos2, vec2,
};

use crate::theme::{text, tokens};
use crate::ui::palette::{self, Row};
use crate::ui::{icons, stars};

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
    /// An empty star: no stars.
    NoStars,
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
}

impl<A> Item<A> {
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
}

/// Which groups are open.
#[derive(Debug)]
pub struct State {
    open: BTreeSet<String>,
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
        Self { open }
    }

    /// The open groups, for the settings.
    pub fn saved(&self) -> String {
        self.open
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(",")
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
    /// A group folded or unfolded: its state is saved.
    pub folded: bool,
}

impl<A> Default for Output<A> {
    fn default() -> Self {
        Self {
            run: None,
            row: None,
            folded: false,
        }
    }
}

/// Draws the bar in `rect`: a click runs a row or a segment, or folds a group.
pub fn show<A: Copy>(ui: &mut Ui, rect: Rect, state: &mut State, bar: &Bar<A>) -> Output<A> {
    let mut out = Output::default();
    let lines = bar.lines(&state.open);
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
                let line_id = id.with(line_key(line));
                match line {
                    Line::Title(s) => {
                        let section = &bar.sections[s];
                        let open = state.open.contains(section.id);
                        if title(ui, area, line_id, &section.title, open) {
                            state.fold(section.id, !open);
                            out.folded = true;
                        }
                    }
                    _ => {
                        let Some(item) = bar.item(line) else {
                            continue;
                        };
                        if let Some(clicked) = draw_item(ui, area, line_id, item) {
                            out.row = Some(area);
                            match clicked {
                                Clicked::Row => run(item, None, &mut out),
                                Clicked::Segment(k) => run(item, Some(k), &mut out),
                            }
                        }
                    }
                }
            }
            ui.add_space(8.0);
        });
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
        }
        Item::Segments(segments) => {
            if segments.disabled.is_some() {
                return;
            }
            if let Some(s) = segments.segments.get(segment.unwrap_or(0)) {
                out.run = Some(s.action);
            }
        }
    }
}

/// A group's title: muted capitals behind a fold triangle. Returns whether it was clicked.
fn title(ui: &mut Ui, area: Rect, id: Id, label: &str, open: bool) -> bool {
    let response = ui
        .interact(area, id, Sense::CLICK)
        .on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    if response.hovered() {
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
fn draw_item<A>(ui: &mut Ui, area: Rect, id: Id, item: &Item<A>) -> Option<Clicked> {
    match item {
        Item::Row(row) => palette::bar_row(ui, area, id, row, false, false)
            .clicked
            .then_some(Clicked::Row),
        Item::List(row) => palette::bar_row(ui, area, id, row, true, false)
            .clicked
            .then_some(Clicked::Row),
        Item::Segments(segments) => draw_segments(ui, area, id, segments),
    }
}

/// A label and its segments – beside it for icons, under it for text.
fn draw_segments<A>(ui: &mut Ui, area: Rect, id: Id, segments: &Segments<A>) -> Option<Clicked> {
    let disabled = segments.disabled.is_some();
    let colour = if disabled {
        tokens::MUTED
    } else {
        tokens::TEXT
    };
    let painter = ui.painter().clone();
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
        if hovered {
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
            Look::NoStars => stars::paint_star(&painter, c, 6.5, false, ink),
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
    use eframe::egui::{Context, Event, Modifiers, Pos2, RawInput};

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

    /// The open groups are saved and come back.
    #[test]
    fn the_open_groups_are_saved() {
        let mut state = State::default();
        assert_eq!(state.saved(), FIRST_OPEN);
        state.fold("view", true);
        assert_eq!(state.saved(), "photo,view");
        assert_eq!(State::restore(Some("view")).saved(), "view");
        assert_eq!(State::restore(Some("")).saved(), "");
    }

    const WINDOW: Rect = Rect {
        min: pos2(0.0, 0.0),
        max: pos2(900.0, 700.0),
    };

    fn area() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH, 700.0))
    }

    /// A click at `at` (moved there, pressed, released): what ran, and the context.
    fn click(state: &mut State, bar: &Bar<u8>, at: Pos2) -> (Option<u8>, Context) {
        let ctx = Context::default();
        let mut ran = None;
        for events in [
            vec![Event::PointerMoved(at)],
            vec![Event::PointerButton {
                pos: at,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
            vec![Event::PointerButton {
                pos: at,
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
                |ui| ran = show(ui, area(), state, bar).run.or(ran),
            );
            output.textures_delta.clear();
        }
        (ran, ctx)
    }

    #[test]
    fn a_click_runs_a_row_and_takes_no_focus() {
        // Lines: 4 + 28 Open folder, 6 + 30 This photo, 28 Colour, then Reject.
        let reject = pos2(100.0, 4.0 + ROW + GAP + HEADER + ROW + ROW / 2.0);
        let (ran, ctx) = click(&mut State::default(), &bar(), reject);
        assert_eq!(ran, Some(2));
        assert!(
            ctx.memory(|m| m.focused().is_none()),
            "a click takes no egui focus: Space would click the row again"
        );
    }

    /// A greyed-out row says why in its tooltip and runs nothing.
    #[test]
    fn a_greyed_out_row_runs_nothing() {
        let bar = Bar {
            top: vec![Item::Row(
                Row::new(1u8, "Verschieben", None).disabled(Some("läuft schon")),
            )],
            sections: Vec::new(),
        };
        let (ran, _) = click(&mut State::default(), &bar, pos2(100.0, 4.0 + ROW / 2.0));
        assert_eq!(ran, None);
        let open = Bar {
            top: vec![Item::Row(Row::new(1u8, "Verschieben", None))],
            sections: Vec::new(),
        };
        let (ran, _) = click(&mut State::default(), &open, pos2(100.0, 4.0 + ROW / 2.0));
        assert_eq!(ran, Some(1), "the same row, not greyed out, runs");
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
                // The stars' segments are text: their label has the line to itself.
                let stars = text_width(painter, t.bar_stars, text::BODY) * 1.05;
                let line = row.width() - 10.0 - LABEL_LEFT;
                assert!(stars <= line, "{lang:?}: {} {stars} > {line}", t.bar_stars);
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
}
