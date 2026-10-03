//! Help page: every shortcut with a short explanation. `H`/`F1` shows it over the photos
//! (and over the start screen). The start screen is a small card: one sentence, "Open folder"
//! and the five keys to begin with.

use eframe::egui::{
    Align2, Area, Color32, Context, CursorIcon, FontId, Id, Order, Painter, Pos2, Rect, ScrollArea,
    Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2,
};

use crate::i18n::{self, HelpRow};
use crate::theme::{self, text, tokens};
use crate::ui::icons;

const MAX_WIDTH: f32 = 1340.0;
/// The start screen is a small card: one sentence, the button and five keys.
const WELCOME_WIDTH: f32 = 560.0;
const PAD: f32 = 28.0;
/// Two columns of shortcuts from this content width on, three from the next.
const TWO_COLUMNS: f32 = 700.0;
const THREE_COLUMNS: f32 = 1100.0;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HelpOutput {
    pub close: bool,
    pub open_folder: bool,
    pub language: bool,
}

/// Modal page over the whole window; a click beside the card closes it.
pub fn overlay(ctx: &Context, window: Rect) -> HelpOutput {
    Area::new(Id::new("help"))
        .order(Order::Foreground)
        .fixed_pos(window.min)
        .show(ctx, |ui| {
            let backdrop = ui.allocate_rect(window, Sense::click());
            ui.painter()
                .rect_filled(window, 0.0, Color32::from_black_alpha(170));
            let mut out = card_with_content(ui, window, false);
            out.close |= backdrop.clicked();
            out
        })
        .inner
}

/// The start screen: the help content with the "Open folder" button, centred in `rect`.
pub fn welcome(ui: &mut Ui, rect: Rect) -> HelpOutput {
    card_with_content(ui, rect, true)
}

/// The card, centred in `space`: as high as its content was last frame, at most the space.
fn card_with_content(ui: &mut Ui, space: Rect, welcome: bool) -> HelpOutput {
    const MARGIN_Y: f32 = 20.0;
    let height_id = Id::new(("help-height", welcome));
    let max_height = (space.height() - 48.0).max(200.0);
    let height = ui
        .data(|d| d.get_temp::<f32>(height_id))
        .map_or(max_height, |content| {
            (content + 2.0 * MARGIN_Y).min(max_height)
        });
    let card = Rect::from_center_size(
        space.center(),
        vec2(
            (space.width() - 48.0).clamp(200.0, if welcome { WELCOME_WIDTH } else { MAX_WIDTH }),
            height,
        ),
    );
    // Swallows clicks on the card, so only the backdrop closes the page.
    ui.interact(card, Id::new(("help-card", welcome)), Sense::click());
    let painter = ui.painter();
    painter.rect_filled(card, 10.0, tokens::SURFACE);
    painter.rect_stroke(
        card,
        10.0,
        Stroke::new(1.0, tokens::LINE),
        StrokeKind::Inside,
    );
    let mut inner = ui.new_child(
        UiBuilder::new()
            .max_rect(card.shrink2(vec2(PAD, MARGIN_Y)))
            .id_salt(("help", welcome)),
    );
    // Not `content_size`: without auto-shrinking it is at least the visible area.
    let (out, content_height) = ScrollArea::vertical()
        .auto_shrink(false)
        .show(&mut inner, |ui| content(ui, welcome))
        .inner;
    if ui.data(|d| d.get_temp::<f32>(height_id)) != Some(content_height) {
        ui.data_mut(|d| d.insert_temp(height_id, content_height));
        ui.ctx().request_repaint();
    }
    out
}

/// Draws the page; returns what was clicked and the height it needs.
fn content(ui: &mut Ui, welcome: bool) -> (HelpOutput, f32) {
    let t = i18n::t();
    let mut out = HelpOutput::default();
    let top = ui.cursor().min;
    let width = ui.available_width();
    let (left, right) = (top.x, top.x + width);
    let painter = ui.painter().clone();
    let mut y = top.y + 4.0;

    // Header: "Cerno 0.7.0 · Help", language flag and (over the photos) a close button.
    let title = painter.layout_no_wrap(
        "Cerno".into(),
        FontId::proportional(text::TITLE),
        tokens::TEXT,
    );
    let title_height = title.size().y;
    let title_width = title.size().x;
    painter.galley(pos2(left, y), title, tokens::TEXT);
    let subtitle = if welcome {
        env!("CARGO_PKG_VERSION").to_owned()
    } else {
        format!("{}   ·   {}", env!("CARGO_PKG_VERSION"), t.help_title)
    };
    painter.text(
        pos2(left + title_width + 10.0, y + title_height - 6.0),
        Align2::LEFT_BOTTOM,
        subtitle,
        FontId::proportional(text::BODY),
        tokens::MUTED,
    );
    let mut x = right;
    if !welcome {
        let area = Rect::from_min_size(pos2(x - 28.0, y), vec2(28.0, 28.0));
        x -= 34.0;
        if header_button(ui, area, t.help_close, |p, c, color| {
            let d = 5.5;
            for (a, b) in [(vec2(-d, -d), vec2(d, d)), (vec2(-d, d), vec2(d, -d))] {
                p.line_segment([c + a, c + b], Stroke::new(1.6, color));
            }
        }) {
            out.close = true;
        }
    }
    // The language as a quiet word (the flag only shows while switching).
    let name = painter.layout_no_wrap(
        i18n::current().name().to_owned(),
        FontId::proportional(text::BODY),
        tokens::MUTED,
    );
    let area = Rect::from_min_size(
        pos2(x - name.size().x - 16.0, y),
        vec2(name.size().x + 16.0, 28.0),
    );
    let tooltip = format!(
        "{} ({})",
        (t.button_language)(i18n::current().name()),
        i18n::with_ctrl("L")
    );
    if header_button(ui, area, &tooltip, |p, c, color| {
        p.text(
            c,
            Align2::CENTER_CENTER,
            i18n::current().name(),
            FontId::proportional(text::BODY),
            color,
        );
    }) {
        out.language = true;
    }
    y += title_height + 12.0;

    // Intro.
    let intro = painter.layout(
        i18n::keep_together(if welcome {
            t.welcome_intro
        } else {
            t.help_intro
        }),
        FontId::proportional(text::BODY),
        tokens::TEXT,
        width.min(760.0),
    );
    let intro_height = intro.size().y;
    painter.galley(pos2(left, y), intro, tokens::TEXT);
    y += intro_height + 18.0;

    if welcome {
        let button = Rect::from_center_size(pos2(left + width / 2.0, y + 19.0), vec2(200.0, 38.0));
        out.open_folder = ui
            .put(button, theme::primary_button(t.open_folder))
            .clicked();
        y += 46.0;
        painter.text(
            pos2(left + width / 2.0, y),
            Align2::CENTER_TOP,
            t.help_drop,
            FontId::proportional(text::BODY),
            tokens::MUTED,
        );
        y += 34.0;
        // Only the keys to begin with; H shows the rest.
        y = section(&painter, left, y, width, "", &t.welcome_keys) + 10.0;
        painter.text(
            pos2(left + width / 2.0, y),
            Align2::CENTER_TOP,
            t.welcome_more,
            FontId::proportional(text::BODY),
            tokens::MUTED,
        );
        y += 24.0;
        ui.allocate_space(vec2(width, y - top.y));
        return (out, y - top.y);
    }

    // Shortcuts in reading order, split into columns of about the same length: three on wide
    // windows (so the page needs no scrolling), else two – left what culling needs (browse,
    // rate, sort out, video), right the view, the panels, editing and the rest.
    let sections: [(&str, &[HelpRow]); 8] = [
        (t.help_sections[0], &t.help_browse),
        (t.help_sections[1], &t.help_rate),
        (t.help_sections[2], &t.help_cull),
        (t.help_sections[3], &t.help_video),
        (t.help_sections[4], &t.help_view),
        (t.help_sections[5], &t.help_panels),
        (t.help_sections[6], &t.help_edit),
        (t.help_sections[7], &t.help_more),
    ];
    let columns: Vec<&[(&str, &[HelpRow])]> = if width >= THREE_COLUMNS {
        vec![&sections[..2], &sections[2..5], &sections[5..]]
    } else if width >= TWO_COLUMNS {
        vec![&sections[..4], &sections[4..]]
    } else {
        vec![&sections[..]]
    };
    let gap = 36.0;
    let column_width = (width - gap * (columns.len() - 1) as f32) / columns.len() as f32;
    let mut bottom = y;
    for (i, column) in columns.iter().enumerate() {
        let x = left + i as f32 * (column_width + gap);
        let mut cy = y;
        for (title, rows) in column.iter() {
            cy = section(&painter, x, cy, column_width, title, rows) + 14.0;
        }
        bottom = bottom.max(cy);
    }
    y = bottom;

    if !welcome {
        painter.text(
            pos2(left + width / 2.0, y + 4.0),
            Align2::CENTER_TOP,
            t.help_close,
            FontId::proportional(text::SMALL),
            tokens::MUTED,
        );
        y += 24.0;
    }
    ui.allocate_space(vec2(width, y - top.y));
    (out, y - top.y)
}

/// Section title and its rows; returns the bottom.
fn section(painter: &Painter, x: f32, y: f32, width: f32, title: &str, rows: &[HelpRow]) -> f32 {
    let mut y = y;
    if !title.is_empty() {
        painter.text(
            pos2(x, y),
            Align2::LEFT_TOP,
            title.to_uppercase(),
            FontId::proportional(text::LABEL),
            tokens::MUTED,
        );
        y += 22.0;
    }
    let keys_width = (width * 0.38).min(190.0);
    for (keys, action) in rows {
        let caps_height = keycaps(painter, pos2(x, y), keys_width, keys);
        let description = painter.layout(
            i18n::keep_together(action),
            FontId::proportional(text::BODY),
            tokens::TEXT,
            width - keys_width - 12.0,
        );
        let description_height = description.size().y;
        painter.galley(
            pos2(x + keys_width + 12.0, y + 3.0),
            description,
            tokens::TEXT,
        );
        y += caps_height.max(description_height + 3.0) + 7.0;
    }
    y
}

/// Keys of one row as key caps, wrapping within `max_width`; returns the height used.
fn keycaps(painter: &Painter, origin: Pos2, max_width: f32, keys: &str) -> f32 {
    const GAP: f32 = 5.0;
    let (mut x, mut y) = (origin.x, origin.y);
    let mut line_height: f32 = 0.0;
    for key in keys.split(", ") {
        let galley = painter.layout_no_wrap(
            key.to_owned(),
            FontId::proportional(text::SMALL),
            tokens::TEXT,
        );
        let size = galley.size() + vec2(14.0, 6.0);
        if x > origin.x && x + size.x > origin.x + max_width {
            x = origin.x;
            y += line_height + GAP;
        }
        let cap = Rect::from_min_size(pos2(x, y), size);
        painter.rect_filled(cap, 4.0, tokens::SURFACE_MUTED);
        painter.rect_stroke(cap, 4.0, Stroke::new(1.0, tokens::LINE), StrokeKind::Inside);
        painter.galley(cap.min + vec2(7.0, 3.0), galley, tokens::TEXT);
        x += size.x + GAP;
        line_height = line_height.max(size.y);
    }
    y + line_height - origin.y
}

fn header_button(
    ui: &Ui,
    area: Rect,
    tooltip: &str,
    paint: impl FnOnce(&Painter, Pos2, Color32),
) -> bool {
    let response = ui
        .interact(area, ui.id().with(("help-button", tooltip)), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    icons::button_background(painter, area, response.hovered(), false);
    let color = if response.hovered() {
        tokens::ACCENT_STRONG
    } else {
        tokens::MUTED
    };
    paint(painter, area.center(), color);
    response.on_hover_text(tooltip).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::RawInput;

    /// The start screen is a small card that fits a 1280 × 720 window without scrolling.
    #[test]
    fn start_screen_fits_a_small_window() {
        let ctx = Context::default();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 720.0));
        // The card takes the content height of the previous frame.
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(window),
                    ..Default::default()
                },
                |ui| {
                    welcome(ui, window);
                },
            );
            output.textures_delta.clear();
        }
        let content = ctx
            .data(|d| d.get_temp::<f32>(Id::new(("help-height", true))))
            .expect("content height");
        assert!(
            content + 40.0 <= window.height() - 48.0,
            "start screen needs {content} px"
        );
    }
}
