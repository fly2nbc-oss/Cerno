use std::f32::consts::{FRAC_PI_2, PI};

use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, Vec2};

pub fn paint_star(painter: &Painter, center: Pos2, radius: f32, filled: bool, color: Color32) {
    paint(painter, center, radius, filled.then_some(color), color);
}

/// A star filled in one colour and outlined in another: For you's light hint in the info bar.
pub fn paint_hint_star(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    fill: Color32,
    outline: Color32,
) {
    paint(painter, center, radius, Some(fill), outline);
}

fn paint(painter: &Painter, center: Pos2, radius: f32, fill: Option<Color32>, outline: Color32) {
    let inner = radius * 0.45;
    let points: Vec<Pos2> = (0..10)
        .map(|i| {
            let r = if i % 2 == 0 { radius } else { inner };
            center + r * Vec2::angled(-FRAC_PI_2 + i as f32 * PI / 5.0)
        })
        .collect();
    if let Some(fill) = fill {
        // epaint only fills convex shapes: inner pentagon plus five tip triangles.
        let pentagon: Vec<Pos2> = points.iter().skip(1).step_by(2).copied().collect();
        painter.add(Shape::convex_polygon(pentagon, fill, Stroke::NONE));
        for tip in (0..10).step_by(2) {
            let triangle = vec![points[(tip + 9) % 10], points[tip], points[tip + 1]];
            painter.add(Shape::convex_polygon(triangle, fill, Stroke::NONE));
        }
    }
    painter.add(Shape::closed_line(
        points,
        Stroke::new((radius * 0.15).clamp(0.8, 1.4), outline),
    ));
}

/// `count` small filled stars centred at `center` (filmstrip).
pub fn paint_mini_rating(painter: &Painter, center: Pos2, count: u8, radius: f32, color: Color32) {
    let gap = radius * 2.4;
    let left = center.x - gap * f32::from(count.saturating_sub(1)) / 2.0;
    for i in 0..count {
        paint_star(
            painter,
            Pos2::new(left + gap * f32::from(i), center.y),
            radius,
            true,
            color,
        );
    }
}
