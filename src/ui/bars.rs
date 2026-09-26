//! Toolbar (top), info bar (bottom), notices, empty state and the drop hint.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    Align, Align2, Color32, ComboBox, CursorIcon, FontId, Layout, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::analysis::{AestheticsState, Status};
use crate::db::Scores;
use crate::loader::LoadedImage;
use crate::theme::{self, tokens};
use crate::ui::stars;
use crate::view::{BLURRY_PERCENTILE, RatingFilter, SortKey, ViewOptions};

pub const TOOLBAR_HEIGHT: f32 = 40.0;
pub const INFO_HEIGHT: f32 = 52.0;
const STAR_SIZE: f32 = 16.0;
const STAR_GAP: f32 = 6.0;

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
            if ui
                .button("Open…")
                .on_hover_text("Open folder (Ctrl+O)")
                .clicked()
            {
                out.open = true;
            }
            if let Some(folder) = info.folder {
                ui.label(RichText::new(folder).color(tokens::TEXT));
                let count = if info.shown == info.total {
                    format!("{} photos", info.total)
                } else {
                    format!("{} of {} photos", info.shown, info.total)
                };
                ui.label(RichText::new(count).color(tokens::MUTED));
            }
            ui.separator();
            ComboBox::from_id_salt("sort")
                .selected_text(format!("Sort: {}", options.sort.label()))
                .show_ui(ui, |ui| {
                    for key in SortKey::ALL {
                        ui.selectable_value(&mut options.sort, key, key.label());
                    }
                });
            ComboBox::from_id_salt("filter")
                .selected_text(format!("Show: {}", options.filter.label()))
                .show_ui(ui, |ui| {
                    for filter in RatingFilter::ALL {
                        ui.selectable_value(&mut options.filter, filter, filter.label());
                    }
                });
            ui.checkbox(&mut options.hide_blurry, "Hide blurry")
                .on_hover_text("Hide the blurriest 20 % of this folder (by sharpness score)");
            if info.stale
                && ui
                    .button("Refresh order")
                    .on_hover_text("New scores arrived since sorting and filtering")
                    .clicked()
            {
                out.refresh = true;
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                aesthetics_status(ui, &info.status.aesthetics, &mut out);
                let Status { done, total, .. } = *info.status;
                if total > 0 {
                    let text = if done < total {
                        format!("Analyzing {done} / {total}")
                    } else {
                        format!("{total} analyzed")
                    };
                    ui.label(RichText::new(text).color(tokens::MUTED));
                }
            });
        },
    );
    out.options_changed = *options != before;
    out
}

fn aesthetics_status(ui: &mut Ui, state: &AestheticsState, out: &mut ToolbarOutput) {
    let muted = |text: String| RichText::new(text).color(tokens::MUTED);
    match state {
        AestheticsState::Missing => {
            if ui
                .button("Enable aesthetics…")
                .on_hover_text("Downloads the CLIP image model (1.2 GB) once")
                .clicked()
            {
                out.download_model = true;
            }
        }
        AestheticsState::Downloading { received, total } => {
            let percent = *received as f64 / (*total).max(1) as f64 * 100.0;
            ui.label(muted(format!("Downloading model {percent:.0} %")));
        }
        AestheticsState::Available => {
            ui.label(muted("Aesthetics: ready".into()));
        }
        AestheticsState::Loading => {
            ui.label(muted("Aesthetics: loading model…".into()));
        }
        AestheticsState::Ready { backend } => {
            ui.label(muted(format!("Aesthetics: {backend}")))
                .on_hover_text("Where the aesthetics model runs");
        }
        AestheticsState::Failed(message) => {
            ui.label(RichText::new("Aesthetics: failed").color(tokens::STATUS_ERROR))
                .on_hover_text(message);
        }
    }
}

pub struct InfoBar<'a> {
    pub name: &'a str,
    /// 1-based position and count.
    pub position: (usize, usize),
    pub image: Option<&'a LoadedImage>,
    pub rating: Option<u8>,
    pub scores: Option<Scores>,
    pub sharpness_percentile: Option<f32>,
    pub saving: bool,
}

/// Two rows: name / position · date · size (left), stars / scores (centre), exposure / camera
/// and lens (right). Returns the rating the user clicked, if any.
pub fn info_bar(ui: &Ui, rect: Rect, bar: &InfoBar<'_>) -> Option<Option<u8>> {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, tokens::SURFACE);
    painter.hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(1.0, tokens::LINE),
    );
    let (row1, row2) = (rect.top() + 17.0, rect.top() + 37.0);
    let stars_width = 5.0 * STAR_SIZE + 4.0 * STAR_GAP;
    let centre_half = 150.0_f32.max(stars_width / 2.0);
    let centre = rect.center().x;

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
    if let Some(image) = bar.image {
        if let Some(taken) = &image.camera.taken {
            facts.push(taken.clone());
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

    // Right.
    let right = painter.with_clip_rect(Rect::from_min_max(
        pos2(centre + centre_half + 12.0, rect.min.y),
        rect.max,
    ));
    let x = rect.right() - 12.0;
    if let Some(image) = bar.image {
        right.text(
            pos2(x, row1),
            Align2::RIGHT_CENTER,
            image.camera.exposure_line(),
            FontId::proportional(12.5),
            tokens::TEXT,
        );
    }
    let mut gear = Vec::new();
    if bar.saving {
        gear.push("Saving…".to_owned());
    }
    if let Some(image) = bar.image {
        gear.push(image.camera.gear_line());
    }
    right.text(
        pos2(x, row2),
        Align2::RIGHT_CENTER,
        gear.join("   ·   "),
        FontId::proportional(11.5),
        tokens::MUTED,
    );

    // Centre: scores below the stars.
    let mut job = LayoutJob::default();
    let format = |color| TextFormat {
        font_id: FontId::proportional(11.5),
        color,
        ..TextFormat::default()
    };
    match bar.scores {
        Some(scores) => {
            let mut parts = Vec::new();
            if let Some(aesthetic) = scores.aesthetic {
                parts.push(format!("Aesthetics {aesthetic:.1}"));
            }
            if let Some(p) = bar
                .sharpness_percentile
                .filter(|_| scores.sharpness.is_some())
            {
                parts.push(format!("Sharpness {:.0} %", p * 100.0));
            }
            job.append(&parts.join("   ·   "), 0.0, format(tokens::MUTED));
            if bar
                .sharpness_percentile
                .is_some_and(|p| p < BLURRY_PERCENTILE)
            {
                job.append("   ·   probably blurry", 0.0, format(tokens::STATUS_WARN));
            }
        }
        None => job.append("Analyzing…", 0.0, format(tokens::MUTED)),
    }
    let galley = painter.layout_job(job);
    painter.galley(
        pos2(centre - galley.size().x / 2.0, row2 - galley.size().y / 2.0),
        galley,
        tokens::MUTED,
    );

    let stars_left = centre - stars_width / 2.0;
    let mut clicked = None;
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
        let filled = bar.rating.is_some_and(|r| n <= r);
        let color = if filled || response.hovered() {
            tokens::ACCENT
        } else {
            tokens::MUTED
        };
        stars::paint_star(painter, star.center(), STAR_SIZE / 2.0, filled, color);
        if response.clicked() {
            // Clicking the current rating again clears it.
            clicked = Some(if bar.rating == Some(n) { None } else { Some(n) });
        }
        response.on_hover_text(format!("{n} – key {n}"));
    }
    clicked
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

/// Returns whether "Open folder" was clicked.
pub fn empty_state(ui: &mut Ui, rect: Rect) -> bool {
    let arrows = ui
        .ctx()
        .fonts_mut(|f| f.has_glyphs(&FontId::proportional(11.0), "←→"));
    let browse = if arrows { "← →" } else { "Arrow keys" };
    let content = Rect::from_center_size(rect.center(), vec2(520.0, 190.0));
    let mut open = false;
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(content)
            .layout(Layout::top_down(Align::Center)),
        |ui| {
            ui.label(RichText::new("Cerno").size(22.0).color(tokens::TEXT));
            ui.add_space(6.0);
            ui.label(
                RichText::new("Drop a folder or photo here, or press Ctrl+O.").color(tokens::MUTED),
            );
            ui.add_space(18.0);
            open = ui.add(theme::primary_button("Open folder")).clicked();
            ui.add_space(18.0);
            ui.label(
                RichText::new(format!(
                    "{browse} browse   ·   1–5 rate   ·   Z zoom   ·   T filmstrip   ·   F11 fullscreen   ·   I info"
                ))
                .size(11.0)
                .color(tokens::MUTED),
            );
        },
    );
    open
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
        "Drop to open",
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
