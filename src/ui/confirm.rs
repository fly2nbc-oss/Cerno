//! Confirmation card for the downloads, for the two actions that cannot be undone (resetting
//! For you, deleting the models) and for the first start's question about the update check.
//! Enter confirms, Esc cancels. It replaces the system message boxes, which appeared light in
//! the dark window and blocked it.

use eframe::egui::{Align, Context, FontId, Id, Key, Label, Layout, Modifiers, Rect, RichText};

use crate::i18n;
use crate::theme::{self, text, tokens};
use crate::ui::modal;

pub struct Confirm<'a> {
    pub title: &'a str,
    pub text: String,
    pub confirm: &'a str,
    /// The other button: *Cancel*, or *No* where the card asks a question.
    pub cancel: &'a str,
    /// The confirm button in the error colour (the action deletes something).
    pub danger: bool,
}

/// `Some(true)` confirmed, `Some(false)` cancelled, `None` while it waits.
pub fn show(ctx: &Context, window: Rect, confirm: &Confirm<'_>) -> Option<bool> {
    let (enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    let modal = modal::show(ctx, window, Id::new("confirm"), 460.0, |ui| {
        ui.label(
            RichText::new(confirm.title)
                .font(theme::strong(text::VALUE))
                .color(tokens::TEXT),
        );
        ui.add_space(12.0);
        ui.add(
            Label::new(
                RichText::new(i18n::keep_together(&confirm.text))
                    .font(FontId::proportional(text::BODY))
                    .color(tokens::TEXT),
            )
            .wrap(),
        );
        ui.add_space(24.0);
        let mut answer = None;
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let yes = if confirm.danger {
                    theme::danger_button(confirm.confirm)
                } else {
                    theme::primary_button(confirm.confirm)
                };
                if ui.add(yes).clicked() {
                    answer = Some(true);
                }
                if ui
                    .add(theme::secondary_button(confirm.cancel).min_width(100.0))
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
    use eframe::egui::{Event, RawInput, pos2, vec2};

    fn answer(key: Key) -> Option<bool> {
        let ctx = crate::theme::test_context();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let confirm = Confirm {
            title: "Delete?",
            text: "Gone for good.".into(),
            confirm: "Delete",
            cancel: "Cancel",
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
