//! The info bar at the bottom: name and facts, stars and scores, exposure and camera, the
//! help and menu buttons.

use std::sync::Arc;

use eframe::egui::{
    Align2, Color32, CursorIcon, FontId, Galley, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2,
    pos2, vec2,
};

use crate::analysis::aesthetic;
use crate::i18n;
use crate::loader::LoadedImage;
use crate::metadata::{Label, Rating};
use crate::theme::{text, tokens};
use crate::ui::{icons, stars};

pub const INFO_HEIGHT: f32 = 60.0;
const STAR_SIZE: f32 = 16.0;
const STAR_GAP: f32 = 6.0;
/// Click area of the info bar buttons (design system: at least 32 px).
const BUTTON: f32 = 32.0;

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
    /// The aesthetics score (`aesthetic::combined`), 1..10.
    pub aesthetics: Option<f32>,
    /// Personal taste model, 0..=5: light stars while the photo has no rating of its own.
    pub personal: Option<f32>,
    /// (percentile within the folder, measured at the eyes).
    pub sharpness: Option<(f32, bool)>,
    /// The subject is probably out of focus (`view::is_blurry`).
    pub blurry: bool,
    pub saving: bool,
    /// Viewer zoom in percent while zoomed in.
    pub zoom: Option<f32>,
    /// Which check overlay is on, while one is.
    pub overlay: Option<&'a str>,
    /// How alike the photo is to the one "similar photos" is about (0..=1), while that filter
    /// is on.
    pub similarity: Option<f32>,
    /// A deleted photo (in `.originals`), shown through the 🗑 box.
    pub deleted: bool,
    /// A RAW file: what shows is the JPEG preview inside it, and 100 % is that preview's size.
    pub raw_preview: bool,
    /// The camera's clock was set right by this much (*Camera time …*): the date shows moved.
    pub time_offset: Option<i64>,
}

#[derive(Default)]
pub struct InfoBarOutput {
    /// The star the user clicked (`Some(None)` clears the rating).
    pub rating: Option<Rating>,
    pub help: bool,
    pub menu: bool,
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

    let buttons_left = buttons(ui, rect, &mut out);

    // Centre: scores as value (+ bar) below the stars; its width decides the side columns.
    let meters = if bar.analysed {
        meters(bar)
    } else {
        vec![Meter {
            label: String::new(),
            value: t.analyzing.to_owned(),
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
    if bar.deleted {
        facts.push(t.deleted_mark.to_owned());
    }
    if bar.raw_preview {
        facts.push(t.raw_preview_fact.to_owned());
    }
    if bar.auto_advance {
        facts.push(t.auto_advance_on.to_owned());
    }
    if let Some(zoom) = bar.zoom {
        facts.push(if bar.raw_preview {
            (t.zoom_preview)(zoom)
        } else {
            (t.zoom)(zoom)
        });
    }
    if let Some(overlay) = bar.overlay {
        facts.push(overlay.to_owned());
    }
    if let Some(similarity) = bar.similarity {
        facts.push((t.similar_fact)(similarity * 100.0));
    }
    if let Some((index, len)) = bar.series {
        facts.push((t.series_position)(index, len));
    }
    if let Some(name) = &bar.duplicate_of {
        facts.push((t.duplicate_of)(name));
    }
    // A camera set right shows the moved time and the offset; the tooltip has the file's.
    let mut date_tooltip = None;
    if let Some(image) = bar.image
        && let Some(taken) = image.camera.taken.as_ref()
    {
        match (bar.time_offset, image.camera.taken_ms) {
            (Some(offset), Some(ms)) => {
                let shift = crate::camera_time::format_offset(offset);
                let moved = crate::metadata::format_millis(ms + offset);
                facts.push(format!("{} ({shift})", i18n::date(&moved)));
                date_tooltip = Some((t.camera_time_tooltip)(&i18n::date(taken), &shift));
            }
            _ => facts.push(i18n::date(taken)),
        }
    }
    let font = FontId::proportional(text::SMALL);
    let room = (centre - centre_half - 12.0 - x).max(0.0);
    let text = fit_parts(painter, facts, "   ·   ", &font, room, Drop::Back);
    let line = left.text(
        pos2(x, row2),
        Align2::LEFT_CENTER,
        text,
        font,
        tokens::MUTED,
    );
    if let Some(tooltip) = date_tooltip {
        ui.interact(line, ui.id().with("facts-line"), Sense::hover())
            .on_hover_text(tooltip);
    }

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
    let hint = hint_stars(bar.rating, bar.personal);
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
        let outline = if filled || response.hovered() {
            tokens::ACCENT
        } else {
            tokens::MUTED
        };
        if filled {
            stars::paint_star(painter, star.center(), STAR_SIZE / 2.0, true, outline);
        } else if n <= hint {
            // For you: a light fill, recognisably not the user's own stars.
            stars::paint_hint_star(painter, star.center(), STAR_SIZE / 2.0, HINT_FILL, outline);
        } else {
            stars::paint_star(painter, star.center(), STAR_SIZE / 2.0, false, outline);
        }
        if response.clicked() {
            // Clicking the current rating again clears it.
            out.rating = Some(if bar.rating == Rating::Stars(n) {
                Rating::Unrated
            } else {
                Rating::Stars(n)
            });
        }
        let tooltip = match bar.personal {
            Some(personal) if hint > 0 => {
                format!("{}\n{}", (t.star_tooltip)(n), (t.personal_hint)(personal))
            }
            _ => (t.star_tooltip)(n),
        };
        response.on_hover_text(tooltip);
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

/// How many light stars For you shows: its prediction rounded to whole stars, only while the
/// photo has no rating of its own (a rejection counts as one). Below half a star: none.
fn hint_stars(rating: Rating, personal: Option<f32>) -> u8 {
    match (rating, personal) {
        (Rating::Unrated, Some(stars)) => stars.round().clamp(0.0, 5.0) as u8,
        _ => 0,
    }
}

/// The light fill of a For-you star: the accent, faint.
const HINT_FILL: Color32 = Color32::from_rgba_premultiplied(0x2E, 0x47, 0x62, 0x80);

/// Aesthetics and sharpness, both in percent with a bar – nothing else under the photo.
fn meters(bar: &InfoBar<'_>) -> Vec<Meter> {
    let mut meters = Vec::new();
    if let Some(score) = bar.aesthetics {
        meters.push(aesthetics_meter(aesthetic::as_percent(score)));
    }
    if let Some((p, eyes)) = bar.sharpness {
        meters.push(sharpness_meter(p, eyes, bar.blurry));
    }
    meters
}

/// The one aesthetics score on its fixed scale (`aesthetic::as_percent`).
fn aesthetics_meter(share: f32) -> Meter {
    let t = i18n::t();
    Meter {
        label: t.section_aesthetics.to_owned(),
        value: format!("{:.0} %", share * 100.0),
        fraction: Some(share),
        color: tokens::ACCENT,
        tooltip: Some(t.meter_aesthetics_tooltip),
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
        value: format!("{:.0} %", percentile * 100.0),
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
            let meters = [aesthetics_meter(1.0), sharpness_meter(1.0, eyes, true)];
            layout_meters(painter, &meters).width
        })
        .fold(0.0, f32::max)
}

/// Buttons at the right end, laid out from the right edge: the menu, then help. Returns their
/// left edge.
fn buttons(ui: &Ui, rect: Rect, out: &mut InfoBarOutput) -> f32 {
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

#[derive(Clone)]
struct Meter {
    label: String,
    value: String,
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
            let value =
                painter.layout_no_wrap(m.value.clone(), FontId::proportional(size), value_color);
            LaidMeter {
                label,
                value,
                meter: m.clone(),
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

/// `LABEL  62 %  ▬▬▬▬▭▭` for each meter, centred on `centre`. Returns each meter's area and
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
        } = item;
        let start = x;
        if label.size().x > 0.0 {
            let lw = label.size().x;
            painter.galley(pos2(x, y - label.size().y / 2.0), label, tokens::MUTED);
            x += lw + METER_GAP;
        }
        let origin = pos2(x, y - value.size().y / 2.0);
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

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, RawInput, Shape};

    fn shapes_of(bar: &InfoBar<'_>, width: f32) -> Vec<Shape> {
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
            .into_iter()
            .map(|clipped| clipped.shape)
            .collect()
    }

    fn texts_of(bar: &InfoBar<'_>, width: f32) -> Vec<(String, Rect)> {
        shapes_of(bar, width)
            .iter()
            .filter_map(|shape| match shape {
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
            aesthetics: Some(5.6),
            personal: Some(3.1),
            sharpness: Some((0.1, true)),
            blurry,
            saving: false,
            zoom: Some(100.0),
            overlay: None,
            deleted: false,
            raw_preview: false,
            similarity: None,
            time_offset: None,
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
            .find(|(text, _)| text == "AESTHETICS")
            .map(|(_, rect)| rect.left())
            .expect("aesthetics meter");
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

    /// Under the photo only aesthetics and sharpness, both in percent.
    #[test]
    fn centre_shows_aesthetics_and_sharpness_only() {
        let texts: Vec<String> = texts_of(&bar(false), 1400.0)
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        assert!(texts.iter().any(|t| t == "AESTHETICS"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "60 %"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "EYES"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "10 %"), "{texts:?}");
        assert!(
            !texts.iter().any(|t| t.contains("3.1")),
            "For you is no number here"
        );
    }

    #[test]
    fn hint_stars_round_and_only_show_without_a_rating() {
        assert_eq!(hint_stars(Rating::Unrated, Some(3.1)), 3);
        assert_eq!(hint_stars(Rating::Unrated, Some(4.6)), 5);
        assert_eq!(hint_stars(Rating::Unrated, Some(0.4)), 0);
        assert_eq!(hint_stars(Rating::Unrated, None), 0);
        assert_eq!(hint_stars(Rating::Stars(3), Some(3.1)), 0);
        assert_eq!(hint_stars(Rating::Rejected, Some(4.0)), 0);
    }

    /// For you fills three stars lightly while there is no rating, none once there is one.
    #[test]
    fn for_you_paints_light_stars() {
        let light = |rating| {
            let mut bar = bar(false);
            bar.rating = rating;
            shapes_of(&bar, 1400.0)
                .iter()
                .filter(|shape| matches!(shape, Shape::Path(path) if path.fill == HINT_FILL))
                .count()
        };
        // A pentagon and five tips per star.
        assert_eq!(light(Rating::Unrated), 3 * 6);
        assert_eq!(light(Rating::Stars(3)), 0);
        assert_eq!(light(Rating::Rejected), 0);
    }
}
