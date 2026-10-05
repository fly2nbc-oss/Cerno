//! The faces of the current photo: the details panel's faces tab (`G`, the faces one under the
//! other) and the grid of all of them over the photo (`Shift+G`, for group photos). A click on
//! a face zooms to it. What is drawn comes from `app/faces.rs`.

use eframe::egui::{
    Align2, Area, Color32, Context, CursorIcon, FontId, Id, Key, Order, Rect, ScrollArea, Sense,
    Stroke, StrokeKind, TextureHandle, Ui, UiBuilder, pos2, vec2,
};

use crate::i18n;
use crate::theme::{text, tokens};
use crate::ui::icons;

/// One face, cut out of the photo.
pub struct Crop {
    pub texture: TextureHandle,
    /// Its eyes measured below the ceiling of "probably blurry".
    pub eyes_blurry: bool,
}

/// What the faces of the photo are, as far as known.
pub enum Shown<'a> {
    /// Being looked for or cut out.
    Loading,
    /// The face detection has not seen the photo (not analysed yet, or a video).
    Unknown,
    /// It found none.
    None,
    /// The faces large enough to judge, left to right, and how many smaller ones there are.
    Ready { crops: &'a [Crop], small: usize },
}

const PAD: f32 = 14.0;
const THUMB: f32 = 112.0;

/// The faces tab: each face with its number (left to right in the photo) and, when its eyes are
/// probably blurry, a note. Returns the face clicked.
pub fn tab(ui: &mut Ui, rect: Rect, shown: &Shown<'_>) -> Option<usize> {
    let t = i18n::t();
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, tokens::SURFACE);
    painter.vline(
        rect.left() + 0.5,
        rect.y_range(),
        Stroke::new(1.0, tokens::LINE),
    );
    let note = |text: &str| {
        painter.text(
            rect.left_top() + vec2(PAD, PAD),
            Align2::LEFT_TOP,
            text,
            FontId::proportional(text::BODY),
            tokens::MUTED,
        );
    };
    let (crops, small) = match shown {
        Shown::Loading => {
            note(t.faces_loading);
            return None;
        }
        Shown::Unknown => {
            note(t.faces_unknown);
            return None;
        }
        Shown::None => {
            note(t.faces_none);
            return None;
        }
        Shown::Ready { crops, small } => (*crops, *small),
    };
    let mut clicked = None;
    let mut panel = ui.new_child(UiBuilder::new().max_rect(rect).id_salt("faces-tab"));
    ScrollArea::vertical()
        .auto_shrink(false)
        .show(&mut panel, |ui| {
            ui.set_width(rect.width());
            ui.add_space(PAD);
            if crops.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(PAD);
                    ui.label(eframe::egui::RichText::new(t.faces_only_small).color(tokens::MUTED));
                });
            }
            for (i, crop) in crops.iter().enumerate() {
                let (row, response) =
                    ui.allocate_exact_size(vec2(rect.width(), THUMB + 10.0), Sense::click());
                let response = response
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text(t.faces_zoom_hint);
                if response.hovered() {
                    ui.painter().rect_filled(
                        row.shrink2(vec2(6.0, 1.0)),
                        6.0,
                        tokens::SURFACE_MUTED,
                    );
                }
                let thumb =
                    Rect::from_min_size(row.left_top() + vec2(PAD, 5.0), vec2(THUMB, THUMB));
                paint_crop(ui, thumb, &crop.texture);
                let x = thumb.right() + 12.0;
                ui.painter().text(
                    pos2(x, thumb.top() + 4.0),
                    Align2::LEFT_TOP,
                    (t.face_number)(i + 1),
                    FontId::proportional(text::BODY),
                    tokens::TEXT,
                );
                if crop.eyes_blurry {
                    let at = pos2(x + 7.0, thumb.top() + 38.0);
                    icons::warning(ui.painter(), at);
                    ui.painter().text(
                        pos2(x + 20.0, at.y),
                        Align2::LEFT_CENTER,
                        t.face_eyes_blurry,
                        FontId::proportional(text::SMALL),
                        tokens::TEXT,
                    );
                }
                if response.clicked() {
                    clicked = Some(i);
                }
            }
            if small > 0 && !crops.is_empty() {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(PAD);
                    ui.label(
                        eframe::egui::RichText::new((t.faces_small)(small))
                            .font(FontId::proportional(text::SMALL))
                            .color(tokens::MUTED),
                    );
                });
            }
            ui.add_space(PAD);
        });
    clicked
}

/// A crop fitted into `area`, keeping its shape, on a muted ground.
fn paint_crop(ui: &Ui, area: Rect, texture: &TextureHandle) {
    let painter = ui.painter();
    painter.rect_filled(area, 4.0, tokens::SURFACE_MUTED);
    let size = texture.size_vec2();
    let scale = (area.width() / size.x).min(area.height() / size.y);
    let shown = Rect::from_center_size(area.center(), size * scale);
    painter.image(
        texture.id(),
        shown,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

#[derive(Default)]
pub struct GridOutput {
    pub clicked: Option<usize>,
    pub close: bool,
}

/// All faces large over the photo area, numbered like the tab; a click or its number (1–9)
/// zooms to one, Esc, `Shift+G` or a click beside them closes.
pub fn grid(ctx: &Context, area: Rect, shown: &Shown<'_>) -> GridOutput {
    let t = i18n::t();
    let mut out = GridOutput::default();
    // Esc (`Shift+G` closes it in `handle_keys`; the other keys pause while it is open).
    if ctx.input_mut(|i| i.consume_key(eframe::egui::Modifiers::NONE, Key::Escape)) {
        out.close = true;
    }
    const DIGITS: [Key; 9] = [
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
        Key::Num7,
        Key::Num8,
        Key::Num9,
    ];
    let count = match shown {
        Shown::Ready { crops, .. } => crops.len(),
        _ => 0,
    };
    out.clicked = ctx.input_mut(|i| {
        DIGITS
            .iter()
            .position(|key| i.consume_key(eframe::egui::Modifiers::NONE, *key))
            .filter(|n| *n < count)
    });
    Area::new(Id::new("faces-grid"))
        .order(Order::Foreground)
        .fixed_pos(area.min)
        .show(ctx, |ui| {
            let backdrop = ui.allocate_rect(area, Sense::click());
            ui.painter()
                .rect_filled(area, 0.0, Color32::from_black_alpha(215));
            let message = |text: &str| {
                ui.painter().text(
                    area.center(),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(text::LARGE),
                    tokens::MUTED,
                );
            };
            let crops = match shown {
                Shown::Loading => {
                    message(t.faces_loading);
                    &[][..]
                }
                Shown::Unknown => {
                    message(t.faces_unknown);
                    &[][..]
                }
                Shown::None => {
                    message(t.faces_none);
                    &[][..]
                }
                Shown::Ready { crops: [], .. } => {
                    message(t.faces_only_small);
                    &[][..]
                }
                Shown::Ready { crops, .. } => *crops,
            };
            let mut on_face = false;
            if !crops.is_empty() {
                let n = crops.len();
                let inner = area.shrink(24.0);
                // The column count that makes the squares largest.
                let (columns, side) = (1..=n)
                    .map(|columns| {
                        let rows = n.div_ceil(columns);
                        let side =
                            (inner.width() / columns as f32).min(inner.height() / rows as f32);
                        (columns, side)
                    })
                    .fold(
                        (1, 0.0),
                        |best, next| if next.1 > best.1 { next } else { best },
                    );
                let rows = n.div_ceil(columns);
                let used = vec2(columns as f32 * side, rows as f32 * side);
                let origin = inner.center() - used / 2.0;
                for (i, crop) in crops.iter().enumerate() {
                    let cell = Rect::from_min_size(
                        origin + vec2((i % columns) as f32 * side, (i / columns) as f32 * side),
                        vec2(side, side),
                    )
                    .shrink(6.0);
                    let response = ui
                        .interact(cell, Id::new(("faces-grid-cell", i)), Sense::click())
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(t.faces_zoom_hint);
                    on_face |= response.hovered();
                    paint_crop(ui, cell, &crop.texture);
                    let stroke = if response.hovered() {
                        Stroke::new(2.0, tokens::ACCENT)
                    } else {
                        Stroke::new(1.0, tokens::LINE)
                    };
                    ui.painter()
                        .rect_stroke(cell, 4.0, stroke, StrokeKind::Outside);
                    let badge = Rect::from_min_size(cell.min + vec2(6.0, 6.0), vec2(22.0, 20.0));
                    ui.painter().rect_filled(badge, 4.0, tokens::SURFACE);
                    ui.painter().text(
                        badge.center(),
                        Align2::CENTER_CENTER,
                        (i + 1).to_string(),
                        FontId::proportional(text::SMALL),
                        tokens::TEXT,
                    );
                    if crop.eyes_blurry {
                        icons::warning(ui.painter(), pos2(cell.right() - 14.0, cell.top() + 14.0));
                    }
                    if response.clicked() {
                        out.clicked = Some(i);
                    }
                }
            }
            if !crops.is_empty() {
                ui.painter().text(
                    pos2(area.center().x, area.bottom() - 12.0),
                    Align2::CENTER_BOTTOM,
                    t.faces_grid_hint,
                    FontId::proportional(text::SMALL),
                    tokens::MUTED,
                );
            }
            if backdrop.clicked() && !on_face && out.clicked.is_none() {
                out.close = true;
            }
        });
    out
}
