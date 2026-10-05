//! The card for the file-name list filter (*Filter ▸ By file list …*): a field to paste the
//! names a client chose into, what the list finds while it is typed, Apply (`Ctrl+Enter`) and
//! Cancel (`Esc`). The matching itself is `name_list`.

use eframe::egui::{
    Align, Button, Context, FontId, Id, Key, Layout, Modifiers, Rect, RichText, TextEdit, vec2,
};

use crate::i18n;
use crate::theme::{self, text, tokens};
use crate::ui::modal;

/// What the list in the field finds right now.
pub struct Preview<'a> {
    pub found: usize,
    pub total: usize,
    pub missing: &'a [String],
    pub ambiguous: &'a [String],
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    pub apply: bool,
    pub close: bool,
}

/// How many missing or ambiguous names are spelled out before "…".
const LISTED: usize = 12;

pub fn show(ctx: &Context, window: Rect, text: &mut String, preview: &Preview<'_>) -> Output {
    let t = i18n::t();
    // Taken before the field sees them: Enter alone is a line break there.
    let (apply_key, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::COMMAND, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    let modal = modal::show(ctx, window, Id::new("name-list"), 560.0, |ui| {
        let mut out = Output::default();
        let top_right = ui.max_rect().right_top() + vec2(8.0, -8.0);
        ui.label(
            RichText::new(t.name_list_title)
                .font(FontId::proportional(text::VALUE))
                .color(tokens::TEXT),
        );
        out.close = modal::close_button(ui, top_right, t.btn_close);
        ui.add_space(8.0);
        ui.label(RichText::new(i18n::keep_together(t.name_list_intro)).color(tokens::MUTED));
        ui.add_space(10.0);
        let field = ui.add(
            TextEdit::multiline(text)
                .desired_rows(8)
                .desired_width(f32::INFINITY)
                .hint_text(t.name_list_hint),
        );
        // The cursor goes into the field when the card opens.
        let focused_id = Id::new("name-list-focused");
        if !ui.data(|d| d.get_temp::<bool>(focused_id).unwrap_or(false)) {
            field.request_focus();
            ui.data_mut(|d| d.insert_temp(focused_id, true));
        }
        ui.add_space(8.0);
        if preview.total > 0 {
            ui.label(
                RichText::new((t.name_list_found)(preview.found, preview.total))
                    .color(tokens::TEXT),
            );
        }
        for (label, names) in [
            (t.name_list_missing, preview.missing),
            (t.name_list_ambiguous, preview.ambiguous),
        ] {
            if names.is_empty() {
                continue;
            }
            let mut shown = names[..names.len().min(LISTED)].join(", ");
            if names.len() > LISTED {
                shown.push_str(", …");
            }
            ui.label(
                RichText::new(format!("{label} {shown}"))
                    .font(FontId::proportional(text::SMALL))
                    .color(tokens::MUTED),
            );
        }
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let apply = ui
                .add_enabled(preview.found > 0, theme::primary_button(t.name_list_apply))
                .on_hover_text(i18n::with_ctrl("Enter"));
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
    out.apply |= apply_key && preview.found > 0;
    out.close |= escape || modal.outside;
    if out.apply || out.close {
        ctx.data_mut(|d| d.remove::<bool>(Id::new("name-list-focused")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, RawInput, pos2};

    fn press(key: Key, modifiers: Modifiers, found: usize) -> (Output, String) {
        let ctx = Context::default();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let mut text = "IMG_1".to_owned();
        let preview = Preview {
            found,
            total: 1,
            missing: &[],
            ambiguous: &[],
        };
        let mut out = Output::default();
        // The first frame measures the card (and puts the cursor into the field).
        let pressed = vec![
            Event::ModifiersChanged(modifiers),
            key_event(key, modifiers),
        ];
        for events in [Vec::new(), pressed] {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(window),
                    events,
                    ..Default::default()
                },
                |ui| out = show(ui.ctx(), window, &mut text, &preview),
            );
            output.textures_delta.clear();
        }
        (out, text)
    }

    fn key_event(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// `Ctrl+Enter` applies a list that finds something, `Enter` alone is a line in the field,
    /// `Esc` closes.
    #[test]
    fn ctrl_enter_applies_and_escape_closes() {
        let ctrl = Modifiers::COMMAND | Modifiers::CTRL;
        let (out, text) = press(Key::Enter, ctrl, 1);
        assert!(out.apply);
        assert_eq!(text, "IMG_1", "the key never reached the field");
        assert!(!press(Key::Enter, ctrl, 0).0.apply, "nothing found");
        let (out, text) = press(Key::Enter, Modifiers::NONE, 1);
        assert_eq!(out, Output::default());
        assert_eq!(text, "IMG_1\n");
        assert!(press(Key::Escape, Modifiers::NONE, 1).0.close);
    }
}
