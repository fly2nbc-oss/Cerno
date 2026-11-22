//! The rename card (`F2` the current photo, `Shift+F2` every photo shown): the four patterns
//! with what each makes of the name, `↑`/`↓` or `F1`–`F4` choose; for one photo the new
//! name in a field that can be overwritten, for several the list *old → new*. `Enter` renames,
//! `Esc` cancels – both taken before the field sees them.

use eframe::egui::{
    Align, Button, Context, FontId, Id, InputState, Key, Layout, Modifiers, Rect, RichText,
    ScrollArea, TextEdit, vec2,
};

use crate::i18n;
use crate::theme::{self, text, tokens};
use crate::ui::modal;

/// What the card keeps from frame to frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    /// The pattern chosen (`rename::Pattern::ALL`'s index).
    pub pattern: usize,
    /// One photo: the new name without its extension.
    pub name: String,
    /// The name was typed over: choosing a pattern sets it again.
    pub edited: bool,
    /// The Event pattern's text.
    pub text: String,
    /// The card has been drawn once: the `F2` that opened it doesn't also choose pattern 2.
    pub shown: bool,
}

/// One pattern row: its name and what it makes of the (first) photo's name – `None` when the
/// photo has no capture time.
pub struct PatternRow {
    pub label: &'static str,
    pub example: Option<String>,
}

pub enum Body<'a> {
    /// One photo: its extension (shown after the field) and why the name can't be.
    One { ext: &'a str, error: Option<String> },
    /// Several photos: how many change, and each as *old → new* (`true`: it changes);
    /// `text`: the pattern takes a text, and its field shows.
    All {
        count: String,
        rows: &'a [(String, String, bool)],
        text: bool,
    },
}

pub struct View<'a> {
    pub title: String,
    pub patterns: Vec<PatternRow>,
    pub body: Body<'a>,
    /// Why Rename is greyed out.
    pub blocked: Option<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    pub apply: bool,
    pub close: bool,
}

const WIDTH: f32 = 560.0;

/// `F1`–`F4` choose a pattern. Function keys type nothing, so the name field never gets a
/// character from them (`Alt`+digit did: the digit landed in the name).
fn take_pattern_key(i: &mut InputState) -> Option<usize> {
    [Key::F1, Key::F2, Key::F3, Key::F4]
        .into_iter()
        .position(|key| i.consume_key(Modifiers::NONE, key))
}

pub fn show(ctx: &Context, window: Rect, state: &mut State, view: &View) -> Output {
    let t = i18n::t();
    let choosable = |i: usize| {
        view.patterns
            .get(i)
            .is_some_and(|row| row.example.is_some())
    };
    let (enter, escape, up, down, chosen) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::Enter)
                || i.consume_key(Modifiers::COMMAND, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Escape),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            if state.shown {
                take_pattern_key(i)
            } else {
                None
            },
        )
    });
    state.shown = true;
    let pick = |to: usize, state: &mut State| {
        if choosable(to) && to != state.pattern {
            state.pattern = to;
            state.edited = false;
        }
    };
    if let Some(to) = chosen {
        pick(to, state);
    }
    let count = view.patterns.len();
    if down && let Some(to) = (state.pattern + 1..count).find(|&i| choosable(i)) {
        pick(to, state);
    }
    if up && let Some(to) = (0..state.pattern).rev().find(|&i| choosable(i)) {
        pick(to, state);
    }
    let modal = modal::show(ctx, window, Id::unique("rename-card"), WIDTH, |ui| {
        let mut out = Output::default();
        let top_right = ui.max_rect().right_top() + vec2(8.0, -8.0);
        ui.label(
            RichText::new(&view.title)
                .font(theme::strong(text::VALUE))
                .color(tokens::TEXT),
        );
        out.close = modal::close_button(ui, top_right, t.btn_close);
        ui.add_space(12.0);
        for (i, row) in view.patterns.iter().enumerate() {
            let example = row.example.as_deref().unwrap_or(t.rename_no_time);
            let text = format!("F{}   {}   ·   {example}", i + 1, row.label);
            let button = Button::selectable(state.pattern == i, text);
            let response = ui.add_enabled(row.example.is_some(), button);
            if response.clicked() {
                pick(i, state);
            }
        }
        ui.add_space(12.0);
        match &view.body {
            Body::One { ext, error } => {
                ui.label(RichText::new(t.rename_name).color(tokens::MUTED));
                ui.horizontal(|ui| {
                    let mut field = TextEdit::singleline(&mut state.name);
                    if error.is_some() {
                        field = field.text_color(tokens::STATUS_ERROR);
                    }
                    let width = (ui.available_width() - 80.0).max(120.0);
                    let response = theme::field_scope(ui, |ui| {
                        ui.add_sized(vec2(width, theme::CONTROL), field)
                    });
                    // The field has the cursor from the start: typing renames.
                    let focused = Id::unique("rename-card-focused");
                    if !ui.data(|d| d.get_temp::<bool>(focused).unwrap_or(false)) {
                        response.request_focus();
                        ui.data_mut(|d| d.insert_temp(focused, true));
                    }
                    if response.changed() {
                        state.edited = true;
                    }
                    if !ext.is_empty() {
                        ui.label(RichText::new(format!(".{ext}")).color(tokens::MUTED));
                    }
                });
                if let Some(error) = error {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(error)
                            .font(FontId::proportional(text::SMALL))
                            .color(tokens::STATUS_ERROR),
                    );
                }
            }
            Body::All { count, rows, text } => {
                if *text {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t.rename_text).color(tokens::MUTED));
                        theme::field_scope(ui, |ui| {
                            ui.add_sized(
                                vec2(240.0, theme::CONTROL),
                                TextEdit::singleline(&mut state.text),
                            )
                        });
                    });
                    ui.add_space(8.0);
                }
                ui.label(RichText::new(count).color(tokens::MUTED));
                ui.add_space(4.0);
                let height = ui.text_style_height(&eframe::egui::TextStyle::Body) + 4.0;
                ScrollArea::vertical()
                    .max_height(240.0)
                    .auto_shrink([false, true])
                    .show_rows(ui, height, rows.len(), |ui, range| {
                        // The new name stands out in the accent (it is what gets chosen); a
                        // photo that keeps its name stays muted.
                        for (old, new, changes) in &rows[range] {
                            let font = FontId::proportional(text::SMALL);
                            let mut job = eframe::egui::text::LayoutJob::default();
                            let mut part = |text: &str, color| {
                                job.append(
                                    text,
                                    0.0,
                                    eframe::egui::TextFormat {
                                        font_id: font.clone(),
                                        color,
                                        ..Default::default()
                                    },
                                );
                            };
                            part(old, tokens::MUTED);
                            part("  →  ", tokens::MUTED);
                            part(
                                new,
                                if *changes {
                                    tokens::ACCENT_STRONG
                                } else {
                                    tokens::MUTED
                                },
                            );
                            ui.label(job);
                        }
                    });
            }
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new(t.rename_keys)
                .font(FontId::proportional(text::SMALL))
                .color(tokens::MUTED),
        );
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let apply = ui
                .add_enabled(
                    view.blocked.is_none(),
                    theme::primary_button(t.rename_apply),
                )
                .on_hover_text("Enter");
            let apply = match &view.blocked {
                Some(reason) => apply.on_disabled_hover_text(reason),
                None => apply,
            };
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
    out.apply |= enter && view.blocked.is_none();
    out.close |= escape || modal.outside;
    if out.apply || out.close {
        ctx.data_mut(|d| d.remove::<bool>(Id::unique("rename-card-focused")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, RawInput, pos2};

    fn view(blocked: Option<String>) -> View<'static> {
        View {
            title: "Rename".into(),
            patterns: vec![
                PatternRow {
                    label: "Standard",
                    example: Some("2026_08_23__191546".into()),
                },
                PatternRow {
                    label: "Cloud",
                    example: Some("2026-08-23 19.15.46".into()),
                },
                PatternRow {
                    label: "Android",
                    example: None,
                },
                PatternRow {
                    label: "Event",
                    example: Some("2026-08-23_Trip_001".into()),
                },
            ],
            body: Body::One {
                ext: "jpg",
                error: None,
            },
            blocked,
        }
    }

    fn frames(state: &mut State, presses: Vec<Event>, blocked: Option<String>) -> Output {
        let ctx = crate::theme::test_context();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let view = view(blocked);
        let mut out = Output::default();
        for events in [Vec::new(), presses] {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(window),
                    events,
                    ..Default::default()
                },
                |ui| out = show(ui.ctx(), window, state, &view),
            );
            output.textures_delta.clear();
        }
        out
    }

    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// `Enter` renames (not while greyed out), `Esc` cancels; `F2` and the arrows choose a
    /// pattern – past one without a capture time – and choosing sets the name again.
    #[test]
    fn keys_choose_patterns_and_rename() {
        let mut state = State {
            edited: true,
            shown: true,
            ..State::default()
        };
        assert!(frames(&mut state, vec![key(Key::Enter, Modifiers::NONE)], None).apply);
        assert!(
            !frames(
                &mut state,
                vec![key(Key::Enter, Modifiers::NONE)],
                Some("taken".into())
            )
            .apply
        );
        assert!(frames(&mut state, vec![key(Key::Escape, Modifiers::NONE)], None).close);
        frames(&mut state, vec![key(Key::F2, Modifiers::NONE)], None);
        assert_eq!(state.pattern, 1);
        assert!(!state.edited, "a pattern chosen sets the name again");
        frames(&mut state, vec![key(Key::ArrowDown, Modifiers::NONE)], None);
        assert_eq!(
            state.pattern, 3,
            "the pattern without a time is passed over"
        );
        frames(&mut state, vec![key(Key::ArrowUp, Modifiers::NONE)], None);
        assert_eq!(state.pattern, 1);
        frames(&mut state, vec![key(Key::F3, Modifiers::NONE)], None);
        assert_eq!(state.pattern, 1, "no capture time: not chosen");
    }
}
