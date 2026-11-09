//! The card of the marks that could not be written (ticket CER-57): which files and why, *Try
//! again* (`Enter`) and *Close* (`Esc`) – and, when Cerno is being closed, *Quit anyway*.

use eframe::egui::{
    Align, Context, FontId, Id, Key, Label, Layout, Modifiers, Rect, RichText, Sense, vec2,
};

use crate::i18n;
use crate::theme::{self, text, tokens};
use crate::ui::modal;

/// How many files the card names; the rest is a count.
pub const LISTED: usize = 8;

/// One file whose marks are not written.
pub struct File {
    pub name: String,
    /// The whole path, as the name's tooltip.
    pub path: String,
    pub error: String,
}

pub struct Card<'a> {
    pub files: &'a [File],
    /// Cerno is being closed: the card offers *Quit anyway*.
    pub exiting: bool,
    /// A try is running: *Try again* waits.
    pub retrying: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    pub retry: bool,
    /// *Quit anyway* (only while exiting).
    pub quit: bool,
    /// Closed: back to Cerno.
    pub close: bool,
}

pub fn show(ctx: &Context, window: Rect, card: &Card<'_>) -> Output {
    let t = i18n::t();
    let (enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    let cancel = if card.exiting {
        t.btn_cancel
    } else {
        t.btn_close
    };
    let modal = modal::show(ctx, window, Id::new("unsaved"), 520.0, |ui| {
        let mut out = Output::default();
        let top_right = ui.max_rect().right_top() + vec2(8.0, -8.0);
        ui.label(
            RichText::new((t.unsaved_fact)(card.files.len()))
                .font(theme::strong(text::VALUE))
                .color(tokens::TEXT),
        );
        out.close = modal::close_button(ui, top_right, cancel);
        ui.add_space(8.0);
        ui.add(
            Label::new(
                RichText::new(i18n::keep_together(t.unsaved_text))
                    .font(FontId::proportional(text::BODY))
                    .color(tokens::MUTED),
            )
            .wrap(),
        );
        if card.exiting {
            ui.add_space(8.0);
            ui.add(
                Label::new(
                    RichText::new(i18n::keep_together(t.unsaved_exit_text))
                        .font(FontId::proportional(text::BODY))
                        .color(tokens::TEXT),
                )
                .wrap(),
            );
        }
        ui.add_space(12.0);
        for file in card.files.iter().take(LISTED) {
            ui.add(
                Label::new(
                    RichText::new(&file.name)
                        .font(FontId::proportional(text::BODY))
                        .color(tokens::TEXT),
                )
                .truncate()
                .sense(Sense::hover()),
            )
            .on_hover_text(&file.path);
            ui.add(
                Label::new(
                    RichText::new(&file.error)
                        .font(FontId::proportional(text::SMALL))
                        .color(tokens::STATUS_WARN),
                )
                .wrap(),
            );
            ui.add_space(6.0);
        }
        if card.files.len() > LISTED {
            ui.label(
                RichText::new((t.unsaved_more)(card.files.len() - LISTED))
                    .font(FontId::proportional(text::SMALL))
                    .color(tokens::MUTED),
            );
        }
        ui.add_space(18.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = if card.retrying {
                t.unsaved_retrying
            } else {
                t.btn_retry
            };
            if ui
                .add_enabled(!card.retrying, theme::primary_button(label))
                .on_hover_text("Enter")
                .clicked()
            {
                out.retry = true;
            }
            if card.exiting && ui.add(theme::danger_button(t.btn_quit_anyway)).clicked() {
                out.quit = true;
            }
            if ui.add(theme::secondary_button(cancel)).clicked() {
                out.close = true;
            }
        });
        out
    });
    let mut out = modal.inner;
    out.retry |= enter && !card.retrying;
    out.close |= escape || modal.outside;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, RawInput, Shape, pos2};

    fn files(n: usize) -> Vec<File> {
        (1..=n)
            .map(|i| File {
                name: format!("IMG_{i}.jpg"),
                path: format!("D:/Photos/IMG_{i}.jpg"),
                error: "The process cannot access the file".to_owned(),
            })
            .collect()
    }

    /// The card's answer to `key` in its second frame (the first measures it), and its texts.
    fn shown(card: &Card<'_>, key: Option<Key>) -> (Output, Vec<String>) {
        let ctx = crate::theme::test_context();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let mut out = Output::default();
        let mut texts = Vec::new();
        let pressed = key.map_or_else(Vec::new, |key| {
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }]
        });
        for events in [Vec::new(), pressed] {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(window),
                    events,
                    ..Default::default()
                },
                |ui| out = show(ui.ctx(), window, card),
            );
            output.textures_delta.clear();
            texts = output
                .shapes
                .iter()
                .filter_map(|clipped| match &clipped.shape {
                    Shape::Text(text) => Some(text.galley.text().to_owned()),
                    _ => None,
                })
                .collect();
        }
        (out, texts)
    }

    #[test]
    fn enter_tries_again_and_escape_closes() {
        let files = files(2);
        let card = Card {
            files: &files,
            exiting: false,
            retrying: false,
        };
        assert!(shown(&card, Some(Key::Enter)).0.retry);
        assert!(shown(&card, Some(Key::Escape)).0.close);
        assert_eq!(shown(&card, None).0, Output::default());
        let running = Card {
            retrying: true,
            ..card
        };
        assert!(
            !shown(&running, Some(Key::Enter)).0.retry,
            "one try at a time"
        );
    }

    /// It names the files and why, a long list up to `LISTED` and a count; at exit it says
    /// what quitting means and offers it.
    #[test]
    fn it_names_the_files_and_the_reason() {
        let many = files(LISTED + 3);
        let (_, texts) = shown(
            &Card {
                files: &many,
                exiting: false,
                retrying: false,
            },
            None,
        );
        assert!(texts.iter().any(|t| t == "11 marks not saved"));
        assert!(texts.iter().any(|t| t == "IMG_1.jpg"));
        assert!(!texts.iter().any(|t| t == "IMG_9.jpg"), "beyond the list");
        assert!(texts.iter().any(|t| t == "… and 3 more"));
        assert!(
            texts
                .iter()
                .any(|t| t.contains("The process cannot access the file"))
        );
        assert!(!texts.iter().any(|t| t == "Quit anyway"));

        let one = files(1);
        let (_, texts) = shown(
            &Card {
                files: &one,
                exiting: true,
                retrying: false,
            },
            None,
        );
        assert!(texts.iter().any(|t| t == "1 mark not saved"));
        assert!(texts.iter().any(|t| t == "Quit anyway"));
        assert!(texts.iter().any(|t| t.contains("If you quit now")));
    }
}
