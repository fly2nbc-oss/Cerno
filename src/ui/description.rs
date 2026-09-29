//! The details panel's second tab: the photo's comment and keywords, editable. A change goes to
//! the rating writer like a star – into the file (IPTC and XMP), dates kept.
//!
//! Keywords are added with `Enter` (several at once separated by commas) and removed with their
//! ×; the comment is taken when its field is left (click elsewhere, `Esc`, another photo).

use std::path::PathBuf;

use eframe::egui::{
    Align, Button, FontId, Id, Key, Label, Layout, Rect, RichText, ScrollArea, Stroke, TextEdit,
    Ui, UiBuilder, vec2,
};

use crate::i18n;
use crate::metadata::Description;
use crate::theme::{text, tokens};

const PAD: f32 = 16.0;

pub fn comment_id() -> Id {
    Id::new("description-comment")
}

pub fn keyword_id() -> Id {
    Id::new("description-keyword")
}

/// What is being typed, across frames.
#[derive(Debug, Default)]
pub struct Drafts {
    /// The photo the drafts belong to.
    pub path: Option<PathBuf>,
    /// Its description when the drafts started (or were last written).
    pub base: Option<Description>,
    pub comment: String,
    pub keyword: String,
    /// Put the cursor into the keyword field (`B`).
    pub focus_keyword: bool,
}

impl Drafts {
    /// The comment typed differs from the photo's.
    pub fn comment_changed(&self) -> bool {
        self.base
            .as_ref()
            .is_some_and(|base| self.comment.trim() != base.comment)
    }

    /// Starts over for `path`, whose description is `current` (`None` while still loading).
    pub fn reset(&mut self, path: PathBuf, current: Option<&Description>) {
        self.path = Some(path);
        self.base = current.cloned();
        self.comment = current.map(|d| d.comment.clone()).unwrap_or_default();
        self.keyword.clear();
    }
}

/// `changed`: the whole new description, to be written.
#[derive(Debug, Default)]
pub struct Output {
    pub changed: Option<Description>,
}

/// `current`: the photo's description, `None` while it is still loading. `blocked`: why it
/// can't be changed right now (e.g. the photo is being moved).
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    current: Option<&Description>,
    drafts: &mut Drafts,
    blocked: Option<&'static str>,
) -> Output {
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, tokens::SURFACE);
    painter.vline(
        rect.left() + 0.5,
        rect.y_range(),
        Stroke::new(1.0, tokens::LINE),
    );
    let mut out = Output::default();
    let mut panel = ui.new_child(UiBuilder::new().max_rect(rect).id_salt("description"));
    ScrollArea::vertical()
        .auto_shrink(false)
        .show(&mut panel, |ui| {
            ui.set_width(rect.width());
            ui.add_space(PAD);
            ui.spacing_mut().item_spacing.y = 6.0;
            let Some(current) = current else {
                note(ui, i18n::t().description_waiting);
                return;
            };
            let width = rect.width() - 2.0 * PAD;
            ui.add_enabled_ui(blocked.is_none(), |ui| {
                out.changed = fields(ui, width, current, drafts);
            });
            if let Some(reason) = blocked {
                note(ui, reason);
            }
            ui.add_space(4.0);
            note(ui, i18n::t().description_note);
            ui.add_space(PAD);
        });
    out
}

/// Both parts are applied to one new description, so a comment left by clicking a keyword's ×
/// is not lost to the removal.
fn fields(
    ui: &mut Ui,
    width: f32,
    current: &Description,
    drafts: &mut Drafts,
) -> Option<Description> {
    let t = i18n::t();
    let mut next = current.clone();

    section(ui, t.section_comment);
    let comment = indented(ui, |ui| {
        ui.add(
            TextEdit::multiline(&mut drafts.comment)
                .id(comment_id())
                .hint_text(t.comment_hint)
                .desired_rows(4)
                .desired_width(width),
        )
    });
    if comment.lost_focus() && drafts.comment.trim() != current.comment {
        next.comment = drafts.comment.trim().to_owned();
    }

    section(ui, t.section_keywords);
    let mut removed = None;
    if !current.keywords.is_empty() {
        indented(ui, |ui| {
            let row = Layout::left_to_right(Align::Center).with_main_wrap(true);
            ui.allocate_ui_with_layout(vec2(width, 0.0), row, |ui| {
                ui.set_max_width(width);
                for (index, keyword) in current.keywords.iter().enumerate() {
                    let chip = Button::new(
                        RichText::new(format!("{keyword}  ×"))
                            .font(FontId::proportional(text::SMALL)),
                    )
                    .fill(tokens::SURFACE_MUTED);
                    if ui.add(chip).on_hover_text(t.keyword_remove).clicked() {
                        removed = Some(index);
                    }
                }
            });
        });
    }
    if let Some(index) = removed {
        next.keywords.remove(index);
    }
    let field = indented(ui, |ui| {
        ui.add(
            TextEdit::singleline(&mut drafts.keyword)
                .id(keyword_id())
                .hint_text(t.keyword_hint)
                .desired_width(width),
        )
    });
    if field.lost_focus() {
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            for keyword in drafts.keyword.split([',', ';']) {
                next.add_keyword(keyword);
            }
            drafts.keyword.clear();
            // Stay in the field for the next one.
            field.request_focus();
        } else if ui.input(|i| i.key_pressed(Key::Escape)) {
            drafts.keyword.clear();
        }
    }
    if drafts.focus_keyword {
        drafts.focus_keyword = false;
        field.request_focus();
    }
    (next != *current).then_some(next)
}

fn indented<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.add_space(PAD);
        add(ui)
    })
    .inner
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(4.0);
    indented(ui, |ui| {
        ui.label(
            RichText::new(title.to_uppercase())
                .font(FontId::proportional(text::LABEL))
                .color(tokens::MUTED),
        );
    });
}

/// Wrapped text needs a top-down block: in a horizontal layout egui never wraps it.
fn note(ui: &mut Ui, message: &str) {
    let width = (ui.available_width() - 2.0 * PAD).max(40.0);
    indented(ui, |ui| {
        ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_max_width(width);
            ui.add(
                Label::new(
                    RichText::new(i18n::keep_together(message))
                        .font(FontId::proportional(text::SMALL))
                        .color(tokens::MUTED),
                )
                .wrap(),
            );
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, Event, Modifiers, RawInput, pos2, vec2};

    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn frame(
        ctx: &Context,
        drafts: &mut Drafts,
        current: Option<&Description>,
        events: Vec<Event>,
    ) -> Output {
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let panel = Rect::from_min_size(pos2(580.0, 0.0), vec2(320.0, 700.0));
        let mut out = None;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(window),
                events,
                ..Default::default()
            },
            |ui| out = Some(draw(ui, panel, current, drafts, None)),
        );
        output.textures_delta.clear();
        out.expect("drawn")
    }

    fn photo(comment: &str, keywords: &[&str]) -> Description {
        Description {
            comment: comment.to_owned(),
            keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
        }
    }

    /// `B` puts the cursor into the keyword field; `Enter` adds (several with commas, none
    /// twice) and the cursor stays for the next one.
    #[test]
    fn enter_adds_keywords_and_keeps_the_cursor() {
        let ctx = Context::default();
        let current = photo("alt", &["Urlaub"]);
        let mut drafts = Drafts::default();
        drafts.reset(PathBuf::from("a.jpg"), Some(&current));
        drafts.focus_keyword = true;
        frame(&ctx, &mut drafts, Some(&current), vec![]);
        frame(&ctx, &mut drafts, Some(&current), vec![]);
        assert!(ctx.memory(|m| m.has_focus(keyword_id())));

        let out = frame(
            &ctx,
            &mut drafts,
            Some(&current),
            vec![Event::Text("Strand, urlaub; Berge".into()), key(Key::Enter)],
        );
        let changed = out.changed.expect("keywords added");
        assert_eq!(changed, photo("alt", &["Urlaub", "Strand", "Berge"]));
        assert!(drafts.keyword.is_empty());
        frame(&ctx, &mut drafts, Some(&changed), vec![]);
        assert!(
            ctx.memory(|m| m.has_focus(keyword_id())),
            "ready for the next"
        );
    }

    /// Nothing is written while typing a comment; leaving the field takes it.
    #[test]
    fn the_comment_is_taken_when_its_field_is_left() {
        let ctx = Context::default();
        let current = photo("", &["Urlaub"]);
        let mut drafts = Drafts::default();
        drafts.reset(PathBuf::from("a.jpg"), Some(&current));
        frame(&ctx, &mut drafts, Some(&current), vec![]);
        ctx.memory_mut(|m| m.request_focus(comment_id()));
        frame(&ctx, &mut drafts, Some(&current), vec![]);

        let typing = frame(
            &ctx,
            &mut drafts,
            Some(&current),
            vec![Event::Text("Grüße aus Köln".into())],
        );
        assert!(typing.changed.is_none(), "not while typing");
        assert!(drafts.comment_changed());
        let left = frame(&ctx, &mut drafts, Some(&current), vec![key(Key::Escape)]);
        assert_eq!(left.changed, Some(photo("Grüße aus Köln", &["Urlaub"])));
    }

    /// Before the file is read nothing can be typed – a write would replace keywords that are
    /// not known yet.
    #[test]
    fn nothing_to_edit_while_loading() {
        let ctx = Context::default();
        let mut drafts = Drafts {
            focus_keyword: true,
            ..Drafts::default()
        };
        for _ in 0..2 {
            let out = frame(&ctx, &mut drafts, None, vec![Event::Text("x".into())]);
            assert!(out.changed.is_none());
        }
        assert!(!ctx.memory(|m| m.has_focus(keyword_id())));
        assert!(drafts.keyword.is_empty());
    }
}
