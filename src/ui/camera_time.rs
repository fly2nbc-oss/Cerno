//! The camera time card (*Visible photos ▸ Camera time …*): every camera of the folder with
//! its photo count and an offset field (`±h:mm:ss`, `camera_time::parse_offset`), Reset per
//! camera, Apply (`Enter`) and Cancel (`Esc`).

use eframe::egui::{
    Align, Button, Context, FontId, Grid, Id, Key, Layout, Modifiers, Rect, RichText, TextEdit,
    vec2,
};

use crate::camera_time;
use crate::i18n;
use crate::theme::{self, text, tokens};
use crate::ui::modal;

/// One camera of the folder.
pub struct Row {
    pub name: String,
    pub count: usize,
    /// The offset as typed; empty is none.
    pub text: String,
}

/// Room for `−26:00:00`.
const FIELD_WIDTH: f32 = 104.0;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    pub apply: bool,
    pub close: bool,
}

pub fn show(ctx: &Context, window: Rect, rows: &mut [Row]) -> Output {
    let t = i18n::t();
    // Taken before the fields see them: Enter would only leave the field.
    let (apply_key, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::Enter)
                || i.consume_key(Modifiers::COMMAND, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    let valid = rows
        .iter()
        .all(|row| camera_time::parse_offset(&row.text).is_some());
    let modal = modal::show(ctx, window, Id::new("camera-time"), 520.0, |ui| {
        let mut out = Output::default();
        let top_right = ui.max_rect().right_top() + vec2(8.0, -8.0);
        ui.label(
            RichText::new(t.camera_time_title)
                .font(FontId::proportional(text::VALUE))
                .color(tokens::TEXT),
        );
        out.close = modal::close_button(ui, top_right, t.btn_close);
        ui.add_space(8.0);
        ui.label(RichText::new(i18n::keep_together(t.camera_time_intro)).color(tokens::MUTED));
        ui.add_space(12.0);
        if rows.is_empty() {
            ui.label(RichText::new(t.camera_time_none).color(tokens::MUTED));
        }
        Grid::new("camera-time-rows")
            .num_columns(4)
            .spacing(vec2(14.0, 8.0))
            .show(ui, |ui| {
                for row in rows.iter_mut() {
                    ui.label(RichText::new(&row.name).color(tokens::TEXT));
                    ui.label(
                        RichText::new((t.camera_time_photos)(row.count))
                            .font(FontId::proportional(text::SMALL))
                            .color(tokens::MUTED),
                    );
                    let wrong = camera_time::parse_offset(&row.text).is_none();
                    let mut field = TextEdit::singleline(&mut row.text).hint_text("0:00:00");
                    if wrong {
                        field = field.text_color(tokens::STATUS_ERROR);
                    }
                    // A grid cell would squeeze the field to its first frame's width.
                    ui.add_sized(vec2(FIELD_WIDTH, 22.0), field);
                    if ui
                        .add_enabled(
                            !row.text.trim().is_empty(),
                            Button::new(t.camera_time_reset),
                        )
                        .clicked()
                    {
                        row.text.clear();
                    }
                    ui.end_row();
                }
            });
        if !valid {
            ui.add_space(6.0);
            ui.label(
                RichText::new(t.camera_time_invalid)
                    .font(FontId::proportional(text::SMALL))
                    .color(tokens::STATUS_ERROR),
            );
        }
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let apply = ui
                .add_enabled(
                    valid && !rows.is_empty(),
                    theme::primary_button(t.camera_time_apply),
                )
                .on_hover_text("Enter");
            if apply.clicked() {
                out.apply = true;
            }
            if ui.add(Button::new(t.btn_cancel)).clicked() {
                out.close = true;
            }
        });
        out
    });
    let mut out = modal.inner;
    out.apply |= apply_key && valid && !rows.is_empty();
    out.close |= escape || modal.outside;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, RawInput, pos2};

    fn press(key: Key, text: &str) -> Output {
        let ctx = Context::default();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let mut rows = vec![Row {
            name: "Pixel 7a".into(),
            count: 3,
            text: text.into(),
        }];
        let mut out = Output::default();
        let pressed = vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }];
        // The first frame measures the card.
        for events in [Vec::new(), pressed] {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(window),
                    events,
                    ..Default::default()
                },
                |ui| out = show(ui.ctx(), window, &mut rows),
            );
            output.textures_delta.clear();
        }
        out
    }

    /// `Enter` applies valid offsets only, `Esc` cancels.
    #[test]
    fn enter_applies_valid_offsets_and_escape_cancels() {
        assert!(press(Key::Enter, "+1:30:00").apply);
        assert!(!press(Key::Enter, "1:75").apply, "not an offset");
        assert!(press(Key::Escape, "").close);
    }
}
