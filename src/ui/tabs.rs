//! A strip of tabs – the details panel (values | description) and the help page (shortcuts |
//! tips): the active one in the text colour with an accent line under it, the others muted.

use eframe::egui::{Align2, CursorIcon, FontId, Id, Rect, Sense, Stroke, Ui, pos2, vec2};

use crate::theme::{text, tokens};

/// How the tabs share the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Widths {
    /// Each takes the same part of the whole strip.
    Equal,
    /// Each is as wide as its label plus padding, from the left.
    Natural,
}

/// Draws the tabs into `rect`; returns the one clicked when it is not the current one.
pub fn strip(
    ui: &mut Ui,
    rect: Rect,
    labels: &[&str],
    current: usize,
    id: Id,
    widths: Widths,
) -> Option<usize> {
    const PAD: f32 = 18.0;
    let painter = ui.painter().with_clip_rect(rect);
    let font = FontId::proportional(text::BODY);
    let mut x = rect.left();
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let width = match widths {
            Widths::Equal => rect.width() / labels.len().max(1) as f32,
            Widths::Natural => {
                painter
                    .layout_no_wrap((*label).to_owned(), font.clone(), tokens::TEXT)
                    .size()
                    .x
                    + 2.0 * PAD
            }
        };
        let cell = Rect::from_min_size(pos2(x, rect.top()), vec2(width, rect.height()));
        x += width;
        let response = ui
            .interact(cell, id.with(i), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let active = i == current;
        let colour = if active || response.hovered() {
            tokens::TEXT
        } else {
            tokens::MUTED
        };
        painter.text(
            cell.center(),
            Align2::CENTER_CENTER,
            label,
            font.clone(),
            colour,
        );
        if active {
            let inset = if widths == Widths::Equal {
                14.0
            } else {
                PAD - 4.0
            };
            painter.hline(
                cell.x_range().shrink(inset),
                rect.bottom() - 1.5,
                Stroke::new(2.0, tokens::ACCENT),
            );
        }
        if response.clicked() && !active {
            clicked = Some(i);
        }
    }
    clicked
}
