//! Drawn over the photo area: compare labels, the deletion countdown, the copy progress,
//! notices, the drop hint, placeholder text and the language flag.

use eframe::egui::{
    Align2, Color32, CursorIcon, FontId, Id, Painter, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};

use crate::analysis::aesthetic;
use crate::i18n::{self, Lang};
use crate::metadata::Rating;
use crate::theme::{text, tokens};
use crate::ui::{icons, stars};

/// Over a video's frame, near the bottom: a button with a painted play sign and "Play
/// (Enter)"; above it, when there is no frame, why (`note`).
pub fn video_badge(ui: &Ui, area: Rect, note: Option<&str>, slot: usize) -> Response {
    let painter = ui.painter().with_clip_rect(area);
    let galley = painter.layout_no_wrap(
        i18n::t().video_play_hint.to_owned(),
        FontId::proportional(text::BODY),
        tokens::TEXT,
    );
    let size = vec2(galley.size().x + 46.0, 34.0);
    let pill = Rect::from_center_size(bottom_slot(area, 0), size);
    // Clickable: the play button plays the video.
    let response = ui
        .interact(pill, Id::new(("video-play", slot)), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let shade = if response.hovered() { 220 } else { 170 };
    painter.rect_filled(pill, 17.0, Color32::from_black_alpha(shade));
    let c = pos2(pill.left() + 20.0, pill.center().y);
    painter.add(eframe::egui::Shape::convex_polygon(
        vec![
            c + vec2(-4.0, -7.0),
            c + vec2(8.0, 0.0),
            c + vec2(-4.0, 7.0),
        ],
        tokens::TEXT,
        Stroke::NONE,
    ));
    let text_pos = pos2(pill.left() + 34.0, pill.center().y - galley.size().y / 2.0);
    painter.galley(text_pos, galley, tokens::TEXT);
    if let Some(note) = note {
        painter.text(
            pos2(area.center().x, pill.top() - 10.0),
            Align2::CENTER_BOTTOM,
            note,
            FontId::proportional(text::SMALL),
            tokens::MUTED,
        );
    }
    response
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

/// Compare mode: aesthetics and sharpness under the photo, just below the side label –
/// `Aesthetics 62 %   Eyes 80 %`, like the info bar.
pub fn compare_scores(
    ui: &Ui,
    area: Rect,
    aesthetics: Option<f32>,
    sharpness: Option<(f32, bool)>,
) {
    let t = i18n::t();
    let mut parts = Vec::new();
    if let Some(score) = aesthetics {
        parts.push(format!(
            "{} {:.0} %",
            t.section_aesthetics,
            aesthetic::as_percent(score) * 100.0
        ));
    }
    if let Some((p, eyes)) = sharpness {
        let name = if eyes {
            t.meter_eyes
        } else {
            t.meter_sharpness
        };
        parts.push(format!("{name} {:.0} %", p * 100.0));
    }
    if parts.is_empty() {
        return;
    }
    let painter = ui.painter().with_clip_rect(area);
    let galley = painter.layout_no_wrap(
        parts.join("   "),
        FontId::proportional(text::BODY),
        tokens::TEXT,
    );
    let pill = Rect::from_min_size(
        area.min + vec2(10.0, 44.0),
        vec2(galley.size().x + 24.0, 26.0),
    );
    painter.rect_filled(pill, 6.0, tokens::SURFACE.gamma_multiply(0.92));
    painter.rect_stroke(
        pill,
        6.0,
        Stroke::new(1.0, tokens::LINE),
        StrokeKind::Inside,
    );
    let y = pill.center().y;
    painter.galley(
        pos2(pill.left() + 12.0, y - galley.size().y / 2.0),
        galley,
        tokens::TEXT,
    );
}

/// Where the `slot`-th pill from the bottom of the photo area is centred. A video's play
/// button takes the lowest place; the deletion countdown and the copy progress stack above
/// whatever is shown below them.
pub fn bottom_slot(area: Rect, slot: usize) -> Pos2 {
    pos2(area.center().x, area.bottom() - 44.0 - 70.0 * slot as f32)
}

/// Countdown for pending deletions. The bar runs out, Esc brings everything back.
pub fn delete_countdown(ui: &Ui, centre: Pos2, count: usize, left: f32) {
    let text = (i18n::t().deleting)(count);
    bar_pill(ui, centre, text, left, tokens::STATUS_WARN);
}

/// How far a copy or move is: what it does as text, the share done as a bar in the accent
/// (the accent marks progress).
pub fn transfer_progress(ui: &Ui, centre: Pos2, text: String, done: f32) {
    bar_pill(ui, centre, text, done, tokens::ACCENT);
}

fn bar_pill(ui: &Ui, centre: Pos2, text: String, share: f32, colour: Color32) {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(text, FontId::proportional(text::BODY), tokens::TEXT);
    let width = (galley.size().x + 32.0).max(300.0);
    let pill = Rect::from_center_size(centre, vec2(width, 48.0));
    painter.rect_filled(pill, 8.0, tokens::SURFACE);
    painter.rect_stroke(pill, 8.0, Stroke::new(1.0, colour), StrokeKind::Inside);
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
        Rect::from_min_size(
            track.min,
            vec2(track.width() * share.clamp(0.0, 1.0), track.height()),
        ),
        3.0,
        colour,
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

    /// The copy progress stacks above the slot below it, says what it does and fills its bar
    /// in the accent as far as it is.
    #[test]
    fn the_copy_progress_sits_above_and_fills_its_share() {
        let ctx = Context::default();
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(area),
                ..Default::default()
            },
            |ui| {
                transfer_progress(ui, bottom_slot(area, 1), "Copying 3 / 4".to_owned(), 0.5);
            },
        );
        output.textures_delta.clear();
        let shapes: Vec<Shape> = output.shapes.into_iter().map(|c| c.shape).collect();
        let text = shapes
            .iter()
            .find_map(|shape| match shape {
                Shape::Text(text) if text.galley.text() == "Copying 3 / 4" => {
                    Some(text.visual_bounding_rect())
                }
                _ => None,
            })
            .expect("the text");
        assert!(
            text.bottom() < bottom_slot(area, 0).y - 17.0,
            "above the play button"
        );
        let bars: Vec<Rect> = shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Rect(rect) if rect.fill == tokens::ACCENT => Some(rect.rect),
                _ => None,
            })
            .collect();
        let track = shapes
            .iter()
            .find_map(|shape| match shape {
                Shape::Rect(rect) if rect.fill == tokens::LINE => Some(rect.rect),
                _ => None,
            })
            .expect("the track");
        assert_eq!(bars.len(), 1);
        assert!((bars[0].width() - track.width() / 2.0).abs() < 0.5);
    }
}
