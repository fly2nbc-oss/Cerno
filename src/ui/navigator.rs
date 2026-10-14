//! The navigator while zoomed (ticket CER-55, U2): the whole photo small in the bottom right
//! corner of the photo area, the part the view shows framed in the accent, the rest dimmed. A
//! click or a drag on it moves the view there. Only in the single view – compare mode and the
//! four-up view share one zoom over several photos.

use eframe::egui::{
    Color32, CursorIcon, Id, Rect, Sense, Stroke, StrokeKind, TextureId, Ui, Vec2, pos2, vec2,
};

use crate::theme::{self, radius, tokens};

/// The longer side of the small photo.
const LONG_SIDE: f32 = 160.0;
/// The card's distance from the photo area's corner.
const INSET: f32 = 16.0;
/// Room between the card's edge and the photo in it.
const PAD: f32 = 4.0;

/// Where the small photo lies in `area`: bottom right, its longer side `LONG_SIDE`.
pub fn place(area: Rect, aspect: f32) -> Rect {
    let size = if aspect >= 1.0 {
        vec2(LONG_SIDE, LONG_SIDE / aspect)
    } else {
        vec2(LONG_SIDE * aspect, LONG_SIDE)
    };
    Rect::from_min_size(area.max - size - Vec2::splat(INSET + PAD), size)
}

/// Draws the navigator for `texture` (the photo, upright) with `visible` – the part the view
/// shows, in 0..1 – framed. Returns the image point (0..1) to centre the view on while it is
/// clicked or dragged.
pub fn navigator(
    ui: &Ui,
    area: Rect,
    texture: TextureId,
    aspect: f32,
    visible: Rect,
) -> Option<Vec2> {
    let photo = place(area, aspect);
    let card = photo.expand(PAD);
    // Never focusable: `Space` must not click it again.
    let response = ui.interact(card, Id::new("navigator"), Sense::CLICK | Sense::DRAG);
    let painter = ui.painter().with_clip_rect(area);
    theme::paint_card(&painter, card, radius::LG);
    painter.image(
        texture,
        photo,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    let shown = Rect::from_min_max(
        photo.min + visible.min.to_vec2() * photo.size(),
        photo.min + visible.max.to_vec2() * photo.size(),
    )
    .intersect(photo);
    // The hidden rest dimmed in four strips around the part shown.
    for strip in [
        Rect::from_min_max(photo.min, pos2(photo.right(), shown.top())),
        Rect::from_min_max(pos2(photo.left(), shown.bottom()), photo.max),
        Rect::from_min_max(
            pos2(photo.left(), shown.top()),
            pos2(shown.left(), shown.bottom()),
        ),
        Rect::from_min_max(
            pos2(shown.right(), shown.top()),
            pos2(photo.right(), shown.bottom()),
        ),
    ] {
        if strip.is_positive() {
            painter.rect_filled(strip, 0.0, tokens::SCRIM_LIGHT);
        }
    }
    painter.rect_stroke(
        shown,
        0.0,
        Stroke::new(2.0, tokens::ACCENT),
        StrokeKind::Outside,
    );
    let held = response.is_pointer_button_down_on() || response.dragged();
    let response = response.on_hover_cursor(if held {
        CursorIcon::Grabbing
    } else {
        CursorIcon::PointingHand
    });
    if !held {
        return None;
    }
    let pos = response.interact_pointer_pos()?;
    Some(((pos - photo.min) / photo.size()).clamp(Vec2::ZERO, Vec2::splat(1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, Modifiers, PointerButton, RawInput, TextureOptions};

    /// A press on the navigator asks for that point at once, a drag follows the pointer, and
    /// it takes no keyboard focus; a press beside it asks for nothing.
    #[test]
    fn a_press_and_a_drag_move_the_view() {
        let ctx = crate::theme::test_context();
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 700.0));
        let texture = ctx.load_texture(
            "navigator-test",
            eframe::egui::ColorImage::filled([4, 2], Color32::GRAY),
            TextureOptions::LINEAR,
        );
        let photo = place(area, 2.0);
        assert_eq!(photo.size(), vec2(160.0, 80.0));
        assert!((area.max - photo.max).x >= INSET && (area.max - photo.max).y >= INSET);
        let visible = Rect::from_min_max(pos2(0.4, 0.4), pos2(0.6, 0.6));
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let quarter = photo.min + photo.size() * vec2(0.25, 0.5);
        let end = photo.min + photo.size() * vec2(0.75, 0.25);
        let beside = photo.min - vec2(40.0, 40.0);
        let mut asked = Vec::new();
        for events in [
            vec![Event::PointerMoved(quarter)],
            vec![button(quarter, true)],
            vec![Event::PointerMoved(end)],
            vec![button(end, false)],
            vec![Event::PointerMoved(beside)],
            vec![button(beside, true)],
            vec![button(beside, false)],
        ] {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(area),
                    events,
                    ..Default::default()
                },
                |ui| asked.push(navigator(ui, area, texture.id(), 2.0, visible)),
            );
            output.textures_delta.clear();
        }
        let near = |got: Option<Vec2>, want: Vec2| got.is_some_and(|v| (v - want).length() < 1e-3);
        assert_eq!(asked[0], None);
        assert!(near(asked[1], vec2(0.25, 0.5)), "{asked:?}");
        assert!(near(asked[2], vec2(0.75, 0.25)), "{asked:?}");
        assert!(asked[4..].iter().all(Option::is_none), "{asked:?}");
        assert!(ctx.memory(|m| m.focused().is_none()));
    }
}
