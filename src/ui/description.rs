//! The details panel's second tab: the photo's comment and keywords, editable. A change goes to
//! the rating writer like a star – into the file (IPTC and XMP), dates kept.
//!
//! Keywords are added with `Enter` (several at once separated by commas) and removed with their
//! ×; the comment is taken when its field is left (click elsewhere, `Esc`, another photo).
//! While a keyword is typed, the folder's keywords that fit are offered below the field (the
//! most used first, then the rest of the index's – CER-68): `↓`/`↑` pick one, `Enter` or a
//! click takes it. A word the folder knows in another case takes the folder's spelling.

use std::path::PathBuf;

use eframe::egui::{
    Align, Button, Color32, FontId, Id, Key, Label, Layout, Modifiers, Rect, RichText, ScrollArea,
    Sense, Stroke, TextEdit, Ui, UiBuilder, vec2,
};

use crate::i18n;
use crate::keywords::Vocabulary;
use crate::metadata::{self, Description};
use crate::theme::{self, text, tokens};
use crate::ui::palette;

const PAD: f32 = 16.0;
/// Suggestions shown under the keyword field at most.
const SUGGESTIONS: usize = 6;
const SUGGESTION_HEIGHT: f32 = 26.0;

pub fn comment_id() -> Id {
    Id::unique("description-comment")
}

pub fn keyword_id() -> Id {
    Id::unique("description-keyword")
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
    /// Put the cursor into the keyword field (`Enter` on this tab, the menu row).
    pub focus_keyword: bool,
    /// The suggestion `↓`/`↑` are on; `None`: the text as typed.
    pub suggestion: Option<usize>,
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
        self.suggestion = None;
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
    vocabulary: &Vocabulary,
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
                out.changed = fields(ui, width, current, drafts, vocabulary);
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
    vocabulary: &Vocabulary,
) -> Option<Description> {
    let t = i18n::t();
    let mut next = current.clone();

    section(ui, t.section_comment);
    // Said while it is typed – above the field, which grows with a long text – and written
    // whole all the same (CER-67).
    if metadata::exceeds_iim(&drafts.comment, metadata::IIM_COMMENT_MAX) {
        warning(ui, t.comment_too_long);
    }
    let comment = indented(ui, |ui| {
        theme::field_scope(ui, |ui| {
            ui.add(
                TextEdit::multiline(&mut drafts.comment)
                    .id(comment_id())
                    .hint_text(t.comment_hint)
                    .desired_rows(4)
                    .desired_width(width),
            )
        })
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
                    // Longer than IPTC IIM keeps: the chip says so in the warning colour.
                    let long = metadata::exceeds_iim(keyword, metadata::IIM_KEYWORD_MAX);
                    let label = if long {
                        format!(
                            "{}  {keyword}  {}",
                            theme::icon::TRIANGLE_ALERT,
                            theme::icon::X
                        )
                    } else {
                        format!("{keyword}  {}", theme::icon::X)
                    };
                    let mut label = RichText::new(label).font(FontId::proportional(text::SMALL));
                    if long {
                        label = label.color(tokens::STATUS_WARN);
                    }
                    let chip = Button::new(label)
                        .fill(tokens::SURFACE_MUTED)
                        .min_size(vec2(0.0, theme::COMPACT));
                    let hint = if long {
                        format!(
                            "{}

{}",
                            t.keyword_too_long, t.keyword_remove
                        )
                    } else {
                        t.keyword_remove.to_owned()
                    };
                    if ui.add(chip).on_hover_text(hint).clicked() {
                        removed = Some(index);
                    }
                }
            });
        });
    }
    if let Some(index) = removed {
        next.keywords.remove(index);
    }
    // The arrows pick a suggestion: taken before the field, which would move its cursor.
    let typed = drafts.keyword.clone();
    let offered = vocabulary
        .suggest(&typed, &current.keywords, SUGGESTIONS)
        .len();
    if offered > 0 && ui.memory(|m| m.has_focus(keyword_id())) {
        let (up, down) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
            )
        });
        if down {
            drafts.suggestion = Some(drafts.suggestion.map_or(0, |i| (i + 1).min(offered - 1)));
        }
        if up {
            drafts.suggestion = drafts.suggestion.and_then(|i| i.checked_sub(1));
        }
    }
    let field = indented(ui, |ui| {
        theme::field_scope(ui, |ui| {
            ui.add(
                TextEdit::singleline(&mut drafts.keyword)
                    .id(keyword_id())
                    .hint_text(t.keyword_hint)
                    .desired_width(width),
            )
        })
    });
    if drafts.keyword != typed {
        drafts.suggestion = None;
    }
    let suggestions = vocabulary.suggest(&drafts.keyword, &current.keywords, SUGGESTIONS);
    drafts.suggestion = drafts.suggestion.filter(|&i| i < suggestions.len());
    let mut taken = None;
    if field.lost_focus() {
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            match drafts.suggestion {
                Some(i) => taken = Some(suggestions[i].text.clone()),
                None => {
                    // A word the vocabulary knows takes its spelling (CER-68).
                    for keyword in drafts.keyword.split([',', ';']) {
                        next.add_keyword(&vocabulary.spelling(keyword));
                    }
                }
            }
            drafts.keyword.clear();
            drafts.suggestion = None;
            // Stay in the field for the next one.
            field.request_focus();
        } else if ui.input(|i| i.key_pressed(Key::Escape)) {
            drafts.keyword.clear();
            drafts.suggestion = None;
        }
    }
    if !drafts.keyword.trim().is_empty() && !suggestions.is_empty() {
        indented(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (i, entry) in suggestions.iter().enumerate() {
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(width, SUGGESTION_HEIGHT), Sense::hover());
                    // The folder's count; a word only other folders have shows none.
                    let count = (entry.folder > 0).then(|| entry.folder.to_string());
                    let row = palette::Row::new((), entry.text.clone(), count);
                    let lit = drafts.suggestion == Some(i);
                    let id = Id::unique(("keyword-suggestion", i));
                    if palette::bar_row(ui, rect, id, &row, false, lit).clicked {
                        taken = Some(entry.text.clone());
                    }
                }
            });
        });
    }
    if let Some(keyword) = taken {
        next.add_keyword(&keyword);
        drafts.keyword.clear();
        drafts.suggestion = None;
        field.request_focus();
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
        ui.label(theme::overline(title));
    });
}

/// A note in the warning colour, with its sign.
fn warning(ui: &mut Ui, message: &str) {
    let message = format!("{}  {message}", theme::icon::TRIANGLE_ALERT);
    wrapped(ui, &message, tokens::STATUS_WARN);
}

fn note(ui: &mut Ui, message: &str) {
    wrapped(ui, message, tokens::MUTED);
}

/// Wrapped text needs a top-down block: in a horizontal layout egui never wraps it.
fn wrapped(ui: &mut Ui, message: &str, colour: Color32) {
    let width = (ui.available_width() - 2.0 * PAD).max(40.0);
    indented(ui, |ui| {
        ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_max_width(width);
            ui.add(
                Label::new(
                    RichText::new(i18n::keep_together(message))
                        .font(FontId::proportional(text::SMALL))
                        .color(colour),
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
        frame_with(ctx, drafts, current, &Vocabulary::default(), events)
    }

    fn frame_with(
        ctx: &Context,
        drafts: &mut Drafts,
        current: Option<&Description>,
        vocabulary: &Vocabulary,
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
            |ui| out = Some(draw(ui, panel, current, drafts, vocabulary, None)),
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

    /// `Enter` (on this tab) puts the cursor into the keyword field; `Enter` adds (several with commas, none
    /// twice) and the cursor stays for the next one.
    #[test]
    fn enter_adds_keywords_and_keeps_the_cursor() {
        let ctx = crate::theme::test_context();
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

    /// While a keyword is typed the folder's words that fit show below the field: `↓` and
    /// `Enter` take one; a word typed in another case takes the folder's spelling (CER-68).
    #[test]
    fn suggestions_are_picked_with_the_arrows() {
        let ctx = crate::theme::test_context();
        let folder = [
            vec!["Bodensee".to_owned()],
            vec!["Bodensee".to_owned(), "Boot".to_owned()],
        ];
        let vocabulary = Vocabulary::build(folder.iter().map(Vec::as_slice), &[]);
        let current = photo("", &["Urlaub"]);
        let mut drafts = Drafts::default();
        drafts.reset(PathBuf::from("a.jpg"), Some(&current));
        drafts.focus_keyword = true;
        let step = |drafts: &mut Drafts, events| {
            frame_with(&ctx, drafts, Some(&current), &vocabulary, events)
        };
        step(&mut drafts, vec![]);
        step(&mut drafts, vec![Event::Text("bo".into())]);
        step(&mut drafts, vec![key(Key::ArrowDown)]);
        step(&mut drafts, vec![key(Key::ArrowDown)]);
        assert_eq!(drafts.suggestion, Some(1), "Boot, the second");
        assert_eq!(drafts.keyword, "bo", "the arrows never reached the field");
        let out = step(&mut drafts, vec![key(Key::Enter)]);
        assert_eq!(out.changed, Some(photo("", &["Urlaub", "Boot"])));
        assert!(drafts.keyword.is_empty() && drafts.suggestion.is_none());

        let out = step(
            &mut drafts,
            vec![Event::Text("bodensee".into()), key(Key::Enter)],
        );
        assert_eq!(out.changed, Some(photo("", &["Urlaub", "Bodensee"])));
    }

    /// Nothing is written while typing a comment; leaving the field takes it.
    #[test]
    fn the_comment_is_taken_when_its_field_is_left() {
        let ctx = crate::theme::test_context();
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

    /// Every text the panel draws, nested shapes included.
    fn texts(ctx: &Context, current: &Description, drafts: &mut Drafts) -> Vec<String> {
        fn walk(shape: &eframe::egui::epaint::Shape, out: &mut Vec<String>) {
            match shape {
                eframe::egui::epaint::Shape::Text(text) => out.push(text.galley.text().to_owned()),
                eframe::egui::epaint::Shape::Vec(shapes) => {
                    shapes.iter().for_each(|s| walk(s, out))
                }
                _ => {}
            }
        }
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let panel = Rect::from_min_size(pos2(580.0, 0.0), vec2(320.0, 700.0));
        let mut out = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(window),
                    ..Default::default()
                },
                |ui| {
                    draw(
                        ui,
                        panel,
                        Some(current),
                        drafts,
                        &Vocabulary::default(),
                        None,
                    );
                },
            );
            output.textures_delta.clear();
            out.clear();
            for clipped in &output.shapes {
                walk(&clipped.shape, &mut out);
            }
        }
        out
    }

    /// A keyword longer than IPTC IIM keeps (64 bytes, an umlaut counts twice) carries the
    /// warning sign; 64 bytes don't. The same for a comment over 2000 bytes (CER-67).
    #[test]
    fn a_keyword_over_64_bytes_is_marked() {
        let ctx = crate::theme::test_context();
        let warned = |current: &Description, drafts: &mut Drafts| {
            texts(&ctx, current, drafts)
                .iter()
                .any(|t| t.contains(theme::icon::TRIANGLE_ALERT))
        };
        let mut drafts = Drafts::default();
        let fits = photo("", &[&"a".repeat(64), &"ä".repeat(32)]);
        drafts.reset(PathBuf::from("a.jpg"), Some(&fits));
        assert!(!warned(&fits, &mut drafts), "64 bytes fit");
        for long in ["a".repeat(65), "ä".repeat(33)] {
            let current = photo("", &[&long]);
            drafts.reset(PathBuf::from("a.jpg"), Some(&current));
            assert!(warned(&current, &mut drafts), "{} bytes", long.len());
        }
        let comment = photo(&"a".repeat(2000), &[]);
        drafts.reset(PathBuf::from("a.jpg"), Some(&comment));
        assert!(!warned(&comment, &mut drafts), "2000 bytes fit");
        drafts.comment.push('ü');
        assert!(warned(&comment, &mut drafts), "the draft counts");
    }

    /// Before the file is read nothing can be typed – a write would replace keywords that are
    /// not known yet.
    #[test]
    fn nothing_to_edit_while_loading() {
        let ctx = crate::theme::test_context();
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
