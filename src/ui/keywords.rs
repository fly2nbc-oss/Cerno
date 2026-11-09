//! The keyword filter's card (*Visible photos ▸ By keyword …*, CER-68): the folder's keywords
//! with how many photos have each, the most used first, and a row for the photos without any.
//! A field narrows the list while typing; `↑`/`↓` move, `Enter` ticks the row, `Ctrl+Enter`
//! applies, `Esc` cancels – the keys are taken before the field sees them. A click ticks too.
//! What is ticked is `keywords::Pick`; the vocabulary is `keywords`.

use eframe::egui::{
    Align, Context, FontId, Id, Key, Layout, Modifiers, Rect, RichText, ScrollArea, TextEdit, vec2,
};

use crate::i18n;
use crate::keywords::{self, Entry, Pick};
use crate::theme::{self, text, tokens};
use crate::ui::{modal, palette};

const ROW_HEIGHT: f32 = 30.0;
const LIST_HEIGHT: f32 = 320.0;

/// The card's state while it is open.
#[derive(Debug, Default)]
pub struct Card {
    /// The open folder's keywords, the most used first.
    pub entries: Vec<Entry>,
    /// Photos whose keywords are known and empty.
    pub untagged: usize,
    /// Photos the analysis has not read yet: their keywords are not counted.
    pub unread: usize,
    pub pick: Pick,
    /// What narrows the list.
    pub query: String,
    /// The row the keyboard is on, among those shown.
    pub cursor: usize,
    /// The filter is on now: Apply with nothing ticked ends it.
    pub was_on: bool,
}

/// One row of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// The photos without any keyword.
    None,
    /// `entries[i]`.
    Word(usize),
}

impl Card {
    /// The rows for what is typed: "without keywords" first while nothing is typed, then the
    /// words containing it.
    pub fn rows(&self) -> Vec<Row> {
        let typed = keywords::fold(&self.query);
        let none = typed.is_empty().then_some(Row::None);
        none.into_iter()
            .chain(
                self.entries
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| {
                        typed.is_empty() || keywords::fold(&entry.text).contains(&typed)
                    })
                    .map(|(index, _)| Row::Word(index)),
            )
            .collect()
    }

    /// Ticks or unticks a row.
    pub fn toggle(&mut self, row: Row) {
        match row {
            Row::None => self.pick.none = !self.pick.none,
            Row::Word(index) => {
                if let Some(entry) = self.entries.get(index) {
                    let text = entry.text.clone();
                    self.pick.toggle(&text);
                }
            }
        }
    }

    fn can_apply(&self) -> bool {
        !self.pick.is_empty() || self.was_on
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    pub apply: bool,
    pub close: bool,
}

pub fn show(ctx: &Context, window: Rect, card: &mut Card) -> Output {
    let t = i18n::t();
    // Taken before the field sees them: Enter would leave it, the arrows move its cursor.
    let (apply_key, escape, enter, up, down) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::COMMAND, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Escape),
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
        )
    });
    let rows = card.rows();
    if down {
        card.cursor = (card.cursor + 1).min(rows.len().saturating_sub(1));
    }
    if up {
        card.cursor = card.cursor.saturating_sub(1);
    }
    if enter && let Some(&row) = rows.get(card.cursor) {
        card.toggle(row);
    }
    let modal = modal::show(ctx, window, Id::unique("keyword-filter"), 480.0, |ui| {
        let mut out = Output::default();
        let top_right = ui.max_rect().right_top() + vec2(8.0, -8.0);
        ui.label(
            RichText::new(t.keywords_title)
                .font(theme::strong(text::VALUE))
                .color(tokens::TEXT),
        );
        out.close = modal::close_button(ui, top_right, t.btn_close);
        ui.add_space(8.0);
        ui.label(RichText::new(i18n::keep_together(t.keywords_intro)).color(tokens::MUTED));
        ui.add_space(12.0);
        let before = card.query.clone();
        let field = theme::field_scope(ui, |ui| {
            ui.add(
                TextEdit::singleline(&mut card.query)
                    .desired_width(f32::INFINITY)
                    .hint_text(t.keywords_search_hint),
            )
        });
        if card.query != before {
            card.cursor = 0;
        }
        // The cursor goes into the field when the card opens.
        let focused_id = Id::unique("keyword-filter-focused");
        if !ui.data(|d| d.get_temp::<bool>(focused_id).unwrap_or(false)) {
            field.request_focus();
            ui.data_mut(|d| d.insert_temp(focused_id, true));
        }
        ui.add_space(8.0);
        let rows = card.rows();
        card.cursor = card.cursor.min(rows.len().saturating_sub(1));
        if card.entries.is_empty() && card.query.is_empty() {
            ui.label(
                RichText::new(t.keywords_empty)
                    .font(FontId::proportional(text::SMALL))
                    .color(tokens::MUTED),
            );
        }
        let width = ui.available_width();
        let mut clicked = None;
        ScrollArea::vertical()
            .max_height(LIST_HEIGHT)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (at, &row) in rows.iter().enumerate() {
                    let (label, count, on) = match row {
                        Row::None => (t.keywords_none.to_owned(), card.untagged, card.pick.none),
                        Row::Word(index) => {
                            let entry = &card.entries[index];
                            (
                                entry.text.clone(),
                                entry.folder,
                                card.pick.contains(&entry.text),
                            )
                        }
                    };
                    let (rect, _) = ui
                        .allocate_exact_size(vec2(width, ROW_HEIGHT), eframe::egui::Sense::hover());
                    let item = palette::Row::new((), label, Some(count.to_string())).toggle(on);
                    let lit = at == card.cursor;
                    let response = palette::bar_row(
                        ui,
                        rect,
                        Id::unique(("keyword-row", at)),
                        &item,
                        false,
                        lit,
                    );
                    if lit && (up || down) {
                        ui.scroll_to_rect(rect, None);
                    }
                    if response.clicked {
                        clicked = Some((at, row));
                    }
                }
            });
        if let Some((at, row)) = clicked {
            card.cursor = at;
            card.toggle(row);
            // Back into the field, so the keys go on working.
            field.request_focus();
        }
        if card.unread > 0 {
            ui.add_space(4.0);
            ui.label(
                RichText::new((t.keywords_unread)(card.unread))
                    .font(FontId::proportional(text::SMALL))
                    .color(tokens::MUTED),
            );
        }
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let apply = ui
                .add_enabled(card.can_apply(), theme::primary_button(t.name_list_apply))
                .on_hover_text(i18n::with_ctrl("Enter"));
            if apply.clicked() {
                out.apply = true;
            }
            if ui.add(theme::secondary_button(t.btn_cancel)).clicked() {
                out.close = true;
            }
        });
        out
    });
    let mut out = modal.inner;
    out.apply |= apply_key && card.can_apply();
    out.close |= escape || modal.outside;
    if out.apply || out.close {
        ctx.data_mut(|d| d.remove::<bool>(Id::unique("keyword-filter-focused")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, RawInput, pos2};

    fn card() -> Card {
        let entry = |text: &str, folder| Entry {
            text: text.to_owned(),
            folder,
            index: folder,
        };
        Card {
            entries: vec![
                entry("Familie", 4),
                entry("Strand", 2),
                entry("Bodensee", 1),
            ],
            untagged: 3,
            ..Card::default()
        }
    }

    fn key(key: Key, modifiers: Modifiers) -> Vec<Event> {
        vec![
            Event::ModifiersChanged(modifiers),
            Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ]
    }

    /// Runs the card through a measuring frame, then one frame per event list.
    fn frames(card: &mut Card, steps: Vec<Vec<Event>>) -> Output {
        let ctx = crate::theme::test_context();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let mut out = Output::default();
        for events in std::iter::once(Vec::new()).chain(steps) {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(window),
                    events,
                    ..Default::default()
                },
                |ui| out = show(ui.ctx(), window, card),
            );
            output.textures_delta.clear();
        }
        out
    }

    #[test]
    fn typing_narrows_and_without_keywords_leads() {
        let mut card = card();
        assert_eq!(
            card.rows(),
            [Row::None, Row::Word(0), Row::Word(1), Row::Word(2)]
        );
        card.query = "an".into();
        assert_eq!(card.rows(), [Row::Word(1)]);
        card.query = "SEE".into();
        assert_eq!(card.rows(), [Row::Word(2)]);
    }

    /// `↓` and `Enter` tick a row without leaving the field; `Ctrl+Enter` applies; `Esc`
    /// closes. Nothing ticked: no Apply, unless the filter is on (then it ends it).
    #[test]
    fn the_keyboard_ticks_and_applies() {
        let mut card = card();
        let ctrl = Modifiers::COMMAND | Modifiers::CTRL;
        let out = frames(
            &mut card,
            vec![
                key(Key::ArrowDown, Modifiers::NONE),
                key(Key::Enter, Modifiers::NONE),
                key(Key::ArrowDown, Modifiers::NONE),
                key(Key::Enter, Modifiers::NONE),
            ],
        );
        assert_eq!(out, Output::default());
        assert_eq!(card.pick.keywords, ["Familie", "Strand"]);
        assert!(!card.pick.none);
        assert!(card.query.is_empty(), "the keys never reached the field");
        assert!(frames(&mut card, vec![key(Key::Enter, ctrl)]).apply);

        let mut empty = Card::default();
        assert!(!frames(&mut empty, vec![key(Key::Enter, ctrl)]).apply);
        empty.was_on = true;
        assert!(frames(&mut empty, vec![key(Key::Enter, ctrl)]).apply);
        assert!(frames(&mut card, vec![key(Key::Escape, Modifiers::NONE)]).close);
    }
}
