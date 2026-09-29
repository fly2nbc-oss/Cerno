//! Menus: the burger menu (`Ctrl+K` and the button at the bottom right) and the action menu
//! under the filter bar's "Action" button (`Ctrl+M`). Groups open a submenu – a group inside a
//! group opens another one beside it – everything else is a row; shortcuts sit on the right.
//!
//! Keyboard, always in the deepest open list: ↑/↓ move, Enter runs (or opens a submenu), →
//! opens and ← closes a submenu, a letter jumps to the next row starting with it, Esc closes
//! the deepest submenu first, then the menu.

use eframe::egui::{
    Align2, Area, Color32, Context, CursorIcon, Event, FontId, Id, Key, Order, Rect, Sense, Stroke,
    StrokeKind, pos2, vec2,
};

use eframe::egui::text::{LayoutJob, TextWrapping};

use crate::theme::{text, tokens};
use crate::ui::icons;

const WIDTH: f32 = 340.0;
/// The action menu under its button is narrower.
const ANCHORED_WIDTH: f32 = 240.0;
const ROW_HEIGHT: f32 = 32.0;

/// How a row shows its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// A plain action; running it closes the menu.
    None,
    /// A switch: a box, ticked when on. Clicking keeps the menu open.
    Toggle(bool),
    /// One of several values: a tick on the current one only. Clicking keeps the menu open.
    Choice(bool),
}

pub struct Row<A> {
    pub action: A,
    pub label: String,
    pub shortcut: Option<String>,
    pub mark: Mark,
    /// A colour dot in front of the label (colour labels).
    pub swatch: Option<Color32>,
    /// Why the row can't run right now (greyed out, the reason as its tooltip).
    pub disabled: Option<&'static str>,
}

impl<A> Row<A> {
    pub fn new(action: A, label: impl Into<String>, shortcut: Option<String>) -> Self {
        Self {
            action,
            label: label.into(),
            shortcut,
            mark: Mark::None,
            swatch: None,
            disabled: None,
        }
    }

    pub fn disabled(mut self, reason: Option<&'static str>) -> Self {
        self.disabled = reason;
        self
    }

    pub fn toggle(mut self, on: bool) -> Self {
        self.mark = Mark::Toggle(on);
        self
    }

    pub fn choice(mut self, current: bool) -> Self {
        self.mark = Mark::Choice(current);
        self
    }

    pub fn swatch(mut self, colour: Color32) -> Self {
        self.swatch = Some(colour);
        self
    }
}

pub struct Group<A> {
    pub label: String,
    pub shortcut: Option<String>,
    pub entries: Vec<Entry<A>>,
}

impl<A> Group<A> {
    /// A submenu of rows.
    pub fn new(label: impl Into<String>, shortcut: Option<String>, rows: Vec<Row<A>>) -> Self {
        Self::nested(label, shortcut, rows.into_iter().map(Entry::Row).collect())
    }

    /// A submenu that holds further submenus too.
    pub fn nested(
        label: impl Into<String>,
        shortcut: Option<String>,
        entries: Vec<Entry<A>>,
    ) -> Self {
        Self {
            label: label.into(),
            shortcut,
            entries,
        }
    }
}

pub enum Entry<A> {
    Row(Row<A>),
    Group(Group<A>),
}

impl<A> Entry<A> {
    fn label(&self) -> &str {
        match self {
            Self::Row(row) => &row.label,
            Self::Group(group) => &group.label,
        }
    }

    /// A submenu with this label.
    pub fn is_group(&self, label: &str) -> bool {
        matches!(self, Self::Group(group) if group.label == label)
    }
}

/// The list reached by opening the groups on `path` (one index per level), or `None` when the
/// path no longer leads through groups.
fn level<'a, A>(entries: &'a [Entry<A>], path: &[usize]) -> Option<&'a [Entry<A>]> {
    let mut list = entries;
    for &index in path {
        match list.get(index)? {
            Entry::Group(group) => list = &group.entries,
            Entry::Row(_) => return None,
        }
    }
    Some(list)
}

/// Where the keyboard lands in a list it opens: on the ticked choice, else on the first row
/// that can run (a greyed-out "Show all" heads the filters while nothing is filtered).
fn start_row<A>(entries: &[Entry<A>]) -> Option<usize> {
    entries
        .iter()
        .position(|entry| matches!(entry, Entry::Row(row) if row.mark == Mark::Choice(true)))
        .or_else(|| {
            entries
                .iter()
                .position(|entry| !matches!(entry, Entry::Row(row) if row.disabled.is_some()))
        })
        .or(Some(0))
}

/// Where the menu opens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// Above the menu button at the bottom right of the window.
    BottomRight,
    /// Under a button, right-aligned with it.
    Below(Rect),
}

/// Open submenus and the highlighted row on each level (keyboard or mouse).
#[derive(Debug, Default)]
pub struct State {
    /// The open group on each level, from the menu down: `[2, 0]` is the first entry of the
    /// third one's submenu, opened.
    open: Vec<usize>,
    /// The highlighted row per level – one more than `open`, the deepest list has one too.
    cursors: Vec<Option<usize>>,
}

impl State {
    /// The menu opened down to a submenu: `path` holds the group's index on each level, the
    /// cursor starts on the submenu's first row. A path that does not fit is cut on the first
    /// frame (`fit`).
    pub fn opened(path: Vec<usize>) -> Self {
        let mut cursors: Vec<Option<usize>> = path.iter().map(|&index| Some(index)).collect();
        cursors.push(Some(0));
        Self {
            open: path,
            cursors,
        }
    }

    /// The menu is rebuilt every frame and can shrink while it is open (a deletion ran out,
    /// "Refresh order" went away). The open path is cut where it no longer leads through a
    /// group, and cursors that no longer point at a row are dropped.
    fn fit<A>(&mut self, entries: &[Entry<A>]) {
        let depth = (0..self.open.len())
            .find(|&k| level(entries, &self.open[..=k]).is_none())
            .unwrap_or(self.open.len());
        self.open.truncate(depth);
        self.cursors.resize(depth + 1, None);
        for k in 0..=depth {
            let len = level(entries, &self.open[..k]).map_or(0, <[_]>::len);
            self.cursors[k] = self.cursors[k].filter(|&i| i < len);
        }
    }

    /// Opens the group at `index` on level `k`, closing whatever was open below it.
    fn open_group(&mut self, k: usize, index: usize, start: Option<usize>) {
        self.close_below(k);
        self.cursors[k] = Some(index);
        self.open.push(index);
        self.cursors.push(start);
    }

    /// Closes the submenus below level `k`; the cursor on level `k` stays.
    fn close_below(&mut self, k: usize) {
        self.open.truncate(k);
        self.cursors.truncate(k + 1);
    }
}

pub struct Output<A> {
    pub run: Option<A>,
    pub close: bool,
}

pub fn show<A: Copy>(
    ctx: &Context,
    window: Rect,
    state: &mut State,
    entries: &[Entry<A>],
    placement: Placement,
) -> Output<A> {
    let mut out = Output {
        run: None,
        close: false,
    };
    state.fit(entries);
    keyboard(ctx, state, entries, &mut out);

    let (menu, id) = match placement {
        Placement::BottomRight => {
            let height = entries.len() as f32 * ROW_HEIGHT + 8.0;
            let menu = Rect::from_min_size(
                pos2(
                    window.right() - WIDTH - 12.0,
                    (window.bottom() - height - 12.0).max(window.top() + 8.0),
                ),
                vec2(WIDTH, height.min(window.height() - 24.0)),
            );
            (menu, Id::new("burger"))
        }
        Placement::Below(anchor) => {
            let height = entries.len() as f32 * ROW_HEIGHT + 8.0;
            let left = (anchor.right() - ANCHORED_WIDTH)
                .min(window.right() - ANCHORED_WIDTH - 8.0)
                .max(window.left() + 8.0);
            let menu = Rect::from_min_size(
                pos2(left, anchor.bottom() + 4.0),
                vec2(ANCHORED_WIDTH, height),
            );
            (menu, Id::new("action-menu"))
        }
    };

    Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(window.min)
        .show(ctx, |ui| {
            let backdrop = ui.allocate_rect(window, Sense::click());
            ui.painter()
                .rect_filled(window, 0.0, Color32::from_black_alpha(80));

            // The menu, then each open submenu beside the one it opened from.
            let mut cards = vec![menu];
            let mut list = entries;
            let mut k = 0;
            loop {
                let card = cards[k];
                paint_card(ui.painter(), card);
                let mut next = None;
                for (index, entry) in list.iter().enumerate() {
                    let row = Rect::from_min_size(
                        pos2(
                            card.left() + 4.0,
                            card.top() + 4.0 + index as f32 * ROW_HEIGHT,
                        ),
                        vec2(card.width() - 8.0, ROW_HEIGHT),
                    );
                    if row.bottom() > card.bottom() {
                        break;
                    }
                    let deepest = k == state.open.len();
                    let lit = state.cursors[k] == Some(index)
                        && state.open.get(k).is_none_or(|&o| o == index);
                    match entry {
                        Entry::Row(item) => {
                            let response = row_button(
                                ui,
                                row,
                                id.with(("row", k, index)),
                                RowLook::of(item),
                                lit,
                            );
                            if response.hovered {
                                state.cursors[k] = Some(index);
                            }
                            if response.clicked {
                                run_row(item, &mut out);
                            }
                        }
                        Entry::Group(group) => {
                            let look = RowLook {
                                label: &group.label,
                                shortcut: group.shortcut.as_deref(),
                                mark: Mark::None,
                                swatch: None,
                                submenu: true,
                                disabled: None,
                            };
                            let response =
                                row_button(ui, row, id.with(("group", k, index)), look, lit);
                            if response.hovered && deepest {
                                state.cursors[k] = Some(index);
                            }
                            if response.clicked {
                                if state.open.get(k) == Some(&index) {
                                    state.close_below(k);
                                    state.cursors[k] = Some(index);
                                } else {
                                    state.open_group(k, index, None);
                                }
                            }
                            if state.open.get(k) == Some(&index) {
                                let sub = submenu_rect(window, card, row, group.entries.len());
                                next = Some((sub, group.entries.as_slice()));
                            }
                        }
                    }
                }
                let Some((sub, sub_list)) = next else {
                    break;
                };
                cards.push(sub);
                list = sub_list;
                k += 1;
            }

            if backdrop.clicked() {
                let pos = ui.input(|i| i.pointer.interact_pos());
                let on_card = pos.is_some_and(|p| cards.iter().any(|card| card.contains(p)));
                if !on_card {
                    out.close = true;
                }
            }
        });
    out
}

/// Runs a row: a plain action closes the menu, a switch or a choice keeps it open. A disabled
/// row does nothing; its tooltip says why.
fn run_row<A: Copy>(row: &Row<A>, out: &mut Output<A>) {
    if row.disabled.is_some() {
        return;
    }
    out.run = Some(row.action);
    if row.mark == Mark::None {
        out.close = true;
    }
}

/// Keys without modifiers, in the order they were pressed, taken out of the input so nothing
/// below the menu sees them. They act on the deepest open list. Typed letters jump.
fn keyboard<A: Copy>(ctx: &Context, state: &mut State, entries: &[Entry<A>], out: &mut Output<A>) {
    let (keys, letters) = ctx.input_mut(|i| {
        let mut keys = Vec::new();
        let mut letters = Vec::new();
        i.events.retain(|event| match event {
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if modifiers.is_none()
                && matches!(
                    key,
                    Key::ArrowUp
                        | Key::ArrowDown
                        | Key::ArrowLeft
                        | Key::ArrowRight
                        | Key::Enter
                        | Key::Escape
                ) =>
            {
                keys.push(*key);
                false
            }
            Event::Text(text) => {
                letters.extend(text.chars().filter(|c| c.is_alphanumeric()));
                false
            }
            _ => true,
        });
        (keys, letters)
    });

    for key in keys {
        let depth = state.open.len();
        let Some(list) = level(entries, &state.open) else {
            continue;
        };
        let cursor = state.cursors[depth];
        match key {
            Key::Escape | Key::ArrowLeft if depth > 0 => {
                let parent = state.open[depth - 1];
                state.close_below(depth - 1);
                state.cursors[depth - 1] = Some(parent);
            }
            Key::Escape => out.close = true,
            Key::ArrowDown | Key::ArrowUp => {
                state.cursors[depth] = step(cursor, list.len(), key == Key::ArrowDown);
            }
            Key::Enter | Key::ArrowRight => match cursor.and_then(|i| Some((i, list.get(i)?))) {
                Some((index, Entry::Group(group))) => {
                    state.open_group(depth, index, start_row(&group.entries));
                }
                Some((_, Entry::Row(row))) if key == Key::Enter => run_row(row, out),
                _ => {}
            },
            _ => {}
        }
    }

    for letter in letters {
        let depth = state.open.len();
        if let Some(list) = level(entries, &state.open) {
            let labels: Vec<&str> = list.iter().map(Entry::label).collect();
            if let Some(i) = jump(&labels, state.cursors[depth], letter) {
                state.cursors[depth] = Some(i);
            }
        }
    }
}

/// Next (or previous) row, wrapping; from nothing, Down starts at the top, Up at the bottom.
fn step(cursor: Option<usize>, len: usize, down: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match (cursor, down) {
        (None, true) => 0,
        (None, false) => len - 1,
        (Some(i), true) => (i + 1) % len,
        (Some(i), false) => (i + len - 1) % len,
    })
}

/// The next label after `cursor` that starts with `letter` (case- and accent-insensitive).
fn jump(labels: &[&str], cursor: Option<usize>, letter: char) -> Option<usize> {
    let wanted = crate::library::sort_key(letter);
    let start = cursor.map_or(0, |c| c + 1);
    (0..labels.len())
        .map(|k| (start + k) % labels.len())
        .find(|&i| {
            labels[i]
                .chars()
                .next()
                .is_some_and(|c| crate::library::sort_key(c) == wanted)
        })
}

/// A submenu to the left of the list it opens from – or to its right when the window has no
/// room on the left (a third level on a narrow window) – shifted so a long list stays inside.
fn submenu_rect(window: Rect, parent: Rect, row: Rect, rows: usize) -> Rect {
    let sub_h = rows as f32 * ROW_HEIGHT + 8.0;
    let max_h = (window.height() - 16.0).max(ROW_HEIGHT + 8.0);
    let h = sub_h.min(max_h);
    let y = (row.top() - 4.0)
        .min(window.bottom() - 8.0 - h)
        .max(window.top() + 8.0);
    let left = parent.left() - WIDTH - 4.0;
    let x = if left >= window.left() + 8.0 {
        left
    } else {
        (parent.right() + 4.0)
            .min(window.right() - WIDTH - 8.0)
            .max(window.left() + 8.0)
    };
    Rect::from_min_size(pos2(x, y), vec2(WIDTH, h))
}

fn paint_card(painter: &eframe::egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 8.0, tokens::SURFACE);
    painter.rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0, tokens::LINE),
        StrokeKind::Inside,
    );
}

struct RowLook<'a> {
    label: &'a str,
    shortcut: Option<&'a str>,
    mark: Mark,
    swatch: Option<Color32>,
    submenu: bool,
    disabled: Option<&'static str>,
}

impl<'a> RowLook<'a> {
    fn of<A>(row: &'a Row<A>) -> Self {
        Self {
            label: &row.label,
            shortcut: row.shortcut.as_deref(),
            mark: row.mark,
            swatch: row.swatch,
            submenu: false,
            disabled: row.disabled,
        }
    }
}

struct RowResponse {
    clicked: bool,
    hovered: bool,
}

fn row_button(
    ui: &mut eframe::egui::Ui,
    row: Rect,
    id: Id,
    look: RowLook<'_>,
    lit: bool,
) -> RowResponse {
    let mut response = ui.interact(row, id, Sense::click());
    response = match look.disabled {
        Some(reason) => response.on_hover_text(reason),
        None => response.on_hover_cursor(CursorIcon::PointingHand),
    };
    let text_colour = if look.disabled.is_some() {
        tokens::MUTED
    } else {
        tokens::TEXT
    };
    let painter = ui.painter();
    if response.hovered() || lit {
        painter.rect_filled(row, 5.0, tokens::ACCENT_SUBTLE);
    }
    let y = row.center().y;
    let c = pos2(row.left() + 16.0, y);
    let tick = |colour: Color32| {
        let stroke = Stroke::new(1.6, colour);
        painter.line_segment([c + vec2(-4.5, 0.0), c + vec2(-1.5, 3.5)], stroke);
        painter.line_segment([c + vec2(-1.5, 3.5), c + vec2(4.5, -4.0)], stroke);
    };
    match look.mark {
        Mark::Toggle(on) => {
            let square = Rect::from_center_size(c, vec2(13.0, 13.0));
            painter.rect_stroke(
                square,
                3.0,
                Stroke::new(1.0, if on { tokens::ACCENT } else { tokens::MUTED }),
                StrokeKind::Inside,
            );
            if on {
                tick(tokens::ACCENT_STRONG);
            }
        }
        Mark::Choice(true) => tick(tokens::ACCENT_STRONG),
        Mark::Choice(false) | Mark::None => {}
    }
    let mut x = row.left() + 30.0;
    if let Some(colour) = look.swatch {
        painter.circle_filled(pos2(x + 5.0, y), 5.0, colour);
        x += 16.0;
    }
    let right_reserve = if look.submenu { 22.0 } else { 10.0 };
    let mut label_right = row.right() - right_reserve;
    if let Some(shortcut) = look.shortcut {
        let rect = painter.text(
            pos2(row.right() - right_reserve, y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::proportional(text::SMALL),
            tokens::MUTED,
        );
        label_right = rect.left() - 12.0;
    }
    let mut job = LayoutJob::simple_singleline(
        look.label.to_owned(),
        FontId::proportional(text::BODY),
        text_colour,
    );
    job.wrap = TextWrapping::truncate_at_width((label_right - x).max(20.0));
    let galley = painter.layout_job(job);
    let size = galley.size();
    painter.galley(pos2(x, y - size.y / 2.0), galley, text_colour);
    if look.submenu {
        icons::chevron(painter, pos2(row.right() - 12.0, y), false, tokens::MUTED);
    }
    RowResponse {
        clicked: response.clicked(),
        hovered: response.hovered(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Modifiers, RawInput};

    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn entries() -> Vec<Entry<u8>> {
        vec![
            Entry::Row(Row::new(1, "Ordner öffnen", None)),
            Entry::Group(Group::new(
                "Sortieren",
                None,
                vec![
                    Row::new(2, "Name", None).choice(false),
                    Row::new(3, "Sterne", None).choice(true),
                ],
            )),
            Entry::Row(Row::new(4, "Hilfe", Some("H".into()))),
        ]
    }

    fn frame(state: &mut State, entries: &[Entry<u8>], events: Vec<Event>) -> Output<u8> {
        let ctx = Context::default();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let mut result = None;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(window),
                events,
                ..Default::default()
            },
            |ui| {
                result = Some(show(
                    ui.ctx(),
                    window,
                    state,
                    entries,
                    Placement::BottomRight,
                ))
            },
        );
        output.textures_delta.clear();
        result.expect("menu shown")
    }

    #[test]
    fn escape_closes() {
        let out = frame(&mut State::default(), &entries(), vec![key(Key::Escape)]);
        assert!(out.close);
        assert!(out.run.is_none());
    }

    #[test]
    fn arrows_and_enter_run_a_row() {
        let entries = entries();
        let mut state = State::default();
        // Down to "Ordner öffnen", Up wraps to "Hilfe".
        let out = frame(
            &mut state,
            &entries,
            vec![key(Key::ArrowDown), key(Key::ArrowUp), key(Key::Enter)],
        );
        assert_eq!(out.run, Some(4));
        assert!(out.close);
    }

    #[test]
    fn right_opens_a_submenu_at_the_current_choice() {
        let entries = entries();
        let mut state = State::default();
        let out = frame(
            &mut state,
            &entries,
            vec![
                key(Key::ArrowDown),
                key(Key::ArrowDown),
                key(Key::ArrowRight),
            ],
        );
        assert!(out.run.is_none());
        assert_eq!(state.open, [1]);
        assert_eq!(state.cursors[1], Some(1), "starts on the ticked value");
        // Up to "Name", Enter runs it and the menu stays open (a choice).
        let out = frame(
            &mut state,
            &entries,
            vec![key(Key::ArrowUp), key(Key::Enter)],
        );
        assert_eq!(out.run, Some(2));
        assert!(!out.close);
        // Esc closes the submenu only, a second Esc the menu.
        let out = frame(&mut state, &entries, vec![key(Key::Escape)]);
        assert!(!out.close);
        assert!(state.open.is_empty());
        assert_eq!(state.cursors[0], Some(1), "back on the group");
        let out = frame(&mut state, &entries, vec![key(Key::Escape)]);
        assert!(out.close);
    }

    /// Dieses Foto › Sterne › 3: a submenu inside a submenu.
    #[test]
    fn submenus_nest() {
        let entries = vec![
            Entry::Row(Row::new(1, "Ordner öffnen", None)),
            Entry::Group(Group::nested(
                "Dieses Foto",
                None,
                vec![
                    Entry::Group(Group::new(
                        "Sterne",
                        None,
                        vec![
                            Row::new(10, "Ohne Sterne", Some("0".into())).choice(false),
                            Row::new(13, "3 Sterne", Some("3".into())).choice(true),
                        ],
                    )),
                    Entry::Row(Row::new(20, "Ablehnen", Some("X".into())).toggle(false)),
                ],
            )),
        ];
        let mut state = State::default();
        // Down twice to "Dieses Foto", → opens it on its first entry, → opens "Sterne" on
        // the ticked value.
        frame(
            &mut state,
            &entries,
            vec![
                key(Key::ArrowDown),
                key(Key::ArrowDown),
                key(Key::ArrowRight),
                key(Key::ArrowRight),
            ],
        );
        assert_eq!(state.open, [1, 0]);
        assert_eq!(state.cursors, [Some(1), Some(0), Some(1)]);
        let out = frame(&mut state, &entries, vec![key(Key::Enter)]);
        assert_eq!(out.run, Some(13));
        assert!(!out.close, "a choice keeps the menu open");
        // ← closes only the deepest level; letters then jump inside "Dieses Foto".
        frame(&mut state, &entries, vec![key(Key::ArrowLeft)]);
        assert_eq!(state.open, [1]);
        frame(&mut state, &entries, vec![Event::Text("a".into())]);
        let out = frame(&mut state, &entries, vec![key(Key::Enter)]);
        assert_eq!(out.run, Some(20));
        // Up to "Sterne", → opens it again.
        frame(
            &mut state,
            &entries,
            vec![key(Key::ArrowUp), key(Key::ArrowRight)],
        );
        assert_eq!(state.open, [1, 0]);
        // A menu that lost the nested group while open falls back instead of panicking.
        let flat = vec![Entry::Row(Row::new(1, "Ordner öffnen", None))];
        let out = frame(&mut state, &flat, vec![key(Key::Enter)]);
        assert!(state.open.is_empty());
        assert!(out.run.is_none());
    }

    #[test]
    fn letters_jump() {
        let entries = entries();
        let mut state = State::default();
        let out = frame(&mut state, &entries, vec![Event::Text("h".into())]);
        assert!(out.run.is_none());
        assert_eq!(state.cursors[0], Some(2));
        // "o" matches "Ordner öffnen" regardless of the accent on "ö" further in.
        frame(&mut state, &entries, vec![Event::Text("O".into())]);
        assert_eq!(state.cursors[0], Some(0));
    }

    #[test]
    fn a_menu_that_shrinks_while_open_does_not_panic() {
        let entries = entries();
        let mut state = State::default();
        // Cursor on the last row, then the submenu of the group open.
        frame(&mut state, &entries, vec![key(Key::ArrowUp)]);
        assert_eq!(state.cursors[0], Some(2));
        let short = vec![Entry::Row(Row::new(1, "Ordner öffnen", None))];
        for key_event in [Key::ArrowDown, Key::Enter, Key::ArrowRight, Key::Escape] {
            let mut state = State {
                open: vec![1],
                cursors: vec![Some(2), Some(1)],
            };
            frame(&mut state, &short, vec![key(key_event)]);
            frame(&mut state, &short, vec![Event::Text("x".into())]);
        }
        let out = frame(&mut state, &short, vec![key(Key::Enter)]);
        assert!(out.run.is_none(), "the old cursor points nowhere now");
        assert_eq!(state.cursors[0], None);
    }

    #[test]
    fn a_list_opens_on_its_first_row_that_can_run() {
        let entries: Vec<Entry<u8>> = vec![
            Entry::Row(Row::new(0, "Alle anzeigen", None).disabled(Some("kein Filter"))),
            Entry::Row(Row::new(1, "1★", None).toggle(false)),
            Entry::Row(Row::new(2, "2★", None).toggle(false)),
        ];
        assert_eq!(start_row(&entries), Some(1));
        let choices: Vec<Entry<u8>> = vec![
            Entry::Row(Row::new(0, "Name", None).choice(false)),
            Entry::Row(Row::new(1, "Datum", None).choice(true)),
        ];
        assert_eq!(start_row(&choices), Some(1), "a ticked choice still wins");
    }

    #[test]
    fn a_disabled_row_does_not_run() {
        let entries = vec![
            Entry::Row(Row::new(1, "Verschieben", None).disabled(Some("läuft schon"))),
            Entry::Row(Row::new(2, "Hilfe", None)),
        ];
        let mut state = State::default();
        let out = frame(
            &mut state,
            &entries,
            vec![key(Key::ArrowDown), key(Key::Enter)],
        );
        assert!(out.run.is_none());
        assert!(!out.close, "the menu stays open, the tooltip says why");
        let out = frame(
            &mut state,
            &entries,
            vec![key(Key::ArrowDown), key(Key::Enter)],
        );
        assert_eq!(out.run, Some(2));
    }

    #[test]
    fn step_wraps() {
        assert_eq!(step(None, 3, true), Some(0));
        assert_eq!(step(None, 3, false), Some(2));
        assert_eq!(step(Some(2), 3, true), Some(0));
        assert_eq!(step(Some(0), 3, false), Some(2));
        assert_eq!(step(None, 0, true), None);
    }
}
