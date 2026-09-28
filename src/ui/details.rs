//! Side panel with analysis values of the current photo; explanations fold out per row.

use std::collections::HashSet;

use eframe::egui::{
    Align, Color32, FontId, Id, Label, Layout, Rect, RichText, ScrollArea, Sense, Stroke, Ui,
    UiBuilder, vec2,
};

use crate::analysis::{ModelState, Status, aesthetic, exposure};
use crate::db::Scores;
use crate::histogram::RgbHistogram;
use crate::i18n;
use crate::theme::tokens;
use crate::ui::icons;
use crate::view::BLURRY_PERCENTILE;

pub const WIDTH: f32 = 320.0;
const PAD: f32 = 16.0;
const BAR_HEIGHT: f32 = 4.0;
const ROW_INNER: f32 = 6.0;
const HIST_HEIGHT: f32 = 72.0;

/// `Tab` shows or hides the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailsMode {
    Off,
    On,
}

impl DetailsMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "off" => Some(Self::Off),
            "on" | "values" | "explained" => Some(Self::On),
            _ => None,
        }
    }
}

/// Rows that can expand to show an explanation (and CLIP attributes on the model row).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetailRow {
    Laion,
    V25,
    Personal,
    Frame,
    Eyes,
    Highlights,
    Shadows,
}

pub const EXPANDABLE_ROWS: &[DetailRow] = &[
    DetailRow::Laion,
    DetailRow::V25,
    DetailRow::Personal,
    DetailRow::Frame,
    DetailRow::Eyes,
    DetailRow::Highlights,
    DetailRow::Shadows,
];

pub fn all_expanded(expanded: &HashSet<DetailRow>) -> bool {
    EXPANDABLE_ROWS.iter().all(|row| expanded.contains(row))
}

pub fn set_all_expanded(expanded: &mut HashSet<DetailRow>, on: bool) {
    expanded.clear();
    if on {
        expanded.extend(EXPANDABLE_ROWS);
    }
}

pub struct Details<'a> {
    pub scores: Option<Scores>,
    pub personal: Option<f32>,
    pub frame_percentile: Option<f32>,
    pub eyes_percentile: Option<f32>,
    pub attributes: Option<[f32; 6]>,
    pub histogram: Option<&'a RgbHistogram>,
    pub status: &'a Status,
    /// Original size in pixels and how long the display image took to load.
    pub file: Option<([u32; 2], u128)>,
}

struct Value {
    text: String,
    fill: Option<f32>,
    warn: bool,
}

impl Value {
    fn score(text: String, fill: f32) -> Self {
        Self {
            text,
            fill: Some(fill),
            warn: false,
        }
    }

    fn note(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fill: None,
            warn: false,
        }
    }
}

pub fn draw(ui: &mut Ui, rect: Rect, d: &Details<'_>, expanded: &mut HashSet<DetailRow>) {
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, tokens::SURFACE);
    painter.vline(
        rect.left() + 0.5,
        rect.y_range(),
        Stroke::new(1.0, tokens::LINE),
    );

    let mut panel = ui.new_child(UiBuilder::new().max_rect(rect).id_salt("details"));
    ScrollArea::vertical()
        .auto_shrink(false)
        .show(&mut panel, |ui| {
            ui.set_width(rect.width());
            ui.add_space(PAD);
            ui.spacing_mut().item_spacing.y = 4.0;
            if let Some(hist) = d.histogram {
                histogram(ui, hist);
                ui.add_space(8.0);
            }
            content(ui, d, expanded);
            ui.add_space(PAD);
        });
}

fn content(ui: &mut Ui, d: &Details<'_>, expanded: &mut HashSet<DetailRow>) {
    let t = i18n::t();
    let scores = d.scores.unwrap_or_default();
    let status = d.status;
    let taste = &status.taste;

    section(ui, t.section_aesthetics);
    metric_row(
        ui,
        expanded,
        DetailRow::Laion,
        t.row_laion,
        match scores.aesthetic {
            Some(v) => stars_value(aesthetic::as_stars(v)),
            None => Value::note(model_note(&status.aesthetics)),
        },
        t.explain_laion,
    );
    if expanded.contains(&DetailRow::Laion) {
        clip_details(ui, d);
    }
    metric_row(
        ui,
        expanded,
        DetailRow::V25,
        t.row_v25,
        match scores.aesthetic25 {
            Some(v) => stars_value(aesthetic::as_stars(v)),
            None => Value::note(model_note(&status.v25)),
        },
        t.explain_v25,
    );

    section(ui, t.row_personal);
    metric_row(
        ui,
        expanded,
        DetailRow::Personal,
        t.row_personal,
        match (d.personal, taste.model) {
            (Some(v), _) => Value::score(format!("{v:.1} ★"), v / 5.0),
            (None, Some(_)) => Value::note(t.note_no_embedding),
            (None, None) => Value::note((t.note_learning)(
                taste.examples,
                crate::analysis::taste::MIN_EXAMPLES,
            )),
        },
        t.explain_personal,
    );

    section(ui, t.section_sharpness);
    metric_row(
        ui,
        expanded,
        DetailRow::Frame,
        t.row_frame,
        match d.frame_percentile {
            Some(p) => Value {
                text: format!("{:.0} %", p * 100.0),
                fill: Some(p),
                warn: p < BLURRY_PERCENTILE && d.eyes_percentile.is_none(),
            },
            None => Value::note(t.note_analysing),
        },
        t.explain_frame,
    );
    metric_row(
        ui,
        expanded,
        DetailRow::Eyes,
        t.row_eyes,
        match (d.eyes_percentile, scores.faces) {
            (Some(p), _) => Value {
                text: format!("{:.0} %", p * 100.0),
                fill: Some(p),
                warn: p < BLURRY_PERCENTILE,
            },
            (None, Some(0)) => Value::note(t.note_no_face),
            (None, Some(n)) => Value::note((t.note_faces_too_small)(n)),
            (None, None) => Value::note(t.note_analysing),
        },
        t.explain_eyes,
    );

    section(ui, t.section_exposure);
    let clipped = |share: Option<f32>, limit: f32| match share {
        Some(s) => Value {
            text: format!("{:.1} %", s * 100.0),
            fill: Some((s / (limit * 4.0)).min(1.0)),
            warn: s > limit,
        },
        None => Value::note(t.note_analysing),
    };
    metric_row(
        ui,
        expanded,
        DetailRow::Highlights,
        t.row_highlights,
        clipped(scores.highlights, exposure::HIGHLIGHTS_WARN),
        t.explain_highlights,
    );
    metric_row(
        ui,
        expanded,
        DetailRow::Shadows,
        t.row_shadows,
        clipped(scores.shadows, exposure::SHADOWS_WARN),
        t.explain_shadows,
    );

    if let Some(([w, h], load_ms)) = d.file {
        section(ui, t.section_file);
        plain_row(ui, t.row_size, format!("{w} × {h}"));
        plain_row(ui, t.row_load_time, format!("{load_ms} ms"));
    }
}

fn histogram(ui: &mut Ui, hist: &RgbHistogram) {
    let t = i18n::t();
    section(ui, t.section_histogram);
    let width = ui.available_width() - 2.0 * PAD;
    let (rect, _) = ui.allocate_exact_size(vec2(width, HIST_HEIGHT), Sense::hover());
    let rect = rect.translate(vec2(PAD, 0.0));
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 2.0, tokens::SURFACE_MUTED);
    let max = hist
        .iter()
        .flat_map(|channel| channel.iter())
        .copied()
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let colours = [
        Color32::from_rgba_unmultiplied(0xe0, 0x5a, 0x4e, 140),
        Color32::from_rgba_unmultiplied(0x6a, 0xc4, 0x6a, 140),
        Color32::from_rgba_unmultiplied(0x5b, 0x8e, 0xc4, 140),
    ];
    let bar_w = rect.width() / 256.0;
    for (channel, colour) in hist.iter().zip(colours) {
        for (bin, &count) in channel.iter().enumerate() {
            if count == 0 {
                continue;
            }
            let h = rect.height() * (count as f32 / max);
            let x = rect.left() + bar_w * bin as f32;
            let y = rect.bottom() - h;
            painter.rect_filled(
                Rect::from_min_max(
                    eframe::egui::pos2(x, y),
                    eframe::egui::pos2(x + bar_w.max(1.0), rect.bottom()),
                ),
                0.0,
                colour,
            );
        }
    }
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(PAD);
        ui.label(
            RichText::new(title.to_uppercase())
                .font(FontId::proportional(10.5))
                .color(tokens::ACCENT),
        );
    });
    ui.add_space(2.0);
}

fn metric_row(
    ui: &mut Ui,
    expanded: &mut HashSet<DetailRow>,
    id: DetailRow,
    label: &str,
    value: Value,
    explain: &str,
) {
    let open = expanded.contains(&id);
    let row_id = Id::new(("detail_row", id));
    let response = ui
        .horizontal(|ui| {
            ui.add_space(PAD);
            let (chevron_rect, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
            icons::chevron(ui.painter(), chevron_rect.center(), open, tokens::MUTED);
            ui.label(
                RichText::new(label)
                    .font(FontId::proportional(12.5))
                    .color(tokens::TEXT),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(PAD);
                paint_value(ui, &value);
            });
        })
        .response
        .interact(Sense::click());
    if response.clicked() {
        if open {
            expanded.remove(&id);
        } else {
            expanded.insert(id);
        }
        ui.ctx().request_repaint();
    }
    if value.fill.is_some() {
        value_bar(ui, row_id, &value);
    }
    if open {
        ui.add_space(ROW_INNER);
        explanation(ui, explain);
        ui.add_space(ROW_INNER);
    } else {
        ui.add_space(4.0);
    }
}

fn clip_details(ui: &mut Ui, d: &Details<'_>) {
    let t = i18n::t();
    explanation(ui, t.explain_attributes);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(PAD + 14.0);
        ui.label(
            RichText::new(t.section_attributes.to_uppercase())
                .font(FontId::proportional(10.5))
                .color(tokens::ACCENT),
        );
    });
    ui.add_space(ROW_INNER);
    for (i, name) in t.attributes.iter().enumerate() {
        let value = match d.attributes {
            Some(a) => Value::score(format!("{:.0} %", a[i] * 100.0), a[i]),
            None => Value::note(t.note_needs_clip),
        };
        ui.horizontal(|ui| {
            ui.add_space(PAD + 14.0);
            ui.label(RichText::new(*name).font(FontId::proportional(12.0)))
                .on_hover_text(t.explain_attribute[i]);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(PAD);
                paint_value(ui, &value);
            });
        });
        if value.fill.is_some() {
            value_bar(ui, Id::new(("clip_attr", i)), &value);
        }
        ui.add_space(4.0);
    }
}

/// Muted label and a plain value, without a bar or a fold.
fn plain_row(ui: &mut Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.add_space(PAD + 14.0);
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(12.0))
                .color(tokens::MUTED),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(PAD);
            ui.label(
                RichText::new(value)
                    .font(FontId::proportional(12.0))
                    .color(tokens::TEXT),
            );
        });
    });
    ui.add_space(2.0);
}

fn paint_value(ui: &mut Ui, value: &Value) {
    let colour = if value.warn {
        tokens::STATUS_WARN
    } else {
        tokens::TEXT
    };
    let size = if value.fill.is_some() { 15.0 } else { 12.0 };
    let text_colour = if value.fill.is_some() {
        colour
    } else {
        tokens::MUTED
    };
    ui.label(
        RichText::new(value.text.clone())
            .font(FontId::proportional(size))
            .color(text_colour),
    );
}

fn value_bar(ui: &mut Ui, _id: Id, value: &Value) {
    let Some(fill) = value.fill else {
        return;
    };
    let width = ui.available_width() - 2.0 * PAD;
    let (rect, _) = ui.allocate_exact_size(vec2(width, BAR_HEIGHT + 4.0), Sense::hover());
    let track = rect.translate(vec2(PAD, 2.0)).shrink2(vec2(0.0, 0.0));
    let track = Rect::from_min_size(track.min, vec2(track.width(), BAR_HEIGHT));
    let painter = ui.painter();
    painter.rect_filled(track, 2.0, tokens::LINE);
    let bar_colour = if value.warn {
        tokens::STATUS_WARN
    } else {
        tokens::ACCENT
    };
    painter.rect_filled(
        Rect::from_min_size(
            track.min,
            vec2(track.width() * fill.clamp(0.0, 1.0), BAR_HEIGHT),
        ),
        2.0,
        bar_colour,
    );
}

/// Muted text that wraps inside the panel. A label in a left-to-right layout never wraps in
/// egui – it would widen the scroll area and push the values of every later row out of view.
fn explanation(ui: &mut Ui, text: &str) {
    let width = (ui.available_width() - 2.0 * PAD).max(40.0);
    ui.horizontal(|ui| {
        ui.add_space(PAD);
        ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_max_width(width);
            ui.add(
                Label::new(
                    RichText::new(i18n::keep_together(text))
                        .font(FontId::proportional(13.0))
                        .color(tokens::MUTED),
                )
                .wrap(),
            );
        });
    });
}

fn stars_value(stars: f32) -> Value {
    Value::score(format!("{stars:.1} ★"), stars / 5.0)
}

/// A model's state as a short word (`bereit`, `DirectML`, `lädt 42 %` …).
pub fn model_note(state: &ModelState) -> String {
    let t = i18n::t();
    match state {
        ModelState::Missing => t.model_missing.to_owned(),
        ModelState::Downloading { received, total } => {
            (t.model_downloading)(*received as f64 / (*total).max(1) as f64 * 100.0)
        }
        ModelState::Available => t.model_ready.to_owned(),
        ModelState::Loading => t.model_loading.to_owned(),
        ModelState::Ready { backend } => (*backend).to_owned(),
        ModelState::Failed(_) => t.model_failed.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::TasteStatus;
    use eframe::egui::{Context, RawInput, Shape, pos2, vec2};

    fn status() -> Status {
        Status {
            done: 0,
            total: 0,
            aesthetics: ModelState::Missing,
            v25: ModelState::Missing,
            faces: ModelState::Missing,
            taste: TasteStatus {
                examples: 0,
                model: None,
            },
        }
    }

    #[test]
    fn details_mode_maps_legacy_settings() {
        assert_eq!(DetailsMode::from_id("explained"), Some(DetailsMode::On));
        assert_eq!(DetailsMode::from_id("values"), Some(DetailsMode::On));
        assert_eq!(DetailsMode::from_id("off"), Some(DetailsMode::Off));
    }

    #[test]
    fn clip_fold_shows_attribute_labels() {
        let ctx = Context::default();
        let status = status();
        let details = Details {
            scores: None,
            personal: None,
            frame_percentile: None,
            eyes_percentile: None,
            attributes: Some([0.5; 6]),
            histogram: None,
            status: &status,
            file: None,
        };
        let mut expanded = HashSet::from([DetailRow::Laion]);
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH, 800.0));
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| {
                draw(ui, screen, &details, &mut expanded);
            },
        );
        output.textures_delta.clear();
        let has_quality = output.shapes.iter().any(|clipped| {
            matches!(
                &clipped.shape,
                Shape::Text(text) if text.galley.text().contains("Overall quality")
            )
        });
        assert!(has_quality, "CLIP attributes visible when expanded");
    }

    /// `I` expands every row: the explanations wrap inside the panel and every value stays
    /// visible.
    #[test]
    fn expanded_rows_stay_inside_the_panel() {
        let ctx = Context::default();
        let status = status();
        let details = Details {
            scores: Some(Scores {
                sharpness: Some(1.0),
                aesthetic: Some(5.0),
                aesthetic25: Some(6.0),
                highlights: Some(0.0),
                shadows: Some(0.0),
                eyes: None,
                faces: Some(0),
            }),
            personal: Some(2.8),
            frame_percentile: Some(0.62),
            eyes_percentile: None,
            attributes: Some([0.5; 6]),
            histogram: None,
            status: &status,
            file: None,
        };
        let mut expanded = HashSet::new();
        set_all_expanded(&mut expanded, true);
        let panel = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH, 2400.0));
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH * 3.0, 2400.0));
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| {
                draw(ui, panel, &details, &mut expanded);
            },
        );
        output.textures_delta.clear();
        let texts: Vec<(String, Rect)> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Text(text) => {
                    Some((text.galley.text().to_owned(), text.visual_bounding_rect()))
                }
                _ => None,
            })
            .collect();
        let v25 = format!("{:.1} ★", aesthetic::as_stars(6.0));
        for value in [v25.as_str(), "2.8 ★", "62 %", "50 %"] {
            assert!(
                texts.iter().any(|(text, _)| text == value),
                "value {value} is drawn"
            );
        }
        for (text, rect) in &texts {
            assert!(
                rect.right() <= panel.right() + 0.5,
                "{text:?} ends at {} beyond the panel ({})",
                rect.right(),
                panel.right()
            );
        }
    }

    #[test]
    fn expand_all_helpers() {
        let mut expanded = HashSet::new();
        assert!(!all_expanded(&expanded));
        set_all_expanded(&mut expanded, true);
        assert!(all_expanded(&expanded));
        set_all_expanded(&mut expanded, false);
        assert!(expanded.is_empty());
    }
}
