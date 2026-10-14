//! A card over a dimmed window, shared by the confirmations and the models page. It is an
//! `egui::Area` in `Order::Foreground`, so nothing below reacts while it is open; its height is
//! the content height remembered from the previous frame.

use eframe::egui::{
    Align, Area, Context, CursorIcon, Id, Layout, Order, Painter, Pos2, Rect, Sense, Ui, UiBuilder,
    pos2, vec2,
};

use crate::theme::{self, radius, tokens};
use crate::ui::icons;

const PAD: f32 = 24.0;

/// What happened on the card this frame, besides whatever `add` reports.
pub struct Modal<R> {
    pub inner: R,
    /// A click beside the card.
    pub outside: bool,
}

/// Draws the card centred in `window`, `width` wide, with `add` laying out its content.
pub fn show<R>(
    ctx: &Context,
    window: Rect,
    id: Id,
    width: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> Modal<R> {
    Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(window.min)
        .show(ctx, |ui| {
            let backdrop = ui.allocate_rect(window, Sense::click());
            ui.painter().rect_filled(window, 0.0, tokens::SCRIM);
            let height_id = id.with("height");
            let max_height = (window.height() - 48.0).max(120.0);
            let height = ui
                .data(|d| d.get_temp::<f32>(height_id))
                .unwrap_or(160.0)
                .min(max_height - 2.0 * PAD);
            let card = Rect::from_center_size(
                window.center(),
                vec2(
                    width.min(window.width() - 32.0).max(200.0),
                    height + 2.0 * PAD,
                ),
            );
            // Swallows clicks on the card, so only the backdrop counts as "outside".
            ui.interact(card, id.with("card"), Sense::click());
            theme::paint_card(ui.painter(), card, radius::XL);
            let mut inner = ui.new_child(
                UiBuilder::new()
                    .max_rect(card.shrink(PAD))
                    .layout(Layout::top_down(Align::Min))
                    .id_salt(id.with("content")),
            );
            let result = add(&mut inner);
            let used = inner.min_rect().height();
            if (used - height).abs() > 0.5 {
                ui.data_mut(|d| d.insert_temp(height_id, used));
                ui.ctx().request_repaint();
            }
            let pointer = ui.input(|i| i.pointer.interact_pos());
            Modal {
                inner: result,
                outside: backdrop.clicked() && !pointer.is_some_and(|p| card.contains(p)),
            }
        })
        .inner
}

/// The × in a card's top right corner; `true` when clicked.
pub fn close_button(ui: &Ui, top_right: Pos2, tooltip: &str) -> bool {
    let area = Rect::from_min_size(pos2(top_right.x - 28.0, top_right.y), vec2(28.0, 28.0));
    let response = ui
        .interact(area, ui.id().with("modal-close"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let painter: &Painter = ui.painter();
    icons::button_background(painter, area, response.hovered(), false);
    let color = if response.hovered() {
        tokens::ACCENT_STRONG
    } else {
        tokens::MUTED
    };
    icons::close(painter, area.center(), icons::BAR, color);
    response.on_hover_text(tooltip).clicked()
}
