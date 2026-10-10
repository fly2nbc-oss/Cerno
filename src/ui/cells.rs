//! One thumbnail cell as the filmstrip and the grid draw it: the picture, the frame of the
//! current (or pinned) photo, the stars or the reject cross below it, the colour stripe, the
//! blurry and duplicate marks, the play button over a video – and one tooltip that names
//! every mark.

use eframe::egui::{
    Align2, Color32, CursorIcon, FontId, Id, Painter, Rect, Response, Sense, Stroke, StrokeKind,
    TextureHandle, Ui, pos2, vec2,
};

use crate::i18n;
use crate::metadata::{Label, Rating};
use crate::theme::{text, tokens};
use crate::ui::icons;
use crate::ui::stars;

/// Room below a cell for its stars.
pub const STARS_ROW: f32 = 22.0;
/// The play button over a video in the filmstrip; the grid's larger cells get a larger one.
const PLAY_RADIUS: f32 = 13.0;

/// What a cell shows about one photo besides its thumbnail.
#[derive(Default)]
pub struct CellInfo {
    /// The current photo: the accent frame.
    pub current: bool,
    pub rating: Rating,
    /// Shown as a warning marker with this explanation.
    pub blurry: Option<String>,
    /// A JPEG that ends inside its image data: a torn page in the corner.
    pub incomplete: bool,
    /// The photo pinned on the left in compare mode.
    pub pinned: bool,
    pub label: Option<Label>,
    /// Set when this photo belongs to a series.
    pub series_id: Option<u32>,
    /// Same series as the current photo.
    pub in_current_series: bool,
    /// Display name of the original when this photo is an exact copy.
    pub duplicate_of: Option<String>,
    /// A video: the play button over the cell.
    pub video: bool,
    /// Deleted (in `.originals`), shown through the 🗑 box: dimmed, with a bin.
    pub deleted: bool,
    /// A RAW + JPG pair: its note for the tooltip, and whether the RAW's marks differ (the
    /// badge then in the warning colour).
    pub pair: Option<(String, bool)>,
}

/// What an empty cell (no thumbnail yet) says about its photo.
#[derive(Clone, Copy, Default)]
pub struct Empty<'a> {
    /// The file name, small, while the thumbnail is missing – not for a video, whose play
    /// button already says what it is.
    pub name: Option<&'a str>,
}

/// Draws the cell into `cell` (the picture's box; the stars go below it, into
/// [`STARS_ROW`]) and returns its click response.
pub fn paint(
    ui: &Ui,
    painter: &Painter,
    cell: Rect,
    id: Id,
    texture: Option<&TextureHandle>,
    empty: Empty<'_>,
    info: &CellInfo,
) -> Response {
    let response = ui
        .interact(cell, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);

    painter.rect_filled(cell, 4.0, tokens::SURFACE_MUTED);
    match texture {
        Some(texture) => {
            let size = texture.size_vec2();
            let scale = (cell.width() / size.x).min(cell.height() / size.y);
            let image_rect = Rect::from_center_size(cell.center(), size * scale);
            painter.image(
                texture.id(),
                image_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        None if info.video => {}
        None => {
            if let Some(name) = empty.name {
                painter.with_clip_rect(cell.shrink(4.0)).text(
                    cell.center(),
                    Align2::CENTER_CENTER,
                    name,
                    FontId::proportional(text::LABEL),
                    tokens::MUTED,
                );
            }
        }
    }

    // Also before the frame has arrived: the button alone already says "video".
    if info.video {
        icons::play(
            painter,
            cell.center(),
            PLAY_RADIUS.max(cell.width().min(cell.height()) * 0.1),
        );
    }

    // Rejects and deleted photos are dimmed – below the selection frame.
    if info.rating == Rating::Rejected || info.deleted {
        painter.rect_filled(cell, 4.0, Color32::from_black_alpha(150));
    }
    if info.current {
        painter.rect_stroke(
            cell,
            4.0,
            Stroke::new(2.0, tokens::ACCENT),
            StrokeKind::Outside,
        );
    } else if info.pinned {
        // Compare mode: the left photo, marked in the neutral text colour with an "L".
        painter.rect_stroke(
            cell,
            4.0,
            Stroke::new(2.0, tokens::TEXT),
            StrokeKind::Outside,
        );
        let badge = Rect::from_min_size(cell.min + vec2(4.0, 4.0), vec2(16.0, 16.0));
        painter.rect_filled(badge, 3.0, tokens::TEXT);
        painter.text(
            badge.center(),
            Align2::CENTER_CENTER,
            i18n::t().compare_left_badge,
            FontId::proportional(text::LABEL),
            tokens::BG,
        );
    } else if response.hovered() {
        painter.rect_stroke(
            cell,
            4.0,
            Stroke::new(1.0, tokens::MUTED),
            StrokeKind::Outside,
        );
    }
    match info.rating {
        Rating::Stars(stars) => stars::paint_mini_rating(
            painter,
            pos2(cell.center().x, cell.bottom() + 11.0),
            stars,
            4.0,
            tokens::ACCENT,
        ),
        Rating::Rejected => {
            icons::reject_mark(
                painter,
                pos2(cell.center().x, cell.bottom() + 11.0),
                8.0,
                tokens::STATUS_ERROR,
            );
        }
        Rating::Unrated => {}
    }
    // Every mark explains itself in one tooltip.
    let mut tooltip = Vec::new();
    if info.video {
        tooltip.push(i18n::t().filmstrip_video.to_owned());
    }
    if info.incomplete {
        // Left of the blurry mark when there is one too.
        let right = if info.blurry.is_some() { 26.0 } else { 4.0 };
        let badge = Rect::from_min_size(
            pos2(cell.right() - right - 18.0, cell.top() + 1.0),
            vec2(18.0, 18.0),
        );
        painter.rect_filled(badge, 3.0, tokens::SURFACE);
        icons::torn_file(painter, badge.center(), tokens::STATUS_WARN);
        tooltip.push(i18n::t().incomplete_fact.to_owned());
    }
    if let Some(reason) = &info.blurry {
        icons::warning(painter, pos2(cell.right() - 10.0, cell.top() + 10.0));
        tooltip.push(reason.clone());
    }
    if info.deleted {
        let badge = Rect::from_min_size(
            pos2(cell.left() + 4.0, cell.bottom() - 24.0),
            vec2(20.0, 20.0),
        );
        painter.rect_filled(badge, 3.0, tokens::SURFACE);
        icons::trash(painter, badge.center(), 0.9, tokens::TEXT);
        tooltip.push(i18n::t().deleted_mark.to_owned());
    }
    if info.rating == Rating::Rejected {
        tooltip.push(i18n::t().rejected.to_owned());
    }
    if let Some(label) = info.label {
        let stripe = Rect::from_min_max(
            pos2(cell.left() + 4.0, cell.bottom() - 4.0),
            pos2(cell.right() - 4.0, cell.bottom() - 1.0),
        );
        painter.rect_filled(stripe, 1.0, crate::theme::label_color(label));
        tooltip.push(i18n::label_name(label).to_owned());
    }
    if let Some((note, differs)) = &info.pair {
        let colour = if *differs {
            tokens::STATUS_WARN
        } else {
            tokens::MUTED
        };
        let tag = painter.layout_no_wrap(
            i18n::t().pair_badge.to_owned(),
            FontId::proportional(text::LABEL),
            colour,
        );
        let size = tag.size() + vec2(8.0, 2.0);
        let badge = Rect::from_min_size(
            pos2(cell.right() - 4.0 - size.x, cell.bottom() - 6.0 - size.y),
            size,
        );
        painter.rect_filled(badge, 3.0, tokens::SURFACE.gamma_multiply(0.9));
        painter.galley(badge.min + vec2(4.0, 1.0), tag, colour);
        tooltip.push(note.clone());
    }
    if let Some(original) = &info.duplicate_of {
        if !info.pinned {
            let badge = Rect::from_min_size(cell.min + vec2(4.0, 4.0), vec2(18.0, 18.0));
            painter.rect_filled(badge, 3.0, tokens::SURFACE);
            icons::copy(painter, badge.center(), tokens::TEXT);
        }
        tooltip.push((i18n::t().duplicate_of)(original));
    }
    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip.join("\n"))
    }
}

/// The current series' line under a cell: muted, not the accent – the accent frame is the
/// current photo alone.
pub fn series_line(painter: &Painter, cell: Rect, y: f32) {
    painter.hline(
        (cell.left() + 8.0)..=(cell.right() - 8.0),
        y,
        Stroke::new(2.0, tokens::MUTED),
    );
}

/// Whether a series ends between two neighbouring photos.
pub fn series_boundary(prev: Option<u32>, next: Option<u32>) -> bool {
    match (prev, next) {
        (Some(a), Some(b)) => a != b,
        (Some(_), None) | (None, Some(_)) => true,
        (None, None) => false,
    }
}
