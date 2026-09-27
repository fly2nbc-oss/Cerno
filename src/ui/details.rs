//! Side panel with every analysis value of the current photo (`P`), each with a short
//! explanation in plain language.

use eframe::egui::{Align2, FontId, Painter, Rect, ScrollArea, Stroke, Ui, UiBuilder, pos2, vec2};

use crate::analysis::{ModelState, Status, aesthetic, exposure};
use crate::db::Scores;
use crate::i18n;
use crate::theme::tokens;
use crate::view::BLURRY_PERCENTILE;

pub const WIDTH: f32 = 320.0;
const PAD: f32 = 16.0;
const BAR_HEIGHT: f32 = 4.0;
/// Space between the explanation of one row and the next row.
const ROW_GAP: f32 = 12.0;
/// Space below a row without explanation.
const COMPACT_GAP: f32 = 10.0;

/// How much the panel shows: `Tab` shows or hides it, `I` steps through the three stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailsMode {
    Off,
    Values,
    /// Values with a plain-language explanation under each.
    Explained,
}

impl DetailsMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Values,
            Self::Values => Self::Explained,
            Self::Explained => Self::Off,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Values => "values",
            Self::Explained => "explained",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [Self::Off, Self::Values, Self::Explained]
            .into_iter()
            .find(|m| m.id() == id)
    }
}

pub struct Details<'a> {
    pub scores: Option<Scores>,
    pub personal: Option<f32>,
    /// Whole-frame and eye sharpness percentiles within the folder.
    pub frame_percentile: Option<f32>,
    pub eyes_percentile: Option<f32>,
    pub attributes: Option<[f32; 6]>,
    pub status: &'a Status,
    /// Show the explanations (third stage).
    pub explained: bool,
}

/// Value text, bar fill (0..1) and whether it deserves a warning colour.
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

/// Scrolls when the window is too low for all rows (the mouse wheel over the photo zooms, over
/// the panel it scrolls).
pub fn draw(ui: &mut Ui, rect: Rect, d: &Details<'_>) {
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
            let top = ui.cursor().min;
            let frame = Rect::from_min_size(top, vec2(ui.available_width(), 0.0));
            let bottom = content(ui.painter(), frame, d);
            ui.allocate_space(vec2(frame.width(), bottom - top.y));
        });
}

/// Draws all rows below `rect.top()` and returns where they end.
fn content(painter: &Painter, rect: Rect, d: &Details<'_>) -> f32 {
    let t = i18n::t();
    let mut y = rect.top() + PAD;
    let scores = d.scores.unwrap_or_default();
    let status = d.status;
    let row = |y: &mut f32, label: &str, value: Value, explain: &str| {
        row(
            painter,
            rect,
            y,
            label,
            value,
            d.explained.then_some(explain),
        );
    };

    section(painter, rect, &mut y, t.section_aesthetics);
    row(
        &mut y,
        t.row_laion,
        match scores.aesthetic {
            Some(v) => stars_value(aesthetic::as_stars(v)),
            None => Value::note(model_note(&status.aesthetics)),
        },
        t.explain_laion,
    );
    row(
        &mut y,
        t.row_v25,
        match scores.aesthetic25 {
            Some(v) => stars_value(aesthetic::as_stars(v)),
            None => Value::note(model_note(&status.v25)),
        },
        t.explain_v25,
    );
    let taste = &status.taste;
    row(
        &mut y,
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

    section(painter, rect, &mut y, t.section_sharpness);
    row(
        &mut y,
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
    row(
        &mut y,
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

    section(painter, rect, &mut y, t.section_exposure);
    let clipped = |share: Option<f32>, limit: f32| match share {
        Some(s) => Value {
            text: format!("{:.1} %", s * 100.0),
            fill: Some((s / (limit * 4.0)).min(1.0)),
            warn: s > limit,
        },
        None => Value::note(t.note_analysing),
    };
    row(
        &mut y,
        t.row_highlights,
        clipped(scores.highlights, exposure::HIGHLIGHTS_WARN),
        t.explain_highlights,
    );
    row(
        &mut y,
        t.row_shadows,
        clipped(scores.shadows, exposure::SHADOWS_WARN),
        t.explain_shadows,
    );

    section(painter, rect, &mut y, t.section_attributes);
    if d.explained {
        y = explanation(painter, rect, y, t.explain_attributes) + ROW_GAP;
    }
    for (i, name) in t.attributes.iter().enumerate() {
        row(
            &mut y,
            name,
            match d.attributes {
                Some(a) => Value::score(format!("{:.0} %", a[i] * 100.0), a[i]),
                None => Value::note(t.note_needs_clip),
            },
            t.explain_attribute[i],
        );
    }

    section(painter, rect, &mut y, t.section_models);
    let personal = match taste.model {
        Some((n, error)) if error.is_finite() => (t.taste_trained)(n, error),
        Some((n, _)) => (t.taste_photos)(n),
        None => t.taste_untrained.to_owned(),
    };
    for (name, text) in [
        ("CLIP", model_note(&status.aesthetics)),
        ("V2.5", model_note(&status.v25)),
        (t.model_faces, model_note(&status.faces)),
        (t.model_personal, personal),
    ] {
        painter.text(
            pos2(rect.left() + PAD, y),
            Align2::LEFT_TOP,
            name,
            FontId::proportional(12.0),
            tokens::MUTED,
        );
        painter.text(
            pos2(rect.right() - PAD, y),
            Align2::RIGHT_TOP,
            text,
            FontId::proportional(12.0),
            tokens::TEXT,
        );
        y += 20.0;
    }
    if d.explained {
        y = explanation(painter, rect, y + 4.0, t.explain_models);
    }
    y + PAD
}

/// `3.4 ★` with its bar.
fn stars_value(stars: f32) -> Value {
    Value::score(format!("{stars:.1} ★"), stars / 5.0)
}

fn model_note(state: &ModelState) -> String {
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

fn section(painter: &Painter, rect: Rect, y: &mut f32, title: &str) {
    *y += 6.0;
    painter.text(
        pos2(rect.left() + PAD, *y),
        Align2::LEFT_TOP,
        title.to_uppercase(),
        FontId::proportional(10.5),
        tokens::ACCENT,
    );
    *y += 20.0;
}

/// Wrapped muted text at `y`; returns its bottom.
fn explanation(painter: &Painter, rect: Rect, y: f32, text: &str) -> f32 {
    let galley = painter.layout(
        i18n::keep_together(text),
        FontId::proportional(11.0),
        tokens::MUTED,
        rect.width() - 2.0 * PAD,
    );
    let height = galley.size().y;
    painter.galley(pos2(rect.left() + PAD, y), galley, tokens::MUTED);
    y + height
}

/// Label and value, a bar below (if the value has one), then the explanation (if shown).
fn row(
    painter: &Painter,
    rect: Rect,
    y: &mut f32,
    label: &str,
    value: Value,
    explain: Option<&str>,
) {
    let (left, right) = (rect.left() + PAD, rect.right() - PAD);
    painter.text(
        pos2(left, *y),
        Align2::LEFT_TOP,
        label,
        FontId::proportional(12.5),
        tokens::TEXT,
    );
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
    painter.text(
        pos2(right, *y - 1.0),
        Align2::RIGHT_TOP,
        value.text,
        FontId::proportional(size),
        text_colour,
    );
    let mut bottom = *y + 19.0;
    if let Some(fill) = value.fill {
        let track = Rect::from_min_size(pos2(left, *y + 20.0), vec2(right - left, BAR_HEIGHT));
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
        bottom = track.bottom();
    }
    *y = match explain {
        Some(text) => explanation(painter, rect, bottom + 5.0, text) + ROW_GAP,
        None => bottom + COMPACT_GAP,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::TasteStatus;
    use eframe::egui::{
        Context, Event, FullOutput, Modifiers, MouseWheelUnit, RawInput, Shape, TouchPhase,
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

    /// Where the first section title was painted.
    fn title_y(output: &FullOutput) -> f32 {
        output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                Shape::Text(text) if text.galley.text() == "AESTHETICS" => Some(text.pos.y),
                _ => None,
            })
            .expect("section title painted")
    }

    #[test]
    fn i_steps_through_three_stages() {
        let mut mode = DetailsMode::Off;
        let mut seen = Vec::new();
        for _ in 0..3 {
            mode = mode.next();
            seen.push(mode);
        }
        assert_eq!(
            seen,
            [
                DetailsMode::Values,
                DetailsMode::Explained,
                DetailsMode::Off
            ]
        );
        for mode in seen {
            assert_eq!(DetailsMode::from_id(mode.id()), Some(mode));
        }
    }

    #[test]
    fn a_low_window_scrolls_the_panel() {
        let ctx = Context::default();
        let status = status();
        let details = Details {
            scores: None,
            personal: None,
            frame_percentile: None,
            eyes_percentile: None,
            attributes: None,
            status: &status,
            explained: true,
        };
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH, 300.0));
        let frame = |events: Vec<Event>, time: f64| {
            let input = RawInput {
                screen_rect: Some(screen),
                time: Some(time),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| draw(ui, screen, &details));
            // No renderer here to take the font atlas.
            output.textures_delta.clear();
            output
        };

        let pointer = vec![Event::PointerMoved(pos2(150.0, 150.0))];
        let before = title_y(&frame(pointer, 0.0));
        let wheel = Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, -200.0),
            phase: TouchPhase::Move,
            modifiers: Modifiers::NONE,
        };
        let mut output = frame(vec![wheel], 0.1);
        for i in 2..40 {
            output = frame(Vec::new(), i as f64 * 0.05);
        }
        assert!(title_y(&output) < before - 50.0, "panel did not scroll");
    }
}
