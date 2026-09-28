//! Confirmation card for the model download and for the two actions that cannot be undone
//! (resetting For you, deleting the models). Enter confirms, Esc cancels. It replaces the
//! system message boxes, which appeared light in the dark window and blocked it.

use eframe::egui::{
    Align, Button, Color32, Context, FontId, Id, Key, Label, Layout, Modifiers, Rect, RichText,
    vec2,
};

use crate::i18n;
use crate::theme::{text, tokens};
use crate::ui::modal;

pub struct Confirm<'a> {
    pub title: &'a str,
    pub text: String,
    pub confirm: &'a str,
    /// The confirm button in the error colour (the action deletes something).
    pub danger: bool,
}

/// `Some(true)` confirmed, `Some(false)` cancelled, `None` while it waits.
pub fn show(ctx: &Context, window: Rect, confirm: &Confirm<'_>) -> Option<bool> {
    let t = i18n::t();
    let (enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    let modal = modal::show(ctx, window, Id::new("confirm"), 460.0, |ui| {
        ui.label(
            RichText::new(confirm.title)
                .font(FontId::proportional(text::VALUE))
                .color(tokens::TEXT),
        );
        ui.add_space(10.0);
        ui.add(
            Label::new(
                RichText::new(i18n::keep_together(&confirm.text))
                    .font(FontId::proportional(text::BODY))
                    .color(tokens::TEXT),
            )
            .wrap(),
        );
        ui.add_space(20.0);
        let mut answer = None;
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let fill = if confirm.danger {
                    tokens::STATUS_ERROR
                } else {
                    tokens::ACCENT
                };
                let yes = Button::new(RichText::new(confirm.confirm).color(Color32::WHITE))
                    .fill(fill)
                    .min_size(vec2(120.0, 32.0));
                if ui.add(yes).clicked() {
                    answer = Some(true);
                }
                if ui
                    .add(Button::new(t.btn_cancel).min_size(vec2(100.0, 32.0)))
                    .clicked()
                {
                    answer = Some(false);
                }
            });
        });
        answer
    });
    if enter {
        return Some(true);
    }
    if escape || modal.outside {
        return Some(false);
    }
    modal.inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, RawInput, pos2};

    fn answer(key: Key) -> Option<bool> {
        let ctx = Context::default();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let confirm = Confirm {
            title: "Delete?",
            text: "Gone for good.".into(),
            confirm: "Delete",
            danger: true,
        };
        let mut result = None;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(window),
                events: vec![Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| result = show(ui.ctx(), window, &confirm),
        );
        output.textures_delta.clear();
        result
    }

    #[test]
    fn enter_confirms_and_escape_cancels() {
        assert_eq!(answer(Key::Enter), Some(true));
        assert_eq!(answer(Key::Escape), Some(false));
        assert_eq!(answer(Key::A), None);
    }
}
