//! Models & data: which model is installed and where it runs, where the model files are, and
//! the maintenance actions (download, reset For you, delete the models). Opened from the menu
//! – the details panel stays about the current photo.

use eframe::egui::{
    Align, Button, Color32, Context, FontId, Id, Key, Label, Layout, Modifiers, Rect, RichText,
    Sense, Ui, vec2,
};

use crate::analysis::Status;
use crate::analysis::manifest::Pack;
use crate::i18n;
use crate::theme::{text, tokens};
use crate::ui::details::model_note;
use crate::ui::{icons, modal};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ModelsOutput {
    pub close: bool,
    pub download: bool,
    pub reset_taste: bool,
    pub delete_models: bool,
}

/// The download button's label and tooltip for what is missing: aesthetics as a whole, or
/// V2.5 alone once CLIP is there – the same action either way.
pub fn download_label(missing: &[Pack]) -> (&'static str, String) {
    let t = i18n::t();
    let size = i18n::size(missing.iter().map(|pack| pack.bytes()).sum());
    if missing.contains(&Pack::Clip) {
        (t.enable_aesthetics, (t.enable_aesthetics_tooltip)(&size))
    } else {
        (t.add_v25, (t.add_v25_tooltip)(&size))
    }
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
            .font(FontId::proportional(text::VALUE))
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
    // The count above is more than the photos with stars: rejected and deleted ones count too.
    let sources = status.taste.sources;
    if status.taste.examples > 0 {
        ui.add_space(4.0);
        ui.add(
            Label::new(
                RichText::new(i18n::keep_together(&(t.taste_sources)(
                    sources.stars,
                    sources.rejected,
                    sources.deleted,
                )))
                .font(FontId::proportional(text::BODY))
                .color(tokens::MUTED),
            )
            .wrap(),
        );
    }
    ui.add_space(8.0);
    ui.add(
        Label::new(
            RichText::new(i18n::keep_together(t.explain_models))
                .font(FontId::proportional(text::BODY))
                .color(tokens::MUTED),
        )
        .wrap(),
    );
    ui.add_space(12.0);
    models_folder(ui);
    ui.add_space(18.0);

    let missing = status.missing();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if !missing.is_empty() {
            let (label, tooltip) = download_label(&missing);
            let enable = Button::new(RichText::new(label).color(Color32::WHITE))
                .fill(tokens::ACCENT)
                .min_size(vec2(0.0, 32.0));
            if ui.add(enable).on_hover_text(tooltip).clicked() {
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
            // Nothing to delete while files are being downloaded or removed.
            .add_enabled(
                !status.busy(),
                Button::new(t.btn_delete_models).min_size(vec2(0.0, 32.0)),
            )
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
                .font(FontId::proportional(text::BODY))
                .color(tokens::MUTED),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .font(FontId::proportional(text::BODY))
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
                        .font(FontId::proportional(text::SMALL))
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
    use crate::analysis::{ModelState, TasteStatus};
    use eframe::egui::{Event, RawInput, pos2};

    fn status(aesthetics: ModelState, v25: ModelState) -> Status {
        Status {
            done: 0,
            total: 0,
            aesthetics,
            v25,
            faces: ModelState::Missing,
            taste: TasteStatus {
                examples: 0,
                sources: Default::default(),
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
    fn offers_the_download_only_while_a_model_is_missing() {
        let t = crate::i18n::Lang::En.texts();
        let (_, missing) = run(
            &status(ModelState::Missing, ModelState::Missing),
            Vec::new(),
        );
        assert!(missing.iter().any(|text| text == t.enable_aesthetics));
        // CLIP is there, V2.5 is not: the same button, for V2.5.
        let (_, v25) = run(
            &status(ModelState::Available, ModelState::Missing),
            Vec::new(),
        );
        assert!(!v25.iter().any(|text| text == t.enable_aesthetics));
        assert!(v25.iter().any(|text| text == t.add_v25));
        let (_, ready) = run(
            &status(ModelState::Available, ModelState::Available),
            Vec::new(),
        );
        assert!(
            !ready
                .iter()
                .any(|text| text == t.enable_aesthetics || text == t.add_v25)
        );
        assert!(ready.iter().any(|text| text == t.btn_delete_models));
        // While one downloads, the other waits: no button.
        let downloading = ModelState::Downloading {
            received: 5,
            total: 10,
        };
        let (_, busy) = run(&status(downloading, ModelState::Missing), Vec::new());
        assert!(
            !busy
                .iter()
                .any(|text| text == t.enable_aesthetics || text == t.add_v25)
        );
    }

    #[test]
    fn the_label_names_what_is_missing() {
        let t = crate::i18n::Lang::En.texts();
        // Tests never switch the language: `download_label` reads English.
        let (label, tooltip) = download_label(&[Pack::Clip, Pack::V25]);
        assert_eq!(label, t.enable_aesthetics);
        assert!(tooltip.contains("2.9 GB"), "{tooltip}");
        let (label, tooltip) = download_label(&[Pack::V25]);
        assert_eq!(label, t.add_v25);
        assert!(tooltip.contains("1.7 GB"), "{tooltip}");
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
        let (out, _) = run(
            &status(ModelState::Missing, ModelState::Missing),
            vec![escape],
        );
        assert!(out.close);
        assert!(!out.download && !out.reset_taste && !out.delete_models);
    }
}
