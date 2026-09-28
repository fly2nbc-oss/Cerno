//! Burger menu (`Ctrl+K` and the button at the bottom right). No search field: groups with
//! several values open a submenu, everything else is a row. Shortcuts sit on the right.

use eframe::egui::{
    Align2, Area, Color32, Context, CursorIcon, FontId, Id, Key, Modifiers, Order, Rect, Sense,
    Stroke, StrokeKind, pos2, vec2,
};

use crate::theme::tokens;
use crate::ui::icons;

const WIDTH: f32 = 340.0;
const ROW_HEIGHT: f32 = 32.0;

pub struct Row<A> {
    pub action: A,
    pub label: String,
    pub shortcut: Option<String>,
    /// `Some` marks a toggle or the current choice. Clicking it leaves the menu open.
    pub checked: Option<bool>,
}

impl<A> Row<A> {
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

pub struct Group<A> {
    pub label: String,
    pub shortcut: Option<String>,
    pub rows: Vec<Row<A>>,
}

impl<A> Group<A> {
    pub fn new(label: impl Into<String>, shortcut: Option<String>, rows: Vec<Row<A>>) -> Self {
        Self {
            label: label.into(),
            shortcut,
            rows,
        }
    }
}

pub enum Entry<A> {
    Row(Row<A>),
    Group(Group<A>),
}

/// Which submenu is open.
#[derive(Debug, Default)]
pub struct State {
    open: Option<usize>,
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
) -> Output<A> {
    let mut out = Output {
        run: None,
        close: false,
    };
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        out.close = true;
    }

    let height = entries.len() as f32 * ROW_HEIGHT + 8.0;
    let menu = Rect::from_min_size(
        pos2(
            window.right() - WIDTH - 12.0,
            (window.bottom() - height - 12.0).max(window.top() + 8.0),
        ),
        vec2(WIDTH, height.min(window.height() - 24.0)),
    );

    Area::new(Id::new("burger"))
        .order(Order::Foreground)
        .fixed_pos(window.min)
        .show(ctx, |ui| {
            let backdrop = ui.allocate_rect(window, Sense::click());
            ui.painter()
                .rect_filled(window, 0.0, Color32::from_black_alpha(80));
            paint_card(ui.painter(), menu);

            let mut submenu: Option<(usize, Rect)> = None;
            for (index, entry) in entries.iter().enumerate() {
                let row = Rect::from_min_size(
                    pos2(
                        menu.left() + 4.0,
                        menu.top() + 4.0 + index as f32 * ROW_HEIGHT,
                    ),
                    vec2(menu.width() - 8.0, ROW_HEIGHT),
                );
                match entry {
                    Entry::Row(item) => {
                        if row_button(
                            ui,
                            row,
                            Id::new(("menu-row", index)),
                            &item.label,
                            item.shortcut.as_deref(),
                            item.checked,
                            false,
                        ) && let Some(action) = clicked_action(item)
                        {
                            out.run = Some(action);
                            if item.checked.is_none() {
                                out.close = true;
                            }
                        }
                    }
                    Entry::Group(group) => {
                        let open = state.open == Some(index);
                        if row_button(
                            ui,
                            row,
                            Id::new(("menu-group", index)),
                            &group.label,
                            group.shortcut.as_deref(),
                            None,
                            true,
                        ) {
                            state.open = if open { None } else { Some(index) };
                        }
                        if state.open == Some(index) {
                            submenu =
                                Some((index, submenu_rect(window, menu, row, group.rows.len())));
                        }
                    }
                }
            }

            if let Some((index, sub)) = submenu
                && let Entry::Group(group) = &entries[index]
            {
                paint_card(ui.painter(), sub);
                for (j, item) in group.rows.iter().enumerate() {
                    let row = Rect::from_min_size(
                        pos2(sub.left() + 4.0, sub.top() + 4.0 + j as f32 * ROW_HEIGHT),
                        vec2(sub.width() - 8.0, ROW_HEIGHT),
                    );
                    if row.bottom() > sub.bottom() {
                        break;
                    }
                    if row_button(
                        ui,
                        row,
                        Id::new(("menu-sub", index, j)),
                        &item.label,
                        item.shortcut.as_deref(),
                        item.checked,
                        false,
                    ) && let Some(action) = clicked_action(item)
                    {
                        out.run = Some(action);
                        if item.checked.is_none() {
                            out.close = true;
                        }
                    }
                }
            }

            if backdrop.clicked() {
                let pos = ui.input(|i| i.pointer.interact_pos());
                let on_card = pos.is_some_and(|p| {
                    menu.contains(p) || submenu.is_some_and(|(_, rect)| rect.contains(p))
                });
                if !on_card {
                    out.close = true;
                }
            }
        });
    out
}

/// Submenu to the left of its group, shifted so a long list stays inside the window.
fn submenu_rect(window: Rect, menu: Rect, row: Rect, rows: usize) -> Rect {
    let sub_h = rows as f32 * ROW_HEIGHT + 8.0;
    let max_h = (window.height() - 16.0).max(ROW_HEIGHT + 8.0);
    let h = sub_h.min(max_h);
    let y = (row.top() - 4.0)
        .min(window.bottom() - 8.0 - h)
        .max(window.top() + 8.0);
    Rect::from_min_size(pos2(menu.left() - WIDTH - 4.0, y), vec2(WIDTH, h))
}

fn clicked_action<A: Copy>(item: &Row<A>) -> Option<A> {
    Some(item.action)
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

fn row_button(
    ui: &mut eframe::egui::Ui,
    row: Rect,
    id: Id,
    label: &str,
    shortcut: Option<&str>,
    checked: Option<bool>,
    submenu: bool,
) -> bool {
    let response = ui
        .interact(row, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(row, 5.0, tokens::ACCENT_SUBTLE);
    }
    let y = row.center().y;
    if checked == Some(true) {
        let c = pos2(row.left() + 16.0, y);
        let stroke = Stroke::new(1.6, tokens::ACCENT_STRONG);
        painter.line_segment([c + vec2(-4.5, 0.0), c + vec2(-1.5, 3.5)], stroke);
        painter.line_segment([c + vec2(-1.5, 3.5), c + vec2(4.5, -4.0)], stroke);
    }
    painter.text(
        pos2(row.left() + 30.0, y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.5),
        tokens::TEXT,
    );
    let right_reserve = if submenu { 22.0 } else { 10.0 };
    if let Some(shortcut) = shortcut {
        painter.text(
            pos2(row.right() - right_reserve, y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::proportional(12.0),
            tokens::MUTED,
        );
    }
    if submenu {
        icons::chevron(painter, pos2(row.right() - 12.0, y), false, tokens::MUTED);
    }
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, RawInput};

    #[test]
    fn escape_closes() {
        let ctx = Context::default();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let entries = [Entry::Row(Row::new(1u8, "Vollbild", Some("F11".into())))];
        let mut state = State::default();
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(window),
                events: vec![eframe::egui::Event::Key {
                    key: Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                let out = show(ui.ctx(), window, &mut state, &entries);
                assert!(out.close);
                assert!(out.run.is_none());
            },
        );
        output.textures_delta.clear();
    }
}
