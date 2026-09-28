//! Straighten grid and the crop frame. State lives in `app.rs`.

use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke, pos2, vec2};

use crate::edit::{Corner, Crop};
use crate::theme::{text, tokens};

const HANDLE: f32 = 10.0;
/// Divisions of the shorter side. Fine enough to judge a horizon, not a rule of thirds.
const GRID: f32 = 24.0;

/// Where the pointer is relative to the crop frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Corner(Corner),
    Inside,
    Outside,
}

pub fn hit_test(frame: Rect, pointer: Pos2) -> Hit {
    let corners = [
        (Corner::Nw, frame.left_top()),
        (Corner::Ne, frame.right_top()),
        (Corner::Sw, frame.left_bottom()),
        (Corner::Se, frame.right_bottom()),
    ];
    for (corner, pos) in corners {
        if pointer.distance(pos) <= HANDLE {
            return Hit::Corner(corner);
        }
    }
    if frame.contains(pointer) {
        Hit::Inside
    } else {
        Hit::Outside
    }
}

/// Axis-aligned grid over the visible frame, plus stronger centre lines.
pub fn grid(painter: &Painter, rect: Rect) {
    let short = rect.width().min(rect.height());
    if short < 8.0 {
        return;
    }
    let step = short / GRID;
    let thin = Stroke::new(1.0, Color32::from_white_alpha(70));
    let strong = Stroke::new(1.4, Color32::from_white_alpha(150));
    let mut x = rect.left();
    while x <= rect.right() + 0.5 {
        let centre = (x - rect.center().x).abs() < step * 0.5;
        painter.vline(x, rect.y_range(), if centre { strong } else { thin });
        x += step;
    }
    let mut y = rect.top();
    while y <= rect.bottom() + 0.5 {
        let centre = (y - rect.center().y).abs() < step * 0.5;
        painter.hline(rect.x_range(), y, if centre { strong } else { thin });
        y += step;
    }
}

/// Dims everything outside `crop` and draws the frame with corner handles.
pub fn crop_frame(painter: &Painter, image: Rect, crop: Rect) {
    let dim = Color32::from_black_alpha(140);
    if crop.top() > image.top() {
        painter.rect_filled(
            Rect::from_min_max(image.min, pos2(image.right(), crop.top())),
            0.0,
            dim,
        );
    }
    if crop.bottom() < image.bottom() {
        painter.rect_filled(
            Rect::from_min_max(pos2(image.left(), crop.bottom()), image.max),
            0.0,
            dim,
        );
    }
    if crop.left() > image.left() {
        painter.rect_filled(
            Rect::from_min_max(
                pos2(image.left(), crop.top()),
                pos2(crop.left(), crop.bottom()),
            ),
            0.0,
            dim,
        );
    }
    if crop.right() < image.right() {
        painter.rect_filled(
            Rect::from_min_max(
                pos2(crop.right(), crop.top()),
                pos2(image.right(), crop.bottom()),
            ),
            0.0,
            dim,
        );
    }
    painter.rect_stroke(
        crop,
        0.0,
        Stroke::new(1.5, Color32::WHITE),
        eframe::egui::StrokeKind::Outside,
    );
    for pos in [
        crop.left_top(),
        crop.right_top(),
        crop.left_bottom(),
        crop.right_bottom(),
    ] {
        painter.rect_filled(
            Rect::from_center_size(pos, vec2(8.0, 8.0)),
            1.0,
            Color32::WHITE,
        );
    }
}

/// Maps a pixel crop onto the photo's screen rectangle.
pub fn crop_to_screen(image: Rect, image_size: [u32; 2], crop: Crop) -> Rect {
    let scale_x = image.width() / image_size[0] as f32;
    let scale_y = image.height() / image_size[1] as f32;
    Rect::from_min_size(
        image.min + vec2(crop.x as f32 * scale_x, crop.y as f32 * scale_y),
        vec2(crop.w as f32 * scale_x, crop.h as f32 * scale_y),
    )
}

/// Maps a screen point into image pixels.
pub fn screen_to_image(image: Rect, image_size: [u32; 2], pos: Pos2) -> (f64, f64) {
    let x = f64::from((pos.x - image.min.x) / image.width()) * f64::from(image_size[0]);
    let y = f64::from((pos.y - image.min.y) / image.height()) * f64::from(image_size[1]);
    (x, y)
}

/// Screen pixels to image pixels, for a drag delta.
pub fn screen_delta(image: Rect, image_size: [u32; 2], delta: eframe::egui::Vec2) -> (f64, f64) {
    (
        f64::from(delta.x / image.width()) * f64::from(image_size[0]),
        f64::from(delta.y / image.height()) * f64::from(image_size[1]),
    )
}

/// A small label over the photo: the angle or the ratio, and how to confirm.
pub fn banner(painter: &Painter, area: Rect, primary: &str, hint: &str) {
    use eframe::egui::FontId;

    let primary_galley = painter.layout_no_wrap(
        primary.to_owned(),
        FontId::proportional(text::LARGE),
        tokens::TEXT,
    );
    let hint_galley = painter.layout_no_wrap(
        hint.to_owned(),
        FontId::proportional(text::SMALL),
        tokens::MUTED,
    );
    let width = primary_galley.size().x.max(hint_galley.size().x) + 28.0;
    let bar = Rect::from_center_size(pos2(area.center().x, area.top() + 28.0), vec2(width, 40.0));
    painter.rect_filled(bar, 8.0, Color32::from_black_alpha(180));
    painter.galley(
        pos2(
            bar.center().x - primary_galley.size().x / 2.0,
            bar.top() + 4.0,
        ),
        primary_galley,
        tokens::TEXT,
    );
    painter.galley(
        pos2(
            bar.center().x - hint_galley.size().x / 2.0,
            bar.top() + 22.0,
        ),
        hint_galley,
        tokens::MUTED,
    );
}

/// Cursor for a crop hit.
pub fn cursor(hit: Hit) -> eframe::egui::CursorIcon {
    use eframe::egui::CursorIcon;
    match hit {
        Hit::Corner(Corner::Nw | Corner::Se) => CursorIcon::ResizeNwSe,
        Hit::Corner(Corner::Ne | Corner::Sw) => CursorIcon::ResizeNeSw,
        Hit::Inside => CursorIcon::Grab,
        Hit::Outside => CursorIcon::Crosshair,
    }
}
