//! The window. `CernoApp` holds the state; each submodule adds the methods for one part of it:
//!
//! - `browse`: opening a folder, the view (sort, filter), moving through it
//! - `marks`: stars, rejection, colour labels
//! - `files`: copy, move, delete with the countdown
//! - `editing`: straighten, crop, quarter turns, `Ctrl+Z`
//! - `gate`: one action at a time on a photo
//! - `keys`: the keyboard
//! - `photos`: the photo area, compare mode, mouse zoom and pan
//! - `frame`: the layout and the bars around the photo
//! - `menu`: menus, help, models card, confirmations
//! - `panels`: which bars show, the language
//! - `notice`: messages over the photo
//!
//! Drawing of the widgets themselves lives in `ui/`.

mod browse;
mod editing;
mod files;
mod frame;
mod gate;
mod keys;
mod marks;
mod menu;
mod notice;
mod panels;
mod photos;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use eframe::egui::{self, Key, Rect, Vec2, ViewportCommand, vec2};

use crate::analysis::{Analyzer, ScoreBoard};
use crate::db::Db;
use crate::deletion::{self, DeleteQueue};
use crate::filelock::FileLocks;
use crate::i18n::{self, Lang};
use crate::loader::Loader;
use crate::metadata::{Label, Rating};
use crate::paths;
use crate::rating::RatingWriter;
use crate::theme::tokens;
use crate::thumbs::Thumbs;
use crate::transfer::Queue as TransferQueue;
use crate::ui::details::{DetailRow, DetailsMode};
use crate::ui::overlays;
use crate::ui::{palette, viewer};
use crate::view::{FilterKind, Percentiles, PhotoFilter, SortKey, View, ViewOptions};

use editing::EditSession;
use menu::ConfirmAction;
use notice::Notice;

/// Decode size before the window exists, so the first photo decodes while the GPU starts up.
/// Covers screens up to 4K; the real monitor size replaces it on the first frame (a larger
/// screen re-decodes).
const START_TARGET: [u32; 2] = [3840, 2160];

/// Decode size if the monitor size is unknown (Wayland never reports it).
const FALLBACK_TARGET: [u32; 2] = [2560, 1440];

/// Set once the hint about the aesthetics model has been shown. Before 0.10 the same key
/// meant "the download dialog was declined", which also ends the hint.
const CLIP_OFFER_SHOWN: &str = "clip_download_declined";

pub struct CernoApp {
    db: Arc<Db>,
    /// Who reads or writes which photo (writer, loader, copy/move, edit render).
    files: Arc<FileLocks>,
    thumbs: Arc<Thumbs>,
    board: Arc<ScoreBoard>,
    loader: Loader,
    analyzer: Analyzer,
    writer: RatingWriter,
    /// Deleted photos wait here, already hidden, until the countdown runs out.
    deletions: DeleteQueue,
    /// Copy or move of the photos the filter currently shows.
    transfers: TransferQueue,

    dir: Option<PathBuf>,
    /// Every photo of the folder, in name order.
    all: Arc<Vec<PathBuf>>,
    all_index: HashMap<PathBuf, usize>,
    /// What is shown, after sorting, filtering and hiding pending deletions.
    view: View,
    current: usize,
    /// Compare mode: the photo pinned on the left. The current photo is shown on the right.
    pinned: Option<PathBuf>,
    options: ViewOptions,
    /// Score board version the view was built from.
    view_version: u64,
    /// When the view was last built (quiet refreshes are spaced out).
    view_built: Instant,
    /// Sorted sharpness values of the folder, for percentiles (board version, values).
    percentiles: (u64, Percentiles),

    /// Ratings given in this session; they win over the value read from the file, whose
    /// write may still be pending.
    session_ratings: HashMap<PathBuf, Rating>,
    /// Colour labels given in this session (`None` clears). They win over the file the same way.
    session_labels: HashMap<PathBuf, Option<Label>>,
    /// `0`–`5`, `X` and `6`–`9` also move to the next photo.
    auto_advance: bool,
    /// The open folder includes nested folders.
    subfolders: bool,
    target: Option<[u32; 2]>,
    zoom: viewer::Zoom,
    /// Top bar (`F`), filmstrip (`F6`) and details panel (`Tab`); the info bar always shows.
    show_toolbar: bool,
    show_filmstrip: bool,
    details: DetailsMode,
    /// The stage `Tab` brings back.
    details_last: DetailsMode,
    /// Which detail rows show their explanation (session-wide).
    details_expanded: HashSet<DetailRow>,
    /// Help page over the photos (`H`, `F1`, `?`).
    help_open: bool,
    /// Models & data card (menu).
    models_open: bool,
    /// A confirmation waiting for Enter or Esc; `true` reopens the models card afterwards.
    confirm: Option<(ConfirmAction, bool)>,
    /// Burger menu (`Ctrl+K` and the button) while open.
    palette: Option<palette::State>,
    /// Action menu under the filter bar's button (`Ctrl+M`): copy, move or delete the photos
    /// on screen.
    action_menu: Option<palette::State>,
    /// Where the "Action" button was last drawn; the menu opens under it.
    action_anchor: Option<Rect>,
    /// `Ctrl+M` showed the hidden filter bar; closing the menu hides it again.
    toolbar_before_actions: Option<bool>,
    /// `Tab` presses taken out of egui's input (`true` = with Shift), see `raw_input_hook`.
    tab_presses: Vec<bool>,
    /// When the language was last switched (the flag shows for a moment).
    language_flash: Option<Instant>,
    /// Message over the photo; hints fade, errors wait for Esc or a click.
    notice: Option<Notice>,
    /// Process start, for the start-up log lines.
    started: Instant,
    logged_first_frame: bool,
    logged_first_photo: bool,
    /// Straighten or crop, while it is open. The saved zoom comes back on Enter or Esc.
    edit: Option<EditSession>,
    /// Encodes a confirmed edit. Joined on exit so the write is not lost.
    edit_thread: Option<JoinHandle<()>>,
    /// A quarter turn or a re-encode is still in the writer.
    edit_busy: bool,
}

impl CernoApp {
    /// Runs before the window exists (see `main`): opening the start folder here lets the first
    /// decode overlap with the GPU and window set-up.
    pub fn new(ctx: &egui::Context, start_path: Option<PathBuf>, started: Instant) -> Self {
        let ctx = ctx.clone();
        i18n::set(i18n::system_default());
        let mut notice = None;
        let db = paths::database_path()
            .and_then(|path| Db::open(&path))
            .unwrap_or_else(|err| {
                log::error!("index database: {err:#}");
                notice = Some(Notice::error((i18n::t().db_unavailable)(&format!(
                    "{err:#}"
                ))));
                Db::open_in_memory().expect("in-memory SQLite")
            });
        if let Some(lang) = db.setting("language").and_then(|c| Lang::from_code(&c)) {
            i18n::set(lang);
        }
        let db = Arc::new(db);
        let files = Arc::new(FileLocks::default());
        let thumbs = Arc::new(Thumbs::new(ctx.clone(), Arc::clone(&db)));
        let board = Arc::new(ScoreBoard::default());

        let mut filter = db
            .setting("filter")
            .map(|s| PhotoFilter::from_stored(&s))
            .unwrap_or_default();
        let mut migrated_filter = false;
        if db.setting("hide_blurry").as_deref() == Some("1") {
            filter.set(FilterKind::Blurry, true);
            migrated_filter = true;
        }
        if db.setting("only_duplicates").as_deref() == Some("1") {
            filter.set(FilterKind::Duplicate, true);
            migrated_filter = true;
        }
        if let Some(label) = db
            .setting("label_filter")
            .as_deref()
            .filter(|s| !s.is_empty())
            .and_then(Label::from_stored)
        {
            filter.set(FilterKind::Colour(label), true);
            migrated_filter = true;
        }
        if migrated_filter {
            db.put_setting("filter", &filter.id());
            db.put_setting("hide_blurry", "0");
            db.put_setting("only_duplicates", "0");
            db.put_setting("label_filter", "");
        }
        let options = ViewOptions {
            sort: db
                .setting("sort")
                .and_then(|s| SortKey::from_id(&s))
                .unwrap_or(SortKey::Name),
            filter,
            best_of_series: db.setting("best_of_series").as_deref() == Some("1"),
        };
        let auto_advance = db.setting("auto_advance").as_deref() == Some("1");
        let subfolders = db.setting("subfolders").as_deref() == Some("1");
        // Only the photo, the filmstrip and the info bar by default.
        let show_toolbar = db.setting("top_bar").as_deref() == Some("1");
        let show_filmstrip = db.setting("filmstrip").as_deref() != Some("0");
        let details = db
            .setting("details_mode")
            .and_then(|m| DetailsMode::from_id(&m))
            .unwrap_or(DetailsMode::Off);

        let mut app = Self {
            loader: Loader::new(
                ctx.clone(),
                START_TARGET,
                Arc::clone(&thumbs),
                Arc::clone(&files),
            ),
            analyzer: Analyzer::new(
                ctx.clone(),
                Arc::clone(&db),
                Arc::clone(&board),
                Arc::clone(&thumbs),
                Arc::clone(&files),
            ),
            writer: RatingWriter::new(ctx.clone(), Arc::clone(&db), Arc::clone(&files)),
            deletions: DeleteQueue::new(deletion::move_to_trash),
            transfers: TransferQueue::new(Arc::clone(&files)),
            db,
            files,
            thumbs,
            board,
            dir: None,
            all: Arc::new(Vec::new()),
            all_index: HashMap::new(),
            view: View::default(),
            current: 0,
            pinned: None,
            options,
            view_version: 0,
            view_built: Instant::now(),
            percentiles: (u64::MAX, Percentiles::default()),
            session_ratings: HashMap::new(),
            session_labels: HashMap::new(),
            auto_advance,
            subfolders,
            target: None,
            zoom: viewer::Zoom::default(),
            show_toolbar,
            show_filmstrip,
            details,
            details_last: if details == DetailsMode::Off {
                DetailsMode::On
            } else {
                details
            },
            details_expanded: HashSet::new(),
            help_open: false,
            models_open: false,
            confirm: None,
            palette: None,
            action_menu: None,
            action_anchor: None,
            toolbar_before_actions: None,
            tab_presses: Vec::new(),
            language_flash: None,
            notice,
            started,
            logged_first_frame: false,
            logged_first_photo: false,
            edit: None,
            edit_thread: None,
            edit_busy: false,
        };
        if let Some(path) = start_path {
            app.open(&ctx, &path);
        }
        app
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

    /// Results from the background: deletions, copy/move, edits, new analysis marks and a
    /// finished "Delete models".
    fn poll_background(&mut self, ctx: &egui::Context) {
        self.process_deletions(ctx);
        self.poll_transfer(ctx);
        self.poll_edits();
        self.refresh_marks(ctx);
        if let Some(removal) = self.analyzer.take_removal() {
            self.notice = Some(match removal {
                Ok(()) => Notice::hint(i18n::t().models_deleted),
                Err(err) => Notice::error(err),
            });
        }
    }
}

impl eframe::App for CernoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let window = ui.max_rect();
        if !self.logged_first_frame {
            self.logged_first_frame = true;
            // Fill the work area of the monitor the window is on. Maximized stays on one
            // screen; F11 is the separate fullscreen switch.
            ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
            self.loader.start_prefetch();
            log::info!(
                "start-up: first frame after {} ms",
                self.started.elapsed().as_millis()
            );
        }
        self.update_target(&ctx, window.size());
        self.poll_background(&ctx);

        let layout = self.layout(window);
        let frames: Vec<viewer::Frame> = self
            .slots(layout.area)
            .iter()
            .filter_map(|slot| self.frame_of(&ctx, slot))
            .collect();
        self.handle_keys(&ctx, &frames);

        ui.painter().rect_filled(window, 0.0, tokens::CANVAS);
        self.draw_centre(ui, window, layout.area);
        if let Some(rect) = layout.filmstrip {
            self.draw_filmstrip(ui, rect);
        }
        if let Some(rect) = layout.info {
            self.draw_info_bar(ui, rect, layout.details);
        }
        if let Some(rect) = layout.toolbar {
            self.draw_toolbar(ui, rect);
        }
        self.draw_messages(ui, layout.area);
        self.draw_overlays(&ctx, window, &frames);
        let flash = if self.all.is_empty() {
            window
        } else {
            layout.area
        };
        self.draw_language_flash(&ctx, flash);
        overlays::drop_hint(ui, window);
    }

    /// `Tab` is Lightroom's panel key. egui would move keyboard focus to the next widget with
    /// it – and `Space` would then click that widget – so it never reaches egui.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        raw_input.events.retain(|event| match event {
            egui::Event::Key {
                key: Key::Tab,
                pressed,
                repeat,
                modifiers,
                ..
            } => {
                if *pressed && !*repeat && !modifiers.command && !modifiers.alt {
                    self.tab_presses.push(modifiers.shift);
                }
                false
            }
            _ => true,
        });
    }

    fn on_exit(&mut self) {
        // A rating given right before closing must still reach the file, and a deletion that
        // wasn't undone is carried out. A confirmed edit encodes first, then the writer
        // applies it.
        if let Some(thread) = self.edit_thread.take() {
            let _ = thread.join();
        }
        self.writer.shutdown();
        for outcome in self.transfers.finish_now() {
            self.retarget_moved(&outcome);
        }
        // Photos that really went to the trash teach For you, as during the session.
        for path in self.deletions.finish_now().deleted {
            if let Err(err) = self.db.record_deletion(&path.to_string_lossy()) {
                log::warn!("index: {err:#}");
            }
        }
    }
}
