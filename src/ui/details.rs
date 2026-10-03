//! Side panel with analysis values of the current photo. Each row explains itself when the
//! pointer rests on it; only the CLIP attributes fold out (they are further values).

use eframe::egui::{
    Align, Align2, Color32, CursorIcon, FontId, Hyperlink, Id, Label, Layout, Rect, Response,
    RichText, ScrollArea, Sense, Stroke, Ui, UiBuilder, vec2,
};

use crate::analysis::{ModelState, Status, aesthetic, exposure};
use crate::db::Scores;
use crate::histogram::RgbHistogram;
use crate::i18n;
use crate::metadata;
use crate::overlay;
use crate::playback::MediaInfo;
use crate::theme::{text, tokens};
use crate::ui::{icons, video_controls};
use crate::view::is_blurry;

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

/// What the panel shows: the analysis values or the photo's comment and keywords (`B`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailsTab {
    Values,
    Description,
}

impl DetailsTab {
    pub fn id(self) -> &'static str {
        match self {
            Self::Values => "values",
            Self::Description => "description",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [Self::Values, Self::Description]
            .into_iter()
            .find(|tab| tab.id() == id)
    }
}

/// Height of the tab strip at the top of the panel.
pub const TABS_HEIGHT: f32 = 34.0;

/// The two tabs at the top of the panel. Returns the other tab when it was clicked.
pub fn tabs(ui: &mut Ui, rect: Rect, current: DetailsTab) -> Option<DetailsTab> {
    let t = i18n::t();
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, tokens::SURFACE);
    let line = Stroke::new(1.0, tokens::LINE);
    painter.vline(rect.left() + 0.5, rect.y_range(), line);
    painter.hline(rect.x_range(), rect.bottom() - 0.5, line);
    let half = rect.width() / 2.0;
    let mut clicked = None;
    for (i, (tab, label)) in [
        (DetailsTab::Values, t.tab_values),
        (DetailsTab::Description, t.tab_description),
    ]
    .into_iter()
    .enumerate()
    {
        let cell = Rect::from_min_size(
            rect.min + vec2(half * i as f32, 0.0),
            vec2(half, rect.height()),
        );
        let response = ui
            .interact(cell, Id::new(("details-tab", i)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let active = tab == current;
        painter.text(
            cell.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(text::BODY),
            if active { tokens::TEXT } else { tokens::MUTED },
        );
        if active {
            painter.hline(
                cell.x_range().shrink(14.0),
                rect.bottom() - 1.5,
                Stroke::new(2.0, tokens::ACCENT),
            );
        }
        if response.clicked() && !active {
            clicked = Some(tab);
        }
    }
    clicked
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
    /// GPS position (latitude, longitude) from the EXIF data.
    pub position: Option<(f64, f64)>,
    /// A video's streams (`playback::probe`): shown instead of the size and load time.
    pub media: Option<&'a MediaInfo>,
    /// What the check overlay shows; its eye is lit in that section.
    pub overlay: overlay::Mode,
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

    /// A share of 0..=1 as `62 %` with its bar.
    fn percent(share: f32) -> Self {
        Self::score(format!("{:.0} %", share * 100.0), share)
    }

    fn note(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fill: None,
            warn: false,
        }
    }
}

/// Draws the panel. `attributes_open`: the CLIP attributes are folded out (a click on their
/// row toggles it). Returns the overlay an eye asks for (a lit eye turns it off).
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    d: &Details<'_>,
    attributes_open: &mut bool,
) -> Option<overlay::Mode> {
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
            let overlay = content(ui, d, attributes_open);
            ui.add_space(PAD);
            overlay
        })
        .inner
}

fn content(ui: &mut Ui, d: &Details<'_>, attributes_open: &mut bool) -> Option<overlay::Mode> {
    let t = i18n::t();
    let scores = d.scores.unwrap_or_default();
    let status = d.status;
    let taste = &status.taste;
    let model_value = |score: Option<f32>, state: &ModelState| match score {
        Some(v) => Value::percent(aesthetic::as_percent(v)),
        None => Value::note(model_note(state)),
    };

    section(ui, t.section_aesthetics);
    metric_row(
        ui,
        t.row_aesthetics,
        match aesthetic::combined(scores.aesthetic, scores.aesthetic25) {
            Some(v) => Value::percent(aesthetic::as_percent(v)),
            None => Value::note(model_note(&status.aesthetics)),
        },
        t.explain_aesthetics,
    );
    metric_row(
        ui,
        t.row_laion,
        model_value(scores.aesthetic, &status.aesthetics),
        t.explain_laion,
    );
    metric_row(
        ui,
        t.row_v25,
        model_value(scores.aesthetic25, &status.v25),
        t.explain_v25,
    );
    attributes(ui, d, attributes_open);

    section(ui, t.row_personal);
    metric_row(
        ui,
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

    let mut overlay = None;
    // The eye shows this section on the photo; a lit one turns the overlay off again.
    let mut eye = |ui: &mut Ui, title: &str, mode: overlay::Mode| {
        if section_with_eye(ui, title, d.overlay == mode) {
            overlay = Some(if d.overlay == mode {
                overlay::Mode::Off
            } else {
                mode
            });
        }
    };
    eye(ui, t.section_sharpness, overlay::Mode::Sharpness);
    metric_row(
        ui,
        t.row_frame,
        match d.frame_percentile {
            Some(p) => Value {
                text: format!("{:.0} %", p * 100.0),
                fill: Some(p),
                warn: d.eyes_percentile.is_none()
                    && scores.sharpness.is_some_and(|raw| is_blurry(p, raw, false)),
            },
            None => Value::note(t.note_analysing),
        },
        t.explain_frame,
    );
    metric_row(
        ui,
        t.row_eyes,
        match (d.eyes_percentile, scores.faces) {
            (Some(p), _) => Value {
                text: format!("{:.0} %", p * 100.0),
                fill: Some(p),
                warn: scores.eyes.is_some_and(|raw| is_blurry(p, raw, true)),
            },
            (None, Some(0)) => Value::note(t.note_no_face),
            (None, Some(n)) => Value::note((t.note_faces_too_small)(n)),
            (None, None) => Value::note(t.note_analysing),
        },
        t.explain_eyes,
    );

    eye(ui, t.section_exposure, overlay::Mode::Exposure);
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
        t.row_highlights,
        clipped(scores.highlights, exposure::HIGHLIGHTS_WARN),
        t.explain_highlights,
    );
    metric_row(
        ui,
        t.row_shadows,
        clipped(scores.shadows, exposure::SHADOWS_WARN),
        t.explain_shadows,
    );

    if d.file.is_some() || d.media.is_some() {
        section(ui, t.section_file);
        match (d.media, d.file) {
            (Some(media), _) => {
                for (label, value) in media_rows(media) {
                    plain_row(ui, label, value);
                }
            }
            (None, Some(([w, h], load_ms))) => {
                plain_row(ui, t.row_size, format!("{w} × {h}"));
                plain_row(ui, t.row_load_time, format!("{load_ms} ms"));
            }
            (None, None) => {}
        }
        if let Some((lat, lon)) = d.position {
            plain_row(ui, t.row_location, i18n::coordinates(lat, lon));
            map_links(ui, (lat, lon));
        }
    }
    overlay
}

/// A video's file rows: container, duration, the video stream (codec and HDR, size, frame
/// rate, bitrate – "≈" when estimated from the total), the sound and the total bitrate.
fn media_rows(media: &MediaInfo) -> Vec<(&'static str, String)> {
    let t = i18n::t();
    let mut rows = Vec::new();
    if let Some(container) = &media.container {
        rows.push((t.row_container, container.clone()));
    }
    if let Some(duration) = media.duration {
        rows.push((t.row_duration, video_controls::clock(duration)));
    }
    if let Some(video) = &media.video {
        let codec = match video.hdr {
            Some(hdr) => format!("{} · HDR ({hdr})", video.codec),
            None => video.codec.clone(),
        };
        rows.push((t.row_video, codec));
        rows.push((t.row_size, format!("{} × {}", video.width, video.height)));
        if let Some(fps) = video.fps {
            rows.push((t.row_frame_rate, frame_rate(fps)));
        }
        if let Some(rate) = video.bitrate {
            let approx = if video.bitrate_estimated { "≈ " } else { "" };
            rows.push((t.row_video_bitrate, format!("{approx}{}", bitrate(rate))));
        }
    }
    match &media.audio {
        Some(audio) => {
            let mut parts = vec![audio.codec.clone(), (t.channels)(audio.channels)];
            if audio.sample_rate > 0 {
                parts.push(format!("{} kHz", f64::from(audio.sample_rate) / 1000.0));
            }
            if let Some(language) = &audio.language {
                parts.push(language.clone());
            }
            rows.push((t.row_audio, parts.join(" · ")));
            if let Some(rate) = audio.bitrate {
                rows.push((t.row_audio_bitrate, bitrate(rate)));
            }
        }
        None => rows.push((t.row_audio, t.no_audio.to_owned())),
    }
    if let Some(rate) = media.bitrate {
        rows.push((t.row_bitrate, bitrate(rate)));
    }
    rows
}

/// `5.8 Mbit/s`, `128 kbit/s` (decimal point in every language, like the other values).
fn bitrate(bits_per_second: u64) -> String {
    if bits_per_second >= 1_000_000 {
        format!("{:.1} Mbit/s", bits_per_second as f64 / 1e6)
    } else {
        format!("{:.0} kbit/s", bits_per_second as f64 / 1e3)
    }
}

/// `30 fps`, `29.97 fps`.
fn frame_rate(fps: f64) -> String {
    if (fps - fps.round()).abs() < 0.005 {
        format!("{fps:.0} fps")
    } else {
        format!("{fps:.2} fps")
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
                .font(FontId::proportional(text::LABEL))
                .color(tokens::MUTED),
        );
    });
    ui.add_space(2.0);
}

/// A section title with the eye that shows the section's values on the photo, at the right
/// like the values below it. Whether the eye was clicked.
fn section_with_eye(ui: &mut Ui, title: &str, on: bool) -> bool {
    ui.add_space(6.0);
    let clicked = ui
        .horizontal(|ui| {
            ui.add_space(PAD);
            ui.label(
                RichText::new(title.to_uppercase())
                    .font(FontId::proportional(text::LABEL))
                    .color(tokens::MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(PAD);
                let (rect, response) = ui.allocate_exact_size(vec2(22.0, 14.0), Sense::click());
                let colour = if on {
                    tokens::ACCENT
                } else if response.hovered() {
                    tokens::TEXT
                } else {
                    tokens::MUTED
                };
                icons::eye(ui.painter(), rect.center(), on, colour);
                response
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text(i18n::t().overlay_show_on_photo)
                    .clicked()
            })
            .inner
        })
        .inner;
    ui.add_space(2.0);
    clicked
}

/// Label and value with its bar; the explanation shows while the pointer rests on the row.
fn metric_row(ui: &mut Ui, label: &str, value: Value, explain: &str) {
    let explain = i18n::keep_together(explain);
    ui.horizontal(|ui| {
        ui.add_space(PAD + 14.0);
        // Not selectable: a selectable label takes the pointer, and the row's tooltip with it.
        ui.add(
            Label::new(
                RichText::new(label)
                    .font(FontId::proportional(text::BODY))
                    .color(tokens::TEXT),
            )
            .selectable(false),
        )
        .on_hover_text(&explain);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(PAD);
            paint_value(ui, &value).on_hover_text(&explain);
        });
    })
    .response
    .interact(Sense::hover())
    .on_hover_text(&explain);
    if value.fill.is_some() {
        value_bar(ui, &value);
    }
    ui.add_space(4.0);
}

/// The one row that folds out: the six CLIP attributes, each a value of its own.
fn attributes(ui: &mut Ui, d: &Details<'_>, open: &mut bool) {
    let t = i18n::t();
    let response = ui
        .horizontal(|ui| {
            ui.add_space(PAD);
            let (chevron, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
            icons::chevron(ui.painter(), chevron.center(), *open, tokens::MUTED);
            ui.add(
                Label::new(
                    RichText::new(t.section_attributes)
                        .font(FontId::proportional(text::BODY))
                        .color(tokens::TEXT),
                )
                .selectable(false),
            );
        })
        .response
        .interact(Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(i18n::keep_together(t.explain_attributes));
    if response.clicked() {
        *open = !*open;
        ui.ctx().request_repaint();
    }
    ui.add_space(4.0);
    if !*open {
        return;
    }
    for (i, name) in t.attributes.iter().enumerate() {
        let value = match d.attributes {
            Some(a) => Value::percent(a[i]),
            None => Value::note(t.note_needs_clip),
        };
        ui.horizontal(|ui| {
            ui.add_space(PAD + 14.0);
            ui.label(RichText::new(*name).font(FontId::proportional(text::SMALL)))
                .on_hover_text(t.explain_attribute[i]);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(PAD);
                paint_value(ui, &value);
            });
        });
        if value.fill.is_some() {
            value_bar(ui, &value);
        }
        ui.add_space(4.0);
    }
    ui.add_space(ROW_INNER);
}

/// Muted label and a plain value, without a bar or a fold.
fn plain_row(ui: &mut Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.add_space(PAD + 14.0);
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(text::SMALL))
                .color(tokens::MUTED),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(PAD);
            ui.label(
                RichText::new(value)
                    .font(FontId::proportional(text::SMALL))
                    .color(tokens::TEXT),
            );
        });
    });
    ui.add_space(2.0);
}

/// `Google Maps · OpenStreetMap` under the coordinates, right-aligned like the values. A click
/// opens the browser – and sends the position to that service.
fn map_links(ui: &mut Ui, position: (f64, f64)) {
    let link = |label: &str, url: String| {
        Hyperlink::from_label_and_url(
            RichText::new(label).font(FontId::proportional(text::SMALL)),
            url,
        )
        .open_in_new_tab(true)
    };
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(PAD);
            ui.add(link("OpenStreetMap", metadata::osm_url(position)));
            ui.label(
                RichText::new("·")
                    .font(FontId::proportional(text::SMALL))
                    .color(tokens::MUTED),
            );
            ui.add(link("Google Maps", metadata::maps_url(position)));
        });
    });
    ui.add_space(2.0);
}

fn paint_value(ui: &mut Ui, value: &Value) -> Response {
    let colour = if value.warn {
        tokens::STATUS_WARN
    } else {
        tokens::TEXT
    };
    let size = if value.fill.is_some() {
        text::LARGE
    } else {
        text::SMALL
    };
    let text_colour = if value.fill.is_some() {
        colour
    } else {
        tokens::MUTED
    };
    ui.add(
        Label::new(
            RichText::new(value.text.clone())
                .font(FontId::proportional(size))
                .color(text_colour),
        )
        .selectable(false),
    )
}

fn value_bar(ui: &mut Ui, value: &Value) {
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
        ModelState::Removing => t.model_removing.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::TasteStatus;
    use eframe::egui::{
        Context, Event, Modifiers, OutputCommand, PointerButton, RawInput, Shape, pos2, vec2,
    };

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

    fn details_with_scores(status: &Status) -> Details<'_> {
        Details {
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
            status,
            file: None,
            position: None,
            media: None,
            overlay: overlay::Mode::Off,
        }
    }

    fn texts(output: &eframe::egui::FullOutput) -> Vec<(String, Rect)> {
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

    #[test]
    fn clip_fold_shows_attribute_labels() {
        let status = status();
        let details = details_with_scores(&status);
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH, 1200.0));
        let shows_quality = |mut open: bool| {
            let ctx = Context::default();
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| {
                    draw(ui, screen, &details, &mut open);
                },
            );
            output.textures_delta.clear();
            texts(&output)
                .iter()
                .any(|(text, _)| text.contains("Overall quality"))
        };
        assert!(
            shows_quality(true),
            "CLIP attributes visible when folded out"
        );
        assert!(!shows_quality(false), "and hidden otherwise");
    }

    /// Every value is drawn and stays inside the panel, the attributes folded out too; the
    /// scores are percentages on the fixed scale.
    #[test]
    fn values_stay_inside_the_panel() {
        let ctx = Context::default();
        let status = status();
        let details = details_with_scores(&status);
        let panel = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH, 2400.0));
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH * 3.0, 2400.0));
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| {
                draw(ui, panel, &details, &mut true);
            },
        );
        output.textures_delta.clear();
        let texts = texts(&output);
        // Aesthetics (5.0 + 6.0) / 2 = 5.5 → 58 %, LAION 5.0 → 50 %, V2.5 6.0 → 67 %.
        for value in ["58 %", "50 %", "67 %", "2.8 ★", "62 %"] {
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

    /// Nothing folds out under a value any more: the explanation is the row's tooltip.
    #[test]
    fn a_row_explains_itself_on_hover() {
        let ctx = Context::default();
        ctx.all_styles_mut(|style| style.interaction.tooltip_delay = 0.0);
        let status = status();
        let details = details_with_scores(&status);
        let panel = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH, 1200.0));
        let frame = |events: Vec<Event>, time: f64| {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(panel),
                    events,
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    draw(ui, panel, &details, &mut false);
                },
            );
            output.textures_delta.clear();
            output
        };
        let explanation = i18n::keep_together(i18n::t().explain_v25);
        let first = frame(Vec::new(), 0.0);
        let shown = texts(&first);
        assert!(
            !shown.iter().any(|(text, _)| *text == explanation),
            "no explanation without the pointer"
        );
        let row = shown
            .iter()
            .find(|(text, _)| text == "V2.5 (SigLIP)")
            .map(|(_, rect)| rect.center())
            .expect("the V2.5 row is drawn");
        // egui waits until the pointer rests before it shows a tooltip.
        frame(vec![Event::PointerMoved(row)], 0.1);
        let mut hovered = Vec::new();
        for n in 1..6 {
            hovered = texts(&frame(Vec::new(), 0.1 + f64::from(n) * 0.3));
        }
        assert!(
            hovered.iter().any(|(text, _)| *text == explanation),
            "the explanation shows while the pointer rests on the row"
        );
    }

    /// The File section shows the coordinates and links them to both maps; a click opens the
    /// browser with that map (egui reports it, nothing is opened in a test).
    #[test]
    fn file_section_links_the_position_to_both_maps() {
        let ctx = Context::default();
        let status = status();
        let position = (48.5216, -9.0576);
        let details = Details {
            scores: None,
            personal: None,
            frame_percentile: None,
            eyes_percentile: None,
            attributes: None,
            histogram: None,
            status: &status,
            file: Some(([6000, 4000], 120)),
            position: Some(position),
            media: None,
            overlay: overlay::Mode::Off,
        };
        let panel = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH, 1200.0));
        let frame = |events: Vec<Event>| {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(panel),
                    events,
                    ..Default::default()
                },
                |ui| {
                    draw(ui, panel, &details, &mut false);
                },
            );
            output.textures_delta.clear();
            output
        };
        let output = frame(Vec::new());
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
        let find = |wanted: &str| {
            texts
                .iter()
                .find(|(text, _)| text == wanted)
                .map(|(_, rect)| *rect)
                .unwrap_or_else(|| panic!("{wanted:?} is drawn"))
        };
        find("48.52160° N, 9.05760° W");
        find("Google Maps");
        let osm = find("OpenStreetMap");
        assert!(
            osm.right() <= panel.right(),
            "the link stays inside the panel"
        );

        let at = osm.center();
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(vec![Event::PointerMoved(at)]);
        frame(vec![button(true)]);
        let opened: Vec<String> = frame(vec![button(false)])
            .platform_output
            .commands
            .into_iter()
            .filter_map(|command| match command {
                OutputCommand::OpenUrl(open) => Some(open.url),
                _ => None,
            })
            .collect();
        assert_eq!(opened, [metadata::osm_url(position)]);
    }

    /// The eye next to "Sharpness" turns the sharpness overlay on, and off again when lit.
    #[test]
    fn the_eye_switches_the_overlay() {
        let ctx = Context::default();
        let status = status();
        let panel = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH, 1200.0));
        let run = |mode: overlay::Mode, events: Vec<Event>| {
            let details = Details {
                scores: None,
                personal: None,
                frame_percentile: None,
                eyes_percentile: None,
                attributes: None,
                histogram: None,
                status: &status,
                file: None,
                position: None,
                media: None,
                overlay: mode,
            };
            let mut chosen = None;
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(panel),
                    events,
                    ..Default::default()
                },
                |ui| chosen = draw(ui, panel, &details, &mut false),
            );
            output.textures_delta.clear();
            (output, chosen)
        };
        let (output, _) = run(overlay::Mode::Off, Vec::new());
        let title = output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                Shape::Text(text) if text.galley.text().starts_with("SHARPNESS") => {
                    Some(text.visual_bounding_rect())
                }
                _ => None,
            })
            .expect("the sharpness section is drawn");
        let eye = pos2(panel.right() - PAD - 11.0, title.center().y);
        let click = |mode| {
            let button = |pressed| Event::PointerButton {
                pos: eye,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            run(mode, vec![Event::PointerMoved(eye)]);
            run(mode, vec![button(true)]);
            run(mode, vec![button(false)]).1
        };
        assert_eq!(click(overlay::Mode::Off), Some(overlay::Mode::Sharpness));
        assert_eq!(click(overlay::Mode::Sharpness), Some(overlay::Mode::Off));
        assert_eq!(
            click(overlay::Mode::Exposure),
            Some(overlay::Mode::Sharpness),
            "from the other overlay straight to this one"
        );
    }

    #[test]
    fn a_video_lists_its_streams() {
        use crate::playback::{AudioStream, VideoStream};
        let media = MediaInfo {
            container: Some("ISO MP4/M4A".into()),
            duration: Some(std::time::Duration::from_secs(75)),
            bitrate: Some(12_400_000),
            video: Some(VideoStream {
                codec: "H.265 (Main 10 Profile)".into(),
                width: 3840,
                height: 2160,
                fps: Some(29.97),
                hdr: Some("HLG"),
                bitrate: Some(12_200_000),
                bitrate_estimated: true,
            }),
            audio: Some(AudioStream {
                codec: "MPEG-4 AAC".into(),
                channels: 2,
                sample_rate: 48_000,
                bitrate: Some(192_000),
                language: None,
            }),
        };
        // Tests never switch the language: English.
        let rows: Vec<String> = media_rows(&media)
            .into_iter()
            .map(|(label, value)| format!("{label}: {value}"))
            .collect();
        assert_eq!(
            rows,
            [
                "Container: ISO MP4/M4A",
                "Duration: 1:15",
                "Video: H.265 (Main 10 Profile) · HDR (HLG)",
                "Size: 3840 × 2160",
                "Frame rate: 29.97 fps",
                "Video bitrate: ≈ 12.2 Mbit/s",
                "Audio: MPEG-4 AAC · Stereo · 48 kHz",
                "Audio bitrate: 192 kbit/s",
                "Total bitrate: 12.4 Mbit/s",
            ]
        );
        let silent = MediaInfo {
            audio: None,
            ..MediaInfo::default()
        };
        assert_eq!(
            media_rows(&silent)
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>(),
            ["no audio track"]
        );
        assert_eq!(frame_rate(30.0), "30 fps");
        assert_eq!(bitrate(950_000), "950 kbit/s");
    }
}
