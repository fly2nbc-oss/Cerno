//! Small painted symbols: language flags, panel toggles, help. Painted instead of
//! taken from a font – egui has no flag emoji and no Lucide icons.

use eframe::egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2,
};

use crate::i18n::Lang;
use crate::theme::{text, tokens};

/// The flag of `lang` filling `rect` (3:2 looks right). English uses the Union Jack.
pub fn flag(painter: &Painter, rect: Rect, lang: Lang) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let rgb = Color32::from_rgb;
    let bands = |colors: &[(Color32, f32)], vertical: bool| {
        let total: f32 = colors.iter().map(|(_, w)| w).sum();
        let mut start = 0.0;
        for (color, weight) in colors {
            let (a, b) = (start / total, (start + weight) / total);
            let band = if vertical {
                Rect::from_x_y_ranges(
                    rect.lerp_inside(vec2(a, 0.0)).x..=rect.lerp_inside(vec2(b, 0.0)).x,
                    rect.y_range(),
                )
            } else {
                Rect::from_x_y_ranges(
                    rect.x_range(),
                    rect.lerp_inside(vec2(0.0, a)).y..=rect.lerp_inside(vec2(0.0, b)).y,
                )
            };
            painter.rect_filled(band, 0.0, *color);
            start += weight;
        }
    };
    match lang {
        Lang::De => bands(
            &[
                (rgb(0x00, 0x00, 0x00), 1.0),
                (rgb(0xDD, 0x00, 0x00), 1.0),
                (rgb(0xFF, 0xCE, 0x00), 1.0),
            ],
            false,
        ),
        Lang::Fr => bands(
            &[
                (rgb(0x00, 0x55, 0xA4), 1.0),
                (Color32::WHITE, 1.0),
                (rgb(0xEF, 0x41, 0x35), 1.0),
            ],
            true,
        ),
        Lang::It => bands(
            &[
                (rgb(0x00, 0x92, 0x46), 1.0),
                (Color32::WHITE, 1.0),
                (rgb(0xCE, 0x2B, 0x37), 1.0),
            ],
            true,
        ),
        Lang::Es => bands(
            &[
                (rgb(0xAA, 0x15, 0x1B), 1.0),
                (rgb(0xF1, 0xBF, 0x00), 2.0),
                (rgb(0xAA, 0x15, 0x1B), 1.0),
            ],
            false,
        ),
        Lang::En => union_jack(&painter, rect),
    }
    painter.rect_stroke(
        rect,
        1.0,
        Stroke::new(1.0, Color32::from_black_alpha(90)),
        StrokeKind::Inside,
    );
}

fn union_jack(painter: &Painter, rect: Rect) {
    let blue = Color32::from_rgb(0x01, 0x21, 0x69);
    let red = Color32::from_rgb(0xC8, 0x10, 0x2E);
    let h = rect.height();
    painter.rect_filled(rect, 0.0, blue);
    for (a, b) in [
        (rect.left_top(), rect.right_bottom()),
        (rect.left_bottom(), rect.right_top()),
    ] {
        painter.line_segment([a, b], Stroke::new(h * 0.2, Color32::WHITE));
        painter.line_segment([a, b], Stroke::new(h * 0.07, red));
    }
    let c = rect.center();
    for (width, color) in [(h / 3.0, Color32::WHITE), (h / 5.0, red)] {
        painter.rect_filled(
            Rect::from_center_size(c, vec2(rect.width(), width)),
            0.0,
            color,
        );
        painter.rect_filled(Rect::from_center_size(c, vec2(width, h)), 0.0, color);
    }
}

/// Which part of the window a panel button toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Top,
    Right,
    Bottom,
}

/// Window outline with the panel's strip. Kept for the view commands; the info bar no longer
/// paints these buttons.
#[allow(dead_code)]
pub fn panel(painter: &Painter, center: Pos2, panel: Panel, shown: bool, color: Color32) {
    let frame = Rect::from_center_size(center, vec2(16.0, 13.0));
    let strip = match panel {
        Panel::Top => Rect::from_min_max(frame.min, pos2(frame.max.x, frame.min.y + 4.5)),
        Panel::Right => Rect::from_min_max(pos2(frame.max.x - 5.5, frame.min.y), frame.max),
        Panel::Bottom => Rect::from_min_max(pos2(frame.min.x, frame.max.y - 4.5), frame.max),
    };
    if shown {
        painter.rect_filled(strip, 1.5, color);
    } else {
        let line = match panel {
            Panel::Top => [
                pos2(frame.min.x, strip.max.y),
                pos2(frame.max.x, strip.max.y),
            ],
            Panel::Right => [
                pos2(strip.min.x, frame.min.y),
                pos2(strip.min.x, frame.max.y),
            ],
            Panel::Bottom => [
                pos2(frame.min.x, strip.min.y),
                pos2(frame.max.x, strip.min.y),
            ],
        };
        painter.line_segment(line, Stroke::new(1.3, color));
    }
    painter.rect_stroke(frame, 2.0, Stroke::new(1.3, color), StrokeKind::Inside);
}

/// The "rejected" cross.
pub fn reject_mark(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let d = size / 2.0;
    let stroke = Stroke::new((size / 6.0).max(1.4), color);
    painter.line_segment([center + vec2(-d, -d), center + vec2(d, d)], stroke);
    painter.line_segment([center + vec2(-d, d), center + vec2(d, -d)], stroke);
}

/// "Probably blurry": a status-coloured disc with a painted exclamation mark (a 9 px text "!"
/// was below the design system's 12 px minimum for icons).
pub fn warning(painter: &Painter, center: Pos2) {
    painter.circle_filled(center, 6.5, tokens::STATUS_WARN);
    let stroke = Stroke::new(1.8, Color32::BLACK);
    painter.line_segment([center + vec2(0.0, -3.6), center + vec2(0.0, 1.0)], stroke);
    painter.circle_filled(center + vec2(0.0, 3.4), 1.0, Color32::BLACK);
}

/// Two overlapping sheets, the "copy" mark.
pub fn copy(painter: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.2, color);
    let back = Rect::from_center_size(center + vec2(1.6, -1.6), vec2(8.0, 9.0));
    let front = Rect::from_center_size(center + vec2(-1.6, 1.6), vec2(8.0, 9.0));
    painter.rect_stroke(back, 1.0, stroke, StrokeKind::Inside);
    painter.rect_filled(front, 1.0, tokens::SURFACE);
    painter.rect_stroke(front, 1.0, stroke, StrokeKind::Inside);
}

/// Three bars, the menu button at the bottom right.
pub fn menu(painter: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    for dy in [-5.0, 0.0, 5.0] {
        painter.line_segment([center + vec2(-6.0, dy), center + vec2(6.0, dy)], stroke);
    }
}

/// Circled question mark.
pub fn help(painter: &Painter, center: Pos2, color: Color32) {
    painter.circle_stroke(center, 7.5, Stroke::new(1.3, color));
    painter.text(
        center + vec2(0.0, 0.5),
        Align2::CENTER_CENTER,
        "?",
        FontId::proportional(text::LABEL),
        color,
    );
}

/// Small triangle for fold rows in the details panel (`open` = expanded).
pub fn chevron(painter: &Painter, center: Pos2, open: bool, color: Color32) {
    let d = 4.0;
    let points = if open {
        [
            center + vec2(-d, d * 0.35),
            center + vec2(d, d * 0.35),
            center + vec2(0.0, -d * 0.9),
        ]
    } else {
        [
            center + vec2(-d * 0.55, -d),
            center + vec2(-d * 0.55, d),
            center + vec2(d * 0.85, 0.0),
        ]
    };
    painter.add(Shape::convex_polygon(points.to_vec(), color, Stroke::NONE));
}

/// An eye: show these values on the photo (the check overlay). The pupil is filled while on.
pub fn eye(painter: &Painter, center: Pos2, on: bool, color: Color32) {
    let (half_width, half_height) = (7.5, 4.5);
    let stroke = Stroke::new(1.4, color);
    for side in [-1.0, 1.0] {
        let lid = (0..=12)
            .map(|i| {
                let t = i as f32 / 6.0 - 1.0;
                center + vec2(t * half_width, side * half_height * (1.0 - t * t))
            })
            .collect();
        painter.add(Shape::line(lid, stroke));
    }
    if on {
        painter.circle_filled(center, 2.6, color);
    } else {
        painter.circle_stroke(center, 2.2, stroke);
    }
}

/// Hover/active background shared by all icon buttons.
pub fn button_background(painter: &Painter, rect: Rect, hovered: bool, active: bool) {
    let fill = match (hovered, active) {
        (true, _) => Some(tokens::ACCENT_SUBTLE),
        (false, true) => Some(tokens::SURFACE_MUTED),
        (false, false) => None,
    };
    if let Some(fill) = fill {
        painter.rect_filled(rect, 4.0, fill);
    }
}
