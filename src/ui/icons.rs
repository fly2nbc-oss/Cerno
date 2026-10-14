//! The interface's icons: Lucide glyphs of the embedded subset (`theme::icon`), drawn as text
//! so they take any colour and size. Painted stay only what must be filled to read on a
//! photo or is the mark itself: the language flags, the play button over a video (`play`),
//! the stars (`ui/stars.rs`) and the colour labels.

use eframe::egui::{
    Align2, Color32, CursorIcon, FontId, Id, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind,
    Ui, Vec2, vec2,
};

use crate::i18n::Lang;
use crate::theme::{icon, radius, tokens};

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

/// Which part of the window a panel button toggles: the menu bar (left), the filter bar
/// (top), the details panel (right), the filmstrip (bottom).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Left,
    Top,
    Right,
    Bottom,
}

/// Text sizes of the icons: inline beside text, and in the bars' buttons (Lucide draws about
/// 20 px there, the design system's size).
pub const INLINE: f32 = 14.0;
pub const BAR: f32 = 17.0;

/// A Lucide icon centred on `center`, at the text size `size`.
pub fn glyph(painter: &Painter, center: Pos2, glyph: char, size: f32, color: Color32) {
    painter.text(
        center,
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(size),
        color,
    );
}

/// The window with the panel's side, for the info bar's panel buttons; whether the panel
/// shows is the button's fill (`button_background`).
pub fn panel(painter: &Painter, center: Pos2, panel: Panel, color: Color32) {
    let glyph_of = match panel {
        Panel::Left => icon::PANEL_LEFT,
        Panel::Top => icon::PANEL_TOP,
        Panel::Right => icon::PANEL_RIGHT,
        Panel::Bottom => icon::PANEL_BOTTOM,
    };
    glyph(painter, center, glyph_of, BAR, color);
}

/// The view switcher's single photo.
pub fn photo(painter: &Painter, center: Pos2, color: Color32) {
    glyph(painter, center, icon::IMAGE, BAR, color);
}

/// The view switcher's grid.
pub fn grid(painter: &Painter, center: Pos2, color: Color32) {
    glyph(painter, center, icon::LAYOUT_GRID, BAR, color);
}

/// The view switcher's faces: a face in a viewfinder.
pub fn face_frame(painter: &Painter, center: Pos2, color: Color32) {
    glyph(painter, center, icon::SCAN_FACE, BAR, color);
}

/// The "rejected" cross, about `size` wide.
pub fn reject_mark(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    glyph(painter, center, icon::X, size * 1.8, color);
}

/// A person – head and shoulders – for the people filter; `crossed`: "without people".
pub fn person(painter: &Painter, center: Pos2, color: Color32, crossed: bool) {
    let glyph_of = if crossed { icon::USER_X } else { icon::USER };
    glyph(painter, center, glyph_of, INLINE + 1.0, color);
}

/// A waste bin for the deleted photos (which lie in `.originals`, not in the system's
/// trash); `size` 1.0 beside text.
pub fn trash(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    glyph(painter, center, icon::TRASH, (INLINE + 1.0) * size, color);
}

/// "Probably blurry" on a thumbnail or a face: the warning triangle on a small raised badge,
/// like the other marks on a cell, so it reads on any photo.
pub fn warning(painter: &Painter, center: Pos2) {
    let badge = Rect::from_center_size(center, vec2(18.0, 18.0));
    painter.rect_filled(badge, radius::SM, tokens::SURFACE_RAISED);
    glyph(
        painter,
        center,
        icon::TRIANGLE_ALERT,
        INLINE - 1.0,
        tokens::STATUS_WARN,
    );
}

/// "File incomplete": a JPEG that ends inside its image data.
pub fn torn_file(painter: &Painter, center: Pos2, color: Color32) {
    glyph(painter, center, icon::FILE_WARNING, INLINE, color);
}

/// A video's play button: a light triangle on a dark disc, both see-through so the frame
/// shows beneath – painted, a filled triangle reads on any frame.
pub fn play(painter: &Painter, center: Pos2, radius: f32) {
    painter.circle_filled(center, radius, tokens::PHOTO_VEIL_LIGHT);
    let r = radius * 0.5;
    // Pushed right by the triangle's centroid, so it looks centred in the disc.
    let c = center + vec2(r * 0.2, 0.0);
    let points = vec![
        c + vec2(-r * 0.8, -r),
        c + vec2(r, 0.0),
        c + vec2(-r * 0.8, r),
    ];
    painter.add(Shape::convex_polygon(
        points,
        Color32::from_white_alpha(190),
        Stroke::NONE,
    ));
}

/// Two overlapping sheets, the "copy" mark.
pub fn copy(painter: &Painter, center: Pos2, color: Color32) {
    glyph(painter, center, icon::COPY, INLINE - 1.0, color);
}

/// Circled question mark.
pub fn help(painter: &Painter, center: Pos2, color: Color32) {
    glyph(painter, center, icon::CIRCLE_HELP, BAR, color);
}

/// A chevron pointing left or right, about `size` high: the photo's browse arrows.
pub fn side_chevron(painter: &Painter, center: Pos2, right: bool, size: f32, color: Color32) {
    let glyph_of = if right {
        icon::CHEVRON_RIGHT
    } else {
        icon::CHEVRON_LEFT
    };
    glyph(painter, center, glyph_of, size * 1.7, color);
}

/// A quarter turn: the menu bar's turn buttons.
pub fn turn(painter: &Painter, center: Pos2, clockwise: bool, color: Color32) {
    let glyph_of = if clockwise {
        icon::ROTATE_CW
    } else {
        icon::ROTATE_CCW
    };
    glyph(painter, center, glyph_of, INLINE, color);
}

/// The chevron of a row that folds (`open`: pointing down) or opens a list beside it.
pub fn chevron(painter: &Painter, center: Pos2, open: bool, color: Color32) {
    let glyph_of = if open {
        icon::CHEVRON_DOWN
    } else {
        icon::CHEVRON_RIGHT
    };
    glyph(painter, center, glyph_of, INLINE - 1.0, color);
}

/// An eye: show these values on the photo (the check overlay). Whether it is on, its colour
/// says.
pub fn eye(painter: &Painter, center: Pos2, color: Color32) {
    glyph(painter, center, icon::EYE, INLINE, color);
}

/// A tick: a chosen row, a ticked box.
pub fn check(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    glyph(painter, center, icon::CHECK, size, color);
}

/// A cross: close a card, clear the filters.
pub fn close(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    glyph(painter, center, icon::X, size, color);
}

/// A button with the copy mark that puts `text` on the clipboard. For a moment after a click
/// it stays lit with a tick in the sheets, and its tooltip says `done` instead of `tip`; `id`
/// keeps that moment.
pub fn copy_button(ui: &mut Ui, size: Vec2, id: Id, text: &str, tip: &str, done: &str) {
    let now = ui.input(|i| i.time);
    let copied = ui
        .data(|data| data.get_temp::<f64>(id))
        .is_some_and(|until| until > now);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let color = if copied || response.hovered() {
        tokens::ACCENT_STRONG
    } else {
        tokens::MUTED
    };
    button_background(ui.painter(), rect, response.hovered(), copied);
    let glyph_of = if copied { icon::COPY_CHECK } else { icon::COPY };
    glyph(ui.painter(), rect.center(), glyph_of, INLINE - 1.0, color);
    let response = response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(if copied { done } else { tip });
    if response.clicked() {
        ui.ctx().copy_text(text.to_owned());
        ui.data_mut(|data| data.insert_temp(id, now + 1.6));
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(1700));
    }
}

/// Hover/active background shared by all icon buttons.
pub fn button_background(painter: &Painter, rect: Rect, hovered: bool, active: bool) {
    // Blue means "on"; under the pointer the fill is neutral.
    let fill = match (hovered, active) {
        (_, true) => Some(tokens::ACCENT_SUBTLE),
        (true, false) => Some(tokens::HOVER),
        (false, false) => None,
    };
    if let Some(fill) = fill {
        painter.rect_filled(rect, crate::theme::radius::MD, fill);
    }
}
