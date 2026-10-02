//! Thumbnail strip centred on the current photo. The mouse wheel over it steps through the
//! photos.

use std::ops::Range;
use std::path::PathBuf;

use eframe::egui::{Event, MouseWheelUnit, Rect, Stroke, Ui, Vec2, pos2, vec2};

use crate::theme::tokens;
use crate::thumbs::Thumbs;
use crate::ui::cells::{self, CellInfo, Empty};

pub const HEIGHT: f32 = 96.0;
const CELL: f32 = 104.0;
const GAP: f32 = 6.0;
/// Extra space where one series ends and the next begins (capture-time order).
const SERIES_GAP: f32 = 16.0;
const THUMB_HEIGHT: f32 = 66.0;

/// Touchpads scroll in points: this many make one photo.
const POINTS_PER_STEP: f32 = 50.0;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StripOutput {
    /// The photo the user clicked.
    pub clicked: Option<usize>,
    /// Photos to move by the mouse wheel (+ = forward).
    pub step: isize,
}

/// Draws the strip and reads clicks and the wheel over it. `grouped` separates neighbouring
/// series; only capture-time order keeps a series together, so pass it then.
pub fn draw(
    ui: &Ui,
    rect: Rect,
    paths: &[PathBuf],
    current: usize,
    thumbs: &Thumbs,
    grouped: bool,
    info: impl Fn(usize) -> CellInfo,
) -> StripOutput {
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, tokens::SURFACE);
    painter.hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(1.0, tokens::LINE),
    );

    if paths.is_empty() {
        return StripOutput::default();
    }
    let current = current.min(paths.len() - 1);
    let step = CELL + GAP;
    let side = ((rect.width() / 2.0 / step).ceil() as usize) + 1;
    let mut clicked = None;
    let visible: Vec<(usize, CellInfo)> = visible_range(current, paths.len(), side)
        .map(|i| (i, info(i)))
        .collect();
    let mut shift = vec![0.0; visible.len()];
    if grouped {
        for i in 1..visible.len() {
            let boundary =
                cells::series_boundary(visible[i - 1].1.series_id, visible[i].1.series_id);
            shift[i] = shift[i - 1] + if boundary { SERIES_GAP } else { 0.0 };
        }
    }
    let current_shift = visible
        .iter()
        .position(|(index, _)| *index == current)
        .map(|i| shift[i])
        .unwrap_or(0.0);

    // Nearest first: videos get their frames in the order the cells ask for them.
    let mut order: Vec<usize> = (0..visible.len()).collect();
    order.sort_by_key(|&n| visible[n].0.abs_diff(current));
    for n in order {
        let (index, cell_info) = &visible[n];
        let path = &paths[*index];
        let offset = *index as f32 - current as f32;
        let cell = Rect::from_center_size(
            pos2(
                rect.center().x + offset * step + shift[n] - current_shift,
                rect.top() + 6.0 + THUMB_HEIGHT / 2.0,
            ),
            vec2(CELL, THUMB_HEIGHT),
        );
        if !cell.intersects(rect) {
            continue;
        }
        let response = cells::paint(
            ui,
            &painter,
            cell,
            ui.id().with(("filmstrip", *index)),
            thumbs.get_or_request(path).as_ref(),
            Empty::default(),
            cell_info,
        );
        if response.clicked() {
            clicked = Some(*index);
        }
        if cell_info.in_current_series {
            cells::series_line(&painter, cell, rect.bottom() - 3.0);
        }
    }
    StripOutput {
        clicked,
        step: wheel_steps(ui, rect),
    }
}

/// View indices within `side` of `current`. Empty for an empty view, and `current` past the
/// end counts as the last photo – the strip must never ask for a cell that isn't there.
fn visible_range(current: usize, len: usize, side: usize) -> Range<usize> {
    let Some(last) = len.checked_sub(1) else {
        return 0..0;
    };
    let current = current.min(last);
    current.saturating_sub(side)..current.saturating_add(side).min(last) + 1
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

    #[test]
    fn the_strip_only_asks_for_cells_that_exist() {
        assert_eq!(visible_range(0, 0, 5), 0..0);
        assert_eq!(visible_range(3, 0, 5), 0..0);
        assert_eq!(visible_range(0, 1, 5), 0..1);
        assert_eq!(visible_range(2, 10, 5), 0..8);
        assert_eq!(visible_range(9, 10, 5), 4..10);
        assert_eq!(visible_range(20, 10, 5), 4..10);
        assert_eq!(visible_range(usize::MAX, 10, 5), 4..10);
    }
}
