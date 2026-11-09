//! The scene view (`F8`): the grid's cells, but every section – a scene of two or more photos,
//! a run of single photos (`view::sections`), the photos without a time – starts a new row
//! under a slim header line: its time, how many photos, the cameras. Every photo keeps its
//! cell; nothing is folded away. The cursor is the current photo, as in the grid; only the rows on screen
//! are drawn.

use std::ops::Range;
use std::path::PathBuf;

use eframe::egui::{
    Id, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, pos2, text::TextWrapping, vec2,
};

use crate::library;
use crate::theme::{self, tokens};
use crate::thumbs::Thumbs;
use crate::ui::cells::{self, CellInfo, Empty};
use crate::ui::grid::{self, GridOutput};

/// The header line above a scene's first row.
pub const HEADER: f32 = 24.0;

/// Where the scenes' headers and cells go, in content coordinates: the grid's columns and cell
/// size, every scene from a new row.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub grid: grid::Layout,
    /// Per scene: its view indices.
    scenes: Vec<Range<usize>>,
    /// Per scene: the top of its header; one more entry for the end of the last scene.
    tops: Vec<f32>,
    /// The empty band above the first header and below the last row (the grid's margin).
    margin: f32,
}

impl Layout {
    pub fn new(width: f32, step: usize, pixels_per_point: f32, scenes: &[Range<usize>]) -> Self {
        let grid = grid::Layout::new(width, step, pixels_per_point);
        let margin = grid.cell(0).min.y;
        let mut tops = Vec::with_capacity(scenes.len() + 1);
        let mut y = margin;
        for scene in scenes {
            tops.push(y);
            y += HEADER + grid.rows(scene.len()) as f32 * grid.pitch.y;
        }
        tops.push(y);
        Self {
            grid,
            scenes: scenes.to_vec(),
            tops,
            margin,
        }
    }

    pub fn height(&self) -> f32 {
        self.tops.last().copied().unwrap_or(0.0) + self.margin
    }

    /// Which scene holds view index `index`.
    fn scene_of(&self, index: usize) -> Option<usize> {
        let at = self.scenes.partition_point(|s| s.end <= index);
        self.scenes
            .get(at)
            .filter(|s| s.contains(&index))
            .map(|_| at)
    }

    /// The picture box of view index `index`.
    pub fn cell(&self, index: usize) -> Rect {
        let scene = self.scene_of(index).unwrap_or(0);
        let start = self.scenes.get(scene).map_or(0, |s| s.start);
        let k = index.saturating_sub(start);
        let columns = self.grid.columns;
        Rect::from_min_size(
            pos2(
                self.grid.left + (k % columns) as f32 * self.grid.pitch.x,
                self.tops.get(scene).copied().unwrap_or(self.margin)
                    + HEADER
                    + (k / columns) as f32 * self.grid.pitch.y,
            ),
            vec2(self.grid.side, self.grid.side),
        )
    }

    /// The header line of scene `scene`, as wide as the columns.
    pub fn header(&self, scene: usize) -> Rect {
        let gap = self.grid.pitch.x - self.grid.side;
        let width = self.grid.columns as f32 * self.grid.pitch.x - gap;
        Rect::from_min_size(
            pos2(self.grid.left, self.tops.get(scene).copied().unwrap_or(0.0)),
            vec2(width, HEADER),
        )
    }

    /// The scenes reaching into `top..bottom`.
    pub fn visible_scenes(&self, top: f32, bottom: f32) -> Range<usize> {
        let count = self.scenes.len();
        // Scene `s` lies between `tops[s]` and `tops[s + 1]`.
        let first = self.tops[1..].partition_point(|&end| end <= top).min(count);
        let end = self.tops[..count].partition_point(|&start| start < bottom);
        first..end.max(first)
    }

    /// The cells whose row reaches into `top..bottom`, as runs of view indices.
    pub fn visible(&self, top: f32, bottom: f32) -> Vec<Range<usize>> {
        let pitch = self.grid.pitch.y;
        self.visible_scenes(top, bottom)
            .filter_map(|s| {
                let scene = &self.scenes[s];
                let first_row = self.tops[s] + HEADER;
                let rows = self.grid.rows(scene.len());
                let from = (((top - first_row) / pitch).floor().max(0.0) as usize).min(rows);
                let to = (((bottom - first_row) / pitch).ceil().max(0.0) as usize).min(rows);
                let columns = self.grid.columns;
                let range = (scene.start + from * columns).min(scene.end)
                    ..(scene.start + to * columns).min(scene.end);
                (!range.is_empty()).then_some(range)
            })
            .collect()
    }
}

/// The rows of cells – every scene from a new row – as runs of view indices.
fn rows(scenes: &[Range<usize>], columns: usize) -> Vec<Range<usize>> {
    let columns = columns.max(1);
    scenes
        .iter()
        .flat_map(|scene| {
            scene
                .clone()
                .step_by(columns)
                .map(move |start| start..(start + columns).min(scene.end))
        })
        .collect()
}

/// `↓` (`down`) or `↑` in the scene view: the row below or above – into the next or the
/// previous scene too – in the same column, or that row's last photo when it is shorter. On
/// the last row, and on the first for `↑`, nothing moves.
pub fn row_step(scenes: &[Range<usize>], columns: usize, current: usize, down: bool) -> usize {
    let rows = rows(scenes, columns);
    let at = rows.partition_point(|row| row.end <= current);
    let Some(row) = rows.get(at).filter(|row| row.contains(&current)) else {
        return current;
    };
    let column = current - row.start;
    let target = if down {
        rows.get(at + 1)
    } else {
        at.checked_sub(1).and_then(|above| rows.get(above))
    };
    target.map_or(current, |row| (row.start + column).min(row.end - 1))
}

/// What the scene view shows this frame.
pub struct Shown<'a> {
    pub paths: &'a [PathBuf],
    /// Every scene's view indices, in order – together every index once.
    pub scenes: &'a [Range<usize>],
    pub current: usize,
    /// Index into [`grid::STEPS`], shared with the grid.
    pub step: usize,
    /// Scroll the current photo into view.
    pub follow: bool,
}

/// Draws the scenes into `rect`: `header` gives each visible scene's line, `info` each cell
/// on screen.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    shown: &Shown<'_>,
    thumbs: &Thumbs,
    info: impl Fn(usize) -> CellInfo,
    header: impl Fn(usize) -> String,
) -> GridOutput {
    let Shown {
        paths,
        scenes,
        current,
        step,
        follow,
    } = *shown;
    ui.painter().rect_filled(rect, 0.0, tokens::CANVAS);
    let layout = Layout::new(rect.width(), step, ui.ctx().pixels_per_point(), scenes);
    let columns = layout.grid.columns;
    let mut out = GridOutput {
        columns,
        page: layout.grid.page_rows(rect.height()) * columns,
        ..GridOutput::default()
    };
    let gap = layout.grid.pitch.x - layout.grid.side;
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).id_salt("scenes"));
    ScrollArea::vertical()
        .id_salt("scenes-scroll")
        .auto_shrink(false)
        .show_viewport(&mut child, |ui, viewport| {
            ui.set_height(layout.height());
            let origin = ui.max_rect().min.to_vec2();
            let painter = ui.painter().clone();
            for scene in layout.visible_scenes(viewport.top(), viewport.bottom()) {
                let line = layout.header(scene).translate(origin);
                header_line(ui, line, scene, &header(scene));
            }
            let runs = layout.visible(viewport.top(), viewport.bottom());
            out.visible = runs.iter().map(Range::len).sum();
            for run in runs {
                let infos: Vec<CellInfo> = run.clone().map(&info).collect();
                // Nearest the current photo first: videos get their frames in that order.
                let mut order: Vec<usize> = (0..run.len()).collect();
                order.sort_by_key(|&n| (run.start + n).abs_diff(current));
                for n in order {
                    let index = run.start + n;
                    let (Some(path), Some(cell_info)) = (paths.get(index), infos.get(n)) else {
                        continue;
                    };
                    let cell = layout.cell(index).translate(origin);
                    let name = library::file_name_lossy(path);
                    let response = cells::paint(
                        ui,
                        &painter,
                        cell,
                        Id::unique(("scenes", index)),
                        thumbs.get_or_request(path).as_ref(),
                        Empty { name: Some(&name) },
                        cell_info,
                    );
                    if response.double_clicked() {
                        out.opened = Some(index);
                    } else if response.clicked() {
                        out.clicked = Some(index);
                    }
                    if cell_info.in_current_series {
                        cells::series_line(&painter, cell, cell.bottom() + cells::STARS_ROW - 2.0);
                    }
                    // Series inside a scene: a line between two in the same row, as the grid.
                    let starts_row = layout.cell(index).min.x <= layout.grid.left + 0.5;
                    if n > 0
                        && !starts_row
                        && cells::series_boundary(infos[n - 1].series_id, cell_info.series_id)
                    {
                        painter.vline(
                            cell.left() - gap / 2.0,
                            cell.y_range().shrink(6.0),
                            Stroke::new(1.0, tokens::MUTED),
                        );
                    }
                }
            }
            if follow && current < paths.len() {
                let cell = layout.cell(current);
                // On a scene's first row its header comes into view too.
                let above = if layout
                    .scene_of(current)
                    .is_some_and(|s| layout.scenes[s].start + columns > current)
                {
                    HEADER
                } else {
                    0.0
                };
                let target = Rect::from_min_max(
                    cell.min - vec2(0.0, above + gap / 2.0),
                    cell.max + vec2(0.0, cells::STARS_ROW + gap / 2.0),
                );
                ui.scroll_to_rect(target.translate(origin), None);
            }
        });
    out.resize = grid::zoom_steps(ui, rect);
    out
}

/// A scene's header: its line in the overline style – cut with "…" when it doesn't fit, whole
/// as the tooltip then – and a thin rule to the end of the row.
fn header_line(ui: &Ui, rect: Rect, scene: usize, text: &str) {
    let mut job = theme::overline_job(text, tokens::MUTED);
    job.wrap = TextWrapping::truncate_at_width(rect.width());
    let galley = ui.painter().layout_job(job);
    let elided = galley.elided;
    let text_rect = Rect::from_min_size(
        pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
        galley.size(),
    );
    let rule_from = text_rect.right() + 8.0;
    ui.painter().galley(text_rect.min, galley, tokens::MUTED);
    if rule_from < rect.right() {
        ui.painter().hline(
            rule_from..=rect.right(),
            rect.center().y.round() + 0.5,
            Stroke::new(1.0, tokens::LINE),
        );
    }
    if elided {
        let _ = ui
            .interact(
                text_rect,
                Id::unique(("scene-header", scene)),
                Sense::hover(),
            )
            .on_hover_text(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Rating;
    use eframe::egui::{Event, Modifiers, PointerButton, RawInput};
    use std::sync::Arc;

    fn plain_cell() -> CellInfo {
        CellInfo {
            rating: Rating::Unrated,
            ..CellInfo::default()
        }
    }

    /// Four columns at 1000 pt and the middle size (as the grid's own test).
    fn layout(scenes: &[Range<usize>]) -> Layout {
        let layout = Layout::new(1000.0, 1, 1.0, scenes);
        assert_eq!(layout.grid.columns, 4);
        layout
    }

    #[test]
    fn every_scene_starts_a_new_row_under_its_header() {
        let scenes = [0..6, 6..7, 7..12];
        let l = layout(&scenes);
        let pitch = l.grid.pitch.y;
        // Scene 0: header, two rows (4 + 2).
        assert_eq!(l.header(0).top(), l.margin);
        assert_eq!(l.cell(0).top(), l.margin + HEADER);
        assert_eq!(l.cell(4).top(), l.cell(0).top() + pitch, "a second row");
        assert_eq!(l.cell(4).left(), l.cell(0).left(), "from the first column");
        // Scene 1 starts its own row after scene 0's header and two rows, under its header.
        assert_eq!(l.header(1).top(), l.margin + HEADER + 2.0 * pitch);
        assert_eq!(l.cell(6).top(), l.header(1).bottom());
        assert_eq!(l.cell(6).left(), l.cell(0).left());
        // Scene 2: one header, two rows.
        assert_eq!(l.cell(7).top(), l.header(2).bottom());
        assert_eq!(
            l.height(),
            l.margin + 3.0 * HEADER + 5.0 * pitch + l.margin,
            "five rows, three headers"
        );
        // The header line spans the columns.
        assert_eq!(l.header(0).left(), l.cell(0).left());
        assert!((l.header(0).right() - l.cell(3).right()).abs() < 1e-3);
    }

    #[test]
    fn only_rows_on_screen_are_drawn() {
        let scenes: Vec<Range<usize>> = (0..50).map(|k| k * 10..k * 10 + 10).collect();
        let l = layout(&scenes);
        let pitch = l.grid.pitch.y;
        // The first screen: scene 0's header and first row.
        let first = l.visible(0.0, l.margin + HEADER + 1.0);
        assert_eq!(first, vec![0..4]);
        // Each scene: a header and three rows (4 + 4 + 2).
        let scene = HEADER + 3.0 * pitch;
        // From scene 10's second row to just into scene 11's first row.
        let middle = l.visible(
            l.margin + 10.0 * scene + HEADER + pitch + 1.0,
            l.margin + 11.0 * scene + HEADER + 1.0,
        );
        assert_eq!(middle, vec![104..110, 110..114]);
        // Only scene 11's header: no cell of it yet.
        let header_only = l.visible(l.margin + 11.0 * scene - 1.0, l.margin + 11.0 * scene + 1.0);
        assert_eq!(header_only, vec![108..110]);
        assert_eq!(l.visible_scenes(0.0, 1e9), 0..50);
        assert_eq!(l.visible_scenes(l.height(), l.height() + 10.0), 50..50);
    }

    #[test]
    fn up_and_down_go_from_row_to_row_across_scenes() {
        let scenes = [0..6, 6..7, 7..12];
        // Rows: 0..4, 4..6, 6..7, 7..11, 11..12.
        assert_eq!(rows(&scenes, 4), vec![0..4, 4..6, 6..7, 7..11, 11..12]);
        assert_eq!(row_step(&scenes, 4, 1, true), 5, "same column");
        assert_eq!(
            row_step(&scenes, 4, 3, true),
            5,
            "a shorter row: its last photo"
        );
        assert_eq!(row_step(&scenes, 4, 5, true), 6, "into the next scene");
        assert_eq!(row_step(&scenes, 4, 6, true), 7);
        assert_eq!(row_step(&scenes, 4, 9, true), 11);
        assert_eq!(row_step(&scenes, 4, 11, true), 11, "the last row stays");
        assert_eq!(row_step(&scenes, 4, 10, false), 6, "a scene of one above");
        assert_eq!(row_step(&scenes, 4, 6, false), 4);
        assert_eq!(row_step(&scenes, 4, 2, false), 2, "the first row stays");
    }

    /// A click on a cell picks it, a double click opens it – also in a later scene.
    #[test]
    fn a_click_picks_a_cell_a_double_click_opens_it() {
        let ctx = crate::theme::test_context();
        let db = Arc::new(crate::db::Db::open_in_memory().unwrap());
        let thumbs = Thumbs::new(ctx.clone(), db);
        let paths: Vec<PathBuf> = (0..30).map(|i| PathBuf::from(format!("{i}.jpg"))).collect();
        let scenes = [0..2, 2..3, 3..30];
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 800.0));
        let mut time = 0.0;
        let mut frame = |events: Vec<Event>| {
            time += 0.05;
            let mut out = GridOutput::default();
            let shown = Shown {
                paths: &paths,
                scenes: &scenes,
                current: 0,
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
                |ui| {
                    out = draw(
                        ui,
                        rect,
                        &shown,
                        &thumbs,
                        |_| plain_cell(),
                        |s| format!("scene {s}"),
                    )
                },
            );
            output.textures_delta.clear();
            out
        };
        let first = frame(Vec::new());
        assert_eq!(first.columns, 4);
        assert!(
            first.visible > 3 && first.visible < paths.len(),
            "{}",
            first.visible
        );
        let at = layout(&scenes).cell(4).center();
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(vec![Event::PointerMoved(at)]);
        frame(vec![button(true)]);
        assert_eq!(frame(vec![button(false)]).clicked, Some(4));
        frame(vec![button(true)]);
        assert_eq!(frame(vec![button(false)]).opened, Some(4));
    }
}
