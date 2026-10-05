//! The grid (`F7`): every photo of the view as a thumbnail cell. The cursor is the current
//! photo, so marks, deleting and moving on work as in the single view. Only the rows on screen
//! are drawn.

use std::ops::Range;
use std::path::PathBuf;

use eframe::egui::{Id, Rect, ScrollArea, Stroke, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::library;
use crate::theme::tokens;
use crate::thumbs::Thumbs;
use crate::ui::cells::{self, CellInfo, Empty};

/// Cell sizes in physical pixels, smallest first. The largest is the thumbnails' own size:
/// bigger cells would only show them softer.
pub const STEPS: [f32; 3] = [128.0, 192.0, 256.0];
/// The size the grid opens with.
pub const DEFAULT_STEP: usize = 1;
const GAP: f32 = 12.0;
const MARGIN: f32 = 16.0;
/// Ctrl + mouse wheel changes the size once this much zoom has come together.
const ZOOM_PER_STEP: f32 = 0.18;

/// Where the cells go, in content coordinates (0, 0 is the top left of the scrolled content).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub columns: usize,
    /// Side of a cell's picture box, in points.
    pub side: f32,
    /// From one cell to the next, across and down (the stars row included).
    pub pitch: Vec2,
    /// Left edge of the first column: the columns are centred.
    pub left: f32,
}

impl Layout {
    pub fn new(width: f32, step: usize, pixels_per_point: f32) -> Self {
        let side = STEPS[step.min(STEPS.len() - 1)] / pixels_per_point;
        let pitch = vec2(side + GAP, side + cells::STARS_ROW + GAP / 2.0);
        let usable = (width - 2.0 * MARGIN + GAP).max(pitch.x);
        let columns = ((usable / pitch.x).floor() as usize).max(1);
        let used = columns as f32 * pitch.x - GAP;
        Self {
            columns,
            side,
            pitch,
            left: ((width - used) / 2.0).max(0.0),
        }
    }

    pub fn rows(&self, count: usize) -> usize {
        count.div_ceil(self.columns)
    }

    pub fn height(&self, count: usize) -> f32 {
        2.0 * MARGIN + self.rows(count) as f32 * self.pitch.y
    }

    /// The picture box of cell `index`.
    pub fn cell(&self, index: usize) -> Rect {
        let (row, column) = (index / self.columns, index % self.columns);
        Rect::from_min_size(
            pos2(
                self.left + column as f32 * self.pitch.x,
                MARGIN + row as f32 * self.pitch.y,
            ),
            Vec2::splat(self.side),
        )
    }

    /// The cells whose row reaches into `top..bottom`.
    pub fn visible(&self, count: usize, top: f32, bottom: f32) -> Range<usize> {
        let first_row = ((top - MARGIN) / self.pitch.y).floor().max(0.0) as usize;
        let end_row = ((bottom - MARGIN) / self.pitch.y).ceil().max(0.0) as usize;
        (first_row * self.columns).min(count)..(end_row * self.columns).min(count)
    }

    /// Whole rows that fit into `height`: one Page Up / Page Down.
    pub fn page_rows(&self, height: f32) -> usize {
        ((height / self.pitch.y).floor() as usize).max(1)
    }
}

/// `↓` (`down`) or `↑` in a grid of `columns`: one row further, staying in the column. From
/// the last full row `↓` goes to the last photo; on the last row, and on the first for `↑`,
/// nothing moves.
pub fn row_step(columns: usize, current: usize, count: usize, down: bool) -> usize {
    let columns = columns.max(1);
    if down {
        let below = current + columns;
        if below < count {
            below
        } else if current / columns < count.div_ceil(columns).saturating_sub(1) {
            count - 1
        } else {
            current
        }
    } else {
        current.checked_sub(columns).unwrap_or(current)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GridOutput {
    /// A cell was clicked: it becomes the current photo.
    pub clicked: Option<usize>,
    /// A cell was double-clicked: the photo opens in the single view.
    pub opened: Option<usize>,
    /// Cells on screen (the thumbnail cache keeps that many).
    pub visible: usize,
    /// Ctrl + mouse wheel: the size one step up (+1) or down (-1).
    pub resize: i32,
    /// Columns and cells per page this frame, for `↑`/`↓` and Page Up / Page Down.
    pub columns: usize,
    pub page: usize,
}

/// What the grid shows this frame.
pub struct Shown<'a> {
    pub paths: &'a [PathBuf],
    pub current: usize,
    /// Capture-time order: a line separates neighbouring series, the current one is
    /// underlined.
    pub grouped: bool,
    /// Index into [`STEPS`].
    pub step: usize,
    /// Scroll the current photo into view – when it moved by the keyboard, not while the user
    /// scrolls with the wheel.
    pub follow: bool,
}

/// Draws the grid into `rect`, with `info` for each cell on screen.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    shown: &Shown<'_>,
    thumbs: &Thumbs,
    info: impl Fn(usize) -> CellInfo,
) -> GridOutput {
    let Shown {
        paths,
        current,
        grouped,
        step,
        follow,
    } = *shown;
    ui.painter().rect_filled(rect, 0.0, tokens::CANVAS);
    let layout = Layout::new(rect.width(), step, ui.ctx().pixels_per_point());
    let mut out = GridOutput {
        columns: layout.columns,
        page: layout.page_rows(rect.height()) * layout.columns,
        ..GridOutput::default()
    };
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).id_salt("grid"));
    ScrollArea::vertical()
        .id_salt("grid-scroll")
        .auto_shrink(false)
        .show_viewport(&mut child, |ui, viewport| {
            ui.set_height(layout.height(paths.len()));
            let origin = ui.max_rect().min.to_vec2();
            let range = layout.visible(paths.len(), viewport.top(), viewport.bottom());
            out.visible = range.len();
            let painter = ui.painter().clone();
            let infos: Vec<CellInfo> = range.clone().map(&info).collect();
            // Nearest the current photo first: videos get their frames in the order the cells
            // ask for them.
            let mut order: Vec<usize> = (0..range.len()).collect();
            order.sort_by_key(|&n| (range.start + n).abs_diff(current));
            for n in order {
                let index = range.start + n;
                let path = &paths[index];
                let cell = layout.cell(index).translate(origin);
                let name = library::file_name_lossy(path);
                let empty = Empty { name: Some(&name) };
                let response = cells::paint(
                    ui,
                    &painter,
                    cell,
                    Id::new(("grid", index)),
                    thumbs.get_or_request(path).as_ref(),
                    empty,
                    &infos[n],
                );
                if response.double_clicked() {
                    out.opened = Some(index);
                } else if response.clicked() {
                    out.clicked = Some(index);
                }
                let cell_info = &infos[n];
                if cell_info.in_current_series {
                    cells::series_line(&painter, cell, cell.bottom() + cells::STARS_ROW - 2.0);
                }
                // Nothing is folded away: a line between two series in the same row.
                if grouped
                    && n > 0
                    && !index.is_multiple_of(layout.columns)
                    && cells::series_boundary(infos[n - 1].series_id, cell_info.series_id)
                {
                    painter.vline(
                        cell.left() - GAP / 2.0,
                        cell.y_range().shrink(6.0),
                        Stroke::new(1.5, tokens::MUTED),
                    );
                }
            }
            if follow && current < paths.len() {
                let target = layout.cell(current).translate(origin);
                ui.scroll_to_rect(
                    target.expand2(vec2(0.0, cells::STARS_ROW / 2.0 + GAP)),
                    None,
                );
            }
        });
    out.resize = zoom_steps(ui, rect);
    out
}

/// Size steps from Ctrl + mouse wheel over the grid (egui turns it into zoom). Fractions carry
/// over to the next frame.
fn zoom_steps(ui: &Ui, rect: Rect) -> i32 {
    if !ui.rect_contains_pointer(rect) {
        return 0;
    }
    let zoom = ui.input(|i| i.zoom_delta());
    if zoom == 1.0 {
        return 0;
    }
    let id = ui.id().with("grid-zoom");
    ui.data_mut(|d| {
        let carry = d.get_temp_mut_or_default::<f32>(id);
        *carry += zoom.ln();
        let steps = (*carry / ZOOM_PER_STEP).trunc();
        *carry -= steps * ZOOM_PER_STEP;
        steps as i32
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Rating;
    use eframe::egui::{Context, Event, Modifiers, PointerButton, RawInput};
    use std::sync::Arc;

    fn plain_cell() -> CellInfo {
        CellInfo {
            current: false,
            rating: Rating::Unrated,
            blurry: None,
            pinned: false,
            label: None,
            series_id: None,
            in_current_series: false,
            duplicate_of: None,
            video: false,
            deleted: false,
        }
    }

    /// A click on a cell picks it, a double click opens it; only the rows on screen are drawn.
    #[test]
    fn a_click_picks_a_cell_a_double_click_opens_it() {
        let ctx = Context::default();
        let db = Arc::new(crate::db::Db::open_in_memory().unwrap());
        let thumbs = Thumbs::new(ctx.clone(), db);
        let paths: Vec<PathBuf> = (0..60).map(|i| PathBuf::from(format!("{i}.jpg"))).collect();
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 600.0));
        let mut time = 0.0;
        let mut frame = |events: Vec<Event>| {
            time += 0.05;
            let mut out = GridOutput::default();
            let shown = Shown {
                paths: &paths,
                current: 0,
                grouped: false,
                step: 1,
                follow: false,
            };
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(rect),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| out = draw(ui, rect, &shown, &thumbs, |_| plain_cell()),
            );
            output.textures_delta.clear();
            out
        };
        let layout = Layout::new(rect.width(), 1, 1.0);
        let first = frame(Vec::new());
        assert_eq!(first.columns, layout.columns);
        assert!(
            first.visible > 0 && first.visible < paths.len(),
            "{}",
            first.visible
        );
        let at = layout.cell(5).center();
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(vec![Event::PointerMoved(at)]);
        frame(vec![button(true)]);
        assert_eq!(frame(vec![button(false)]).clicked, Some(5));
        frame(vec![button(true)]);
        assert_eq!(frame(vec![button(false)]).opened, Some(5));
    }

    #[test]
    fn columns_fill_the_width_and_are_centred() {
        let layout = Layout::new(1000.0, 1, 1.0);
        // 192 px cells plus 12 px gaps in 1000 − 32 px: four columns.
        assert_eq!(layout.columns, 4);
        let used = 4.0 * layout.pitch.x - GAP;
        assert!((layout.left - (1000.0 - used) / 2.0).abs() < 1e-3);
        assert_eq!(layout.cell(5).min.y, MARGIN + layout.pitch.y, "second row");
        assert_eq!(layout.cell(5).min.x, layout.cell(1).min.x, "same column");
        // A narrow window still has one column.
        assert_eq!(Layout::new(100.0, 2, 1.0).columns, 1);
    }

    #[test]
    fn cells_are_physical_pixels() {
        assert_eq!(Layout::new(2000.0, 2, 1.0).side, 256.0);
        assert_eq!(
            Layout::new(2000.0, 2, 2.0).side,
            128.0,
            "no bigger than the thumbnails"
        );
        assert_eq!(Layout::new(2000.0, 9, 1.0).side, 256.0, "steps are clamped");
    }

    #[test]
    fn only_rows_on_screen_are_drawn() {
        let layout = Layout::new(1000.0, 1, 1.0);
        let row = layout.pitch.y;
        assert_eq!(layout.visible(100, 0.0, row), 0..4);
        let middle = layout.visible(100, MARGIN + 10.0 * row + 1.0, MARGIN + 12.0 * row);
        assert_eq!(middle, 40..48);
        assert_eq!(
            layout.visible(10, 0.0, 10_000.0),
            0..10,
            "never past the end"
        );
        assert_eq!(layout.page_rows(3.5 * row), 3);
    }

    #[test]
    fn up_and_down_keep_the_column() {
        let columns = Layout::new(1000.0, 1, 1.0).columns;
        assert_eq!(columns, 4);
        assert_eq!(row_step(columns, 1, 10, true), 5);
        assert_eq!(row_step(columns, 5, 10, false), 1);
        assert_eq!(row_step(columns, 1, 10, false), 1, "the first row stays");
        // 10 photos in rows of 4: from 7 (second row, last column) the last photo is 9.
        assert_eq!(row_step(columns, 7, 10, true), 9);
        assert_eq!(row_step(columns, 9, 10, true), 9, "the last row stays");
    }
}
