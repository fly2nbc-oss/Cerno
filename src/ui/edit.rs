//! Straighten grid and the crop frame. State lives in `app.rs`.

use eframe::egui::{Color32, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

use crate::edit::{Corner, Crop};
use crate::theme::{text, tokens};

/// How far from a corner a drag still takes it, in points. A small frame gets less, so its
/// inside stays reachable for moving it.
const HANDLE: f32 = 16.0;
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
    let reach = HANDLE.min(frame.width().min(frame.height()) / 3.0);
    let corners = [
        (Corner::Nw, frame.left_top()),
        (Corner::Ne, frame.right_top()),
        (Corner::Sw, frame.left_bottom()),
        (Corner::Se, frame.right_bottom()),
    ];
    let nearest = corners
        .into_iter()
        .map(|(corner, pos)| (corner, pointer.distance(pos)))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    match nearest {
        Some((corner, distance)) if distance <= reach => Hit::Corner(corner),
        _ if frame.contains(pointer) => Hit::Inside,
        _ => Hit::Outside,
    }
}

/// Takes the pointer over the photo area during a crop. Drag only: the drag then starts on the
/// press itself, while the pointer is still on the corner. With clicks too, egui waits for 6 pt
/// of movement first, and a quick pull from a corner was taken for a move – which the first
/// frame, the whole photo, can't do.
pub fn pointer_area(ui: &Ui, area: Rect) -> Response {
    ui.interact(area, ui.id().with("crop"), Sense::drag())
}

/// Where a drag that starts this frame began: the press, not where the pointer is now.
pub fn drag_start(ui: &Ui, response: &Response) -> Option<Pos2> {
    if !response.drag_started() {
        return None;
    }
    ui.ctx()
        .input(|i| i.pointer.press_origin())
        .or_else(|| response.interact_pointer_pos())
}

/// What a drag on the crop does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    /// The corner follows the pointer, the opposite one stays put.
    Resize(Corner),
    /// The frame slides.
    Move,
    /// A new frame grows from where the drag began.
    Draw,
}

/// Decided where the button went down. Inside a frame that can't slide – the first frame is
/// the whole photo – a drag draws a new frame, as it does outside.
pub fn gesture(hit: Hit, movable: bool) -> Gesture {
    match hit {
        Hit::Corner(corner) => Gesture::Resize(corner),
        Hit::Inside if movable => Gesture::Move,
        Hit::Inside | Hit::Outside => Gesture::Draw,
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

/// Cursor for what a drag would do.
pub fn cursor(gesture: Gesture) -> eframe::egui::CursorIcon {
    use eframe::egui::CursorIcon;
    match gesture {
        Gesture::Resize(Corner::Nw | Corner::Se) => CursorIcon::ResizeNwSe,
        Gesture::Resize(Corner::Ne | Corner::Sw) => CursorIcon::ResizeNeSw,
        Gesture::Move => CursorIcon::Grab,
        Gesture::Draw => CursorIcon::Crosshair,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, Event, Modifiers, PointerButton, RawInput};

    fn frame() -> Rect {
        Rect::from_min_size(pos2(100.0, 100.0), vec2(600.0, 400.0))
    }

    #[test]
    fn corners_win_near_them_the_inside_elsewhere() {
        let f = frame();
        assert_eq!(hit_test(f, f.left_top()), Hit::Corner(Corner::Nw));
        assert_eq!(
            hit_test(f, f.right_bottom() - vec2(12.0, 9.0)),
            Hit::Corner(Corner::Se)
        );
        assert_eq!(hit_test(f, f.right_top() + vec2(-20.0, 20.0)), Hit::Inside);
        assert_eq!(hit_test(f, f.center()), Hit::Inside);
        assert_eq!(
            hit_test(f, f.left_bottom() + vec2(-30.0, 0.0)),
            Hit::Outside
        );
        // A small frame keeps its inside: corners reach a third of the short side.
        let small = Rect::from_min_size(pos2(0.0, 0.0), vec2(30.0, 30.0));
        assert_eq!(hit_test(small, pos2(15.0, 15.0)), Hit::Inside);
        assert_eq!(hit_test(small, pos2(26.0, 27.0)), Hit::Corner(Corner::Se));
    }

    #[test]
    fn a_frame_that_cannot_slide_is_drawn_anew() {
        let corner = Hit::Corner(Corner::Ne);
        assert_eq!(gesture(corner, false), Gesture::Resize(Corner::Ne));
        assert_eq!(gesture(corner, true), Gesture::Resize(Corner::Ne));
        assert_eq!(gesture(Hit::Inside, true), Gesture::Move);
        assert_eq!(gesture(Hit::Inside, false), Gesture::Draw);
        assert_eq!(gesture(Hit::Outside, true), Gesture::Draw);
        assert_eq!(cursor(Gesture::Draw), eframe::egui::CursorIcon::Crosshair);
    }

    /// The bug: pressed on a corner and pulled 20 pt inward at once, the drag was decided at
    /// the pointer's new place – inside, a move – and the whole-photo frame didn't change.
    #[test]
    fn a_quick_pull_from_a_corner_takes_the_corner() {
        let ctx = Context::default();
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let crop = area;
        let corner = crop.right_bottom() - vec2(2.0, 2.0);
        let pulled = corner - vec2(20.0, 20.0);
        let mut time = 0.0;
        let mut run = |events: Vec<Event>| {
            time += 0.016;
            let mut start = None;
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(area),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let response = pointer_area(ui, area);
                    start = drag_start(ui, &response);
                },
            );
            output.textures_delta.clear();
            start
        };
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(run(vec![Event::PointerMoved(corner)]), None);
        let start = run(vec![button(corner, true), Event::PointerMoved(pulled)])
            .expect("the drag starts on the press");
        assert_eq!(start, corner);
        assert_eq!(
            gesture(hit_test(crop, start), false),
            Gesture::Resize(Corner::Se)
        );
        assert_eq!(gesture(hit_test(crop, pulled), false), Gesture::Draw);
        assert_eq!(
            run(vec![Event::PointerMoved(pulled - vec2(5.0, 5.0))]),
            None
        );
        run(vec![button(pulled, false)]);
    }
}
