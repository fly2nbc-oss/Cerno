//! The filter bar at the top. Left, what is shown: photos, videos or the best N photos, then
//! the filter groups (rating, colour, blurry / duplicates) and "similar photos". Right, what
//! comes of it: the analysis status, the sort, the count with its reset and "Action".

use eframe::egui::containers::scroll_area::ScrollBarVisibility;
use eframe::egui::{
    Align, Button, Color32, ComboBox, CursorIcon, Layout, Painter, Rect, Response, RichText,
    ScrollArea, Sense, Sides, Stroke, StrokeKind, TextStyle, TextWrapMode, Ui, UiBuilder, pos2,
    vec2,
};

use crate::analysis::{ModelState, Status};
use crate::i18n;
use crate::theme::{self, tokens};
use crate::ui::{icons, stars};
use crate::view::{FilterKind, Media, Scope, SortKey, TOP_LEVELS, ViewOptions};

pub const TOOLBAR_HEIGHT: f32 = 40.0;
/// Height of the boxes, chips and the count, so they line up.
const ITEM_HEIGHT: f32 = 24.0;

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
    /// The folder holds at least one video (else "videos only" has nothing to show).
    pub has_videos: bool,
}

#[derive(Default)]
pub struct ToolbarOutput {
    pub options_changed: bool,
    pub refresh: bool,
    pub download_model: bool,
    /// The "similar" chip was clicked; the app picks the photo it is about (like `M`).
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
    // The right side works on copies: both sides can't borrow `options` at once.
    let mut sort = options.sort;
    let mut clear = false;
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(12.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            // One line. The groups scroll sideways when they no longer fit; the first box and
            // the whole right side never do. Nothing changes its width while it is used, so
            // nothing moves away under the pointer.
            Sides::new()
                .height(ui.available_height())
                .shrink_left()
                .show(
                    ui,
                    |ui| {
                        scope_box(ui, options, info.has_videos);
                        separator(ui);
                        let boxes = ScrollArea::horizontal()
                            .id_salt("filter-boxes")
                            .max_width(ui.available_width())
                            .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    for (index, group) in FilterKind::GROUPS.iter().enumerate() {
                                        if index > 0 {
                                            separator(ui);
                                        }
                                        for &kind in *group {
                                            if filter_box(ui, kind, options.filter.contains(kind)) {
                                                options.filter.toggle(kind);
                                            }
                                        }
                                    }
                                    separator(ui);
                                    // Last, so its label – the photo's name while on – moves
                                    // nothing.
                                    similar_clicked =
                                        similar_chip(ui, options.similar, info.similar_to);
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
                        // Right to left: "Action" first, so it stays at the right edge; the
                        // count sits beside it – what "Action" works on – then the sort. What
                        // comes and goes appears left of the sort: "Refresh order", the
                        // analysis progress, a missing, loading or failed model.
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
                        clear = count_badge(ui, info.shown, info.total, before.is_filtered());
                        ComboBox::from_id_salt("sort")
                            .width(sort_width(ui))
                            .selected_text((t.sort)(sort.label()))
                            .show_ui(ui, |ui| {
                                for key in SortKey::ALL {
                                    ui.selectable_value(&mut sort, key, key.label());
                                }
                            });
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
    options.sort = sort;
    if clear {
        options.clear_filters();
    }
    out.actions_anchor = action_rect;
    out.toggle_similar = similar_clicked;
    out.options_changed = *options != before;
    out
}

/// Photos, videos or both – or the best N photos (`Scope`). As wide as its longest choice in
/// this language, and outlined in the accent while Top N is on. "Videos only" is greyed out in
/// a folder without videos, unless it is the current choice and has to be switched back.
fn scope_box(ui: &mut Ui, options: &mut ViewOptions, has_videos: bool) {
    let t = i18n::t();
    let current = Scope::of(options);
    let response = ComboBox::from_id_salt("scope")
        .width(scope_width(ui))
        .selected_text(current.label())
        .show_ui(ui, |ui| {
            ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
            for scope in Scope::all() {
                if scope == Scope::Top(TOP_LEVELS[0]) {
                    ui.separator();
                }
                let choosable =
                    has_videos || scope != Scope::Media(Media::Videos) || scope == current;
                let text = match scope.purpose() {
                    Some(purpose) => format!("{}  ·  {purpose}", scope.label()),
                    None => scope.label(),
                };
                let row = ui
                    .add_enabled(choosable, Button::selectable(scope == current, text))
                    .on_disabled_hover_text(t.media_no_videos);
                if row.clicked() {
                    scope.apply(options);
                }
            }
        })
        .response
        .on_hover_text(t.top_tooltip);
    if options.top.is_some() {
        ui.painter().rect_stroke(
            response.rect,
            ui.visuals().widgets.inactive.corner_radius,
            Stroke::new(1.0, tokens::ACCENT),
            StrokeKind::Inside,
        );
    }
}

/// The first box is as wide as its longest choice, like the sort box.
fn scope_width(ui: &Ui) -> f32 {
    let font = TextStyle::Button.resolve(ui.style());
    let widest = Scope::all()
        .map(|scope| {
            ui.painter()
                .layout_no_wrap(scope.label(), font.clone(), tokens::TEXT)
                .size()
                .x
        })
        .fold(0.0, f32::max);
    let spacing = ui.spacing();
    widest + spacing.icon_spacing + spacing.icon_width + 2.0 * spacing.button_padding.x
}

/// A thin line between groups.
fn separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(5.0, 20.0), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0, tokens::LINE),
    );
}

/// One filter as a chip – rejected (`X`) and no stars (`0`) as symbols, like the keys that set
/// them – or a colour as a square. Whether it was clicked.
fn filter_box(ui: &mut Ui, kind: FilterKind, on: bool) -> bool {
    let t = i18n::t();
    match kind {
        // Colours as small squares: five names would not fit next to the rest.
        FilterKind::Colour(label) => {
            let mut ticked = on;
            colour_box(
                ui,
                &mut ticked,
                theme::label_color(label),
                i18n::label_name(label),
            )
            .changed()
        }
        FilterKind::Rejected => chip(ui, on, Face::Rejected)
            .on_hover_text(format!("{} (X)", kind.label()))
            .clicked(),
        FilterKind::Unrated => chip(ui, on, Face::NoStars)
            .on_hover_text(format!("{} (0)", kind.label()))
            .clicked(),
        FilterKind::Stars(n) => chip(ui, on, Face::Text(&format!("{n}★")))
            .on_hover_text(kind.label())
            .clicked(),
        FilterKind::Blurry => chip(ui, on, Face::Text(&kind.label()))
            .on_hover_text(t.filter_blurry_tooltip)
            .clicked(),
        FilterKind::Duplicate => chip(ui, on, Face::Text(&kind.label()))
            .on_hover_text(t.filter_duplicate_tooltip)
            .clicked(),
    }
}

/// What a chip shows.
enum Face<'a> {
    Text(&'a str),
    /// The reject cross of the filmstrip.
    Rejected,
    /// An empty star: no stars yet.
    NoStars,
}

/// A filter that is on or off: outlined while off, on the accent's fill while on.
fn chip(ui: &mut Ui, on: bool, face: Face<'_>) -> Response {
    const PAD: f32 = 8.0;
    const SYMBOL_WIDTH: f32 = 28.0;
    let galley = match face {
        Face::Text(text) => Some(ui.painter().layout_no_wrap(
            text.to_owned(),
            TextStyle::Button.resolve(ui.style()),
            tokens::TEXT,
        )),
        Face::Rejected | Face::NoStars => None,
    };
    let width = galley
        .as_ref()
        .map_or(SYMBOL_WIDTH, |galley| galley.size().x + 2.0 * PAD);
    let (rect, response) = ui.allocate_exact_size(vec2(width, ITEM_HEIGHT), Sense::click());
    let painter = ui.painter();
    let (fill, line) = if on {
        (tokens::ACCENT_SUBTLE, tokens::ACCENT)
    } else if response.hovered() {
        (tokens::SURFACE_MUTED, tokens::MUTED)
    } else {
        (Color32::TRANSPARENT, tokens::LINE)
    };
    painter.rect(rect, 5.0, fill, Stroke::new(1.0, line), StrokeKind::Inside);
    match (face, galley) {
        (Face::Text(_), Some(galley)) => {
            let at = rect.center() - galley.size() / 2.0;
            painter.galley(at, galley, tokens::TEXT);
        }
        (Face::Rejected, _) => {
            icons::reject_mark(painter, rect.center(), 9.0, tokens::STATUS_ERROR);
        }
        (Face::NoStars, _) => {
            let colour = if on { tokens::TEXT } else { tokens::MUTED };
            stars::paint_star(painter, rect.center(), 6.5, false, colour);
        }
        (Face::Text(_), None) => {}
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// The sort box is as wide as its longest entry in this language, so choosing another sort
/// moves nothing. `ComboBox::width` counts the arrow and the padding too.
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

/// "12 of 340 photos ×" on the accent's subtle fill while a filter is on – what "Action" works
/// on, and the × shows everything again – and a muted "340 photos" otherwise. Its slot is as
/// wide as the longest count the folder can show, so a changing number moves nothing. Whether
/// the × was clicked.
fn count_badge(ui: &mut Ui, shown: usize, total: usize, filtered: bool) -> bool {
    const PAD: f32 = 8.0;
    const CLEAR: f32 = 16.0;
    const GAP: f32 = 6.0;
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
    let (slot, response) = ui.allocate_exact_size(
        vec2(widest + CLEAR + GAP + 2.0 * PAD, ITEM_HEIGHT),
        Sense::hover(),
    );
    let (text, colour) = if filtered {
        ((t.photos_shown)(shown, total), tokens::TEXT)
    } else {
        ((t.photos_count)(total), tokens::MUTED)
    };
    let galley = ui.painter().layout_no_wrap(text, font, colour);
    let size = galley.size();
    // Right-aligned, next to "Action"; the × takes the end while a filter is on.
    let end = if filtered {
        slot.right() - PAD - CLEAR - GAP
    } else {
        slot.right() - PAD
    };
    let at = pos2(end - size.x, slot.center().y - size.y / 2.0);
    let mut clear_clicked = false;
    if filtered {
        let pill = Rect::from_min_max(pos2(at.x - PAD, slot.top()), slot.max);
        ui.painter().rect_filled(pill, 4.0, tokens::ACCENT_SUBTLE);
        let clear_rect = Rect::from_center_size(
            pos2(slot.right() - PAD - CLEAR / 2.0, slot.center().y),
            vec2(CLEAR, CLEAR),
        );
        let clear = ui
            .interact(clear_rect, ui.id().with("clear-filters"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(t.filter_clear);
        if clear.hovered() {
            ui.painter()
                .rect_filled(clear_rect, 3.0, tokens::ACCENT.gamma_multiply(0.35));
        }
        let c = clear_rect.center();
        let stroke = Stroke::new(1.5, tokens::TEXT);
        ui.painter()
            .line_segment([c + vec2(-3.5, -3.5), c + vec2(3.5, 3.5)], stroke);
        ui.painter()
            .line_segment([c + vec2(-3.5, 3.5), c + vec2(3.5, -3.5)], stroke);
        clear_clicked = clear.clicked();
    }
    ui.painter().galley(at, galley, colour);
    response.on_hover_text(t.photos_badge_tooltip);
    clear_clicked
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
fn similar_chip(ui: &mut Ui, on: bool, reference: Option<&str>) -> bool {
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
    chip(ui, on, Face::Text(&label))
        .on_hover_text(tooltip)
        .clicked()
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
    use eframe::egui::{self, Context, RawInput, Shape, pos2};

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
                        has_videos: true,
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

    /// Ticking a filter moves no chip: the groups start where they did, and the count keeps
    /// its slot while its × appears.
    #[test]
    fn chips_stay_when_a_filter_is_ticked() {
        let done = status(1, 1);
        let none = ViewOptions::default();
        let mut some = none;
        some.filter.set(FilterKind::Stars(3), true);
        let (before, action) = bar(none, false, &done);
        let (after, action_after) = bar(some, false, &done);
        for chip in ["1★", "5★", "Blurry", "Duplicates"] {
            assert_eq!(left_of(&before, chip), left_of(&after, chip), "{chip}");
        }
        assert_eq!(action, action_after);
        assert!(left_of(&before, "1★") < left_of(&before, "Blurry"));
    }

    /// The first box's list names every choice on one line, the Top levels with what they
    /// are for – the list may be wider than the box.
    #[test]
    fn the_scope_list_shows_every_choice_on_one_line() {
        let ctx = Context::default();
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1800.0, 900.0));
        let bar = Rect::from_min_size(pos2(0.0, 0.0), vec2(1800.0, TOOLBAR_HEIGHT));
        let done = status(1, 1);
        let mut options = ViewOptions::default();
        let mut combo = None;
        let mut texts: Vec<(String, usize)> = Vec::new();
        for step in 0..8 {
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let events = match (step, combo) {
                (1, Some(at)) => vec![egui::Event::PointerMoved(at)],
                (2, Some(at)) => vec![button(at, true)],
                (3, Some(at)) => vec![button(at, false)],
                _ => vec![],
            };
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(screen),
                    time: Some(f64::from(step) * 0.1),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let info = ToolbarInfo {
                        stale: false,
                        status: &done,
                        actions_open: false,
                        similar_to: None,
                        shown: 340,
                        total: 340,
                        has_videos: true,
                    };
                    toolbar(ui, bar, &mut options, &info);
                },
            );
            output.textures_delta.clear();
            texts.clear();
            for clipped in &output.shapes {
                if let Shape::Text(text) = &clipped.shape {
                    let label = text.galley.text().to_owned();
                    if combo.is_none() && label == Media::All.label() {
                        combo = Some(text.visual_bounding_rect().center());
                    }
                    texts.push((label, text.galley.rows.len()));
                }
            }
        }
        for scope in Scope::all() {
            let wanted = match scope.purpose() {
                Some(purpose) => format!("{}  ·  {purpose}", scope.label()),
                None => scope.label(),
            };
            let rows = texts
                .iter()
                .find(|(text, _)| *text == wanted)
                .map(|(_, rows)| *rows);
            assert_eq!(rows, Some(1), "{wanted:?} in {texts:?}");
        }
    }

    /// The × in the count shows everything again – Top N, similar photos and the boxes.
    #[test]
    fn the_cross_in_the_count_shows_all() {
        let ctx = Context::default();
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1800.0, TOOLBAR_HEIGHT));
        let mut options = ViewOptions {
            top: Some(50),
            similar: true,
            ..ViewOptions::default()
        };
        options.filter.set(FilterKind::Rejected, true);
        let done = status(1, 1);
        let mut cross = None;
        for (step, events) in [vec![], vec![], vec![true], vec![false]]
            .into_iter()
            .enumerate()
        {
            let events = match (cross, events.first()) {
                (Some(at), Some(&pressed)) => vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                (Some(at), None) => vec![egui::Event::PointerMoved(at)],
                (None, _) => vec![],
            };
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(screen),
                    time: Some(step as f64 * 0.1),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let info = ToolbarInfo {
                        stale: false,
                        status: &done,
                        actions_open: false,
                        similar_to: Some("IMG_1.JPG"),
                        shown: 12,
                        total: 340,
                        has_videos: true,
                    };
                    let out = toolbar(ui, screen, &mut options, &info);
                    let action = out.actions_anchor.expect("the Action button is drawn");
                    // The × is the last thing in the count, just left of "Action".
                    cross = Some(pos2(action.left() - 8.0 - 8.0 - 8.0, action.center().y));
                },
            );
            output.textures_delta.clear();
        }
        assert_eq!(options.top, None);
        assert!(!options.similar);
        assert!(options.filter.is_all());
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

    /// Photos, videos or both, or the best N: the first box keeps its width, so no chip moves.
    #[test]
    fn another_scope_moves_no_chip() {
        let done = status(1, 1);
        let lefts: Vec<f32> = Scope::all()
            .map(|scope| {
                let mut options = ViewOptions::default();
                scope.apply(&mut options);
                left_of(&bar(options, false, &done).0, "1★")
            })
            .collect();
        assert!(lefts.windows(2).all(|pair| pair[0] == pair[1]), "{lefts:?}");
        let top = ViewOptions {
            top: Some(250),
            ..ViewOptions::default()
        };
        left_of(&bar(top, false, &done).0, &Scope::Top(250).label());
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
