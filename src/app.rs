//! The viewer window: keyboard model, drawing, drag & drop.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontId, Key, Layout, Pos2, Rect, RichText, Sense,
    Shape, Stroke, StrokeKind, UiBuilder, Vec2, ViewportCommand, pos2, vec2,
};

use crate::library::{self, Library};
use crate::loader::{LoadedImage, Loader, Lookup};
use crate::rating::RatingWriter;
use crate::theme::{self, tokens};

const BAR_HEIGHT: f32 = 40.0;
const STAR_SIZE: f32 = 18.0;
const STAR_GAP: f32 = 6.0;
/// Decode size until the monitor size is known (Wayland never reports it).
const FALLBACK_TARGET: [u32; 2] = [2560, 1440];
const STAR_KEYS: [(Key, Option<u8>); 6] = [
    (Key::Num0, None),
    (Key::Num1, Some(1)),
    (Key::Num2, Some(2)),
    (Key::Num3, Some(3)),
    (Key::Num4, Some(4)),
    (Key::Num5, Some(5)),
];

pub struct CernoApp {
    loader: Loader,
    writer: RatingWriter,
    dir: Option<PathBuf>,
    paths: Arc<Vec<PathBuf>>,
    current: usize,
    /// Ratings given in this session; they win over the value read from the file, whose write
    /// may still be pending.
    session_ratings: HashMap<PathBuf, Option<u8>>,
    /// Opened on the first frame, once the decode size is known.
    pending_open: Option<PathBuf>,
    target: Option<[u32; 2]>,
    show_info: bool,
    notice: Option<String>,
}

struct KeyInput {
    next: bool,
    prev: bool,
    first: bool,
    last: bool,
    rating: Option<Option<u8>>,
    toggle_fullscreen: bool,
    escape: bool,
    toggle_info: bool,
    open: bool,
    is_fullscreen: bool,
}

impl CernoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, start_path: Option<PathBuf>) -> Self {
        theme::apply(&cc.egui_ctx);
        Self {
            loader: Loader::new(cc.egui_ctx.clone(), FALLBACK_TARGET),
            writer: RatingWriter::new(cc.egui_ctx.clone()),
            dir: None,
            paths: Arc::new(Vec::new()),
            current: 0,
            session_ratings: HashMap::new(),
            pending_open: start_path,
            target: None,
            show_info: true,
            notice: None,
        }
    }

    /// Decode size: the monitor in physical pixels (or the window, if larger), clamped to the
    /// GPU's texture limit. It only grows, so moving to a bigger screen never shrinks the cache.
    fn update_target(&mut self, ctx: &egui::Context, window: Vec2) {
        let ppp = ctx.pixels_per_point();
        let (monitor, max_side) = ctx.input(|i| (i.viewport().monitor_size, i.max_texture_side));
        let fallback = vec2(FALLBACK_TARGET[0] as f32, FALLBACK_TARGET[1] as f32);
        let size = monitor.map_or(fallback, |m| m * ppp).max(window * ppp);
        let max = max_side as f32;
        let wanted = [
            size.x.min(max).round() as u32,
            size.y.min(max).round() as u32,
        ];
        let target = match self.target {
            Some(t) if t[0] >= wanted[0] && t[1] >= wanted[1] => return,
            Some(t) => [t[0].max(wanted[0]), t[1].max(wanted[1])],
            None => wanted,
        };
        self.target = Some(target);
        self.loader.set_target(target);
    }

    fn open(&mut self, ctx: &egui::Context, path: &Path) {
        match Library::open(path) {
            Ok((library, index)) => {
                self.notice = library
                    .paths
                    .is_empty()
                    .then(|| format!("No JPEG or HEIC files in {}", library.dir.display()));
                self.paths = Arc::clone(&library.paths);
                self.current = index;
                self.dir = Some(library.dir);
                self.loader.set_library(Arc::clone(&self.paths), index);
                self.update_title(ctx);
            }
            Err(err) => self.notice = Some(format!("Cannot open {}: {err}", path.display())),
        }
    }

    fn pick_folder(&mut self, ctx: &egui::Context) {
        let mut dialog = rfd::FileDialog::new().set_title("Open folder");
        if let Some(dir) = &self.dir {
            dialog = dialog.set_directory(dir);
        }
        if let Some(dir) = dialog.pick_folder() {
            self.open(ctx, &dir);
        }
    }

    fn go_to(&mut self, ctx: &egui::Context, index: usize) {
        let Some(last) = self.paths.len().checked_sub(1) else {
            return;
        };
        let index = index.min(last);
        if index != self.current {
            self.current = index;
            self.loader.set_current(index);
            self.update_title(ctx);
        }
    }

    fn update_title(&self, ctx: &egui::Context) {
        let title = match self.paths.get(self.current) {
            Some(path) => format!("{} – Cerno", library::file_name_lossy(path)),
            None => "Cerno".to_owned(),
        };
        ctx.send_viewport_cmd(ViewportCommand::Title(title));
    }

    fn set_rating(&mut self, stars: Option<u8>) {
        let Some(path) = self.paths.get(self.current).cloned() else {
            return;
        };
        self.session_ratings.insert(path.clone(), stars);
        self.writer.set(path, stars);
    }

    fn rating_of(&self, index: usize, image: Option<&LoadedImage>) -> Option<u8> {
        match self
            .paths
            .get(index)
            .and_then(|p| self.session_ratings.get(p))
        {
            Some(stars) => *stars,
            None => image.and_then(|i| i.rating.stars),
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        if let Some(path) =
            ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
        {
            self.open(ctx, &path);
        }

        let keys = ctx.input(|i| KeyInput {
            next: [Key::ArrowRight, Key::Space, Key::PageDown]
                .iter()
                .any(|k| i.key_pressed(*k)),
            prev: [Key::ArrowLeft, Key::Backspace, Key::PageUp]
                .iter()
                .any(|k| i.key_pressed(*k)),
            first: i.key_pressed(Key::Home),
            last: i.key_pressed(Key::End),
            rating: STAR_KEYS
                .iter()
                .find(|(k, _)| i.key_pressed(*k))
                .map(|(_, stars)| *stars),
            toggle_fullscreen: i.key_pressed(Key::F11)
                || (i.key_pressed(Key::F) && i.modifiers.is_none()),
            escape: i.key_pressed(Key::Escape),
            toggle_info: i.key_pressed(Key::I) && i.modifiers.is_none(),
            open: i.modifiers.command && i.key_pressed(Key::O),
            is_fullscreen: i.viewport().fullscreen.unwrap_or(false),
        });

        if keys.open {
            self.pick_folder(ctx);
        }
        if keys.next {
            self.go_to(ctx, self.current.saturating_add(1));
        }
        if keys.prev {
            self.go_to(ctx, self.current.saturating_sub(1));
        }
        if keys.first {
            self.go_to(ctx, 0);
        }
        if keys.last {
            self.go_to(ctx, usize::MAX);
        }
        if let Some(stars) = keys.rating {
            self.set_rating(stars);
        }
        if keys.toggle_info {
            self.show_info = !self.show_info;
        }
        if keys.toggle_fullscreen {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!keys.is_fullscreen));
        }
        if keys.escape {
            if keys.is_fullscreen {
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
            } else {
                self.notice = None;
            }
        }
    }

    fn draw_empty_state(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let content = Rect::from_center_size(rect.center(), vec2(460.0, 190.0));
        let mut open_clicked = false;
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(content)
                .layout(Layout::top_down(Align::Center)),
            |ui| {
                ui.label(RichText::new("Cerno").size(22.0).color(tokens::TEXT));
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Drop a folder or photo here, or press Ctrl+O.")
                        .color(tokens::MUTED),
                );
                ui.add_space(18.0);
                open_clicked = ui.add(theme::primary_button("Open folder")).clicked();
                ui.add_space(18.0);
                ui.label(
                    RichText::new(
                        "← → browse   ·   1–5 rate   ·   0 clear   ·   F11 fullscreen   ·   I info",
                    )
                    .size(11.0)
                    .color(tokens::MUTED),
                );
            },
        );
        if open_clicked {
            let ctx = ui.ctx().clone();
            self.pick_folder(&ctx);
        }
    }

    /// Bottom bar: file name and position, stars (clickable), size and load time. Returns the
    /// rating the user clicked, if any.
    fn draw_info_bar(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        image: Option<&LoadedImage>,
    ) -> Option<Option<u8>> {
        let bar = Rect::from_min_max(pos2(rect.min.x, rect.max.y - BAR_HEIGHT), rect.max);
        let painter = ui.painter();
        painter.rect_filled(bar, 0.0, tokens::SURFACE);
        painter.hline(
            bar.x_range(),
            bar.top() + 0.5,
            Stroke::new(1.0, tokens::LINE),
        );
        let y = bar.center().y;

        let stars_width = 5.0 * STAR_SIZE + 4.0 * STAR_GAP;
        let stars_left = bar.center().x - stars_width / 2.0;

        // Left: name and position, clipped so a long name never runs into the stars.
        let left = painter.with_clip_rect(Rect::from_min_max(
            bar.min,
            pos2(stars_left - 16.0, bar.max.y),
        ));
        let name = self
            .paths
            .get(self.current)
            .map(|p| library::file_name_lossy(p))
            .unwrap_or_default();
        let name_rect = left.text(
            pos2(bar.left() + 12.0, y),
            Align2::LEFT_CENTER,
            name,
            FontId::proportional(13.0),
            tokens::TEXT,
        );
        left.text(
            pos2(name_rect.right() + 10.0, y),
            Align2::LEFT_CENTER,
            format!("{} / {}", self.current + 1, self.paths.len()),
            FontId::proportional(12.0),
            tokens::MUTED,
        );

        // Right: image facts and writer state.
        let mut facts = Vec::new();
        if let Some(image) = image {
            let [w, h] = image.original_size;
            facts.push(format!("{w} × {h}"));
            facts.push(format!("{} ms", image.load_ms));
        }
        if self.writer.status().pending > 0 {
            facts.push("Saving…".to_owned());
        }
        let right = painter.with_clip_rect(Rect::from_min_max(
            pos2(stars_left + stars_width + 16.0, bar.min.y),
            bar.max,
        ));
        right.text(
            pos2(bar.right() - 12.0, y),
            Align2::RIGHT_CENTER,
            facts.join("   ·   "),
            FontId::proportional(12.0),
            tokens::MUTED,
        );

        // Center: stars.
        let rating = self.rating_of(self.current, image);
        let mut clicked = None;
        for n in 1..=5u8 {
            let x = stars_left + f32::from(n - 1) * (STAR_SIZE + STAR_GAP);
            let star = Rect::from_min_size(pos2(x, y - STAR_SIZE / 2.0), Vec2::splat(STAR_SIZE));
            let response = ui
                .interact(
                    star.expand(STAR_GAP / 2.0),
                    ui.id().with(("star", n)),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand);
            let filled = rating.is_some_and(|r| n <= r);
            let color = if filled || response.hovered() {
                tokens::ACCENT
            } else {
                tokens::MUTED
            };
            paint_star(painter, star.center(), STAR_SIZE / 2.0, filled, color);
            if response.clicked() {
                // Clicking the current rating again clears it.
                clicked = Some(if rating == Some(n) { None } else { Some(n) });
            }
            response.on_hover_text(format!("{n} – key {n}"));
        }
        clicked
    }

    fn draw_notices(&self, ui: &egui::Ui, rect: Rect) {
        let writer_error = self.writer.status().last_error;
        let (text, is_error) = match (writer_error, &self.notice) {
            (Some(err), _) => (format!("Rating not saved – {err}"), true),
            (None, Some(notice)) => (notice.clone(), false),
            (None, None) => return,
        };
        let painter = ui.painter();
        let galley = painter.layout(
            text,
            FontId::proportional(13.0),
            tokens::TEXT,
            (rect.width() - 64.0).max(120.0),
        );
        let size = galley.size() + vec2(24.0, 14.0);
        let pill = Rect::from_min_size(
            pos2(rect.center().x - size.x / 2.0, rect.top() + 12.0),
            size,
        );
        let (fill, border) = if is_error {
            (tokens::STATUS_ERROR_BG, tokens::STATUS_ERROR)
        } else {
            (tokens::SURFACE, tokens::LINE)
        };
        painter.rect_filled(pill, 6.0, fill);
        painter.rect_stroke(pill, 6.0, Stroke::new(1.0, border), StrokeKind::Inside);
        painter.galley(pill.min + vec2(12.0, 7.0), galley, tokens::TEXT);
    }

    fn draw_drop_hint(ui: &egui::Ui, rect: Rect) {
        if ui.ctx().input(|i| i.raw.hovered_files.is_empty()) {
            return;
        }
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, Color32::from_black_alpha(150));
        painter.rect_stroke(
            rect.shrink(10.0),
            8.0,
            Stroke::new(2.0, tokens::ACCENT),
            StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Drop to open",
            FontId::proportional(18.0),
            tokens::TEXT,
        );
    }
}

impl eframe::App for CernoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let rect = ui.max_rect();
        self.update_target(&ctx, rect.size());
        if let Some(path) = self.pending_open.take() {
            self.open(&ctx, &path);
        }
        self.handle_input(&ctx);

        ui.painter().rect_filled(rect, 0.0, tokens::CANVAS);
        if self.paths.is_empty() {
            self.draw_empty_state(ui, rect);
        } else {
            let lookup = self.loader.get(self.current);
            let image = match &lookup {
                Lookup::Ready(image) => Some(image.as_ref()),
                _ => None,
            };
            let image_area = if self.show_info {
                Rect::from_min_max(rect.min, pos2(rect.max.x, rect.max.y - BAR_HEIGHT))
            } else {
                rect
            };
            draw_image(ui.painter(), image_area, &lookup, ctx.pixels_per_point());
            if self.show_info
                && let Some(stars) = self.draw_info_bar(ui, rect, image)
            {
                self.set_rating(stars);
            }
        }
        self.draw_notices(ui, rect);
        Self::draw_drop_hint(ui, rect);
    }

    fn on_exit(&mut self) {
        // A rating given right before closing must still reach the file.
        self.writer.shutdown();
    }
}

/// Fits the image into `area`, never enlarging it beyond its decoded size.
fn draw_image(painter: &egui::Painter, area: Rect, lookup: &Lookup, pixels_per_point: f32) {
    match lookup {
        Lookup::Ready(image) => {
            let natural = image.texture.size_vec2() / pixels_per_point;
            let scale = (area.width() / natural.x)
                .min(area.height() / natural.y)
                .min(1.0);
            let rect = Rect::from_center_size(area.center(), natural * scale);
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            painter.image(image.texture.id(), rect, uv, Color32::WHITE);
        }
        Lookup::Failed(message) => {
            painter.text(
                area.center(),
                Align2::CENTER_CENTER,
                format!("Cannot show this image\n{message}"),
                FontId::proportional(14.0),
                tokens::STATUS_ERROR,
            );
        }
        Lookup::Pending => {
            painter.text(
                area.center(),
                Align2::CENTER_CENTER,
                "Loading…",
                FontId::proportional(13.0),
                tokens::MUTED,
            );
        }
    }
}

fn paint_star(painter: &egui::Painter, center: Pos2, radius: f32, filled: bool, color: Color32) {
    let inner = radius * 0.45;
    let points: Vec<Pos2> = (0..10)
        .map(|i| {
            let r = if i % 2 == 0 { radius } else { inner };
            center + r * Vec2::angled(-FRAC_PI_2 + i as f32 * PI / 5.0)
        })
        .collect();
    if filled {
        // epaint only fills convex shapes: inner pentagon plus five tip triangles.
        let pentagon: Vec<Pos2> = points.iter().skip(1).step_by(2).copied().collect();
        painter.add(Shape::convex_polygon(pentagon, color, Stroke::NONE));
        for tip in (0..10).step_by(2) {
            let triangle = vec![points[(tip + 9) % 10], points[tip], points[tip + 1]];
            painter.add(Shape::convex_polygon(triangle, color, Stroke::NONE));
        }
    }
    painter.add(Shape::closed_line(points, Stroke::new(1.4, color)));
}
