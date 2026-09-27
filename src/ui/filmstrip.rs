//! Thumbnail strip centred on the current photo. The mouse wheel over it steps through the
//! photos.

use std::path::PathBuf;

use eframe::egui::{
    Align2, Color32, CursorIcon, Event, FontId, MouseWheelUnit, Rect, Sense, Stroke, StrokeKind,
    Ui, Vec2, pos2, vec2,
};

use crate::i18n;
use crate::metadata::Rating;
use crate::theme::tokens;
use crate::thumbs::Thumbs;
use crate::ui::icons;
use crate::ui::stars;

pub const HEIGHT: f32 = 96.0;
const CELL: f32 = 104.0;
const GAP: f32 = 6.0;
const THUMB_HEIGHT: f32 = 66.0;

/// What the strip shows about one photo besides its thumbnail.
pub struct CellInfo {
    pub rating: Rating,
    /// Shown as a warning marker with this explanation.
    pub blurry: Option<String>,
    /// The photo pinned on the left in compare mode.
    pub pinned: bool,
}

/// Touchpads scroll in points: this many make one photo.
const POINTS_PER_STEP: f32 = 50.0;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StripOutput {
    /// The photo the user clicked.
    pub clicked: Option<usize>,
    /// Photos to move by the mouse wheel (+ = forward).
    pub step: isize,
}

/// Draws the strip and reads clicks and the wheel over it.
pub fn draw(
    ui: &Ui,
    rect: Rect,
    paths: &[PathBuf],
    current: usize,
    thumbs: &Thumbs,
    info: impl Fn(usize) -> CellInfo,
) -> StripOutput {
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, tokens::SURFACE);
    painter.hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(1.0, tokens::LINE),
    );

    let step = CELL + GAP;
    let side = ((rect.width() / 2.0 / step).ceil() as usize) + 1;
    let first = current.saturating_sub(side);
    let last = (current + side).min(paths.len().saturating_sub(1));
    let mut clicked = None;

    for (index, path) in paths.iter().enumerate().take(last + 1).skip(first) {
        let offset = index as f32 - current as f32;
        let cell = Rect::from_center_size(
            pos2(
                rect.center().x + offset * step,
                rect.top() + 6.0 + THUMB_HEIGHT / 2.0,
            ),
            vec2(CELL, THUMB_HEIGHT),
        );
        if !cell.intersects(rect) {
            continue;
        }
        let response = ui
            .interact(cell, ui.id().with(("filmstrip", index)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        if response.clicked() {
            clicked = Some(index);
        }

        let cell_info = info(index);
        painter.rect_filled(cell, 4.0, tokens::SURFACE_MUTED);
        if let Some(texture) = thumbs.get_or_request(path) {
            let size = texture.size_vec2();
            let scale = (cell.width() / size.x).min(cell.height() / size.y);
            let image_rect = Rect::from_center_size(cell.center(), size * scale);
            painter.image(
                texture.id(),
                image_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        // Rejects are dimmed, like in Lightroom – below the selection frame.
        if cell_info.rating == Rating::Rejected {
            painter.rect_filled(cell, 4.0, Color32::from_black_alpha(150));
        }
        if index == current {
            painter.rect_stroke(
                cell,
                4.0,
                Stroke::new(2.0, tokens::ACCENT),
                StrokeKind::Outside,
            );
        } else if cell_info.pinned {
            // Compare mode: the left photo, marked in the neutral text colour with an "L".
            painter.rect_stroke(
                cell,
                4.0,
                Stroke::new(2.0, tokens::TEXT),
                StrokeKind::Outside,
            );
            let badge = Rect::from_min_size(cell.min + vec2(4.0, 4.0), vec2(16.0, 16.0));
            painter.rect_filled(badge, 3.0, tokens::TEXT);
            painter.text(
                badge.center(),
                Align2::CENTER_CENTER,
                i18n::t().compare_left_badge,
                FontId::proportional(11.0),
                tokens::BG,
            );
        } else if response.hovered() {
            painter.rect_stroke(
                cell,
                4.0,
                Stroke::new(1.0, tokens::MUTED),
                StrokeKind::Outside,
            );
        }
        match cell_info.rating {
            Rating::Stars(stars) => stars::paint_mini_rating(
                &painter,
                pos2(cell.center().x, cell.bottom() + 11.0),
                stars,
                4.0,
                tokens::ACCENT,
            ),
            Rating::Rejected => {
                icons::reject_mark(
                    &painter,
                    pos2(cell.center().x, cell.bottom() + 11.0),
                    8.0,
                    tokens::STATUS_ERROR,
                );
            }
            Rating::Unrated => {}
        }
        if let Some(reason) = cell_info.blurry {
            let marker = pos2(cell.right() - 8.0, cell.top() + 8.0);
            painter.circle_filled(marker, 5.0, tokens::STATUS_WARN);
            painter.text(
                marker,
                Align2::CENTER_CENTER,
                "!",
                FontId::proportional(9.0),
                Color32::BLACK,
            );
            response.on_hover_text(reason);
        }
    }
    StripOutput {
        clicked,
        step: wheel_steps(ui, rect),
    }
}

/// Whole photos to move for the wheel events of this frame while the pointer is over the
/// strip. Fractions (touchpads, high-resolution wheels) carry over to the next frame.
fn wheel_steps(ui: &Ui, rect: Rect) -> isize {
    if !ui.rect_contains_pointer(rect) {
        return 0;
    }
    let notches: f32 = ui.input(|i| {
        i.raw
            .events
            .iter()
            .map(|event| match event {
                Event::MouseWheel { unit, delta, .. } => notches(*unit, *delta),
                _ => 0.0,
            })
            .sum()
    });
    if notches == 0.0 {
        return 0;
    }
    let id = ui.id().with("filmstrip-wheel");
    ui.data_mut(|d| accumulate(d.get_temp_mut_or_default::<f32>(id), notches))
}

/// Wheel down or swipe left = forward. egui's delta moves the *content*, hence the minus.
fn notches(unit: MouseWheelUnit, delta: Vec2) -> f32 {
    let amount = -(delta.x + delta.y);
    match unit {
        MouseWheelUnit::Line | MouseWheelUnit::Page => amount,
        MouseWheelUnit::Point => amount / POINTS_PER_STEP,
    }
}

fn accumulate(carry: &mut f32, notches: f32) -> isize {
    *carry += notches;
    let whole = carry.trunc();
    *carry -= whole;
    whole as isize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_photo_per_notch() {
        assert_eq!(notches(MouseWheelUnit::Line, vec2(0.0, -1.0)), 1.0);
        assert_eq!(notches(MouseWheelUnit::Line, vec2(0.0, 2.0)), -2.0);
        assert_eq!(notches(MouseWheelUnit::Point, vec2(-25.0, 0.0)), 0.5);
        let mut carry = 0.0;
        assert_eq!(accumulate(&mut carry, 1.0), 1);
        assert_eq!(accumulate(&mut carry, 0.6), 0);
        assert_eq!(accumulate(&mut carry, 0.6), 1);
        assert_eq!(accumulate(&mut carry, -3.0), -2);
    }
}
