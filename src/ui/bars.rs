//! Toolbar (top), info bar (bottom), notices and the drop hint.

use eframe::egui::containers::scroll_area::ScrollBarVisibility;
use eframe::egui::text::{CCursor, LayoutJob, TextFormat};
use eframe::egui::{
    Align, Align2, Button, Color32, ComboBox, CursorIcon, FontId, Galley, Layout, Painter, Pos2,
    Rect, Response, RichText, ScrollArea, Sense, Sides, Stroke, StrokeKind, Ui, UiBuilder, Vec2,
    pos2, vec2,
};
use std::sync::Arc;

use crate::analysis::{ModelState, Status, aesthetic};
use crate::i18n::{self, Lang};
use crate::loader::LoadedImage;
use crate::metadata::{self, Label, Rating};
use crate::theme::{self, text, tokens};
use crate::ui::icons;
use crate::ui::stars;
use crate::view::{FilterKind, SortKey, ViewOptions};

pub const TOOLBAR_HEIGHT: f32 = 40.0;
pub const INFO_HEIGHT: f32 = 60.0;
const STAR_SIZE: f32 = 16.0;
const STAR_GAP: f32 = 6.0;
/// Click area of the info bar buttons (design system: at least 32 px).
const BUTTON: f32 = 32.0;

pub struct ToolbarInfo<'a> {
    /// New scores arrived since the view was sorted/filtered.
    pub stale: bool,
    pub status: &'a Status,
    /// The action menu (copy, move, delete) is open.
    pub actions_open: bool,
}

#[derive(Default)]
pub struct ToolbarOutput {
    pub options_changed: bool,
    pub refresh: bool,
    pub download_model: bool,
    /// The "Action" button was clicked (opens or closes the action menu).
    pub toggle_actions: bool,
    /// Where the "Action" button is, so the menu opens under it.
    pub actions_anchor: Option<Rect>,
}

pub fn toolbar(
    ui: &mut Ui,
    rect: Rect,
    options: &mut ViewOptions,
    info: &ToolbarInfo<'_>,
) -> ToolbarOutput {
    let t = i18n::t();
    ui.painter().rect_filled(rect, 0.0, tokens::SURFACE);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, tokens::LINE),
    );
    let before = *options;
    let mut out = ToolbarOutput::default();
    let mut action_rect = None;
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(12.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            // One line: sort and the filter boxes on the left, action and analysis status on
            // the right. The boxes scroll sideways when they no longer fit.
            Sides::new()
                .height(ui.available_height())
                .shrink_left()
                .show(
                    ui,
                    |ui| {
                        ComboBox::from_id_salt("sort")
                            .selected_text((t.sort)(options.sort.label()))
                            .show_ui(ui, |ui| {
                                for key in SortKey::ALL {
                                    ui.selectable_value(&mut options.sort, key, key.label());
                                }
                            });
                        let boxes = ScrollArea::horizontal()
                            .id_salt("filter-boxes")
                            .max_width(ui.available_width())
                            .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    if !options.filter.is_all()
                                        && ui.small_button(t.filter_clear).clicked()
                                    {
                                        options.filter.clear();
                                    }
                                    for kind in FilterKind::ALL {
                                        let mut on = options.filter.contains(kind);
                                        let response = match kind {
                                            // Colours as small squares: five names would not
                                            // fit next to the rest.
                                            FilterKind::Colour(label) => colour_box(
                                                ui,
                                                &mut on,
                                                theme::label_color(label),
                                                i18n::label_name(label),
                                            ),
                                            FilterKind::Stars(n) => {
                                                ui.checkbox(&mut on, format!("{n}★"))
                                            }
                                            FilterKind::Blurry => ui
                                                .checkbox(&mut on, kind.label())
                                                .on_hover_text(t.filter_blurry_tooltip),
                                            FilterKind::Duplicate => ui
                                                .checkbox(&mut on, kind.label())
                                                .on_hover_text(t.filter_duplicate_tooltip),
                                            _ => ui.checkbox(&mut on, kind.label()),
                                        };
                                        if response.changed() {
                                            options.filter.set(kind, on);
                                        }
                                    }
                                });
                            });
                        overflow_hint(
                            ui.painter(),
                            boxes.inner_rect,
                            boxes.content_size.x,
                            boxes.state.offset.x,
                        );
                    },
                    |ui| {
                        // Only what needs attention: the model is missing, loading or failed, or
                        // the analysis is still running. Where the model runs is on the models
                        // card.
                        aesthetics_status(ui, &info.status.aesthetics, &mut out);
                        let Status { done, total, .. } = *info.status;
                        if done < total {
                            let text = (t.analyzing_progress)(done, total);
                            ui.label(RichText::new(text).color(tokens::MUTED));
                        }
                        if info.stale
                            && ui
                                .button(t.refresh_order)
                                .on_hover_text(t.refresh_order_tooltip)
                                .clicked()
                        {
                            out.refresh = true;
                        }
                        let action = ui
                            .add(Button::new(t.actions).selected(info.actions_open))
                            .on_hover_text(format!(
                                "{}\n{}",
                                t.actions_tooltip,
                                i18n::with_ctrl("M")
                            ));
                        action_rect = Some(action.rect);
                        if action.clicked() {
                            out.toggle_actions = true;
                        }
                    },
                );
        },
    );
    out.actions_anchor = action_rect;
    out.options_changed = *options != before;
    out
}

/// A colour label as a small square – filled and ticked when on; the name is the tooltip.
fn colour_box(ui: &mut Ui, on: &mut bool, colour: Color32, name: &str) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let painter = ui.painter();
    let square = Rect::from_center_size(rect.center(), vec2(14.0, 14.0));
    if *on {
        painter.rect_filled(square, 3.0, colour);
        let c = square.center();
        let stroke = Stroke::new(1.8, tokens::BG);
        painter.line_segment([c + vec2(-3.5, 0.0), c + vec2(-1.0, 2.8)], stroke);
        painter.line_segment([c + vec2(-1.0, 2.8), c + vec2(3.8, -3.2)], stroke);
    } else {
        painter.rect_stroke(square, 3.0, Stroke::new(1.5, colour), StrokeKind::Inside);
    }
    if response.hovered() {
        painter.rect_stroke(
            square.expand(2.0),
            4.0,
            Stroke::new(1.0, tokens::ACCENT),
            StrokeKind::Outside,
        );
    }
    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(name)
}

/// Fades the edge where filter boxes are scrolled out of view, so hidden ones are noticed.
fn overflow_hint(painter: &Painter, inner: Rect, content_width: f32, offset: f32) {
    const WIDTH: f32 = 28.0;
    const STEPS: usize = 14;
    let hidden_left = offset > 1.0;
    let hidden_right = offset + inner.width() < content_width - 1.0;
    let [r, g, b, _] = tokens::SURFACE.to_array();
    for (hidden, edge, direction) in [
        (hidden_left, inner.left(), 1.0),
        (hidden_right, inner.right(), -1.0),
    ] {
        if !hidden {
            continue;
        }
        for step in 0..STEPS {
            let (a, b_) = (step as f32 / STEPS as f32, (step + 1) as f32 / STEPS as f32);
            let (x0, x1) = (edge + direction * a * WIDTH, edge + direction * b_ * WIDTH);
            let alpha = ((1.0 - a) * 255.0) as u8;
            painter.rect_filled(
                Rect::from_x_y_ranges(x0.min(x1)..=x0.max(x1), inner.y_range()),
                0.0,
                Color32::from_rgba_unmultiplied(r, g, b, alpha),
            );
        }
    }
}

fn aesthetics_status(ui: &mut Ui, state: &ModelState, out: &mut ToolbarOutput) {
    let t = i18n::t();
    let muted = |text: String| RichText::new(text).color(tokens::MUTED);
    match state {
        ModelState::Missing => {
            if ui
                .button(t.enable_aesthetics)
                .on_hover_text(t.enable_aesthetics_tooltip)
                .clicked()
            {
                out.download_model = true;
            }
        }
        ModelState::Downloading { received, total } => {
            let percent = *received as f64 / (*total).max(1) as f64 * 100.0;
            ui.label(muted((t.downloading_model)(percent)));
        }
        ModelState::Loading => {
            ui.label(muted(t.aesthetics_loading.into()));
        }
        ModelState::Available | ModelState::Ready { .. } => {}
        ModelState::Failed(message) => {
            ui.label(RichText::new(t.aesthetics_failed).color(tokens::STATUS_ERROR))
                .on_hover_text(message);
        }
    }
}

/// Which optional parts of the window are shown (the info bar always is).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panels {
    pub toolbar: bool,
    pub details: bool,
    pub filmstrip: bool,
}

pub struct InfoBar<'a> {
    pub name: &'a str,
    /// 1-based position and count.
    pub position: (usize, usize),
    pub image: Option<&'a LoadedImage>,
    pub rating: Rating,
    pub label: Option<Label>,
    /// 1-based position in the series and its length.
    pub series: Option<(u32, u32)>,
    /// Display name of the earlier photo with the same pixels.
    pub duplicate_of: Option<String>,
    pub auto_advance: bool,
    /// Some analysis result is known for this photo.
    pub analysed: bool,
    /// LAION and V2.5 scores, 1..10.
    pub aesthetics: [Option<f32>; 2],
    /// Personal taste model, 0..=5.
    pub personal: Option<f32>,
    /// (percentile within the folder, measured at the eyes).
    pub sharpness: Option<(f32, bool)>,
    /// The subject is probably out of focus (`view::is_blurry`).
    pub blurry: bool,
    pub saving: bool,
    /// Viewer zoom in percent while zoomed in.
    pub zoom: Option<f32>,
}

#[derive(Default)]
pub struct InfoBarOutput {
    /// The star the user clicked (`Some(None)` clears the rating).
    pub rating: Option<Rating>,
    pub help: bool,
    pub menu: bool,
    /// Google Maps link of the photo's position.
    pub open_map: Option<String>,
}

/// Two rows: name / position · date · size (left), stars / scores (centre), exposure / camera
/// and lens (right), then the buttons at the far right.
pub fn info_bar(ui: &Ui, rect: Rect, bar: &InfoBar<'_>) -> InfoBarOutput {
    let t = i18n::t();
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, tokens::SURFACE);
    painter.hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(1.0, tokens::LINE),
    );
    let mut out = InfoBarOutput::default();
    let (row1, row2) = (rect.top() + 19.0, rect.top() + 42.0);

    let buttons_left = buttons(ui, rect, bar, &mut out);

    // Centre: scores as value (+ bar) below the stars; its width decides the side columns.
    let meters = if bar.analysed {
        meters(bar)
    } else {
        vec![Meter {
            label: String::new(),
            value: vec![Piece::Value(t.analyzing.to_owned())],
            fraction: None,
            color: tokens::MUTED,
            tooltip: None,
            small: true,
        }]
    };
    let laid = layout_meters(painter, &meters);
    // The centre is as wide as it can get in this language, so the side columns stay put
    // while browsing (the blurry note, "Eyes" instead of "Sharpness", "Analysing…").
    let widest = widest_centre(painter);
    let stars_width = 5.0 * STAR_SIZE + 4.0 * STAR_GAP;
    let centre_half = (laid.width.max(widest) / 2.0).max(stars_width / 2.0) + 8.0;
    let centre = rect.center().x;
    for (area, tooltip) in paint_meters(painter, centre, row2, laid) {
        if let Some(tooltip) = tooltip {
            ui.interact(area, ui.id().with(("meter", tooltip)), Sense::hover())
                .on_hover_text(tooltip);
        }
    }

    // Left.
    let left = painter.with_clip_rect(Rect::from_min_max(
        rect.min,
        pos2(centre - centre_half - 12.0, rect.max.y),
    ));
    let x = rect.left() + 12.0;
    left.text(
        pos2(x, row1),
        Align2::LEFT_CENTER,
        bar.name,
        FontId::proportional(text::BODY),
        tokens::TEXT,
    );
    // Size and load time live in the details panel. What doesn't fit is left out whole, the
    // capture date first.
    let mut facts = vec![format!("{} / {}", bar.position.0, bar.position.1)];
    if bar.auto_advance {
        facts.push(t.auto_advance_on.to_owned());
    }
    if let Some(zoom) = bar.zoom {
        facts.push((t.zoom)(zoom));
    }
    if let Some((index, len)) = bar.series {
        facts.push((t.series_position)(index, len));
    }
    if let Some(name) = &bar.duplicate_of {
        facts.push((t.duplicate_of)(name));
    }
    if let Some(taken) = bar.image.and_then(|image| image.camera.taken.as_ref()) {
        facts.push(i18n::date(taken));
    }
    let font = FontId::proportional(text::SMALL);
    let room = (centre - centre_half - 12.0 - x).max(0.0);
    let text = fit_parts(painter, facts, "   ·   ", &font, room, Drop::Back);
    left.text(
        pos2(x, row2),
        Align2::LEFT_CENTER,
        text,
        font,
        tokens::MUTED,
    );

    // Right, up to the buttons. Parts that don't fit are left out whole.
    let right_left = centre + centre_half + 12.0;
    let x = buttons_left - 12.0;
    let room = (x - right_left).max(0.0);
    let right = painter.with_clip_rect(Rect::from_min_max(
        pos2(right_left, rect.min.y),
        pos2(buttons_left - 10.0, rect.max.y),
    ));
    if let Some(image) = bar.image {
        let mut exposure: Vec<String> = image
            .camera
            .exposure_line()
            .split("  ·  ")
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect();
        if let Some(ratio) = image.camera.digital_zoom {
            exposure.push((t.digital_zoom)(ratio));
        }
        // Aperture, shutter speed and ISO matter most; the focal length goes first.
        let font = FontId::proportional(text::BODY);
        let text = fit_parts(painter, exposure, "  ·  ", &font, room, Drop::Front);
        right.text(
            pos2(x, row1),
            Align2::RIGHT_CENTER,
            text,
            font,
            tokens::TEXT,
        );
    }
    let mut gear = Vec::new();
    if bar.saving {
        gear.push(t.saving.to_owned());
    }
    if let Some(image) = bar.image {
        let line = image.camera.gear_line();
        gear.extend(
            line.split("  ·  ")
                .filter(|p| !p.is_empty())
                .map(str::to_owned),
        );
    }
    // The camera matters more than the lens.
    let font = FontId::proportional(text::SMALL);
    let text = fit_parts(painter, gear, "   ·   ", &font, room, Drop::Back);
    right.text(
        pos2(x, row2),
        Align2::RIGHT_CENTER,
        text,
        font,
        tokens::MUTED,
    );
    let stars_left = centre - stars_width / 2.0;
    if let Some(label) = bar.label {
        let dot = Rect::from_center_size(pos2(stars_left - 14.0, row1), Vec2::splat(12.0));
        painter.circle_filled(dot.center(), 5.0, crate::theme::label_color(label));
        ui.interact(dot, ui.id().with("label-dot"), Sense::hover())
            .on_hover_text(i18n::label_name(label));
    }
    for n in 1..=5u8 {
        let x = stars_left + f32::from(n - 1) * (STAR_SIZE + STAR_GAP);
        let star = Rect::from_min_size(pos2(x, row1 - STAR_SIZE / 2.0), Vec2::splat(STAR_SIZE));
        let response = ui
            .interact(
                star.expand(STAR_GAP / 2.0),
                ui.id().with(("star", n)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let filled = bar.rating.stars().is_some_and(|r| n <= r);
        let color = if filled || response.hovered() {
            tokens::ACCENT
        } else {
            tokens::MUTED
        };
        stars::paint_star(painter, star.center(), STAR_SIZE / 2.0, filled, color);
        if response.clicked() {
            // Clicking the current rating again clears it.
            out.rating = Some(if bar.rating == Rating::Stars(n) {
                Rating::Unrated
            } else {
                Rating::Stars(n)
            });
        }
        response.on_hover_text((t.star_tooltip)(n));
    }
    if bar.rating == Rating::Rejected {
        let x = stars_left + stars_width + 14.0;
        icons::reject_mark(painter, pos2(x + 5.0, row1), 9.0, tokens::STATUS_ERROR);
        painter.text(
            pos2(x + 16.0, row1),
            Align2::LEFT_CENTER,
            t.rejected,
            FontId::proportional(text::SMALL),
            tokens::STATUS_ERROR,
        );
    }
    out
}

/// Which end of a line loses parts first when it is too wide.
#[derive(Clone, Copy)]
enum Drop {
    Front,
    Back,
}

/// Joins `parts`, leaving out whole parts from one end until the line fits into `width` (the
/// last part always stays; the painter's clip handles the rest).
fn fit_parts(
    painter: &Painter,
    mut parts: Vec<String>,
    separator: &str,
    font: &FontId,
    width: f32,
    drop: Drop,
) -> String {
    loop {
        let text = parts.join(separator);
        let fits = painter
            .layout_no_wrap(text.clone(), font.clone(), tokens::TEXT)
            .size()
            .x
            <= width;
        if fits || parts.len() <= 1 {
            return text;
        }
        match drop {
            Drop::Front => parts.remove(0),
            Drop::Back => parts.pop().unwrap_or_default(),
        };
    }
}

/// `L 6.1 / V 6.5 / ☆ 2.4` (LAION / V2.5 / For you, all on the star scale, "–" while not
/// known) and the sharpness meter.
fn meters(bar: &InfoBar<'_>) -> Vec<Meter> {
    let [laion, v25] = bar.aesthetics;
    let mut meters = Vec::new();
    if laion.is_some() || v25.is_some() || bar.personal.is_some() {
        meters.push(scores_meter(
            laion.map(aesthetic::as_stars),
            v25.map(aesthetic::as_stars),
            bar.personal,
        ));
    }
    if let Some((p, eyes)) = bar.sharpness {
        meters.push(sharpness_meter(p, eyes, bar.blurry));
    }
    meters
}

/// The three scores without a group label: the small letters say which model, the outline
/// star is For you – filled stars are only ever the user's own rating.
fn scores_meter(laion: Option<f32>, v25: Option<f32>, personal: Option<f32>) -> Meter {
    let score =
        |v: Option<f32>| Piece::Value(v.map_or_else(|| "–".to_owned(), |v| format!("{v:.1}")));
    let slash = || Piece::Separator(" / ".to_owned());
    Meter {
        label: String::new(),
        value: vec![
            Piece::Prefix("L ".to_owned()),
            score(laion),
            slash(),
            Piece::Prefix("V ".to_owned()),
            score(v25),
            slash(),
            Piece::Star,
            score(personal),
        ],
        fraction: None,
        color: tokens::ACCENT,
        tooltip: Some(i18n::t().meter_aesthetics_tooltip),
        small: false,
    }
}

fn sharpness_meter(percentile: f32, eyes: bool, blurry: bool) -> Meter {
    let t = i18n::t();
    let name = if eyes {
        t.meter_eyes
    } else {
        t.meter_sharpness
    };
    Meter {
        label: if blurry {
            format!("{name} · {}", t.probably_blurry)
        } else {
            name.to_owned()
        },
        value: vec![Piece::Value(format!("{:.0} %", percentile * 100.0))],
        fraction: Some(percentile),
        color: if blurry {
            tokens::STATUS_WARN
        } else {
            tokens::ACCENT
        },
        tooltip: None,
        small: false,
    }
}

/// Width of the centre at its widest in the current language.
fn widest_centre(painter: &Painter) -> f32 {
    [true, false]
        .into_iter()
        .map(|eyes| {
            let meters = [
                scores_meter(Some(8.8), Some(8.8), Some(8.8)),
                sharpness_meter(1.0, eyes, true),
            ];
            layout_meters(painter, &meters).width
        })
        .fold(0.0, f32::max)
}

/// Buttons at the right end, laid out from the right edge: the menu, then help, then the map
/// pin if the photo has a position. Returns their left edge.
fn buttons(ui: &Ui, rect: Rect, bar: &InfoBar<'_>, out: &mut InfoBarOutput) -> f32 {
    let t = i18n::t();
    let y = rect.center().y;
    let mut x = rect.right() - 8.0;
    let next = |x: &mut f32| {
        *x -= BUTTON;
        let area = Rect::from_min_size(pos2(*x, y - BUTTON / 2.0), Vec2::splat(BUTTON));
        *x -= 2.0;
        area
    };

    let area = next(&mut x);
    let tooltip = format!("{} ({})", t.button_menu, i18n::with_ctrl("K"));
    out.menu = icon_button(ui, area, &tooltip, false, |p, c, color| {
        icons::menu(p, c, color);
    });
    x -= 8.0;
    let area = next(&mut x);
    let tooltip = format!("{} (H)", t.button_help);
    out.help = icon_button(ui, area, &tooltip, false, |p, c, color| {
        icons::help(p, c, color);
    });
    if let Some(position) = bar.image.and_then(|i| i.camera.gps) {
        let area = next(&mut x);
        let tooltip = (t.map_tooltip)(&i18n::coordinates(position.0, position.1));
        if icon_button(ui, area, &tooltip, false, |p, c, color| {
            icons::map_pin(p, c, 17.0, color, tokens::SURFACE);
        }) {
            out.open_map = Some(metadata::maps_url(position));
        }
    }
    x
}

/// A square icon button with tooltip; returns whether it was clicked.
fn icon_button(
    ui: &Ui,
    area: Rect,
    tooltip: &str,
    active: bool,
    paint: impl FnOnce(&Painter, Pos2, Color32),
) -> bool {
    let response = ui
        .interact(area, ui.id().with(("icon", tooltip)), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    icons::button_background(painter, area, response.hovered(), false);
    let color = if active || response.hovered() {
        tokens::ACCENT_STRONG
    } else {
        tokens::MUTED
    };
    paint(painter, area.center(), color);
    response.on_hover_text(tooltip).clicked()
}

/// Part of a meter's value text.
#[derive(Clone)]
enum Piece {
    Value(String),
    /// Small muted letter in front of a value.
    Prefix(String),
    /// Muted, value-sized (" / ").
    Separator(String),
    /// Room for the outline star painted in front of For you.
    Star,
}

/// Non-breaking spaces the outline star is painted over; the last one is the gap after it.
const STAR_PLACEHOLDER: &str = "\u{a0}\u{a0}\u{a0}";
const STAR_PLACEHOLDER_CHARS: usize = 3;

#[derive(Clone)]
struct Meter {
    label: String,
    value: Vec<Piece>,
    /// Bar fill, 0.0..=1.0; `None` shows the value only.
    fraction: Option<f32>,
    color: Color32,
    tooltip: Option<&'static str>,
    /// Status text ("Analysing…") at label size instead of a value.
    small: bool,
}

struct LaidMeter {
    label: Arc<Galley>,
    value: Arc<Galley>,
    meter: Meter,
    /// Character index of each star placeholder in `value`.
    stars: Vec<usize>,
}

struct LaidMeters {
    items: Vec<LaidMeter>,
    width: f32,
}

const METER_BAR: Vec2 = Vec2::new(52.0, 6.0);
const METER_GAP: f32 = 7.0;
const METER_SPACING: f32 = 22.0;

fn layout_meters(painter: &Painter, meters: &[Meter]) -> LaidMeters {
    let items: Vec<LaidMeter> = meters
        .iter()
        .map(|m| {
            let label = painter.layout_no_wrap(
                m.label.to_uppercase(),
                FontId::proportional(text::LABEL),
                if m.color == tokens::ACCENT {
                    tokens::MUTED
                } else {
                    m.color
                },
            );
            let value_color = if m.color == tokens::MUTED {
                tokens::MUTED
            } else {
                tokens::TEXT
            };
            let size = if m.small { text::SMALL } else { text::VALUE };
            let mut job = LayoutJob::default();
            let mut stars = Vec::new();
            let mut chars = 0;
            for piece in &m.value {
                let (text, size, color) = match piece {
                    Piece::Value(text) => (text.as_str(), size, value_color),
                    Piece::Prefix(text) => (text.as_str(), text::LABEL, tokens::MUTED),
                    Piece::Separator(text) => (text.as_str(), size, tokens::MUTED),
                    Piece::Star => {
                        stars.push(chars);
                        (STAR_PLACEHOLDER, text::LABEL, tokens::MUTED)
                    }
                };
                let mut format = TextFormat::simple(FontId::proportional(size), color);
                if matches!(piece, Piece::Prefix(_) | Piece::Star) {
                    format.valign = Align::Center;
                }
                chars += text.chars().count();
                job.append(text, 0.0, format);
            }
            let value = painter.layout_job(job);
            LaidMeter {
                label,
                value,
                meter: m.clone(),
                stars,
            }
        })
        .collect();
    let width = items.iter().map(meter_width).sum::<f32>()
        + METER_SPACING * items.len().saturating_sub(1) as f32;
    LaidMeters { items, width }
}

fn meter_width(laid: &LaidMeter) -> f32 {
    let label = if laid.label.size().x > 0.0 {
        laid.label.size().x + METER_GAP
    } else {
        0.0
    };
    let bar = if laid.meter.fraction.is_some() {
        METER_GAP + METER_BAR.x
    } else {
        0.0
    };
    label + laid.value.size().x + bar
}

/// `LABEL  6.1  ▬▬▬▬▭▭` for each meter, centred on `centre`. Returns each meter's area and
/// tooltip.
fn paint_meters(
    painter: &Painter,
    centre: f32,
    y: f32,
    laid: LaidMeters,
) -> Vec<(Rect, Option<&'static str>)> {
    let mut areas = Vec::new();
    let mut x = centre - laid.width / 2.0;
    for item in laid.items {
        let width = meter_width(&item);
        let LaidMeter {
            label,
            value,
            meter,
            stars,
        } = item;
        let start = x;
        if label.size().x > 0.0 {
            let lw = label.size().x;
            painter.galley(pos2(x, y - label.size().y / 2.0), label, tokens::MUTED);
            x += lw + METER_GAP;
        }
        let origin = pos2(x, y - value.size().y / 2.0);
        for index in stars {
            let from = value.pos_from_cursor(CCursor::new(index)).min.x;
            let to = value
                .pos_from_cursor(CCursor::new(index + STAR_PLACEHOLDER_CHARS - 1))
                .min
                .x;
            let centre = pos2(origin.x + (from + to) / 2.0, y);
            stars::paint_star(painter, centre, 5.5, false, tokens::MUTED);
        }
        let vw = value.size().x;
        painter.galley(origin, value, tokens::TEXT);
        x += vw;
        if let Some(fraction) = meter.fraction {
            x += METER_GAP;
            let track = Rect::from_min_size(pos2(x, y - METER_BAR.y / 2.0), METER_BAR);
            painter.rect_filled(track, 3.0, tokens::LINE);
            let fill = Rect::from_min_size(
                track.min,
                vec2(METER_BAR.x * fraction.clamp(0.0, 1.0), METER_BAR.y),
            );
            painter.rect_filled(fill, 3.0, meter.color);
        }
        areas.push((
            Rect::from_min_max(pos2(start, y - 11.0), pos2(start + width, y + 11.0)),
            meter.tooltip,
        ));
        x = start + width + METER_SPACING;
    }
    areas
}

/// Compare mode: `LEFT  name  ★★★  A keeps this` in the top left corner of a photo.
pub fn compare_label(ui: &Ui, area: Rect, side: &str, name: &str, rating: Rating, hint: &str) {
    let painter = ui.painter().with_clip_rect(area);
    let side = painter.layout_no_wrap(
        side.to_uppercase(),
        FontId::proportional(text::LABEL),
        tokens::MUTED,
    );
    let name = painter.layout_no_wrap(
        name.to_owned(),
        FontId::proportional(text::BODY),
        tokens::TEXT,
    );
    let hint = painter.layout_no_wrap(
        hint.to_owned(),
        FontId::proportional(text::SMALL),
        tokens::MUTED,
    );
    // Stars, or the red cross for a rejected photo.
    let stars_width = match rating {
        Rating::Stars(r) => f32::from(r) * 10.0 + 8.0,
        Rating::Rejected => 20.0,
        Rating::Unrated => 0.0,
    };
    let height = 28.0;
    let width =
        12.0 + side.size().x + 10.0 + name.size().x + stars_width + 12.0 + hint.size().x + 12.0;
    let pill = Rect::from_min_size(area.min + vec2(10.0, 10.0), vec2(width, height));
    painter.rect_filled(pill, 6.0, tokens::SURFACE.gamma_multiply(0.92));
    painter.rect_stroke(
        pill,
        6.0,
        Stroke::new(1.0, tokens::LINE),
        StrokeKind::Inside,
    );

    let y = pill.center().y;
    let mut x = pill.left() + 12.0;
    for galley in [side, name] {
        let w = galley.size().x;
        painter.galley(pos2(x, y - galley.size().y / 2.0), galley, tokens::TEXT);
        x += w + 10.0;
    }
    match rating {
        Rating::Stars(stars) => stars::paint_mini_rating(
            &painter,
            pos2(x + f32::from(stars) * 5.0 - 2.0, y),
            stars,
            4.0,
            tokens::ACCENT,
        ),
        Rating::Rejected => {
            icons::reject_mark(&painter, pos2(x + 5.0, y), 9.0, tokens::STATUS_ERROR);
        }
        Rating::Unrated => {}
    }
    x += stars_width;
    painter.galley(pos2(x + 2.0, y - hint.size().y / 2.0), hint, tokens::MUTED);
}

/// Compare mode: aesthetics and sharpness under the photo, just below the side label.
pub fn compare_scores(
    ui: &Ui,
    area: Rect,
    aesthetics: [Option<f32>; 2],
    personal: Option<f32>,
    sharpness: Option<(f32, bool)>,
) {
    if aesthetics.iter().all(Option::is_none) && personal.is_none() && sharpness.is_none() {
        return;
    }
    let t = i18n::t();
    let star = |v: Option<f32>| {
        v.map(|v| format!("{:.1}", aesthetic::as_stars(v)))
            .unwrap_or_else(|| "–".to_owned())
    };
    // `L 2.8 / V 4.0 / ☆ 2.3   Eyes 80 %` – the outline star is painted, like in the info bar.
    const STAR_ROOM: f32 = 16.0;
    let before = format!("L {} / V {} / ", star(aesthetics[0]), star(aesthetics[1]));
    let mut after = personal
        .map(|v| format!("{v:.1}"))
        .unwrap_or_else(|| "–".to_owned());
    if let Some((p, eyes)) = sharpness {
        let name = if eyes {
            t.meter_eyes
        } else {
            t.meter_sharpness
        };
        after.push_str(&format!("   {name} {:.0} %", p * 100.0));
    }
    let painter = ui.painter().with_clip_rect(area);
    let font = FontId::proportional(text::BODY);
    let before = painter.layout_no_wrap(before, font.clone(), tokens::TEXT);
    let after = painter.layout_no_wrap(after, font, tokens::TEXT);
    let pill = Rect::from_min_size(
        area.min + vec2(10.0, 44.0),
        vec2(before.size().x + STAR_ROOM + after.size().x + 24.0, 26.0),
    );
    painter.rect_filled(pill, 6.0, tokens::SURFACE.gamma_multiply(0.92));
    painter.rect_stroke(
        pill,
        6.0,
        Stroke::new(1.0, tokens::LINE),
        StrokeKind::Inside,
    );
    let y = pill.center().y;
    let mut x = pill.left() + 12.0;
    let width = before.size().x;
    painter.galley(pos2(x, y - before.size().y / 2.0), before, tokens::TEXT);
    x += width;
    stars::paint_star(&painter, pos2(x + 6.0, y), 5.0, false, tokens::MUTED);
    x += STAR_ROOM;
    painter.galley(pos2(x, y - after.size().y / 2.0), after, tokens::TEXT);
}

/// Countdown for pending deletions, bottom centre of the photo area. The bar runs out, Esc
/// brings everything back.
pub fn delete_countdown(ui: &Ui, area: Rect, count: usize, left: f32) {
    let painter = ui.painter();
    let text = (i18n::t().deleting)(count);
    let galley = painter.layout_no_wrap(text, FontId::proportional(text::BODY), tokens::TEXT);
    let width = (galley.size().x + 32.0).max(300.0);
    let pill = Rect::from_center_size(
        pos2(area.center().x, area.bottom() - 44.0),
        vec2(width, 48.0),
    );
    painter.rect_filled(pill, 8.0, tokens::SURFACE);
    painter.rect_stroke(
        pill,
        8.0,
        Stroke::new(1.0, tokens::STATUS_WARN),
        StrokeKind::Inside,
    );
    painter.galley(
        pos2(pill.center().x - galley.size().x / 2.0, pill.top() + 9.0),
        galley,
        tokens::TEXT,
    );
    let track = Rect::from_min_size(
        pos2(pill.left() + 16.0, pill.bottom() - 14.0),
        vec2(pill.width() - 32.0, 5.0),
    );
    painter.rect_filled(track, 3.0, tokens::LINE);
    painter.rect_filled(
        Rect::from_min_size(track.min, vec2(track.width() * left, track.height())),
        3.0,
        tokens::STATUS_WARN,
    );
}

/// A message at the top of the photo area: `(text, is_error, opacity)`. Returns whether it was
/// clicked (which dismisses it).
pub fn notices(ui: &Ui, rect: Rect, message: Option<(&str, bool, f32)>) -> bool {
    let Some((text, is_error, opacity)) = message else {
        return false;
    };
    let text = text.to_owned();
    let mut painter = ui.painter().clone();
    painter.set_opacity(opacity);
    let galley = painter.layout(
        text,
        FontId::proportional(text::BODY),
        tokens::TEXT,
        (rect.width() - 64.0).max(120.0),
    );
    let size = galley.size() + vec2(24.0, 14.0);
    let pill = Rect::from_min_size(
        pos2(rect.center().x - size.x / 2.0, rect.top() + 12.0),
        size,
    );
    let (fill, border) = if is_error {
        (tokens::STATUS_ERROR_BG, tokens::STATUS_ERROR)
    } else {
        (tokens::SURFACE, tokens::LINE)
    };
    painter.rect_filled(pill, 6.0, fill);
    painter.rect_stroke(pill, 6.0, Stroke::new(1.0, border), StrokeKind::Inside);
    painter.galley(pill.min + vec2(12.0, 7.0), galley, tokens::TEXT);
    ui.interact(pill, ui.id().with("notice"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
}

pub fn drop_hint(ui: &Ui, rect: Rect) {
    if ui.ctx().input(|i| i.raw.hovered_files.is_empty()) {
        return;
    }
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Color32::from_black_alpha(150));
    painter.rect_stroke(
        rect.shrink(10.0),
        8.0,
        Stroke::new(2.0, tokens::ACCENT),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        i18n::t().drop_to_open,
        FontId::proportional(text::VALUE),
        tokens::TEXT,
    );
}

/// Placeholder text in the image area (loading, errors, empty filter).
pub fn centred_message(ui: &Ui, area: Rect, text: &str, color: Color32) {
    ui.painter().text(
        area.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(text::LARGE),
        color,
    );
}

/// Big flag and language name, shown for a moment after switching (`opacity` fades it out).
pub fn language_flash(painter: &Painter, area: Rect, lang: Lang, opacity: f32) {
    let mut painter = painter.clone();
    painter.set_opacity(opacity);
    let name = painter.layout_no_wrap(
        lang.name().to_owned(),
        FontId::proportional(text::TITLE),
        tokens::TEXT,
    );
    let flag = vec2(96.0, 64.0);
    let size = vec2(
        flag.x.max(name.size().x) + 48.0,
        flag.y + name.size().y + 44.0,
    );
    let card = Rect::from_center_size(area.center(), size);
    painter.rect_filled(card, 10.0, tokens::SURFACE.gamma_multiply(0.96));
    painter.rect_stroke(
        card,
        10.0,
        Stroke::new(1.0, tokens::LINE),
        StrokeKind::Inside,
    );
    let flag_rect = Rect::from_min_size(
        pos2(card.center().x - flag.x / 2.0, card.top() + 18.0),
        flag,
    );
    icons::flag(&painter, flag_rect, lang);
    painter.galley(
        pos2(
            card.center().x - name.size().x / 2.0,
            flag_rect.bottom() + 12.0,
        ),
        name,
        tokens::TEXT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, RawInput, Shape};

    fn texts_of(bar: &InfoBar<'_>, width: f32) -> Vec<(String, Rect)> {
        let ctx = Context::default();
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(width, INFO_HEIGHT));
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 400.0))),
                ..Default::default()
            },
            |ui| {
                info_bar(ui, rect, bar);
            },
        );
        output.textures_delta.clear();
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Text(text) => {
                    Some((text.galley.text().to_owned(), text.visual_bounding_rect()))
                }
                _ => None,
            })
            .collect()
    }

    fn bar(blurry: bool) -> InfoBar<'static> {
        InfoBar {
            name: "IMG_0001.JPG",
            position: (3, 120),
            image: None,
            rating: Rating::Stars(3),
            label: None,
            series: Some((2, 5)),
            duplicate_of: Some("IMG_0000 - Kopie.JPG".to_owned()),
            auto_advance: true,
            analysed: true,
            aesthetics: [Some(6.0), Some(5.5)],
            personal: Some(3.1),
            sharpness: Some((0.1, true)),
            blurry,
            saving: false,
            zoom: Some(100.0),
        }
    }

    /// The facts line leaves out whole parts instead of running under the scores.
    #[test]
    fn left_line_stops_before_the_centre() {
        let width = 700.0;
        let texts = texts_of(&bar(true), width);
        let facts = texts
            .iter()
            .find(|(text, _)| text.starts_with("3 / 120"))
            .expect("facts line");
        let scores_left = texts
            .iter()
            .filter(|(text, _)| text.contains("L "))
            .map(|(_, rect)| rect.left())
            .fold(f32::MAX, f32::min);
        assert!(
            facts.1.right() < scores_left,
            "{:?} runs into the scores at {scores_left}",
            facts
        );
    }

    /// The side columns stay where they are when the blurry note comes and goes: at a width
    /// where the facts line has to leave parts out, it keeps the same parts either way.
    #[test]
    fn centre_width_does_not_follow_the_blurry_note() {
        let facts = |blurry| {
            texts_of(&bar(blurry), 760.0)
                .into_iter()
                .find(|(text, _)| text.starts_with("3 / 120"))
                .map(|(text, _)| text)
                .expect("facts line")
        };
        let full = "Duplicate of";
        assert!(!facts(true).contains(full), "760 px cannot show every part");
        assert_eq!(facts(true), facts(false));
    }
}
