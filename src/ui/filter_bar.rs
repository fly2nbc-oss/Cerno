//! The filter bar at the top. Left, what is shown: photos, videos or the best N photos, one
//! camera or all, then the filter groups (rating, colour, blurry / duplicates) and "similar
//! photos". Right, what comes of it: the analysis status, the sort, the count with its reset.
//! What acts on the photos shown is the menu bar's *Visible photos* (since 1.11).
//!
//! The filter mode (`Ctrl+F`, `app/filter_mode.rs`) walks the bar with a frame Cerno draws
//! itself – never egui's focus, or `Space` would click a box again: [`Slot`] names each place
//! in the bar's order, [`slots`] lists those there now, [`step`] and [`settle`] move the frame.

use eframe::egui::containers::scroll_area::ScrollBarVisibility;
use eframe::egui::{
    Align, Button, Color32, ComboBox, CursorIcon, Layout, Painter, Rect, Response, RichText,
    ScrollArea, Sense, Sides, Stroke, StrokeKind, TextStyle, TextWrapMode, Ui, UiBuilder, pos2,
    vec2,
};

use crate::analysis::{ModelState, Status};
use crate::i18n;
use crate::theme::{self, radius, tokens};
use crate::ui::{icons, stars};
use crate::view::{CameraEntry, FilterKind, Media, Scope, SortKey, TOP_LEVELS, ViewOptions};

pub const TOOLBAR_HEIGHT: f32 = 40.0;
/// Height of the boxes, chips and the count, so they line up.
const ITEM_HEIGHT: f32 = 24.0;

pub struct ToolbarInfo<'a> {
    /// New scores arrived since the view was sorted/filtered.
    pub stale: bool,
    pub status: &'a Status,
    /// Name of the photo "similar photos" is about, while that filter is on.
    pub similar_to: Option<&'a str>,
    /// How many photos the view shows, and how many the folder has.
    pub shown: usize,
    pub total: usize,
    /// The folder holds at least one video (else "videos only" has nothing to show).
    pub has_videos: bool,
    /// The folder has deleted photos (else the 🗑 box is greyed out).
    pub has_deleted: bool,
    /// The pasted file-name list while it filters: found and asked for.
    pub name_list: Option<(usize, usize)>,
    /// The folder's cameras for the camera box, the most photos first (`camera_entries`).
    pub cameras: &'a [CameraEntry],
    /// The camera the view is narrowed to, by name, while the camera filter is on.
    pub camera: Option<&'a str>,
}

/// One place of the filter mode's frame, in the bar's reading order: the first box, the
/// camera box, "without ✕", every filter box, "similar photos", the file list's chip (while a
/// list filters), *Refresh order* (while it shows) and the sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Scope,
    Camera,
    WithoutRejected,
    Filter(FilterKind),
    Similar,
    NameList,
    Refresh,
    Sort,
}

/// The slots the bar shows now, left to right. `name_list`: the file list's chip is there (a
/// list filters); `stale`: *Refresh order* is.
pub fn slots(name_list: bool, stale: bool) -> Vec<Slot> {
    let mut out = vec![Slot::Scope, Slot::Camera, Slot::WithoutRejected];
    out.extend(
        FilterKind::GROUPS
            .iter()
            .flat_map(|group| group.iter().map(|&kind| Slot::Filter(kind))),
    );
    out.push(Slot::Similar);
    if name_list {
        out.push(Slot::NameList);
    }
    if stale {
        out.push(Slot::Refresh);
    }
    out.push(Slot::Sort);
    out
}

/// Where the frame goes with an arrow or `Home` / `End`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Prev,
    Next,
    First,
    Last,
}

/// The frame's next place: one box on or back, stopping at the bar's ends (like `←`/`→` at the
/// first and last photo), or the first or last box.
pub fn step(slots: &[Slot], cursor: Slot, step: Step) -> Slot {
    let Some(&last) = slots.last() else {
        return cursor;
    };
    let here = settle(slots, cursor);
    let at = slots.iter().position(|&slot| slot == here).unwrap_or(0);
    match step {
        Step::Prev => slots[at.saturating_sub(1)],
        Step::Next => slots[(at + 1).min(slots.len() - 1)],
        Step::First => slots[0],
        Step::Last => last,
    }
}

/// The frame stays where it is, or – when its box has gone (the file list ended, the order
/// was refreshed) – goes to the box before it.
pub fn settle(slots: &[Slot], cursor: Slot) -> Slot {
    if slots.contains(&cursor) {
        return cursor;
    }
    let all = self::slots(true, true);
    let rank = |slot: Slot| all.iter().position(|&s| s == slot).unwrap_or(0);
    slots
        .iter()
        .rev()
        .find(|&&slot| rank(slot) < rank(cursor))
        .or(slots.first())
        .copied()
        .unwrap_or(cursor)
}

/// The filter mode's frame: on which box, and whether it just moved there (the boxes then
/// scroll so it shows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub slot: Slot,
    pub scroll: bool,
}

#[derive(Default)]
pub struct ToolbarOutput {
    pub options_changed: bool,
    pub refresh: bool,
    pub download_model: bool,
    /// The "similar" chip was clicked; the app picks the photo it is about (like `M`).
    pub toggle_similar: bool,
    /// The box a click went to: in the filter mode the frame goes there.
    pub clicked: Option<Slot>,
    /// Where the frame's box is: a list opened from the keyboard opens below it.
    pub cursor_rect: Option<Rect>,
    /// Where the count is: it holds the right end (tests).
    #[cfg(test)]
    pub count: Option<Rect>,
}

/// What the slots of one side of the bar reported: a click, the frame's box.
struct Places {
    cursor: Option<Cursor>,
    clicked: Option<Slot>,
    cursor_rect: Option<Rect>,
}

impl Places {
    fn new(cursor: Option<Cursor>) -> Self {
        Self {
            cursor,
            clicked: None,
            cursor_rect: None,
        }
    }

    /// `slot` was drawn as `response`: a click is noted, and the frame is drawn around it when
    /// it is the frame's – painted only, so nothing changes its width (and scrolled into view
    /// when it just moved there).
    fn place(&mut self, ui: &Ui, slot: Slot, response: &Response) {
        if response.clicked() {
            self.clicked = Some(slot);
        }
        let Some(cursor) = self.cursor.filter(|c| c.slot == slot) else {
            return;
        };
        let rect = response.rect;
        // The boxes scroll sideways: the frame reaches a little past their visible part.
        let painter = ui.painter().with_clip_rect(ui.clip_rect().expand(4.0));
        painter.rect_stroke(
            rect.expand(1.0),
            radius::MD + 1,
            Stroke::new(2.0, tokens::ACCENT),
            StrokeKind::Outside,
        );
        // At once: a key's step is no reason to animate, and the frame must not lag behind.
        if cursor.scroll {
            ui.scroll_to_rect_animation(
                rect.expand(8.0),
                None,
                eframe::egui::style::ScrollAnimation::none(),
            );
        }
        self.cursor_rect = Some(rect);
    }

    fn merge(&mut self, other: Places) {
        self.clicked = self.clicked.or(other.clicked);
        self.cursor_rect = self.cursor_rect.or(other.cursor_rect);
    }
}

/// The filter bar; `cursor`: the filter mode's frame while the mode is on.
pub fn toolbar(
    ui: &mut Ui,
    rect: Rect,
    options: &mut ViewOptions,
    info: &ToolbarInfo<'_>,
    cursor: Option<Cursor>,
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
    let mut count_rect = None;
    let mut similar_clicked = false;
    // The right side works on copies: both sides can't borrow `options` at once.
    let mut sort = options.sort;
    let mut clear = false;
    let mut left = Places::new(cursor);
    let mut right = Places::new(cursor);
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(12.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            // Combo boxes and buttons in a bar are compact, as high as the chips.
            ui.spacing_mut().interact_size.y = theme::COMPACT;
            // One line. The groups scroll sideways when they no longer fit; the first box and
            // the whole right side never do. Nothing changes its width while it is used, so
            // nothing moves away under the pointer.
            Sides::new()
                .height(ui.available_height())
                .shrink_left()
                .show(
                    ui,
                    |ui| {
                        let scope = scope_box(ui, options, info.has_videos);
                        left.place(ui, Slot::Scope, &scope);
                        let camera = camera_box(ui, options, info.cameras, info.camera);
                        left.place(ui, Slot::Camera, &camera);
                        separator(ui);
                        let boxes = ScrollArea::horizontal()
                            .id_salt("filter-boxes")
                            .max_width(ui.available_width())
                            .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    // It hides instead of picking out: before the boxes, on
                                    // its own.
                                    let without = without_rejected(ui, options.hide_rejected);
                                    left.place(ui, Slot::WithoutRejected, &without);
                                    if without.clicked() {
                                        options.toggle_hide_rejected();
                                    }
                                    separator(ui);
                                    for (index, group) in FilterKind::GROUPS.iter().enumerate() {
                                        if index > 0 {
                                            separator(ui);
                                        }
                                        for &kind in *group {
                                            let on = options.filter.contains(kind);
                                            let response =
                                                filter_box(ui, kind, on, info.has_deleted);
                                            left.place(ui, Slot::Filter(kind), &response);
                                            if response.clicked() {
                                                options.toggle_filter(kind);
                                            }
                                        }
                                    }
                                    separator(ui);
                                    // Last, so its label – the photo's name while on – moves
                                    // nothing.
                                    let similar =
                                        similar_chip(ui, options.similar, info.similar_to);
                                    left.place(ui, Slot::Similar, &similar);
                                    similar_clicked = similar.clicked();
                                    // While a pasted list filters: how much of it was found; a
                                    // click shows every photo again.
                                    if let Some((found, total)) =
                                        info.name_list.filter(|_| options.name_list)
                                    {
                                        let list = chip(
                                            ui,
                                            true,
                                            Face::Text(&(t.name_list_chip)(found, total)),
                                        )
                                        .on_hover_text(t.name_list_chip_tooltip);
                                        left.place(ui, Slot::NameList, &list);
                                        if list.clicked() {
                                            options.name_list = false;
                                        }
                                    }
                                });
                            });
                        overflow_hint(
                            ui.painter(),
                            boxes.inner_rect,
                            boxes.content_size.x,
                            boxes.state.clamped_offset().x,
                        );
                    },
                    |ui| {
                        // Right to left: the count first, so it stays at the right edge, then
                        // the sort. What comes and goes appears left of the sort: "Refresh
                        // order", the analysis progress, a missing, loading or failed model.
                        let (cleared, slot) =
                            count_badge(ui, info.shown, info.total, before.is_filtered());
                        clear = cleared;
                        count_rect = Some(slot);
                        let sorted = ComboBox::from_id_salt("sort")
                            .icon(combo_chevron)
                            .width(sort_width(ui))
                            .selected_text((t.sort)(sort.label()))
                            .show_ui(ui, |ui| {
                                for key in SortKey::ALL {
                                    ui.selectable_value(&mut sort, key, key.label());
                                }
                            })
                            .response;
                        right.place(ui, Slot::Sort, &sorted);
                        if info.stale {
                            let refresh = ui
                                .add(theme::secondary_button(t.refresh_order).compact())
                                .on_hover_text(t.refresh_order_tooltip);
                            right.place(ui, Slot::Refresh, &refresh);
                            if refresh.clicked() {
                                out.refresh = true;
                            }
                        }
                        let Status { done, total, .. } = *info.status;
                        if done < total {
                            progress(ui, done, total);
                        }
                        aesthetics_status(ui, info.status, &mut out);
                    },
                );
        },
    );
    options.sort = sort;
    if clear {
        options.clear_filters();
    }
    #[cfg(test)]
    {
        out.count = count_rect;
    }
    #[cfg(not(test))]
    let _ = count_rect;
    out.toggle_similar = similar_clicked;
    out.options_changed = *options != before;
    left.merge(right);
    out.clicked = left.clicked;
    out.cursor_rect = left.cursor_rect;
    out
}

/// Photos, videos or both – or the best N photos (`Scope`). As wide as its longest choice in
/// this language, and outlined in the accent while Top N is on. "Videos only" is greyed out in
/// a folder without videos, unless it is the current choice and has to be switched back.
fn scope_box(ui: &mut Ui, options: &mut ViewOptions, has_videos: bool) -> Response {
    let t = i18n::t();
    let current = Scope::of(options);
    let response = ComboBox::from_id_salt("scope")
        .icon(combo_chevron)
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
    response
}

/// Only the photos of one camera: "All cameras", then the folder's cameras with their photo
/// counts, a model with two bodies followed by each body (`camera_entries`). Its width is fixed
/// in the language: the cameras arrive while the analysis reads them, and a box that grew with
/// them would move the chips under the pointer – a longer name is cut short, whole in the
/// tooltip. Greyed out with the reason while the folder has one camera or none known yet,
/// unless it filters and has to be switched back; outlined in the accent while it filters.
fn camera_box(
    ui: &mut Ui,
    options: &mut ViewOptions,
    cameras: &[CameraEntry],
    chosen: Option<&str>,
) -> Response {
    /// Indent of a body below its model in the list.
    const BODY_INDENT: f32 = 12.0;
    let t = i18n::t();
    let width = camera_width(ui);
    let label = chosen
        .filter(|_| options.camera.is_some())
        .unwrap_or(t.camera_all);
    let spacing = ui.spacing();
    let room = width - spacing.icon_spacing - spacing.icon_width - 2.0 * spacing.button_padding.x;
    let shown = elide(ui, label, room);
    let enabled = cameras.len() > 1 || options.camera.is_some();
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ComboBox::from_id_salt("camera")
                .icon(combo_chevron)
                .width(width)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
                    let all = Button::selectable(options.camera.is_none(), t.camera_all);
                    if ui.add(all).clicked() {
                        options.camera = None;
                    }
                    ui.separator();
                    for entry in cameras {
                        let text = format!(
                            "{}  ·  {}",
                            entry.label(),
                            (t.camera_time_photos)(entry.count)
                        );
                        let row = Button::selectable(options.camera == Some(entry.pick), text);
                        let clicked = ui
                            .horizontal(|ui| {
                                if entry.pick.serial.is_some() {
                                    ui.add_space(BODY_INDENT);
                                }
                                ui.add(row).clicked()
                            })
                            .inner;
                        if clicked {
                            options.camera = Some(entry.pick);
                        }
                    }
                })
                .response
        })
        .inner;
    let reason = if cameras.is_empty() {
        t.camera_none
    } else {
        t.camera_one
    };
    let response = response
        .on_disabled_hover_text(reason)
        .on_hover_text(format!("{label}\n{}", t.camera_tooltip));
    if options.camera.is_some() {
        ui.painter().rect_stroke(
            response.rect,
            ui.visuals().widgets.inactive.corner_radius,
            Stroke::new(1.0, tokens::ACCENT),
            StrokeKind::Inside,
        );
    }
    response
}

/// The camera box is as wide as "All cameras" in this language, or as a typical camera with a
/// body's digits – whichever is wider.
fn camera_width(ui: &Ui) -> f32 {
    /// A typical camera name with a body's last digits.
    const SAMPLE: &str = "ILCE-7RM5 ·4821";
    let font = TextStyle::Button.resolve(ui.style());
    let widest = [i18n::t().camera_all, SAMPLE]
        .into_iter()
        .map(|text| {
            ui.painter()
                .layout_no_wrap(text.to_owned(), font.clone(), tokens::TEXT)
                .size()
                .x
        })
        .fold(0.0, f32::max);
    let spacing = ui.spacing();
    widest + spacing.icon_spacing + spacing.icon_width + 2.0 * spacing.button_padding.x
}

/// `text`, or as much of it as fits in `room` with "…" at the end.
fn elide(ui: &Ui, text: &str, room: f32) -> String {
    let font = TextStyle::Button.resolve(ui.style());
    let width = |text: String| {
        ui.painter()
            .layout_no_wrap(text, font.clone(), tokens::TEXT)
            .size()
            .x
    };
    if width(text.to_owned()) <= room {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len())
        .rev()
        .map(|keep| format!("{}…", chars[..keep].iter().collect::<String>().trim_end()))
        .find(|short| width(short.clone()) <= room)
        .unwrap_or_else(|| "…".to_owned())
}

/// The combo boxes' chevron: Lucide's, like the menu bar's, instead of egui's triangle.
fn combo_chevron(ui: &Ui, rect: Rect, _: &eframe::egui::style::WidgetVisuals, _open: bool) {
    icons::chevron(ui.painter(), rect.center(), true, tokens::MUTED);
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
/// them – or a colour as a square. The 🗑 box is greyed out while the folder has no deleted
/// photos, unless it is on and has to be switched off.
fn filter_box(ui: &mut Ui, kind: FilterKind, on: bool, has_deleted: bool) -> Response {
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
        }
        FilterKind::Rejected => {
            chip(ui, on, Face::Rejected).on_hover_text(format!("{} (X)", kind.label()))
        }
        FilterKind::Deleted => {
            let enabled = has_deleted || on;
            let tooltip = if enabled {
                t.filter_deleted_tooltip
            } else {
                t.filter_deleted_none
            };
            chip_with(ui, on, Face::Deleted, enabled)
                .on_hover_text(format!("{}\n{tooltip}", kind.label()))
        }
        FilterKind::Unrated => {
            chip(ui, on, Face::NoStars).on_hover_text(format!("{} (0)", kind.label()))
        }
        FilterKind::Stars(n) => {
            chip(ui, on, Face::Text(&format!("{n}★"))).on_hover_text(kind.label())
        }
        FilterKind::Blurry => {
            chip(ui, on, Face::Text(&kind.label())).on_hover_text(t.filter_blurry_tooltip)
        }
        FilterKind::Duplicate => {
            chip(ui, on, Face::Text(&kind.label())).on_hover_text(t.filter_duplicate_tooltip)
        }
        FilterKind::Incomplete => chip(ui, on, Face::Incomplete).on_hover_text(format!(
            "{}\n{}",
            kind.label(),
            t.filter_incomplete_tooltip
        )),
        // Icons without text: the names are in the tooltip.
        FilterKind::People | FilterKind::NoPeople => chip(
            ui,
            on,
            Face::Person {
                crossed: kind == FilterKind::NoPeople,
            },
        )
        .on_hover_text(format!("{}\n{}", kind.label(), t.filter_people_tooltip)),
    }
}

/// "without ✕": the word and the reject cross.
fn without_rejected(ui: &mut Ui, on: bool) -> Response {
    let t = i18n::t();
    chip(ui, on, Face::Without(t.filter_without)).on_hover_text(format!(
        "{}\n{}",
        t.filter_hide_rejected, t.filter_hide_rejected_tooltip
    ))
}

/// What a chip shows.
enum Face<'a> {
    Text(&'a str),
    /// The reject cross of the filmstrip.
    Rejected,
    /// An empty star: no stars yet.
    NoStars,
    /// A person: with people, or struck through: without.
    Person {
        crossed: bool,
    },
    /// A waste bin: the deleted photos.
    Deleted,
    /// A torn page: JPEGs that end inside their image data.
    Incomplete,
    /// A word and the reject cross: "without ✕".
    Without(&'a str),
}

/// A filter that is on or off: outlined while off, on the accent's fill while on.
fn chip(ui: &mut Ui, on: bool, face: Face<'_>) -> Response {
    chip_with(ui, on, face, true)
}

/// A chip that can be greyed out: then it only explains itself, it takes no click.
fn chip_with(ui: &mut Ui, on: bool, face: Face<'_>, enabled: bool) -> Response {
    const PAD: f32 = 8.0;
    const SYMBOL_WIDTH: f32 = 28.0;
    // The cross after the word of "without ✕", and the gap before it.
    const MARK: f32 = 9.0;
    const MARK_GAP: f32 = 5.0;
    let galley = match face {
        Face::Text(text) | Face::Without(text) => Some(ui.painter().layout_no_wrap(
            text.to_owned(),
            TextStyle::Button.resolve(ui.style()),
            tokens::TEXT,
        )),
        Face::Rejected | Face::NoStars | Face::Person { .. } | Face::Deleted | Face::Incomplete => {
            None
        }
    };
    let mark = if matches!(face, Face::Without(_)) {
        MARK_GAP + MARK
    } else {
        0.0
    };
    let width = galley
        .as_ref()
        .map_or(SYMBOL_WIDTH, |galley| galley.size().x + mark + 2.0 * PAD);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, ITEM_HEIGHT), sense);
    let painter = ui.painter();
    let (fill, line) = if on {
        (tokens::ACCENT_SUBTLE, tokens::ACCENT)
    } else if response.hovered() && enabled {
        (tokens::HOVER, tokens::LINE)
    } else {
        (Color32::TRANSPARENT, tokens::LINE)
    };
    painter.rect(
        rect,
        radius::MD,
        fill,
        Stroke::new(1.0, line),
        StrokeKind::Inside,
    );
    match (face, galley) {
        (Face::Text(_), Some(galley)) => {
            let at = rect.center() - galley.size() / 2.0;
            painter.galley(at, galley, tokens::TEXT);
        }
        (Face::Without(_), Some(galley)) => {
            let at = pos2(rect.left() + PAD, rect.center().y - galley.size().y / 2.0);
            let cross = pos2(rect.right() - PAD - MARK / 2.0, rect.center().y);
            painter.galley(at, galley, tokens::TEXT);
            icons::reject_mark(painter, cross, MARK, tokens::STATUS_ERROR);
        }
        (Face::Rejected, _) => {
            icons::reject_mark(painter, rect.center(), 9.0, tokens::STATUS_ERROR);
        }
        (Face::NoStars, _) => {
            let colour = if on { tokens::TEXT } else { tokens::MUTED };
            stars::paint_star(painter, rect.center(), 6.5, false, colour);
        }
        (Face::Person { crossed }, _) => {
            let colour = if on { tokens::TEXT } else { tokens::MUTED };
            icons::person(painter, rect.center(), colour, crossed);
        }
        (Face::Deleted, _) => {
            let colour = match (on, enabled) {
                (true, _) => tokens::TEXT,
                (false, true) => tokens::MUTED,
                (false, false) => tokens::LINE,
            };
            icons::trash(painter, rect.center(), 1.0, colour);
        }
        (Face::Incomplete, _) => {
            let colour = if on { tokens::TEXT } else { tokens::MUTED };
            icons::torn_file(painter, rect.center(), colour);
        }
        (Face::Text(_) | Face::Without(_), None) => {}
    }
    if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
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

/// "12 of 340 photos ×" on the accent's subtle fill while a filter is on – what *Visible
/// photos* in the menu bar works on, and the × shows everything again – and a muted "340
/// photos" otherwise. Its slot is as wide as the longest count the folder can show, so a
/// changing number moves nothing. Whether the × was clicked, and the slot.
fn count_badge(ui: &mut Ui, shown: usize, total: usize, filtered: bool) -> (bool, Rect) {
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
    // Right-aligned at the bar's end; the × takes the end while a filter is on.
    let end = if filtered {
        slot.right() - PAD - CLEAR - GAP
    } else {
        slot.right() - PAD
    };
    let at = pos2(end - size.x, slot.center().y - size.y / 2.0);
    let mut clear_clicked = false;
    if filtered {
        let pill = Rect::from_min_max(pos2(at.x - PAD, slot.top()), slot.max);
        ui.painter()
            .rect_filled(pill, radius::MD, tokens::ACCENT_SUBTLE);
        let clear_rect = Rect::from_center_size(
            pos2(slot.right() - PAD - CLEAR / 2.0, slot.center().y),
            vec2(CLEAR, CLEAR),
        );
        let clear = ui
            .interact(
                clear_rect,
                ui.scope_id().with("clear-filters"),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(t.filter_clear);
        if clear.hovered() {
            ui.painter().rect_filled(
                clear_rect,
                radius::SM,
                tokens::ACCENT_STRONG.gamma_multiply(0.3),
            );
        }
        icons::close(
            ui.painter(),
            clear_rect.center(),
            icons::INLINE - 1.0,
            tokens::TEXT,
        );
        clear_clicked = clear.clicked();
    }
    ui.painter().galley(at, galley, colour);
    response.on_hover_text(t.photos_badge_tooltip);
    (clear_clicked, slot)
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
/// On a click the app decides which photo it is about.
fn similar_chip(ui: &mut Ui, on: bool, reference: Option<&str>) -> Response {
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
    chip(ui, on, Face::Text(&label)).on_hover_text(tooltip)
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
        painter.rect_filled(square, radius::SM, colour);
        icons::check(painter, square.center(), icons::INLINE - 3.0, tokens::BG);
    } else {
        painter.rect_stroke(
            square,
            radius::SM,
            Stroke::new(1.0, colour),
            StrokeKind::Inside,
        );
    }
    if response.hovered() {
        painter.rect_stroke(
            square.expand(2.0),
            radius::MD,
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

/// A download running, else a button for what is missing (CLIP and V2.5 together, or V2.5
/// alone), else how the CLIP model loads.
fn aesthetics_status(ui: &mut Ui, status: &Status, out: &mut ToolbarOutput) {
    let t = i18n::t();
    let muted = |text: String| RichText::new(text).color(tokens::MUTED);
    if let Some((received, total)) = status.downloading() {
        let percent = received as f64 / total.max(1) as f64 * 100.0;
        ui.label(muted((t.downloading_model)(percent)));
        return;
    }
    let missing = status.missing();
    if !missing.is_empty() {
        let (label, tooltip) = crate::ui::models::download_label(&missing);
        if ui
            .add(theme::secondary_button(label).compact())
            .on_hover_text(tooltip)
            .clicked()
        {
            out.download_model = true;
        }
        return;
    }
    match &status.aesthetics {
        ModelState::Loading => {
            ui.label(muted(t.aesthetics_loading.into()));
        }
        ModelState::Failed(message) => {
            ui.label(RichText::new(t.aesthetics_failed).color(tokens::STATUS_ERROR))
                .on_hover_text(message);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::TasteStatus;
    use eframe::egui::{self, RawInput, Shape, pos2};

    fn status(done: usize, total: usize) -> Status {
        Status {
            done,
            total,
            aesthetics: ModelState::Available,
            v25: ModelState::Missing,
            faces: ModelState::Missing,
            taste: TasteStatus {
                examples: 0,
                sources: Default::default(),
                model: None,
            },
        }
    }

    /// The bar drawn in a wide window: where each text lands, and the count.
    fn bar(options: ViewOptions, stale: bool, status: &Status) -> (Vec<(String, Rect)>, Rect) {
        bar_about(options, stale, status, None, (340, 340))
    }

    /// The same with "similar photos" about `similar_to` and `(shown, total)` photos.
    fn bar_about(
        options: ViewOptions,
        stale: bool,
        status: &Status,
        similar_to: Option<&str>,
        counts: (usize, usize),
    ) -> (Vec<(String, Rect)>, Rect) {
        bar_with(options, stale, status, similar_to, counts, &[], None)
    }

    /// The same with the folder's cameras and the name of the one chosen.
    fn bar_with(
        options: ViewOptions,
        stale: bool,
        status: &Status,
        similar_to: Option<&str>,
        (shown, total): (usize, usize),
        cameras: &[CameraEntry],
        camera: Option<&str>,
    ) -> (Vec<(String, Rect)>, Rect) {
        let ctx = crate::theme::test_context();
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1800.0, TOOLBAR_HEIGHT));
        let mut last = None;
        // Twice: the side-scrolling boxes know their size from the frame before.
        for _ in 0..2 {
            let mut count = None;
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
                        similar_to,
                        shown,
                        total,
                        has_videos: true,
                        has_deleted: false,
                        name_list: None,
                        cameras,
                        camera,
                    };
                    count = toolbar(ui, screen, &mut options, &info, None).count;
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
            last = Some((texts, count.expect("the count is drawn")));
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
        let (before, count) = bar(none, false, &done);
        let (after, count_after) = bar(some, false, &done);
        for chip in ["1★", "5★", "Blurry", "Duplicates"] {
            assert_eq!(left_of(&before, chip), left_of(&after, chip), "{chip}");
        }
        assert_eq!(count, count_after);
        assert!(left_of(&before, "1★") < left_of(&before, "Blurry"));
    }

    /// The first box's list names every choice on one line, the Top levels with what they
    /// are for – the list may be wider than the box.
    #[test]
    fn the_scope_list_shows_every_choice_on_one_line() {
        let ctx = crate::theme::test_context();
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
                        similar_to: None,
                        shown: 340,
                        total: 340,
                        has_videos: true,
                        has_deleted: false,
                        name_list: None,
                        cameras: &[],
                        camera: None,
                    };
                    toolbar(ui, bar, &mut options, &info, None);
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
        let ctx = crate::theme::test_context();
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
                        similar_to: Some("IMG_1.JPG"),
                        shown: 12,
                        total: 340,
                        has_videos: true,
                        has_deleted: false,
                        name_list: None,
                        cameras: &[],
                        camera: None,
                    };
                    let out = toolbar(ui, screen, &mut options, &info, None);
                    let count = out.count.expect("the count is drawn");
                    // The × is the last thing in the count, at its right end.
                    cross = Some(pos2(count.right() - 8.0 - 8.0, count.center().y));
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

    fn camera(model: u64, serial: Option<u64>, name: &str, suffix: Option<&str>) -> CameraEntry {
        CameraEntry {
            pick: crate::view::CameraPick { model, serial },
            name: name.to_owned(),
            suffix: suffix.map(str::to_owned),
            count: 12,
        }
    }

    /// Choosing a camera – a short name, a long one cut short, a body – moves no chip: the
    /// camera box keeps its width whatever it shows and however many cameras the folder has.
    #[test]
    fn another_camera_moves_no_chip() {
        let done = status(1, 1);
        let cameras = [
            camera(1, None, "ILCE-7M4", None),
            camera(1, Some(2), "ILCE-7M4", Some("4821")),
            camera(1, Some(3), "ILCE-7M4", Some("3021")),
            camera(4, None, "Canon EOS R5 Mark II with a very long name", None),
        ];
        let left = |pick: Option<usize>, cameras: &[CameraEntry]| {
            let options = ViewOptions {
                camera: pick.map(|i| cameras[i].pick),
                ..ViewOptions::default()
            };
            let chosen = pick.map(|i| cameras[i].label());
            let (texts, _) = bar_with(
                options,
                false,
                &done,
                None,
                (12, 340),
                cameras,
                chosen.as_deref(),
            );
            (left_of(&texts, "1★"), texts)
        };
        let (all, texts) = left(None, &cameras);
        left_of(&texts, i18n::t().camera_all);
        let (body, texts) = left(Some(1), &cameras);
        left_of(&texts, "ILCE-7M4 ·4821");
        let (long, texts) = left(Some(3), &cameras);
        assert!(
            texts
                .iter()
                .any(|(text, _)| text.starts_with("Canon") && text.ends_with('…')),
            "a long name is cut short: {texts:?}"
        );
        let (none, _) = left(None, &[]);
        assert_eq!([body, long, none], [all; 3]);
    }

    /// In a folder with one camera, or none known yet, the camera box is greyed out: a click
    /// opens no list.
    #[test]
    fn one_camera_greys_the_camera_box_out() {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1800.0, 900.0));
        let bar = Rect::from_min_size(pos2(0.0, 0.0), vec2(1800.0, TOOLBAR_HEIGHT));
        let done = status(1, 1);
        let opened = |cameras: &[CameraEntry]| {
            let ctx = crate::theme::test_context();
            let mut options = ViewOptions::default();
            let mut combo = None;
            let mut listed = false;
            for step in 0..6 {
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
                            similar_to: None,
                            shown: 340,
                            total: 340,
                            has_videos: true,
                            has_deleted: false,
                            name_list: None,
                            cameras,
                            camera: None,
                        };
                        toolbar(ui, bar, &mut options, &info, None);
                    },
                );
                output.textures_delta.clear();
                for clipped in &output.shapes {
                    if let Shape::Text(text) = &clipped.shape {
                        let label = text.galley.text();
                        if combo.is_none() && label == i18n::t().camera_all {
                            combo = Some(text.visual_bounding_rect().center());
                        }
                        listed |= label.starts_with("Pixel 7a");
                    }
                }
            }
            listed
        };
        let one = [camera(1, None, "Pixel 7a", None)];
        let two = [one[0].clone(), camera(2, None, "moto g42", None)];
        assert!(!opened(&one));
        assert!(!opened(&[]));
        assert!(opened(&two));
    }

    /// The count stays at the right edge while the progress and "Refresh order" come and go,
    /// and the analysis ticking up moves nothing either.
    #[test]
    fn the_count_stays_at_the_right_edge() {
        let options = ViewOptions::default();
        let (_, quiet) = bar(options, false, &status(1200, 1200));
        let (early, busy) = bar(options, true, &status(5, 1200));
        let (late, _) = bar(options, true, &status(999, 1200));
        assert_eq!(quiet, busy);
        let refresh = i18n::t().refresh_order;
        assert_eq!(left_of(&early, refresh), left_of(&late, refresh));
    }

    /// The count at the right end: all photos without a filter, "shown of all" with one;
    /// neither the number nor the filter moves it or the progress.
    #[test]
    fn the_count_names_what_the_photos_shown_are() {
        let done = status(5, 340);
        let none = ViewOptions::default();
        let mut some = none;
        some.filter.set(FilterKind::Stars(3), true);
        let (all, count) = bar_about(none, false, &done, None, (340, 340));
        let (few, count_few) = bar_about(some, false, &done, None, (12, 340));
        let (one, count_one) = bar_about(some, false, &done, None, (1, 340));
        let t = i18n::t();
        assert!(all.iter().any(|(text, _)| *text == (t.photos_count)(340)));
        assert!(
            few.iter()
                .any(|(text, _)| *text == (t.photos_shown)(12, 340))
        );
        assert_eq!(count, count_few);
        assert_eq!(count, count_one);
        let progress = (t.analyzing_progress)(5, 340);
        assert_eq!(left_of(&few, &progress), left_of(&one, &progress));
        assert_eq!(left_of(&all, &progress), left_of(&few, &progress));
        assert!(left_of(&few, &progress) < count.left());
    }

    /// Every box of the bar in its reading order: the file list's chip and *Refresh order*
    /// only while they show, the sort last.
    #[test]
    fn the_slots_follow_the_bar() {
        let plain = slots(false, false);
        assert_eq!(
            plain[..3],
            [Slot::Scope, Slot::Camera, Slot::WithoutRejected]
        );
        assert_eq!(plain[3], Slot::Filter(FilterKind::Rejected));
        assert_eq!(plain.len(), 3 + FilterKind::ALL.len() + 2);
        assert_eq!(plain[plain.len() - 2..], [Slot::Similar, Slot::Sort]);
        let all = slots(true, true);
        assert_eq!(
            all[all.len() - 4..],
            [Slot::Similar, Slot::NameList, Slot::Refresh, Slot::Sort]
        );
    }

    /// The frame stops at the bar's ends, and `Home` / `End` reach them.
    #[test]
    fn the_frame_stops_at_the_ends() {
        let all = slots(false, false);
        assert_eq!(step(&all, Slot::Scope, Step::Prev), Slot::Scope);
        assert_eq!(step(&all, Slot::Scope, Step::Next), Slot::Camera);
        assert_eq!(step(&all, Slot::Sort, Step::Next), Slot::Sort);
        assert_eq!(step(&all, Slot::Similar, Step::Next), Slot::Sort);
        assert_eq!(step(&all, Slot::Camera, Step::Last), Slot::Sort);
        assert_eq!(step(&all, Slot::Sort, Step::First), Slot::Scope);
        assert_eq!(
            step(&all, Slot::Filter(FilterKind::Stars(5)), Step::Next),
            Slot::Filter(FilterKind::Colour(crate::metadata::Label::Red))
        );
    }

    /// A box that goes away (the order refreshed, the list ended) leaves the frame on the box
    /// before it; from there the arrows go on as usual.
    #[test]
    fn a_box_that_goes_takes_the_frame_back_one() {
        let now = slots(false, false);
        assert_eq!(settle(&now, Slot::Refresh), Slot::Similar);
        assert_eq!(settle(&now, Slot::NameList), Slot::Similar);
        assert_eq!(settle(&now, Slot::Sort), Slot::Sort);
        assert_eq!(step(&now, Slot::Refresh, Step::Next), Slot::Sort);
        assert_eq!(
            step(&now, Slot::NameList, Step::Prev),
            Slot::Filter(FilterKind::NoPeople)
        );
    }

    /// The bar drawn `frames` times in a window `width` wide with the filter mode's frame on
    /// `cursor`: the texts, the frame's box, and every ring the accent draws 2 px wide.
    fn framed(
        width: f32,
        cursor: Option<Cursor>,
        frames: usize,
    ) -> (Vec<(String, Rect)>, Option<Rect>, Vec<Rect>) {
        let ctx = crate::theme::test_context();
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(width, TOOLBAR_HEIGHT));
        let done = status(1, 1);
        let mut last = None;
        for _ in 0..frames {
            let mut at = None;
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| {
                    let mut options = ViewOptions::default();
                    let info = ToolbarInfo {
                        stale: false,
                        status: &done,
                        similar_to: None,
                        shown: 340,
                        total: 340,
                        has_videos: true,
                        has_deleted: false,
                        name_list: None,
                        cameras: &[],
                        camera: None,
                    };
                    at = toolbar(ui, screen, &mut options, &info, cursor).cursor_rect;
                },
            );
            output.textures_delta.clear();
            let mut texts = Vec::new();
            let mut rings = Vec::new();
            for clipped in &output.shapes {
                match &clipped.shape {
                    Shape::Text(text) => {
                        texts.push((text.galley.text().to_owned(), text.visual_bounding_rect()));
                    }
                    Shape::Rect(rect)
                        if rect.stroke.width == 2.0 && rect.stroke.color == tokens::ACCENT =>
                    {
                        rings.push(rect.rect);
                    }
                    _ => {}
                }
            }
            last = Some((texts, at, rings));
        }
        last.expect("frames")
    }

    /// The frame is painted around its box and moves no box: every chip stays where it was
    /// without it.
    #[test]
    fn the_frame_moves_no_box() {
        let four = Cursor {
            slot: Slot::Filter(FilterKind::Stars(4)),
            scroll: false,
        };
        let (plain, none, no_rings) = framed(1800.0, None, 2);
        let (with, at, rings) = framed(1800.0, Some(four), 2);
        assert_eq!(none, None);
        assert!(no_rings.is_empty());
        for chip in ["1★", "4★", "5★", "Blurry"] {
            assert_eq!(left_of(&plain, chip), left_of(&with, chip), "{chip}");
        }
        let at = at.expect("the frame's box");
        assert_eq!(rings.len(), 1, "{rings:?}");
        assert!(rings[0].contains_rect(at), "{rings:?} around {at:?}");
        let four_star = with.iter().find(|(text, _)| text == "4★").map(|(_, r)| *r);
        assert!(four_star.is_some_and(|r| at.contains_rect(r)));
        // The combo boxes and the sort take the frame too.
        for slot in [Slot::Scope, Slot::Camera, Slot::Sort] {
            let cursor = Cursor {
                slot,
                scroll: false,
            };
            assert!(framed(1800.0, Some(cursor), 2).1.is_some(), "{slot:?}");
        }
    }

    /// In a narrow window the boxes scroll: a frame moved to a box out of sight brings it into
    /// view.
    #[test]
    fn the_frame_scrolls_its_box_into_view() {
        let width = 720.0;
        let last = Slot::Filter(FilterKind::NoPeople);
        let still = Cursor {
            slot: last,
            scroll: false,
        };
        let (_, hidden, _) = framed(width, Some(still), 3);
        assert!(hidden.expect("drawn").right() > width, "out of sight first");
        let moved = Cursor {
            slot: last,
            scroll: true,
        };
        let (_, shown, _) = framed(width, Some(moved), 4);
        assert!(shown.expect("drawn").right() < width, "{shown:?}");
    }
}
