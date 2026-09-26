//! The window: state, keyboard and mouse model, layout. Drawing lives in `ui/`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{
    self, CursorIcon, Key, PointerButton, Rect, Sense, Vec2, ViewportCommand, pos2, vec2,
};

use crate::analysis::{Analyzer, ScoreBoard, sharpness};
use crate::db::Db;
use crate::library::{self, Library};
use crate::loader::{LoadedImage, Loader, Lookup};
use crate::paths;
use crate::rating::RatingWriter;
use crate::theme::{self, tokens};
use crate::thumbs::Thumbs;
use crate::ui::{bars, filmstrip, viewer};
use crate::view::{self, BLURRY_PERCENTILE, RatingFilter, SortKey, ViewOptions};

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
/// Zoom step for `+`/`-`.
const ZOOM_STEP: f32 = 1.25;

pub struct CernoApp {
    db: Arc<Db>,
    thumbs: Arc<Thumbs>,
    board: Arc<ScoreBoard>,
    loader: Loader,
    analyzer: Analyzer,
    writer: RatingWriter,

    dir: Option<PathBuf>,
    /// Every photo of the folder, in name order.
    all: Arc<Vec<PathBuf>>,
    all_index: HashMap<PathBuf, usize>,
    /// What is shown, after sorting and filtering.
    view: Arc<Vec<PathBuf>>,
    current: usize,
    options: ViewOptions,
    /// Score board version the view was built from.
    view_version: u64,
    /// Sorted sharpness values of the folder, for percentiles (board version, values).
    sharpness_sorted: (u64, Vec<f32>),

    /// Ratings given in this session; they win over the value read from the file, whose
    /// write may still be pending.
    session_ratings: HashMap<PathBuf, Option<u8>>,
    /// Opened on the first frame, once the decode size is known.
    pending_open: Option<PathBuf>,
    target: Option<[u32; 2]>,
    zoom: viewer::Zoom,
    show_chrome: bool,
    show_filmstrip: bool,
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
    toggle_chrome: bool,
    toggle_filmstrip: bool,
    toggle_zoom: bool,
    zoom_in: bool,
    zoom_out: bool,
    open: bool,
    is_fullscreen: bool,
}

impl CernoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, start_path: Option<PathBuf>) -> Self {
        theme::apply(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();

        let mut notice = None;
        let db = paths::database_path()
            .and_then(|path| Db::open(&path))
            .unwrap_or_else(|err| {
                log::error!("index database: {err:#}");
                notice = Some(format!("Scores are not saved this session: {err:#}"));
                Db::open_in_memory().expect("in-memory SQLite")
            });
        let db = Arc::new(db);
        let thumbs = Arc::new(Thumbs::new(ctx.clone(), Arc::clone(&db)));
        let board = Arc::new(ScoreBoard::default());

        let options = ViewOptions {
            sort: db
                .setting("sort")
                .and_then(|s| SortKey::from_id(&s))
                .unwrap_or(SortKey::Name),
            filter: db
                .setting("filter")
                .and_then(|s| RatingFilter::from_id(&s))
                .unwrap_or(RatingFilter::All),
            hide_blurry: db.setting("hide_blurry").as_deref() == Some("1"),
        };
        let show_filmstrip = db.setting("filmstrip").as_deref() != Some("0");

        Self {
            loader: Loader::new(ctx.clone(), FALLBACK_TARGET, Arc::clone(&thumbs)),
            analyzer: Analyzer::new(
                ctx.clone(),
                Arc::clone(&db),
                Arc::clone(&board),
                Arc::clone(&thumbs),
            ),
            writer: RatingWriter::new(ctx, Arc::clone(&db)),
            db,
            thumbs,
            board,
            dir: None,
            all: Arc::new(Vec::new()),
            all_index: HashMap::new(),
            view: Arc::new(Vec::new()),
            current: 0,
            options,
            view_version: 0,
            sharpness_sorted: (u64::MAX, Vec::new()),
            session_ratings: HashMap::new(),
            pending_open: start_path,
            target: None,
            zoom: viewer::Zoom::default(),
            show_chrome: true,
            show_filmstrip,
            notice,
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
        let (library, index) = match Library::open(path) {
            Ok(opened) => opened,
            Err(err) => {
                self.notice = Some(format!("Cannot open {}: {err}", path.display()));
                return;
            }
        };
        self.notice = library
            .paths
            .is_empty()
            .then(|| format!("No JPEG or HEIC files in {}", library.dir.display()));
        self.all_index = library
            .paths
            .iter()
            .enumerate()
            .map(|(i, p)| (p.clone(), i))
            .collect();
        // A photo that was opened directly stays selected; a folder starts at the top of the
        // (possibly sorted) view.
        let start = path
            .is_file()
            .then(|| library.paths.get(index).cloned())
            .flatten();
        self.all = Arc::clone(&library.paths);
        self.dir = Some(library.dir);
        self.thumbs.clear();
        self.analyzer.preload(&self.all);
        self.analyzer.set_library(Arc::clone(&self.all), index);
        self.view = Arc::new(Vec::new());
        self.rebuild_view(ctx, start);
    }

    /// Re-applies sorting and filtering, staying on `keep` (or the current photo) if it is
    /// still shown.
    fn rebuild_view(&mut self, ctx: &egui::Context, keep: Option<PathBuf>) {
        let keep = keep.or_else(|| self.view.get(self.current).cloned());
        let board = Arc::clone(&self.board);
        let view = view::build(
            &self.all,
            self.options,
            |p| board.get(p),
            &self.session_ratings,
        );
        self.current = keep
            .and_then(|k| view.iter().position(|p| *p == k))
            .unwrap_or(0);
        self.view = Arc::new(view);
        self.view_version = self.board.version();
        self.loader
            .set_library(Arc::clone(&self.view), self.current);
        self.sync_analyzer();
        self.update_title(ctx);
    }

    fn save_options(&self) {
        self.db.put_setting("sort", self.options.sort.id());
        self.db.put_setting("filter", &self.options.filter.id());
        self.db.put_setting(
            "hide_blurry",
            if self.options.hide_blurry { "1" } else { "0" },
        );
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

    fn confirm_model_download(&self) {
        let answer = rfd::MessageDialog::new()
            .set_title("Enable aesthetics scoring")
            .set_description(format!(
                "Cerno needs the CLIP ViT-L/14 image model to score aesthetics.\n\n\
                 Download it now from Hugging Face (Xenova/clip-vit-large-patch14, {:.1} GB)? \
                 It is stored in Cerno's data folder and only downloaded once.",
                crate::analysis::aesthetic::MODEL_BYTES as f64 / 1e9
            ))
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if answer == rfd::MessageDialogResult::Yes {
            self.analyzer.download_model();
        }
    }

    fn go_to(&mut self, ctx: &egui::Context, index: usize) {
        let Some(last) = self.view.len().checked_sub(1) else {
            return;
        };
        let index = index.min(last);
        if index != self.current {
            self.current = index;
            self.loader.set_current(index);
            self.sync_analyzer();
            self.update_title(ctx);
            let (view, current) = (Arc::clone(&self.view), self.current);
            self.thumbs.retain(|p| {
                view.iter()
                    .position(|q| q == p)
                    .is_some_and(|i| i.abs_diff(current) < 150)
            });
        }
    }

    /// Tells the analysis where the user is (in full-folder terms) and pauses it briefly.
    fn sync_analyzer(&self) {
        if let Some(index) = self
            .view
            .get(self.current)
            .and_then(|p| self.all_index.get(p))
        {
            self.analyzer.set_current(*index);
        }
    }

    fn update_title(&self, ctx: &egui::Context) {
        let title = match self.view.get(self.current) {
            Some(path) => format!("{} – Cerno", library::file_name_lossy(path)),
            None => "Cerno".to_owned(),
        };
        ctx.send_viewport_cmd(ViewportCommand::Title(title));
    }

    fn set_rating(&mut self, stars: Option<u8>) {
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        self.session_ratings.insert(path.clone(), stars);
        self.writer.set(path, stars);
    }

    fn rating_of(&self, path: &Path, image: Option<&LoadedImage>) -> Option<u8> {
        if let Some(stars) = self.session_ratings.get(path) {
            return *stars;
        }
        image
            .map(|i| i.rating.stars)
            .or_else(|| self.board.get(path).map(|k| k.rating))
            .flatten()
    }

    fn sharpness_percentile(&mut self, path: &Path) -> Option<f32> {
        let version = self.board.version();
        if self.sharpness_sorted.0 != version {
            self.sharpness_sorted = (version, self.board.sorted_sharpness(&self.all));
        }
        let value = self.board.get(path)?.scores.sharpness?;
        Some(sharpness::percentile(value, &self.sharpness_sorted.1))
    }

    fn handle_keys(&mut self, ctx: &egui::Context, frame: Option<viewer::Frame>) {
        if let Some(path) =
            ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
        {
            self.open(ctx, &path);
        }

        let keys = ctx.input(|i| {
            let plain = i.modifiers.is_none();
            KeyInput {
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
                    .find(|(k, _)| plain && i.key_pressed(*k))
                    .map(|(_, stars)| *stars),
                toggle_fullscreen: i.key_pressed(Key::F11) || (plain && i.key_pressed(Key::F)),
                escape: i.key_pressed(Key::Escape),
                toggle_chrome: plain && i.key_pressed(Key::I),
                toggle_filmstrip: plain && i.key_pressed(Key::T),
                toggle_zoom: plain && i.key_pressed(Key::Z),
                zoom_in: !i.modifiers.command
                    && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)),
                zoom_out: !i.modifiers.command && i.key_pressed(Key::Minus),
                open: i.modifiers.command && i.key_pressed(Key::O),
                is_fullscreen: i.viewport().fullscreen.unwrap_or(false),
            }
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
        if keys.toggle_chrome {
            self.show_chrome = !self.show_chrome;
        }
        if keys.toggle_filmstrip {
            self.show_filmstrip = !self.show_filmstrip;
            self.db
                .put_setting("filmstrip", if self.show_filmstrip { "1" } else { "0" });
        }
        if let Some(frame) = frame {
            let pointer = ctx.pointer_hover_pos().filter(|p| frame.area.contains(*p));
            let anchor = pointer.unwrap_or(frame.area.center());
            if keys.toggle_zoom {
                self.zoom.toggle(&frame, pointer);
            }
            if keys.zoom_in {
                self.zoom.zoom_by(&frame, ZOOM_STEP, anchor);
            }
            if keys.zoom_out {
                self.zoom.zoom_by(&frame, 1.0 / ZOOM_STEP, anchor);
            }
        }
        if keys.toggle_fullscreen {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!keys.is_fullscreen));
        }
        if keys.escape {
            if self.zoom.is_zoomed() {
                self.zoom.scale = None;
            } else if keys.is_fullscreen {
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
            } else {
                self.notice = None;
            }
        }
    }

    /// Mouse on the photo: double-click toggles 100 %, wheel zooms, drag pans.
    fn handle_mouse(&mut self, ui: &egui::Ui, frame: &viewer::Frame) {
        let response = ui.interact(frame.area, ui.id().with("photo"), Sense::click_and_drag());
        if response.double_clicked() {
            self.zoom.toggle(frame, response.interact_pointer_pos());
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0
                && let Some(pos) = response.hover_pos()
            {
                self.zoom.zoom_by(frame, (scroll / 200.0).exp(), pos);
            }
        }
        if self.zoom.is_zoomed() {
            if response.dragged_by(PointerButton::Primary) {
                self.zoom.pan(frame, response.drag_delta());
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }
        }
    }
}

impl eframe::App for CernoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let window = ui.max_rect();
        self.update_target(&ctx, window.size());
        if let Some(path) = self.pending_open.take() {
            self.open(&ctx, &path);
        }

        // Layout: toolbar | photo | filmstrip | info bar.
        let chrome = self.show_chrome && !self.all.is_empty();
        let mut area = window;
        let toolbar_rect = chrome.then(|| {
            let r = Rect::from_min_size(window.min, vec2(window.width(), bars::TOOLBAR_HEIGHT));
            area.min.y = r.max.y;
            r
        });
        let info_rect = (chrome && !self.view.is_empty()).then(|| {
            let r = Rect::from_min_max(
                pos2(window.min.x, window.max.y - bars::INFO_HEIGHT),
                window.max,
            );
            area.max.y = r.min.y;
            r
        });
        let strip_rect = (info_rect.is_some() && self.show_filmstrip).then(|| {
            let r = Rect::from_min_max(
                pos2(window.min.x, area.max.y - filmstrip::HEIGHT),
                pos2(window.max.x, area.max.y),
            );
            area.max.y = r.min.y;
            r
        });

        let lookup = self.loader.get(self.current);
        let image = match &lookup {
            Lookup::Ready(image) => Some(Arc::clone(image)),
            _ => None,
        };
        let frame = image.as_ref().map(|image| viewer::Frame {
            area,
            image_size: image.original_size,
            pixels_per_point: ctx.pixels_per_point(),
        });
        self.handle_keys(&ctx, frame);
        // Navigation may have changed the current photo; re-read it.
        let lookup = self.loader.get(self.current);
        let image = match &lookup {
            Lookup::Ready(image) => Some(Arc::clone(image)),
            _ => None,
        };
        let frame = image.as_ref().map(|image| viewer::Frame {
            area,
            image_size: image.original_size,
            pixels_per_point: ctx.pixels_per_point(),
        });

        ui.painter().rect_filled(window, 0.0, tokens::CANVAS);
        if self.all.is_empty() {
            if bars::empty_state(ui, window) {
                self.pick_folder(&ctx);
            }
        } else if self.view.is_empty() {
            bars::centred_message(ui, area, "No photos match the filter", tokens::MUTED);
        } else {
            match (&lookup, &image, frame) {
                (_, Some(image), Some(frame)) => {
                    self.handle_mouse(ui, &frame);
                    let full = self.loader.full(self.current);
                    let needs_full =
                        viewer::draw(ui.painter(), &frame, &self.zoom, image, full.as_deref());
                    if needs_full && full.is_none() {
                        self.loader.request_full(self.current);
                    }
                }
                (Lookup::Failed(message), _, _) => bars::centred_message(
                    ui,
                    area,
                    &format!("Cannot show this image\n{message}"),
                    tokens::STATUS_ERROR,
                ),
                _ => bars::centred_message(ui, area, "Loading…", tokens::MUTED),
            }
        }

        if let Some(rect) = strip_rect {
            let view = Arc::clone(&self.view);
            let blurry_cutoff = {
                // Percentiles need `&mut self`; compute them for the visible cells up front.
                let version = self.board.version();
                if self.sharpness_sorted.0 != version {
                    self.sharpness_sorted = (version, self.board.sorted_sharpness(&self.all));
                }
                self.sharpness_sorted.1.clone()
            };
            let clicked = filmstrip::draw(ui, rect, &view, self.current, &self.thumbs, |i| {
                let path = &view[i];
                let known = self.board.get(path);
                let blurry = known
                    .and_then(|k| k.scores.sharpness)
                    .map(|s| sharpness::percentile(s, &blurry_cutoff))
                    .filter(|p| *p < BLURRY_PERCENTILE)
                    .map(|p| {
                        format!(
                            "Probably blurry: sharper than only {:.0} % of this folder",
                            p * 100.0
                        )
                    });
                let rating = match self.session_ratings.get(path) {
                    Some(stars) => *stars,
                    None => known.and_then(|k| k.rating),
                };
                filmstrip::CellInfo { rating, blurry }
            });
            if let Some(index) = clicked {
                self.go_to(&ctx, index);
            }
        }

        if let Some(rect) = info_rect
            && let Some(path) = self.view.get(self.current).cloned()
        {
            let percentile = self.sharpness_percentile(&path);
            let bar = bars::InfoBar {
                name: &library::file_name_lossy(&path),
                position: (self.current + 1, self.view.len()),
                image: image.as_deref(),
                rating: self.rating_of(&path, image.as_deref()),
                scores: self.board.get(&path).map(|k| k.scores),
                sharpness_percentile: percentile,
                saving: self.writer.status().pending > 0,
            };
            if let Some(stars) = bars::info_bar(ui, rect, &bar) {
                self.set_rating(stars);
            }
        }

        if let Some(rect) = toolbar_rect {
            let status = self.analyzer.status();
            let folder = self
                .dir
                .as_ref()
                .and_then(|d| d.file_name())
                .map(|n| n.to_string_lossy().into_owned());
            let info = bars::ToolbarInfo {
                folder: folder.as_deref(),
                shown: self.view.len(),
                total: self.all.len(),
                stale: self.options.depends_on_scores()
                    && self.board.version() != self.view_version,
                status: &status,
            };
            let mut options = self.options;
            let out = bars::toolbar(ui, rect, &mut options, &info);
            if out.options_changed {
                self.options = options;
                self.save_options();
                self.rebuild_view(&ctx, None);
            }
            if out.refresh {
                self.rebuild_view(&ctx, None);
            }
            if out.open {
                self.pick_folder(&ctx);
            }
            if out.download_model {
                self.confirm_model_download();
            }
        }

        let writer_error = self
            .writer
            .status()
            .last_error
            .map(|e| format!("Rating not saved – {e}"));
        bars::notices(ui, area, writer_error.as_deref(), self.notice.as_deref());
        bars::drop_hint(ui, window);
    }

    fn on_exit(&mut self) {
        // A rating given right before closing must still reach the file.
        self.writer.shutdown();
    }
}
