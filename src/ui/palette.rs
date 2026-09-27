//! Command palette (`Ctrl+K`): type a few letters of any command, arrows choose, Enter runs.
//! Everything the toolbar offers is reachable here without the mouse.

use eframe::egui::{
    Align2, Area, Color32, Context, CursorIcon, FontId, Frame, Id, Key, Modifiers, Order, Rect,
    Sense, Stroke, StrokeKind, TextEdit, pos2, vec2,
};

use crate::i18n;
use crate::theme::tokens;

const WIDTH: f32 = 560.0;
const INPUT_HEIGHT: f32 = 46.0;
const ROW_HEIGHT: f32 = 32.0;
/// Rows shown at once; the list scrolls with the selection.
const MAX_ROWS: usize = 10;

pub struct Command<A> {
    pub action: A,
    pub label: String,
    pub shortcut: Option<String>,
    /// `Some(true)` shows a check mark (active toggle or current choice).
    pub checked: Option<bool>,
}

impl<A> Command<A> {
    pub fn new(action: A, label: impl Into<String>, shortcut: Option<String>) -> Self {
        Self {
            action,
            label: label.into(),
            shortcut,
            checked: None,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }
}

/// Query and selection while the palette is open.
#[derive(Debug, Default)]
pub struct State {
    query: String,
    selected: usize,
}

pub struct Output<A> {
    pub run: Option<A>,
    pub close: bool,
}

pub fn show<A: Copy>(
    ctx: &Context,
    window: Rect,
    state: &mut State,
    commands: &[Command<A>],
) -> Output<A> {
    let t = i18n::t();
    let mut out = Output {
        run: None,
        close: false,
    };
    let found: Vec<&Command<A>> = commands
        .iter()
        .filter(|c| matches(&c.label, &state.query))
        .collect();

    // Taken before the text field sees them.
    let (up, down, enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    out.close = escape;
    if let Some(last) = found.len().checked_sub(1) {
        if down {
            state.selected = if state.selected >= last {
                0
            } else {
                state.selected + 1
            };
        }
        if up {
            state.selected = if state.selected == 0 {
                last
            } else {
                state.selected - 1
            };
        }
        state.selected = state.selected.min(last);
        if enter {
            out.run = Some(found[state.selected].action);
        }
    }

    Area::new(Id::new("palette"))
        .order(Order::Foreground)
        .fixed_pos(window.min)
        .show(ctx, |ui| {
            let backdrop = ui.allocate_rect(window, Sense::click());
            ui.painter()
                .rect_filled(window, 0.0, Color32::from_black_alpha(120));

            let width = (window.width() - 48.0).clamp(240.0, WIDTH);
            let rows = found.len().clamp(1, MAX_ROWS);
            let card = Rect::from_min_size(
                pos2(
                    window.center().x - width / 2.0,
                    window.top() + (window.height() * 0.14).max(24.0),
                ),
                vec2(width, INPUT_HEIGHT + rows as f32 * ROW_HEIGHT + 10.0),
            );
            ui.interact(card, Id::new("palette-card"), Sense::click());
            let painter = ui.painter();
            painter.rect_filled(card, 8.0, tokens::SURFACE);
            painter.rect_stroke(
                card,
                8.0,
                Stroke::new(1.0, tokens::LINE),
                StrokeKind::Inside,
            );

            let input = Rect::from_min_size(card.min + vec2(16.0, 8.0), vec2(width - 32.0, 30.0));
            let edit = ui.put(
                input,
                TextEdit::singleline(&mut state.query)
                    .id(Id::new("palette-input"))
                    .hint_text(t.palette_placeholder)
                    .font(FontId::proportional(15.0))
                    .frame(Frame::NONE)
                    .desired_width(input.width()),
            );
            edit.request_focus();
            if edit.changed() {
                state.selected = 0;
            }
            let divider = card.top() + INPUT_HEIGHT - 2.0;
            ui.painter()
                .hline(card.x_range(), divider, Stroke::new(1.0, tokens::LINE));

            if found.is_empty() {
                ui.painter().text(
                    pos2(card.left() + 16.0, divider + 4.0 + ROW_HEIGHT / 2.0),
                    Align2::LEFT_CENTER,
                    t.palette_empty,
                    FontId::proportional(13.0),
                    tokens::MUTED,
                );
            }
            let first = (state.selected + 1).saturating_sub(MAX_ROWS);
            let pointer_moved = ui.input(|i| i.pointer.delta() != vec2(0.0, 0.0));
            for (slot, (index, command)) in found
                .iter()
                .enumerate()
                .skip(first)
                .take(MAX_ROWS)
                .enumerate()
            {
                let row = Rect::from_min_size(
                    pos2(card.left() + 6.0, divider + 4.0 + slot as f32 * ROW_HEIGHT),
                    vec2(width - 12.0, ROW_HEIGHT),
                );
                let response = ui
                    .interact(row, Id::new(("palette-row", index)), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand);
                if response.hovered() && pointer_moved {
                    state.selected = index;
                }
                if response.clicked() {
                    out.run = Some(command.action);
                }
                let painter = ui.painter();
                if index == state.selected {
                    painter.rect_filled(row, 5.0, tokens::ACCENT_SUBTLE);
                }
                let y = row.center().y;
                if command.checked == Some(true) {
                    // Painted: Segoe UI has no check mark glyph.
                    let c = pos2(row.left() + 16.0, y);
                    let stroke = Stroke::new(1.6, tokens::ACCENT_STRONG);
                    painter.line_segment([c + vec2(-4.5, 0.0), c + vec2(-1.5, 3.5)], stroke);
                    painter.line_segment([c + vec2(-1.5, 3.5), c + vec2(4.5, -4.0)], stroke);
                }
                painter.text(
                    pos2(row.left() + 30.0, y),
                    Align2::LEFT_CENTER,
                    &command.label,
                    FontId::proportional(13.5),
                    tokens::TEXT,
                );
                if let Some(shortcut) = &command.shortcut {
                    painter.text(
                        pos2(row.right() - 10.0, y),
                        Align2::RIGHT_CENTER,
                        shortcut,
                        FontId::proportional(12.0),
                        tokens::MUTED,
                    );
                }
            }
            out.close |= backdrop.clicked();
        });
    out
}

/// Every word of the query occurs in the label – case- and accent-insensitive, so "ast"
/// finds "Ästhetik" and "fenetre" finds "fenêtre".
pub fn matches(label: &str, query: &str) -> bool {
    let label = fold(label);
    fold(query)
        .split_whitespace()
        .all(|word| label.contains(word))
}

fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ä' | 'à' | 'á' | 'â' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ö' | 'ó' | 'ò' | 'ô' => 'o',
            'ü' | 'ú' | 'ù' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key) -> eframe::egui::Event {
        eframe::egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    /// Runs the palette for one frame per event list; returns the last output's action.
    fn run(frames: Vec<Vec<eframe::egui::Event>>) -> Option<u8> {
        let ctx = Context::default();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let commands = [
            Command::new(1, "Vollbild", Some("F11".into())),
            Command::new(2, "Vergleichen", Some("C".into())),
            Command::new(3, "Sprache: Deutsch", None).checked(true),
        ];
        let mut state = State::default();
        let mut run = None;
        for (i, events) in frames.into_iter().enumerate() {
            let input = eframe::egui::RawInput {
                screen_rect: Some(window),
                time: Some(i as f64 * 0.1),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                run = show(ui.ctx(), window, &mut state, &commands).run;
            });
            output.textures_delta.clear();
        }
        run
    }

    #[test]
    fn typing_filters_and_enter_runs() {
        let typed = eframe::egui::Event::Text("verg".into());
        // The text field takes focus in the first frame.
        assert_eq!(
            run(vec![vec![], vec![typed], vec![press(Key::Enter)]]),
            Some(2)
        );
        assert_eq!(
            run(vec![vec![], vec![press(Key::ArrowDown), press(Key::Enter)]]),
            Some(2)
        );
        // Up from the first entry wraps to the last.
        assert_eq!(
            run(vec![vec![], vec![press(Key::ArrowUp), press(Key::Enter)]]),
            Some(3)
        );
        assert_eq!(run(vec![vec![], vec![]]), None);
    }

    #[test]
    fn words_match_in_any_order_ignoring_case_and_accents() {
        assert!(matches("Sortierung: Ästhetik (V2.5)", "ast v2"));
        assert!(matches("Sortierung: Ästhetik (V2.5)", "V2 SORT"));
        assert!(matches("Plein écran", "ecran"));
        assert!(matches("Anything", ""));
        assert!(!matches("Vollbild", "voll x"));
    }
}
