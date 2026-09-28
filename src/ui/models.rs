//! Models & data: which model is installed and where it runs, where the model files are, and
//! the maintenance actions (download, reset For you, delete the models). Opened from the menu
//! – the details panel stays about the current photo.

use eframe::egui::{
    Align, Button, Color32, Context, FontId, Id, Key, Label, Layout, Modifiers, Rect, RichText,
    Sense, Ui, vec2,
};

use crate::analysis::{ModelState, Status};
use crate::i18n;
use crate::theme::tokens;
use crate::ui::details::model_note;
use crate::ui::{icons, modal};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ModelsOutput {
    pub close: bool,
    pub download: bool,
    pub reset_taste: bool,
    pub delete_models: bool,
}

pub fn overlay(ctx: &Context, window: Rect, status: &Status) -> ModelsOutput {
    let escape = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    let modal = modal::show(ctx, window, Id::new("models"), 520.0, |ui| {
        content(ui, status)
    });
    let mut out = modal.inner;
    out.close |= escape || modal.outside;
    out
}

fn content(ui: &mut Ui, status: &Status) -> ModelsOutput {
    let t = i18n::t();
    let mut out = ModelsOutput::default();
    let top_right = ui.max_rect().right_top() + vec2(8.0, -8.0);
    ui.label(
        RichText::new(t.menu_models)
            .font(FontId::proportional(17.0))
            .color(tokens::TEXT),
    );
    out.close = modal::close_button(ui, top_right, t.btn_close);
    ui.add_space(14.0);

    let taste = match status.taste.model {
        Some((n, error)) if error.is_finite() => (t.taste_trained)(n, error),
        Some((n, _)) => (t.taste_photos)(n),
        None => t.taste_untrained.to_owned(),
    };
    for (label, value) in [
        ("CLIP", model_note(&status.aesthetics)),
        ("V2.5", model_note(&status.v25)),
        (t.model_faces, model_note(&status.faces)),
        (t.model_personal, taste),
    ] {
        row(ui, label, &value);
    }
    ui.add_space(8.0);
    ui.add(
        Label::new(
            RichText::new(i18n::keep_together(t.explain_models))
                .font(FontId::proportional(12.5))
                .color(tokens::MUTED),
        )
        .wrap(),
    );
    ui.add_space(12.0);
    models_folder(ui);
    ui.add_space(18.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if status.aesthetics == ModelState::Missing {
            let enable = Button::new(RichText::new(t.enable_aesthetics).color(Color32::WHITE))
                .fill(tokens::ACCENT)
                .min_size(vec2(0.0, 32.0));
            if ui
                .add(enable)
                .on_hover_text(t.enable_aesthetics_tooltip)
                .clicked()
            {
                out.download = true;
            }
        }
        if ui
            .add(Button::new(t.btn_reset_taste).min_size(vec2(0.0, 32.0)))
            .clicked()
        {
            out.reset_taste = true;
        }
        if ui
            .add(Button::new(t.btn_delete_models).min_size(vec2(0.0, 32.0)))
            .clicked()
        {
            out.delete_models = true;
        }
    });
    out
}

fn row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(13.0))
                .color(tokens::MUTED),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .font(FontId::proportional(13.0))
                    .color(tokens::TEXT),
            );
        });
    });
    ui.add_space(2.0);
}

/// Folder of the downloaded CLIP and V2.5 files, with a button that copies the path.
fn models_folder(ui: &mut Ui) {
    let t = i18n::t();
    let Ok(dir) = crate::paths::models_dir() else {
        return;
    };
    let text = dir.display().to_string();
    let copied_id = Id::new("models_path_copied_until");
    let now = ui.input(|i| i.time);
    let copied = ui
        .data(|data| data.get_temp::<f64>(copied_id))
        .is_some_and(|until| until > now);

    ui.horizontal(|ui| {
        let button = 24.0;
        let text_w = (ui.available_width() - button - 8.0).max(40.0);
        ui.allocate_ui_with_layout(vec2(text_w, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(text_w);
            ui.add(
                Label::new(
                    RichText::new(&text)
                        .font(FontId::proportional(12.0))
                        .color(tokens::MUTED),
                )
                .wrap(),
            );
        });
        let (rect, response) = ui.allocate_exact_size(vec2(button, button), Sense::click());
        let color = if copied || response.hovered() {
            tokens::ACCENT
        } else {
            tokens::MUTED
        };
        icons::button_background(ui.painter(), rect, response.hovered(), copied);
        icons::copy(ui.painter(), rect.center(), color);
        let tip = if copied {
            t.models_path_copied
        } else {
            t.copy_models_path
        };
        response.clone().on_hover_text(tip);
        if response.clicked() {
            ui.ctx().copy_text(text);
            let until = ui.input(|i| i.time) + 1.6;
            ui.data_mut(|data| data.insert_temp(copied_id, until));
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(1700));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::TasteStatus;
    use eframe::egui::{Event, RawInput, pos2};

    fn status(aesthetics: ModelState) -> Status {
        Status {
            done: 0,
            total: 0,
            aesthetics,
            v25: ModelState::Missing,
            faces: ModelState::Missing,
            taste: TasteStatus {
                examples: 0,
                model: None,
            },
        }
    }

    fn run(status: &Status, events: Vec<Event>) -> (ModelsOutput, Vec<String>) {
        let ctx = Context::default();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let mut out = ModelsOutput::default();
        // An area is invisible in its first frame (egui measures it); the second one paints.
        let mut first = ctx.run_ui(
            RawInput {
                screen_rect: Some(window),
                ..Default::default()
            },
            |ui| {
                overlay(ui.ctx(), window, status);
            },
        );
        first.textures_delta.clear();
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(window),
                events,
                ..Default::default()
            },
            |ui| out = overlay(ui.ctx(), window, status),
        );
        output.textures_delta.clear();
        let texts = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                eframe::egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        (out, texts)
    }

    #[test]
    fn offers_the_download_only_while_the_model_is_missing() {
        let t = crate::i18n::Lang::En.texts();
        let (_, missing) = run(&status(ModelState::Missing), Vec::new());
        assert!(missing.iter().any(|text| text == t.enable_aesthetics));
        let (_, ready) = run(&status(ModelState::Available), Vec::new());
        assert!(!ready.iter().any(|text| text == t.enable_aesthetics));
        assert!(ready.iter().any(|text| text == t.btn_delete_models));
    }

    #[test]
    fn escape_closes() {
        let escape = Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let (out, _) = run(&status(ModelState::Missing), vec![escape]);
        assert!(out.close);
        assert!(!out.download && !out.reset_taste && !out.delete_models);
    }
}
