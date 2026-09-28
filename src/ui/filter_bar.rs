//! The filter bar at the top: sort, filters, analysis status and the "Action" button.

use eframe::egui::containers::scroll_area::ScrollBarVisibility;
use eframe::egui::{
    Align, Button, Color32, ComboBox, CursorIcon, Layout, Painter, Rect, Response, RichText,
    ScrollArea, Sense, Sides, Stroke, StrokeKind, Ui, UiBuilder, vec2,
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
}

#[derive(Default)]
pub struct ToolbarOutput {
    pub options_changed: bool,
    pub refresh: bool,
    pub download_model: bool,
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
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(12.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            // One line: sort and the filter boxes on the left, action and analysis status on
            // the right. The boxes scroll sideways when they no longer fit.
            Sides::new()
                .height(ui.available_height())
                .shrink_left()
                .show(
                    ui,
                    |ui| {
                        ComboBox::from_id_salt("sort")
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
                                    if !options.filter.is_all()
                                        && ui.small_button(t.filter_clear).clicked()
                                    {
                                        options.filter.clear();
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
                        // Only what needs attention: the model is missing, loading or failed, or
                        // the analysis is still running. Where the model runs is on the models
                        // card.
                        aesthetics_status(ui, &info.status.aesthetics, &mut out);
                        let Status { done, total, .. } = *info.status;
                        if done < total {
                            let text = (t.analyzing_progress)(done, total);
                            ui.label(RichText::new(text).color(tokens::MUTED));
                        }
                        if info.stale
                            && ui
                                .button(t.refresh_order)
                                .on_hover_text(t.refresh_order_tooltip)
                                .clicked()
                        {
                            out.refresh = true;
                        }
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
                    },
                );
        },
    );
    out.actions_anchor = action_rect;
    out.options_changed = *options != before;
    out
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
