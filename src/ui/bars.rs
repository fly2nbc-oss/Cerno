//! Toolbar (top), info bar (bottom), notices and the drop hint.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    Align, Align2, Color32, ComboBox, CursorIcon, FontId, Galley, Layout, Painter, Pos2, Rect,
    RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};
use std::sync::Arc;

use crate::analysis::{ModelState, Status, aesthetic};
use crate::i18n::{self, Lang};
use crate::loader::LoadedImage;
use crate::metadata::{self, Rating};
use crate::theme::tokens;
use crate::ui::icons::{self, Panel};
use crate::ui::stars;
use crate::view::{BLURRY_PERCENTILE, RatingFilter, SortKey, ViewOptions};

pub const TOOLBAR_HEIGHT: f32 = 40.0;
pub const INFO_HEIGHT: f32 = 60.0;
const STAR_SIZE: f32 = 16.0;
const STAR_GAP: f32 = 6.0;
const BUTTON: f32 = 28.0;

pub struct ToolbarInfo<'a> {
    pub folder: Option<&'a str>,
    pub shown: usize,
    pub total: usize,
    /// New scores arrived since the view was sorted/filtered.
    pub stale: bool,
    pub status: &'a Status,
}

#[derive(Default)]
pub struct ToolbarOutput {
    pub options_changed: bool,
    pub open: bool,
    pub refresh: bool,
    pub download_model: bool,
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
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(12.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if ui.button(t.open).on_hover_text(t.open_tooltip).clicked() {
                out.open = true;
            }
            if let Some(folder) = info.folder {
                ui.label(RichText::new(folder).color(tokens::TEXT));
                let count = if info.shown == info.total {
                    (t.photos)(info.total)
                } else {
                    (t.photos_shown)(info.shown, info.total)
                };
                ui.label(RichText::new(count).color(tokens::MUTED));
            }
            ui.separator();
            ComboBox::from_id_salt("sort")
                .selected_text((t.sort)(options.sort.label()))
                .show_ui(ui, |ui| {
                    for key in SortKey::ALL {
                        ui.selectable_value(&mut options.sort, key, key.label());
                    }
                });
            ComboBox::from_id_salt("filter")
                .selected_text((t.show)(&options.filter.label()))
                .show_ui(ui, |ui| {
                    for filter in RatingFilter::ALL {
                        ui.selectable_value(&mut options.filter, filter, filter.label());
                    }
                });
            ui.checkbox(&mut options.hide_blurry, t.hide_blurry)
                .on_hover_text(t.hide_blurry_tooltip);
            if info.stale
                && ui
                    .button(t.refresh_order)
                    .on_hover_text(t.refresh_order_tooltip)
                    .clicked()
            {
                out.refresh = true;
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                aesthetics_status(ui, &info.status.aesthetics, &mut out);
                let Status { done, total, .. } = *info.status;
                if total > 0 {
                    let text = if done < total {
                        (t.analyzing_progress)(done, total)
                    } else {
                        (t.analyzed)(total)
                    };
                    ui.label(RichText::new(text).color(tokens::MUTED));
                }
            });
        },
    );
    out.options_changed = *options != before;
    out
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
        ModelState::Available => {
            ui.label(muted(t.aesthetics_ready.into()));
        }
        ModelState::Loading => {
            ui.label(muted(t.aesthetics_loading.into()));
        }
        ModelState::Ready { backend } => {
            ui.label(muted((t.aesthetics_backend)(backend)))
                .on_hover_text(t.aesthetics_backend_tooltip);
        }
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
    /// Some analysis result is known for this photo.
    pub analysed: bool,
    /// LAION and V2.5 scores, 1..10.
    pub aesthetics: [Option<f32>; 2],
    /// Personal taste model, 0..=5.
    pub personal: Option<f32>,
    /// (percentile within the folder, measured at the eyes).
    pub sharpness: Option<(f32, bool)>,
    pub saving: bool,
    /// Viewer zoom in percent while zoomed in.
    pub zoom: Option<f32>,
    pub panels: Panels,
}

#[derive(Default)]
pub struct InfoBarOutput {
    /// The star the user clicked (`Some(None)` clears the rating).
    pub rating: Option<Rating>,
    pub toggle: Option<Panel>,
    pub help: bool,
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
        }]
    };
    let laid = layout_meters(painter, &meters);
    let stars_width = 5.0 * STAR_SIZE + 4.0 * STAR_GAP;
    let centre_half = (laid.width / 2.0).max(stars_width / 2.0) + 8.0;
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
        FontId::proportional(13.0),
        tokens::TEXT,
    );
    let mut facts = vec![format!("{} / {}", bar.position.0, bar.position.1)];
    if let Some(zoom) = bar.zoom {
        facts.push((t.zoom)(zoom));
    }
    if let Some(image) = bar.image {
        if let Some(taken) = &image.camera.taken {
            facts.push(i18n::date(taken));
        }
        let [w, h] = image.original_size;
        facts.push(format!("{w} × {h}"));
        facts.push(format!("{} ms", image.load_ms));
    }
    left.text(
        pos2(x, row2),
        Align2::LEFT_CENTER,
        facts.join("   ·   "),
        FontId::proportional(11.5),
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
        let font = FontId::proportional(12.5);
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
    let font = FontId::proportional(11.5);
    let text = fit_parts(painter, gear, "   ·   ", &font, room, Drop::Back);
    right.text(
        pos2(x, row2),
        Align2::RIGHT_CENTER,
        text,
        font,
        tokens::MUTED,
    );
    let stars_left = centre - stars_width / 2.0;
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
            FontId::proportional(12.0),
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

/// `AESTHETICS 6.1 / 6.5 / 2.4 ★` (LAION / V2.5 / personal) and sharpness.
fn meters(bar: &InfoBar<'_>) -> Vec<Meter> {
    let t = i18n::t();
    let mut meters = Vec::new();
    let [laion, v25] = bar.aesthetics;
    if laion.is_some() || v25.is_some() || bar.personal.is_some() {
        // L 3.4 / V 3.8 / ★ 2.4, all on the star scale – the letters say which model, "–" means not known yet.
        let score =
            |v: Option<f32>| Piece::Value(v.map_or_else(|| "–".to_owned(), |v| format!("{v:.1}")));
        let slash = || Piece::Separator(" / ".to_owned());
        meters.push(Meter {
            label: t.meter_aesthetics.to_owned(),
            value: vec![
                Piece::Prefix("L ".to_owned()),
                score(laion.map(aesthetic::as_stars)),
                slash(),
                Piece::Prefix("V ".to_owned()),
                score(v25.map(aesthetic::as_stars)),
                slash(),
                Piece::Prefix("★ ".to_owned()),
                score(bar.personal),
            ],
            fraction: None,
            color: tokens::ACCENT,
            tooltip: Some(t.meter_aesthetics_tooltip),
        });
    }
    if let Some((p, eyes)) = bar.sharpness {
        let blurry = p < BLURRY_PERCENTILE;
        let name = if eyes {
            t.meter_eyes
        } else {
            t.meter_sharpness
        };
        meters.push(Meter {
            label: if blurry {
                format!("{name} · {}", t.probably_blurry)
            } else {
                name.to_owned()
            },
            value: vec![Piece::Value(format!("{:.0} %", p * 100.0))],
            fraction: Some(p),
            color: if blurry {
                tokens::STATUS_WARN
            } else {
                tokens::ACCENT
            },
            tooltip: None,
        });
    }
    meters
}

/// Buttons at the right end, laid out from the right edge: panel toggles, then help, then
/// the map pin if the photo has a position. Returns their left edge.
fn buttons(ui: &Ui, rect: Rect, bar: &InfoBar<'_>, out: &mut InfoBarOutput) -> f32 {
    let t = i18n::t();
    let y = rect.center().y;
    let mut x = rect.right() - 8.0;
    // The next button to the left of `x`.
    let next = |x: &mut f32| {
        *x -= BUTTON;
        let area = Rect::from_min_size(pos2(*x, y - BUTTON / 2.0), Vec2::splat(BUTTON));
        *x -= 2.0;
        area
    };

    for (panel, shown, label, key) in [
        (
            Panel::Bottom,
            bar.panels.filmstrip,
            t.button_filmstrip,
            "F6",
        ),
        (Panel::Right, bar.panels.details, t.button_details, "Tab"),
        (Panel::Top, bar.panels.toolbar, t.button_toolbar, "T"),
    ] {
        let area = next(&mut x);
        let tooltip = format!("{label} ({key})");
        if icon_button(ui, area, &tooltip, shown, |p, c, color| {
            icons::panel(p, c, panel, shown, color);
        }) {
            out.toggle = Some(panel);
        }
    }
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
}

#[derive(Clone)]
struct Meter {
    label: String,
    value: Vec<Piece>,
    /// Bar fill, 0.0..=1.0; `None` shows the value only.
    fraction: Option<f32>,
    color: Color32,
    tooltip: Option<&'static str>,
}

struct LaidMeters {
    items: Vec<(Arc<Galley>, Arc<Galley>, Meter)>,
    width: f32,
}

const METER_BAR: Vec2 = Vec2::new(52.0, 6.0);
const METER_GAP: f32 = 7.0;
const METER_SPACING: f32 = 22.0;

fn layout_meters(painter: &Painter, meters: &[Meter]) -> LaidMeters {
    let items: Vec<_> = meters
        .iter()
        .map(|m| {
            let label = painter.layout_no_wrap(
                m.label.to_uppercase(),
                FontId::proportional(10.5),
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
            let size = if m.label.is_empty() { 12.0 } else { 17.0 };
            let mut job = LayoutJob::default();
            for piece in &m.value {
                let (text, size, color) = match piece {
                    Piece::Value(text) => (text, size, value_color),
                    Piece::Prefix(text) => (text, 11.0, tokens::MUTED),
                    Piece::Separator(text) => (text, size, tokens::MUTED),
                };
                let mut format = TextFormat::simple(FontId::proportional(size), color);
                if matches!(piece, Piece::Prefix(_)) {
                    format.valign = Align::Center;
                }
                job.append(text, 0.0, format);
            }
            let value = painter.layout_job(job);
            (label, value, m.clone())
        })
        .collect();
    let width = items
        .iter()
        .map(|(label, value, m)| meter_width(label, value, m))
        .sum::<f32>()
        + METER_SPACING * items.len().saturating_sub(1) as f32;
    LaidMeters { items, width }
}

fn meter_width(label: &Galley, value: &Galley, meter: &Meter) -> f32 {
    let label = if label.size().x > 0.0 {
        label.size().x + METER_GAP
    } else {
        0.0
    };
    let bar = if meter.fraction.is_some() {
        METER_GAP + METER_BAR.x
    } else {
        0.0
    };
    label + value.size().x + bar
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
    for (label, value, meter) in laid.items {
        let width = meter_width(&label, &value, &meter);
        let start = x;
        if label.size().x > 0.0 {
            let lw = label.size().x;
            painter.galley(pos2(x, y - label.size().y / 2.0), label, tokens::MUTED);
            x += lw + METER_GAP;
        }
        let vw = value.size().x;
        painter.galley(pos2(x, y - value.size().y / 2.0), value, tokens::TEXT);
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
        FontId::proportional(11.0),
        tokens::ACCENT,
    );
    let name = painter.layout_no_wrap(name.to_owned(), FontId::proportional(13.0), tokens::TEXT);
    let hint = painter.layout_no_wrap(hint.to_owned(), FontId::proportional(11.5), tokens::MUTED);
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

/// Countdown for pending deletions, bottom centre of the photo area. The bar runs out, Esc
/// brings everything back.
pub fn delete_countdown(ui: &Ui, area: Rect, count: usize, left: f32) {
    let painter = ui.painter();
    let text = (i18n::t().deleting)(count);
    let galley = painter.layout_no_wrap(text, FontId::proportional(13.0), tokens::TEXT);
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

pub fn notices(ui: &Ui, rect: Rect, error: Option<&str>, notice: Option<&str>) {
    let (text, is_error) = match (error, notice) {
        (Some(err), _) => (err.to_owned(), true),
        (None, Some(notice)) => (notice.to_owned(), false),
        (None, None) => return,
    };
    let painter = ui.painter();
    let galley = painter.layout(
        text,
        FontId::proportional(13.0),
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
        FontId::proportional(18.0),
        tokens::TEXT,
    );
}

/// Placeholder text in the image area (loading, errors, empty filter).
pub fn centred_message(ui: &Ui, area: Rect, text: &str, color: Color32) {
    ui.painter().text(
        area.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(14.0),
        color,
    );
}

/// Big flag and language name, shown for a moment after switching (`opacity` fades it out).
pub fn language_flash(painter: &Painter, area: Rect, lang: Lang, opacity: f32) {
    let mut painter = painter.clone();
    painter.set_opacity(opacity);
    let name = painter.layout_no_wrap(
        lang.name().to_owned(),
        FontId::proportional(20.0),
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
