//! The filter bar at the top: sort, filters, analysis status and the "Action" button.

use eframe::egui::containers::scroll_area::ScrollBarVisibility;
use eframe::egui::{
    Align, Button, Color32, ComboBox, CursorIcon, Layout, Painter, Rect, Response, RichText,
    ScrollArea, Sense, Sides, Stroke, StrokeKind, TextStyle, Ui, UiBuilder, pos2, vec2,
};

use crate::analysis::{ModelState, Status};
use crate::i18n;
use crate::theme::{self, tokens};
use crate::view::{FilterKind, SortKey, ViewOptions};

pub const TOOLBAR_HEIGHT: f32 = 40.0;

pub struct ToolbarInfo<'a> {
    /// New scores arrived since the view was sorted/filtered.
    pub stale: bool,
    pub status: &'a Status,
    /// The action menu (copy, move, delete) is open.
    pub actions_open: bool,
    /// Name of the photo "similar photos" is about, while that filter is on.
    pub similar_to: Option<&'a str>,
    /// How many photos the view shows, and how many the folder has.
    pub shown: usize,
    pub total: usize,
}

#[derive(Default)]
pub struct ToolbarOutput {
    pub options_changed: bool,
    pub refresh: bool,
    pub download_model: bool,
    /// The "similar" box was clicked; the app picks the photo it is about (like `M`).
    pub toggle_similar: bool,
    /// The "Action" button was clicked (opens or closes the action menu).
    pub toggle_actions: bool,
    /// Where the "Action" button is, so the menu opens under it.
    pub actions_anchor: Option<Rect>,
}

pub fn toolbar(
    ui: &mut Ui,
    rect: Rect,
    options: &mut ViewOptions,
    info: &ToolbarInfo<'_>,
) -> ToolbarOutput {
    let t = i18n::t();
    ui.painter().rect_filled(rect, 0.0, tokens::SURFACE);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, tokens::LINE),
    );
    let before = *options;
    let mut out = ToolbarOutput::default();
    let mut action_rect = None;
    let mut similar_clicked = false;
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(12.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            // One line: sort and the filter boxes on the left, action and analysis status on
            // the right. The boxes scroll sideways when they no longer fit. Nothing changes
            // its width while it is used, so nothing moves away under the pointer.
            Sides::new()
                .height(ui.available_height())
                .shrink_left()
                .show(
                    ui,
                    |ui| {
                        ComboBox::from_id_salt("sort")
                            .width(sort_width(ui))
                            .selected_text((t.sort)(options.sort.label()))
                            .show_ui(ui, |ui| {
                                for key in SortKey::ALL {
                                    ui.selectable_value(&mut options.sort, key, key.label());
                                }
                            });
                        let boxes = ScrollArea::horizontal()
                            .id_salt("filter-boxes")
                            .max_width(ui.available_width())
                            .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    // Always in its place, greyed out while nothing is
                                    // filtered: showing up in front of the boxes pushed them
                                    // all away from the pointer.
                                    if ui
                                        .add_enabled(
                                            !options.filter.is_all() || options.similar,
                                            Button::new(t.filter_clear).small(),
                                        )
                                        .on_disabled_hover_text(t.filter_none_active)
                                        .clicked()
                                    {
                                        options.filter.clear();
                                        options.similar = false;
                                    }
                                    for kind in FilterKind::ALL {
                                        let mut on = options.filter.contains(kind);
                                        let response = match kind {
                                            // Colours as small squares: five names would not
                                            // fit next to the rest.
                                            FilterKind::Colour(label) => colour_box(
                                                ui,
                                                &mut on,
                                                theme::label_color(label),
                                                i18n::label_name(label),
                                            ),
                                            FilterKind::Stars(n) => {
                                                ui.checkbox(&mut on, format!("{n}★"))
                                            }
                                            FilterKind::Blurry => ui
                                                .checkbox(&mut on, kind.label())
                                                .on_hover_text(t.filter_blurry_tooltip),
                                            FilterKind::Duplicate => ui
                                                .checkbox(&mut on, kind.label())
                                                .on_hover_text(t.filter_duplicate_tooltip),
                                            _ => ui.checkbox(&mut on, kind.label()),
                                        };
                                        if response.changed() {
                                            options.filter.set(kind, on);
                                        }
                                    }
                                    // Last, so its label – the photo's name while on – moves
                                    // nothing.
                                    similar_clicked =
                                        similar_box(ui, options.similar, info.similar_to);
                                });
                            });
                        overflow_hint(
                            ui.painter(),
                            boxes.inner_rect,
                            boxes.content_size.x,
                            boxes.state.offset.x,
                        );
                    },
                    |ui| {
                        // Right to left: "Action" first, so it stays at the right edge; what
                        // comes and goes appears left of it. Only what needs attention: the
                        // model is missing, loading or failed, or the analysis is still
                        // running. Where the model runs is on the models card.
                        let action = ui
                            .add(Button::new(t.actions).selected(info.actions_open))
                            .on_hover_text(format!(
                                "{}\n{}",
                                t.actions_tooltip,
                                i18n::with_ctrl("M")
                            ));
                        action_rect = Some(action.rect);
                        if action.clicked() {
                            out.toggle_actions = true;
                        }
                        count_badge(ui, info.shown, info.total, is_filtered(&before));
                        if info.stale
                            && ui
                                .button(t.refresh_order)
                                .on_hover_text(t.refresh_order_tooltip)
                                .clicked()
                        {
                            out.refresh = true;
                        }
                        let Status { done, total, .. } = *info.status;
                        if done < total {
                            progress(ui, done, total);
                        }
                        aesthetics_status(ui, &info.status.aesthetics, &mut out);
                    },
                );
        },
    );
    out.actions_anchor = action_rect;
    out.toggle_similar = similar_clicked;
    out.options_changed = *options != before;
    out
}

/// The sort box is as wide as its longest entry in this language, so choosing another sort
/// doesn't move the filter boxes. `ComboBox::width` counts the arrow and the padding too.
fn sort_width(ui: &Ui) -> f32 {
    let t = i18n::t();
    let font = TextStyle::Button.resolve(ui.style());
    let widest = SortKey::ALL
        .into_iter()
        .map(|key| {
            ui.painter()
                .layout_no_wrap((t.sort)(key.label()), font.clone(), tokens::TEXT)
                .size()
                .x
        })
        .fold(0.0, f32::max);
    let spacing = ui.spacing();
    widest + spacing.icon_spacing + spacing.icon_width + 2.0 * spacing.button_padding.x
}

/// Whether a filter hides photos right now: a box ticked or "similar photos" on.
fn is_filtered(options: &ViewOptions) -> bool {
    !options.filter.is_all() || options.similar
}

/// "12 of 340 photos" on the accent's subtle fill while a filter is on – what "Action" works
/// on – and a muted "340 photos" otherwise. Its slot is as wide as the longest count the
/// folder can show, so a changing number moves nothing.
fn count_badge(ui: &mut Ui, shown: usize, total: usize, filtered: bool) {
    const PAD: f32 = 8.0;
    let t = i18n::t();
    let font = TextStyle::Body.resolve(ui.style());
    let nines = 10usize.pow(total.max(1).to_string().len() as u32) - 1;
    let widest = [(t.photos_shown)(nines, nines), (t.photos_count)(nines)]
        .into_iter()
        .map(|text| {
            ui.painter()
                .layout_no_wrap(text, font.clone(), tokens::TEXT)
                .size()
                .x
        })
        .fold(0.0, f32::max);
    let (slot, response) = ui.allocate_exact_size(vec2(widest + 2.0 * PAD, 24.0), Sense::hover());
    let (text, colour) = if filtered {
        ((t.photos_shown)(shown, total), tokens::TEXT)
    } else {
        ((t.photos_count)(total), tokens::MUTED)
    };
    let galley = ui.painter().layout_no_wrap(text, font, colour);
    let size = galley.size();
    // Right-aligned, next to "Action".
    let at = pos2(slot.right() - PAD - size.x, slot.center().y - size.y / 2.0);
    if filtered {
        let pill = Rect::from_min_size(at, size).expand2(vec2(PAD, 3.0));
        ui.painter().rect_filled(pill, 4.0, tokens::ACCENT_SUBTLE);
    }
    ui.painter().galley(at, galley, colour);
    response.on_hover_text(t.photos_badge_tooltip);
}

/// "Analysing 12 / 340", right-aligned in the width of the longest count, so the numbers
/// ticking up move nothing.
fn progress(ui: &mut Ui, done: usize, total: usize) {
    let t = i18n::t();
    let nines = 10usize.pow(total.to_string().len() as u32) - 1;
    let widest = ui
        .painter()
        .layout_no_wrap(
            (t.analyzing_progress)(nines, nines),
            TextStyle::Body.resolve(ui.style()),
            tokens::MUTED,
        )
        .size()
        .x;
    let text = RichText::new((t.analyzing_progress)(done, total)).color(tokens::MUTED);
    ui.allocate_ui_with_layout(
        vec2(widest, ui.available_height()),
        Layout::right_to_left(Align::Center),
        |ui| {
            ui.set_min_width(widest);
            ui.label(text);
        },
    );
}

/// "≈ Similar", or "≈ like IMG_0012" while on (a long name shortened, whole in the tooltip).
/// Whether it was clicked; the app decides which photo it is about.
fn similar_box(ui: &mut Ui, on: bool, reference: Option<&str>) -> bool {
    const LONGEST: usize = 18;
    let t = i18n::t();
    let label = match reference.filter(|_| on) {
        Some(name) if name.chars().count() > LONGEST => {
            let short: String = name.chars().take(LONGEST - 1).collect();
            (t.filter_similar_to)(&format!("{short}…"))
        }
        Some(name) => (t.filter_similar_to)(name),
        None => t.filter_similar.to_owned(),
    };
    let explain = (t.similar_tooltip)(crate::view::SIMILAR_MIN * 100.0);
    let tooltip = match reference.filter(|_| on) {
        Some(name) => format!("{}\n{explain}", (t.menu_similar_to)(name)),
        None => explain,
    };
    let mut ticked = on;
    ui.checkbox(&mut ticked, label)
        .on_hover_text(tooltip)
        .changed()
}

/// A colour label as a small square – filled and ticked when on; the name is the tooltip.
fn colour_box(ui: &mut Ui, on: &mut bool, colour: Color32, name: &str) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let painter = ui.painter();
    let square = Rect::from_center_size(rect.center(), vec2(14.0, 14.0));
    if *on {
        painter.rect_filled(square, 3.0, colour);
        let c = square.center();
        let stroke = Stroke::new(1.8, tokens::BG);
        painter.line_segment([c + vec2(-3.5, 0.0), c + vec2(-1.0, 2.8)], stroke);
        painter.line_segment([c + vec2(-1.0, 2.8), c + vec2(3.8, -3.2)], stroke);
    } else {
        painter.rect_stroke(square, 3.0, Stroke::new(1.5, colour), StrokeKind::Inside);
    }
    if response.hovered() {
        painter.rect_stroke(
            square.expand(2.0),
            4.0,
            Stroke::new(1.0, tokens::ACCENT),
            StrokeKind::Outside,
        );
    }
    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(name)
}

/// Fades the edge where filter boxes are scrolled out of view, so hidden ones are noticed.
fn overflow_hint(painter: &Painter, inner: Rect, content_width: f32, offset: f32) {
    const WIDTH: f32 = 28.0;
    const STEPS: usize = 14;
    let hidden_left = offset > 1.0;
    let hidden_right = offset + inner.width() < content_width - 1.0;
    let [r, g, b, _] = tokens::SURFACE.to_array();
    for (hidden, edge, direction) in [
        (hidden_left, inner.left(), 1.0),
        (hidden_right, inner.right(), -1.0),
    ] {
        if !hidden {
            continue;
        }
        for step in 0..STEPS {
            let (a, b_) = (step as f32 / STEPS as f32, (step + 1) as f32 / STEPS as f32);
            let (x0, x1) = (edge + direction * a * WIDTH, edge + direction * b_ * WIDTH);
            let alpha = ((1.0 - a) * 255.0) as u8;
            painter.rect_filled(
                Rect::from_x_y_ranges(x0.min(x1)..=x0.max(x1), inner.y_range()),
                0.0,
                Color32::from_rgba_unmultiplied(r, g, b, alpha),
            );
        }
    }
}

fn aesthetics_status(ui: &mut Ui, state: &ModelState, out: &mut ToolbarOutput) {
    let t = i18n::t();
    let muted = |text: String| RichText::new(text).color(tokens::MUTED);
    match state {
        ModelState::Missing => {
            if ui
                .button(t.enable_aesthetics)
                .on_hover_text(t.enable_aesthetics_tooltip)
                .clicked()
            {
                out.download_model = true;
            }
        }
        ModelState::Downloading { received, total } => {
            let percent = *received as f64 / (*total).max(1) as f64 * 100.0;
            ui.label(muted((t.downloading_model)(percent)));
        }
        ModelState::Loading => {
            ui.label(muted(t.aesthetics_loading.into()));
        }
        ModelState::Available | ModelState::Ready { .. } | ModelState::Removing => {}
        ModelState::Failed(message) => {
            ui.label(RichText::new(t.aesthetics_failed).color(tokens::STATUS_ERROR))
                .on_hover_text(message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::TasteStatus;
    use eframe::egui::{Context, RawInput, Shape, pos2};

    fn status(done: usize, total: usize) -> Status {
        Status {
            done,
            total,
            aesthetics: ModelState::Available,
            v25: ModelState::Missing,
            faces: ModelState::Missing,
            taste: TasteStatus {
                examples: 0,
                model: None,
            },
        }
    }

    /// The bar drawn in a wide window: where each text lands, and the Action button.
    fn bar(options: ViewOptions, stale: bool, status: &Status) -> (Vec<(String, Rect)>, Rect) {
        bar_about(options, stale, status, None, (340, 340))
    }

    /// The same with "similar photos" about `similar_to` and `(shown, total)` photos.
    fn bar_about(
        options: ViewOptions,
        stale: bool,
        status: &Status,
        similar_to: Option<&str>,
        (shown, total): (usize, usize),
    ) -> (Vec<(String, Rect)>, Rect) {
        let ctx = Context::default();
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1800.0, TOOLBAR_HEIGHT));
        let mut last = None;
        // Twice: the side-scrolling boxes know their size from the frame before.
        for _ in 0..2 {
            let mut action = None;
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| {
                    let mut options = options;
                    let info = ToolbarInfo {
                        stale,
                        status,
                        actions_open: false,
                        similar_to,
                        shown,
                        total,
                    };
                    action = toolbar(ui, screen, &mut options, &info).actions_anchor;
                },
            );
            output.textures_delta.clear();
            let texts = output
                .shapes
                .iter()
                .filter_map(|clipped| match &clipped.shape {
                    Shape::Text(text) => {
                        Some((text.galley.text().to_owned(), text.visual_bounding_rect()))
                    }
                    _ => None,
                })
                .collect();
            last = Some((texts, action.expect("the Action button is drawn")));
        }
        last.expect("two frames")
    }

    fn left_of(texts: &[(String, Rect)], wanted: &str) -> f32 {
        texts
            .iter()
            .find(|(text, _)| text == wanted)
            .map(|(_, rect)| rect.left())
            .unwrap_or_else(|| panic!("{wanted:?} is drawn"))
    }

    /// "Show all" keeps its place – greyed out – so ticking the first filter moves no box.
    #[test]
    fn boxes_stay_when_a_filter_is_ticked() {
        let done = status(1, 1);
        let none = ViewOptions::default();
        let mut some = none;
        some.filter.set(FilterKind::Stars(3), true);
        let (before, _) = bar(none, false, &done);
        let (after, _) = bar(some, false, &done);
        let show_all = i18n::t().filter_clear;
        assert_eq!(left_of(&before, show_all), left_of(&after, show_all));
        assert_eq!(left_of(&before, "1★"), left_of(&after, "1★"));
    }

    /// The "similar" box is always there, last: switching it on names the photo and moves no
    /// other box.
    #[test]
    fn the_similar_box_sits_last_and_moves_nothing() {
        let done = status(1, 1);
        let (off, _) = bar(ViewOptions::default(), false, &done);
        let on = ViewOptions {
            similar: true,
            ..ViewOptions::default()
        };
        let long = "IMG_20260928_171203_HDR.jpg";
        let (named, _) = bar_about(on, false, &done, Some("IMG_0012.JPG"), (340, 340));
        let (shortened, _) = bar_about(on, false, &done, Some(long), (340, 340));
        let t = i18n::t();
        assert!(left_of(&off, t.filter_similar) > left_of(&off, "Duplicates"));
        assert_eq!(left_of(&off, "1★"), left_of(&named, "1★"));
        left_of(&named, &(t.filter_similar_to)("IMG_0012.JPG"));
        left_of(&shortened, &(t.filter_similar_to)("IMG_20260928_1712…"));
    }

    /// The sort box is as wide for "Name" as for the longest sort.
    #[test]
    fn another_sort_moves_no_box() {
        let done = status(1, 1);
        let lefts: Vec<f32> = SortKey::ALL
            .into_iter()
            .map(|sort| {
                let options = ViewOptions {
                    sort,
                    ..ViewOptions::default()
                };
                left_of(&bar(options, false, &done).0, "1★")
            })
            .collect();
        assert!(lefts.windows(2).all(|pair| pair[0] == pair[1]), "{lefts:?}");
    }

    /// "Action" stays at the right edge while the progress and "Refresh order" come and go,
    /// and the count ticking up moves nothing either.
    #[test]
    fn the_action_button_stays_at_the_right_edge() {
        let options = ViewOptions::default();
        let (_, quiet) = bar(options, false, &status(1200, 1200));
        let (early, busy) = bar(options, true, &status(5, 1200));
        let (late, _) = bar(options, true, &status(999, 1200));
        assert_eq!(quiet, busy);
        let refresh = i18n::t().refresh_order;
        assert_eq!(left_of(&early, refresh), left_of(&late, refresh));
    }

    /// The count sits left of "Action": all photos without a filter, "shown of all" with one;
    /// neither the number nor the filter moves the button or the progress.
    #[test]
    fn the_count_names_what_action_works_on() {
        let done = status(5, 340);
        let none = ViewOptions::default();
        let mut some = none;
        some.filter.set(FilterKind::Stars(3), true);
        let (all, action) = bar_about(none, false, &done, None, (340, 340));
        let (few, action_few) = bar_about(some, false, &done, None, (12, 340));
        let (one, action_one) = bar_about(some, false, &done, None, (1, 340));
        let t = i18n::t();
        assert!(all.iter().any(|(text, _)| *text == (t.photos_count)(340)));
        assert!(
            few.iter()
                .any(|(text, _)| *text == (t.photos_shown)(12, 340))
        );
        assert_eq!(action, action_few);
        assert_eq!(action, action_one);
        let progress = (t.analyzing_progress)(5, 340);
        assert_eq!(left_of(&few, &progress), left_of(&one, &progress));
        assert_eq!(left_of(&all, &progress), left_of(&few, &progress));
        assert!(left_of(&few, &(t.photos_shown)(12, 340)) < action.left());
    }
}
