//! Thumbnail strip centred on the current photo.

use std::path::PathBuf;

use eframe::egui::{
    Align2, Color32, CursorIcon, FontId, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::theme::tokens;
use crate::thumbs::Thumbs;
use crate::ui::stars;

pub const HEIGHT: f32 = 96.0;
const CELL: f32 = 104.0;
const GAP: f32 = 6.0;
const THUMB_HEIGHT: f32 = 66.0;

/// What the strip shows about one photo besides its thumbnail.
pub struct CellInfo {
    pub rating: Option<u8>,
    /// Shown as a warning marker with this explanation.
    pub blurry: Option<String>,
}

/// Draws the strip; returns the index the user clicked.
pub fn draw(
    ui: &Ui,
    rect: Rect,
    paths: &[PathBuf],
    current: usize,
    thumbs: &Thumbs,
    info: impl Fn(usize) -> CellInfo,
) -> Option<usize> {
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

        let cell_info = info(index);
        if index == current {
            painter.rect_stroke(
                cell,
                4.0,
                Stroke::new(2.0, tokens::ACCENT),
                StrokeKind::Outside,
            );
        } else if response.hovered() {
            painter.rect_stroke(
                cell,
                4.0,
                Stroke::new(1.0, tokens::MUTED),
                StrokeKind::Outside,
            );
        }
        if let Some(rating) = cell_info.rating {
            stars::paint_mini_rating(
                &painter,
                pos2(cell.center().x, cell.bottom() + 11.0),
                rating,
                4.0,
                tokens::ACCENT,
            );
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
    clicked
}
