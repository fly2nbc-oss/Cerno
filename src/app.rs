//! The window: state, keyboard and mouse model, layout. Drawing lives in `ui/`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, CursorIcon, Id, Key, LayerId, OpenUrl, Order, PointerButton, Rect, Sense, Vec2,
    ViewportCommand, pos2, vec2,
};

use crate::analysis::{Analyzer, ScoreBoard};
use crate::db::Db;
use crate::deletion::{self, DeleteQueue};
use crate::i18n::{self, Lang};
use crate::library::{self, Library};
use crate::loader::{LoadedImage, Loader, Lookup};
use crate::paths;
use crate::rating::RatingWriter;
use crate::theme::tokens;
use crate::thumbs::Thumbs;
use crate::ui::bars::Panels;
use crate::ui::icons::Panel;
use crate::ui::{bars, details, filmstrip, help, viewer};
use crate::view::{
    self, BLURRY_PERCENTILE, Facts, Percentiles, RatingFilter, SortKey, ViewOptions,
};

/// Decode size before the window exists, so the first photo decodes while the GPU starts up.
/// Covers screens up to 4K; the real monitor size replaces it on the first frame (a larger
/// screen re-decodes).
const START_TARGET: [u32; 2] = [3840, 2160];
/// Decode size if the monitor size is unknown (Wayland never reports it).
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
/// Gap between the two photos in compare mode.
const COMPARE_GUTTER: f32 = 4.0;
/// How long the flag stays after switching the language, and how long it fades out.
const LANGUAGE_FLASH: Duration = Duration::from_millis(1400);
const LANGUAGE_FADE: Duration = Duration::from_millis(450);

pub struct CernoApp {
    db: Arc<Db>,
    thumbs: Arc<Thumbs>,
    board: Arc<ScoreBoard>,
    loader: Loader,
    analyzer: Analyzer,
    writer: RatingWriter,
    /// Deleted photos wait here, already hidden, until the countdown runs out.
    deletions: DeleteQueue,

    dir: Option<PathBuf>,
    /// Every photo of the folder, in name order.
    all: Arc<Vec<PathBuf>>,
    all_index: HashMap<PathBuf, usize>,
    /// What is shown, after sorting, filtering and hiding pending deletions.
    view: Arc<Vec<PathBuf>>,
    current: usize,
    /// Compare mode: the photo pinned on the left. The current photo is shown on the right.
    pinned: Option<PathBuf>,
    options: ViewOptions,
    /// Score board version the view was built from.
    view_version: u64,
    /// Sorted sharpness values of the folder, for percentiles (board version, values).
    percentiles: (u64, Percentiles),

    /// Ratings given in this session; they win over the value read from the file, whose
    /// write may still be pending.
    session_ratings: HashMap<PathBuf, Option<u8>>,
    target: Option<[u32; 2]>,
    zoom: viewer::Zoom,
    /// Top bar (`B`), filmstrip (`T`) and details panel (`P`); the info bar always shows.
    show_toolbar: bool,
    show_filmstrip: bool,
    show_details: bool,
    /// Help page over the photos (`H`, `F1`).
    help_open: bool,
    /// When the language was last switched (the flag shows for a moment).
    language_flash: Option<Instant>,
    notice: Option<String>,
    /// Process start, for the start-up log lines.
    started: Instant,
    logged_first_frame: bool,
    logged_first_photo: bool,
}

struct KeyInput {
    next: bool,
    prev: bool,
    first: bool,
    last: bool,
    rating: Option<Option<u8>>,
    delete: bool,
    compare: bool,
    keep_left: bool,
    keep_right: bool,
    toggle_fullscreen: bool,
    escape: bool,
    toggle_toolbar: bool,
    toggle_panels: bool,
    toggle_filmstrip: bool,
    toggle_details: bool,
    help: bool,
    language: bool,
    toggle_zoom: bool,
    zoom_in: bool,
    zoom_out: bool,
    open: bool,
    is_fullscreen: bool,
}

/// One photo slot on screen: which photo, where, and which side (compare mode).
#[derive(Clone, Copy)]
struct Slot {
    index: usize,
    area: Rect,
    side: Side,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Single,
    Left,
    Right,
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
                notice = Some((i18n::t().db_unavailable)(&format!("{err:#}")));
                Db::open_in_memory().expect("in-memory SQLite")
            });
        if let Some(lang) = db.setting("language").and_then(|c| Lang::from_code(&c)) {
            i18n::set(lang);
        }
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
        let show_toolbar = db.setting("toolbar").as_deref() != Some("0");
        let show_filmstrip = db.setting("filmstrip").as_deref() != Some("0");
        let show_details = db.setting("details").as_deref() == Some("1");

        let mut app = Self {
            loader: Loader::new(ctx.clone(), START_TARGET, Arc::clone(&thumbs)),
            analyzer: Analyzer::new(
                ctx.clone(),
                Arc::clone(&db),
                Arc::clone(&board),
                Arc::clone(&thumbs),
            ),
            writer: RatingWriter::new(ctx.clone(), Arc::clone(&db)),
            deletions: DeleteQueue::new(deletion::move_to_trash),
            db,
            thumbs,
            board,
            dir: None,
            all: Arc::new(Vec::new()),
            all_index: HashMap::new(),
            view: Arc::new(Vec::new()),
            current: 0,
            pinned: None,
            options,
            view_version: 0,
            percentiles: (u64::MAX, Percentiles::default()),
            session_ratings: HashMap::new(),
            target: None,
            zoom: viewer::Zoom::default(),
            show_toolbar,
            show_filmstrip,
            show_details,
            help_open: false,
            language_flash: None,
            notice,
            started,
            logged_first_frame: false,
            logged_first_photo: false,
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

    fn open(&mut self, ctx: &egui::Context, path: &Path) {
        let (library, index) = match Library::open(path) {
            Ok(opened) => opened,
            Err(err) => {
                let path = path.display().to_string();
                self.notice = Some((i18n::t().cannot_open)(&path, &err.to_string()));
                return;
            }
        };
        self.notice = library
            .paths
            .is_empty()
            .then(|| (i18n::t().no_photos_in)(&library.dir.display().to_string()));
        self.all_index = index_of(&library.paths);
        // A photo that was opened directly stays selected; a folder starts at the top of the
        // (possibly sorted) view.
        let start = path
            .is_file()
            .then(|| library.paths.get(index).cloned())
            .flatten();
        self.all = Arc::clone(&library.paths);
        self.dir = Some(library.dir);
        self.pinned = None;
        self.thumbs.clear();
        // Only needed when the view depends on scores; the analysis fills the board anyway,
        // and on big folders the lookups would delay the first frame.
        if self.options.depends_on_scores() {
            self.analyzer.preload(&self.all);
        }
        self.analyzer.set_library(Arc::clone(&self.all), index);
        self.view = Arc::new(Vec::new());
        self.rebuild_view(ctx, start);
    }

    /// Re-applies sorting, filtering and pending deletions, staying on `keep` (or the current
    /// photo) if it is still shown.
    fn rebuild_view(&mut self, ctx: &egui::Context, keep: Option<PathBuf>) {
        let keep = keep.or_else(|| self.view.get(self.current).cloned());
        let mut view = view::build(
            &self.all,
            self.options,
            |p| self.facts(p),
            &self.session_ratings,
        );
        view.retain(|p| !self.deletions.is_hidden(p));

        // Comparing needs the pinned photo plus at least one other.
        if self.pinned.as_ref().is_some_and(|p| !view.contains(p)) || view.len() < 2 {
            self.pinned = None;
        }
        let pinned = self
            .pinned
            .as_ref()
            .and_then(|p| view.iter().position(|q| q == p));
        let current = keep
            .and_then(|k| view.iter().position(|p| *p == k))
            .unwrap_or(0);
        self.current = view::skip_pinned(view.len(), current, pinned, 1).unwrap_or(current);
        self.view = Arc::new(view);
        self.view_version = self.board.version();
        self.loader
            .set_library(Arc::clone(&self.view), self.current, pinned);
        self.sync_analyzer();
        self.update_title(ctx);
    }

    fn pinned_index(&self) -> Option<usize> {
        let pinned = self.pinned.as_ref()?;
        self.view.iter().position(|p| p == pinned)
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
        let mut dialog = rfd::FileDialog::new().set_title(i18n::t().open_folder);
        if let Some(dir) = &self.dir {
            dialog = dialog.set_directory(dir);
        }
        if let Some(dir) = dialog.pick_folder() {
            self.open(ctx, &dir);
        }
    }

    fn confirm_model_download(&self) {
        let t = i18n::t();
        let answer = rfd::MessageDialog::new()
            .set_title(t.download_title)
            .set_description((t.download_text)(
                crate::analysis::aesthetic::MODEL_BYTES as f64 / 1e9,
            ))
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if answer == rfd::MessageDialogResult::Yes {
            self.analyzer.download_model();
        }
    }

    /// Moves to `index`, stepping over the pinned photo in `direction` in compare mode.
    fn go_to(&mut self, ctx: &egui::Context, index: usize, direction: isize) {
        let Some(last) = self.view.len().checked_sub(1) else {
            return;
        };
        let Some(index) = view::skip_pinned(
            self.view.len(),
            index.min(last),
            self.pinned_index(),
            direction,
        ) else {
            return;
        };
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
        self.analyzer.taste_changed();
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

    /// What sorting, filtering and the UI know about a photo.
    fn facts(&self, path: &Path) -> Option<Facts> {
        let known = self.board.get(path)?;
        Some(Facts {
            rating: known.rating,
            scores: known.scores,
            personal: self.analyzer.personal(path),
        })
    }

    /// Sharpness percentiles of the folder, recomputed when scores changed.
    fn percentiles(&mut self) -> &Percentiles {
        let version = self.board.version();
        if self.percentiles.0 != version {
            let board = &self.board;
            let scores: Vec<_> = self
                .all
                .iter()
                .filter_map(|p| board.get(p).map(|k| k.scores))
                .collect();
            self.percentiles = (version, Percentiles::from_scores(scores.iter()));
        }
        &self.percentiles.1
    }

    /// The photo to show after the one at `index` disappears: the next one, otherwise the
    /// previous one – never the pinned photo or one in `exclude`.
    fn neighbour(&self, index: usize, exclude: &[&PathBuf]) -> Option<PathBuf> {
        let usable = |p: &&PathBuf| !exclude.contains(p) && Some(*p) != self.pinned.as_ref();
        let after = self.view.get(index + 1..).unwrap_or_default();
        let before = self.view.get(..index).unwrap_or_default();
        after
            .iter()
            .find(usable)
            .or_else(|| before.iter().rev().find(usable))
            .cloned()
    }

    /// Hides the photo at once; it goes to the trash when the countdown runs out.
    fn delete(&mut self, ctx: &egui::Context, path: PathBuf, keep: Option<PathBuf>) {
        self.deletions.push(path, Instant::now());
        self.rebuild_view(ctx, keep);
    }

    fn delete_current(&mut self, ctx: &egui::Context) {
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        let keep = self.neighbour(self.current, &[&path]);
        self.delete(ctx, path, keep);
    }

    /// Esc while the countdown runs: every waiting photo comes back.
    fn undo_deletions(&mut self, ctx: &egui::Context) {
        if self.deletions.cancel() > 0 {
            self.rebuild_view(ctx, None);
        }
    }

    /// Starts due deletions and applies finished ones.
    fn process_deletions(&mut self, ctx: &egui::Context) {
        let repaint = ctx.clone();
        self.deletions
            .tick(Instant::now(), move || repaint.request_repaint());
        if let Some(done) = self.deletions.poll() {
            if !done.deleted.is_empty() {
                let gone: HashSet<&PathBuf> = done.deleted.iter().collect();
                let all: Vec<PathBuf> = self
                    .all
                    .iter()
                    .filter(|p| !gone.contains(p))
                    .cloned()
                    .collect();
                for path in &done.deleted {
                    self.session_ratings.remove(path);
                    // Deleted photos teach the taste model what the user doesn't like.
                    if let Err(err) = self.db.record_deletion(&path.to_string_lossy()) {
                        log::warn!("index: {err:#}");
                    }
                }
                self.analyzer.taste_changed();
                self.all_index = index_of(&all);
                self.all = Arc::new(all);
                let current = self
                    .view
                    .get(self.current)
                    .and_then(|p| self.all_index.get(p))
                    .copied()
                    .unwrap_or(0);
                self.analyzer.set_library(Arc::clone(&self.all), current);
            }
            if let Some((path, err)) = done.failed.first() {
                self.notice = Some((i18n::t().delete_failed)(
                    done.failed.len(),
                    &library::file_name_lossy(path),
                    err,
                ));
            }
            self.rebuild_view(ctx, None);
        }
        if self.deletions.countdown(Instant::now()).is_some() {
            // Animates the countdown bar and makes sure it fires without input.
            ctx.request_repaint();
        }
    }

    /// `C`: pin the current photo on the left and show the next one on the right – or leave
    /// compare mode.
    fn toggle_compare(&mut self, ctx: &egui::Context) {
        if self.pinned.take().is_some() {
            self.loader.set_pinned(None);
            return;
        }
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        if self.view.len() < 2 {
            self.notice = Some(i18n::t().compare_needs_two.to_owned());
            return;
        }
        let right = self.neighbour(self.current, &[&path]);
        self.pinned = Some(path);
        self.rebuild_view(ctx, right);
    }

    /// `A`: the left photo wins, the right one is deleted and the next photo moves in.
    fn keep_left(&mut self, ctx: &egui::Context) {
        if self.pinned.is_some() {
            self.delete_current(ctx);
        }
    }

    /// `D`: the right photo wins and moves to the left, the left one is deleted.
    fn keep_right(&mut self, ctx: &egui::Context) {
        let (Some(left), Some(right)) = (self.pinned.clone(), self.view.get(self.current).cloned())
        else {
            return;
        };
        let next = self.neighbour(self.current, &[&left, &right]);
        self.pinned = Some(right);
        self.delete(ctx, left, next);
    }

    fn panels(&self) -> Panels {
        Panels {
            toolbar: self.show_toolbar,
            details: self.show_details,
            filmstrip: self.show_filmstrip,
        }
    }

    fn set_panels(&mut self, panels: Panels) {
        self.show_toolbar = panels.toolbar;
        self.show_details = panels.details;
        self.show_filmstrip = panels.filmstrip;
        for (key, shown) in [
            ("toolbar", panels.toolbar),
            ("details", panels.details),
            ("filmstrip", panels.filmstrip),
        ] {
            self.db.put_setting(key, if shown { "1" } else { "0" });
        }
    }

    fn toggle_panel(&mut self, panel: Panel) {
        let mut panels = self.panels();
        match panel {
            Panel::Top => panels.toolbar = !panels.toolbar,
            Panel::Right => panels.details = !panels.details,
            Panel::Bottom => panels.filmstrip = !panels.filmstrip,
        }
        self.set_panels(panels);
    }

    /// `I`: hides top bar, details and filmstrip together – or shows all three if none is.
    /// The info bar stays either way.
    fn toggle_all_panels(&mut self) {
        let Panels {
            toolbar,
            details,
            filmstrip,
        } = self.panels();
        let show = !(toolbar || details || filmstrip);
        self.set_panels(Panels {
            toolbar: show,
            details: show,
            filmstrip: show,
        });
    }

    fn switch_language(&mut self, ctx: &egui::Context) {
        let next = i18n::current().next();
        i18n::set(next);
        self.db.put_setting("language", next.code());
        self.language_flash = Some(Instant::now());
        ctx.request_repaint();
    }

    /// The flag in the middle of the photo area for a moment after switching.
    fn draw_language_flash(&mut self, ctx: &egui::Context, area: Rect) {
        let Some(since) = self.language_flash else {
            return;
        };
        let elapsed = since.elapsed();
        if elapsed >= LANGUAGE_FLASH {
            self.language_flash = None;
            return;
        }
        let fade_start = LANGUAGE_FLASH - LANGUAGE_FADE;
        let opacity = if elapsed <= fade_start {
            1.0
        } else {
            1.0 - (elapsed - fade_start).as_secs_f32() / LANGUAGE_FADE.as_secs_f32()
        };
        let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("language-flash")));
        bars::language_flash(&painter, area, i18n::current(), opacity);
        ctx.request_repaint();
    }

    /// Photo slots on screen: one, or pinned left + current right in compare mode.
    fn slots(&self, area: Rect) -> Vec<Slot> {
        match self.pinned_index() {
            Some(pinned) if pinned != self.current => {
                let half = (area.width() - COMPARE_GUTTER) / 2.0;
                let left = Rect::from_min_size(area.min, vec2(half, area.height()));
                let right = Rect::from_min_max(pos2(area.max.x - half, area.min.y), area.max);
                vec![
                    Slot {
                        index: pinned,
                        area: left,
                        side: Side::Left,
                    },
                    Slot {
                        index: self.current,
                        area: right,
                        side: Side::Right,
                    },
                ]
            }
            _ => vec![Slot {
                index: self.current,
                area,
                side: Side::Single,
            }],
        }
    }

    fn frame_of(&self, ctx: &egui::Context, slot: &Slot) -> Option<viewer::Frame> {
        match self.loader.get(slot.index) {
            Lookup::Ready(image) => Some(viewer::Frame {
                area: slot.area,
                image_size: image.original_size,
                pixels_per_point: ctx.pixels_per_point(),
            }),
            _ => None,
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context, frames: &[viewer::Frame]) {
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
                delete: plain && i.key_pressed(Key::Delete),
                compare: plain && i.key_pressed(Key::C),
                keep_left: plain && i.key_pressed(Key::A),
                keep_right: plain && i.key_pressed(Key::D),
                toggle_fullscreen: i.key_pressed(Key::F11) || (plain && i.key_pressed(Key::F)),
                escape: i.key_pressed(Key::Escape),
                toggle_toolbar: plain && i.key_pressed(Key::B),
                toggle_panels: plain && i.key_pressed(Key::I),
                toggle_filmstrip: plain && i.key_pressed(Key::T),
                toggle_details: plain && i.key_pressed(Key::P),
                help: i.key_pressed(Key::F1) || (plain && i.key_pressed(Key::H)),
                language: plain && i.key_pressed(Key::L),
                toggle_zoom: plain && i.key_pressed(Key::Z),
                zoom_in: !i.modifiers.command
                    && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)),
                zoom_out: !i.modifiers.command && i.key_pressed(Key::Minus),
                open: i.modifiers.command && i.key_pressed(Key::O),
                is_fullscreen: i.viewport().fullscreen.unwrap_or(false),
            }
        });

        if keys.language {
            self.switch_language(ctx);
        }
        // The help page is modal: only closing it (and switching the language) works.
        if self.help_open {
            if keys.help || keys.escape {
                self.help_open = false;
            }
            return;
        }
        // The start screen already is the help page.
        if keys.help && !self.all.is_empty() {
            self.help_open = true;
            return;
        }
        if keys.open {
            self.pick_folder(ctx);
        }
        if keys.next {
            self.go_to(ctx, self.current.saturating_add(1), 1);
        }
        if keys.prev {
            self.go_to(ctx, self.current.saturating_sub(1), -1);
        }
        if keys.first {
            self.go_to(ctx, 0, 1);
        }
        if keys.last {
            self.go_to(ctx, usize::MAX, -1);
        }
        if let Some(stars) = keys.rating {
            self.set_rating(stars);
        }
        if keys.compare {
            self.toggle_compare(ctx);
        }
        if keys.keep_left {
            self.keep_left(ctx);
        }
        if keys.keep_right {
            self.keep_right(ctx);
        }
        if keys.delete {
            self.delete_current(ctx);
        }
        if keys.toggle_panels {
            self.toggle_all_panels();
        }
        if keys.toggle_toolbar {
            self.toggle_panel(Panel::Top);
        }
        if keys.toggle_filmstrip {
            self.toggle_panel(Panel::Bottom);
        }
        if keys.toggle_details {
            self.toggle_panel(Panel::Right);
        }
        // Zoom keys act on the photo under the mouse, otherwise on the current (right) one.
        let pointer = ctx.pointer_hover_pos();
        let hovered = frames
            .iter()
            .find(|f| pointer.is_some_and(|p| f.area.contains(p)))
            .or(frames.last());
        if let Some(frame) = hovered {
            let pointer = pointer.filter(|p| frame.area.contains(*p));
            let anchor = pointer.unwrap_or(frame.area.center());
            if keys.toggle_zoom {
                self.zoom.toggle(frame, pointer);
            }
            if keys.zoom_in {
                self.zoom.zoom_by(frame, ZOOM_STEP, anchor);
            }
            if keys.zoom_out {
                self.zoom.zoom_by(frame, 1.0 / ZOOM_STEP, anchor);
            }
        }
        if keys.toggle_fullscreen {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!keys.is_fullscreen));
        }
        if keys.escape {
            if self.deletions.countdown(Instant::now()).is_some() {
                self.undo_deletions(ctx);
            } else if self.zoom.is_zoomed() {
                self.zoom.scale = None;
            } else if self.pinned.is_some() {
                self.toggle_compare(ctx);
            } else if keys.is_fullscreen {
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
            } else {
                self.notice = None;
            }
        }
    }

    /// Mouse on a photo: double-click toggles 100 %, wheel zooms, drag pans. Both photos in
    /// compare mode share one zoom, so they stay aligned.
    fn handle_mouse(&mut self, ui: &egui::Ui, frame: &viewer::Frame, side: Side) {
        let id = ui.id().with(("photo", side as u8));
        let response = ui.interact(frame.area, id, Sense::click_and_drag());
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

    fn draw_photos(&mut self, ui: &egui::Ui, slots: &[Slot]) {
        let ctx = ui.ctx().clone();
        for slot in slots {
            match self.loader.get(slot.index) {
                Lookup::Ready(image) => {
                    let frame = viewer::Frame {
                        area: slot.area,
                        image_size: image.original_size,
                        pixels_per_point: ctx.pixels_per_point(),
                    };
                    if !self.help_open {
                        self.handle_mouse(ui, &frame, slot.side);
                    }
                    let full = self.loader.full(slot.index);
                    let needs_full =
                        viewer::draw(ui.painter(), &frame, &self.zoom, &image, full.as_deref());
                    if needs_full && full.is_none() {
                        self.loader.request_full(slot.index);
                    }
                    if !self.logged_first_photo {
                        self.logged_first_photo = true;
                        log::info!(
                            "start-up: first photo drawn after {} ms",
                            self.started.elapsed().as_millis()
                        );
                    }
                    if slot.side != Side::Single {
                        let t = i18n::t();
                        let path = &self.view[slot.index];
                        let (side, key) = if slot.side == Side::Left {
                            (t.compare_left, "A")
                        } else {
                            (t.compare_right, "D")
                        };
                        bars::compare_label(
                            ui,
                            slot.area,
                            side,
                            &library::file_name_lossy(path),
                            self.rating_of(path, Some(&image)),
                            &(t.keeps_this)(key),
                        );
                    }
                }
                Lookup::Failed(message) => bars::centred_message(
                    ui,
                    slot.area,
                    &format!("{}\n{message}", i18n::t().cannot_show),
                    tokens::STATUS_ERROR,
                ),
                Lookup::Pending => {
                    bars::centred_message(ui, slot.area, i18n::t().loading, tokens::MUTED);
                }
            }
        }
    }
}

fn index_of(paths: &[PathBuf]) -> HashMap<PathBuf, usize> {
    paths
        .iter()
        .enumerate()
        .map(|(i, p)| (p.clone(), i))
        .collect()
}

impl eframe::App for CernoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let window = ui.max_rect();
        if !self.logged_first_frame {
            self.logged_first_frame = true;
            self.loader.start_prefetch();
            log::info!(
                "start-up: first frame after {} ms",
                self.started.elapsed().as_millis()
            );
        }
        self.update_target(&ctx, window.size());
        self.process_deletions(&ctx);

        // Layout: toolbar | photo(s) + details | filmstrip | info bar. The info bar always
        // shows; the toolbar also when a filter hides everything (to change it back).
        let mut area = window;
        let toolbar_rect = (!self.all.is_empty() && (self.show_toolbar || self.view.is_empty()))
            .then(|| {
                let r = Rect::from_min_size(window.min, vec2(window.width(), bars::TOOLBAR_HEIGHT));
                area.min.y = r.max.y;
                r
            });
        let info_rect = (!self.view.is_empty()).then(|| {
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
        let details_rect = (info_rect.is_some() && self.show_details).then(|| {
            let r = Rect::from_min_max(pos2(area.max.x - details::WIDTH, area.min.y), area.max);
            area.max.x = r.min.x;
            r
        });

        let frames: Vec<viewer::Frame> = self
            .slots(area)
            .iter()
            .filter_map(|slot| self.frame_of(&ctx, slot))
            .collect();
        self.handle_keys(&ctx, &frames);

        ui.painter().rect_filled(window, 0.0, tokens::CANVAS);
        if self.all.is_empty() {
            // Start screen: the help page with the "Open folder" button.
            self.help_open = false;
            let out = help::welcome(ui, window);
            if out.language {
                self.switch_language(&ctx);
            }
            if out.open_folder {
                self.pick_folder(&ctx);
            }
        } else if self.view.is_empty() {
            bars::centred_message(ui, area, i18n::t().no_match, tokens::MUTED);
        } else {
            // Navigation may have changed the photos; lay them out again.
            let slots = self.slots(area);
            self.draw_photos(ui, &slots);
        }

        if let Some(rect) = strip_rect {
            let view = Arc::clone(&self.view);
            let pinned = self.pinned_index();
            // Percentiles need `&mut self`; take them before borrowing `self` in the closure.
            let percentiles = self.percentiles().clone();
            let strip = filmstrip::draw(ui, rect, &view, self.current, &self.thumbs, |i| {
                let path = &view[i];
                let known = self.board.get(path);
                let blurry = known
                    .and_then(|k| percentiles.subject(&k.scores))
                    .filter(|(p, _)| *p < BLURRY_PERCENTILE)
                    .map(|(p, eyes)| (i18n::t().blurry_tooltip)(eyes, p * 100.0));
                let rating = match self.session_ratings.get(path) {
                    Some(stars) => *stars,
                    None => known.and_then(|k| k.rating),
                };
                filmstrip::CellInfo {
                    rating,
                    blurry,
                    pinned: pinned == Some(i),
                }
            });
            if let Some(index) = strip.clicked {
                self.go_to(&ctx, index, 1);
            }
            if strip.step != 0 && !self.help_open {
                let target = self.current.saturating_add_signed(strip.step);
                self.go_to(&ctx, target, strip.step.signum());
            }
        }

        if let Some(rect) = info_rect
            && let Some(path) = self.view.get(self.current).cloned()
        {
            let image = match self.loader.get(self.current) {
                Lookup::Ready(image) => Some(image),
                _ => None,
            };
            let scores = self.board.get(&path).map(|k| k.scores);
            let percentiles = self.percentiles().clone();
            let personal = self.analyzer.personal(&path);
            let bar = bars::InfoBar {
                name: &library::file_name_lossy(&path),
                position: (self.current + 1, self.view.len()),
                image: image.as_deref(),
                rating: self.rating_of(&path, image.as_deref()),
                analysed: scores.is_some(),
                aesthetics: [
                    scores.and_then(|s| s.aesthetic),
                    scores.and_then(|s| s.aesthetic25),
                ],
                personal,
                sharpness: scores.and_then(|s| percentiles.subject(&s)),
                saving: self.writer.status().pending > 0,
                zoom: self.zoom.scale.map(|s| s * 100.0),
                panels: self.panels(),
            };
            let out = bars::info_bar(ui, rect, &bar);
            if let Some(stars) = out.rating {
                self.set_rating(stars);
            }
            if let Some(panel) = out.toggle {
                self.toggle_panel(panel);
            }
            if out.help {
                self.help_open = true;
            }
            if out.language {
                self.switch_language(&ctx);
            }
            if let Some(url) = out.open_map {
                ctx.open_url(OpenUrl::new_tab(url));
            }
            if let Some(rect) = details_rect {
                let status = self.analyzer.status();
                details::draw(
                    ui,
                    rect,
                    &details::Details {
                        scores,
                        personal,
                        frame_percentile: scores.and_then(|s| percentiles.frame(&s)),
                        eyes_percentile: scores.and_then(|s| percentiles.eyes(&s)),
                        attributes: self.analyzer.attributes(&path),
                        status: &status,
                    },
                );
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

        if let Some((count, left)) = self.deletions.countdown(Instant::now()) {
            bars::delete_countdown(ui, area, count, left);
        }
        let writer_error = self
            .writer
            .status()
            .last_error
            .map(|e| (i18n::t().rating_not_saved)(&e));
        bars::notices(ui, area, writer_error.as_deref(), self.notice.as_deref());

        if self.help_open && !self.all.is_empty() {
            let out = help::overlay(&ctx, window);
            if out.language {
                self.switch_language(&ctx);
            }
            if out.close {
                self.help_open = false;
            }
        }
        self.draw_language_flash(&ctx, if self.all.is_empty() { window } else { area });
        bars::drop_hint(ui, window);
    }

    fn on_exit(&mut self) {
        // A rating given right before closing must still reach the file, and a deletion that
        // wasn't undone is carried out.
        self.writer.shutdown();
        self.deletions.finish_now();
    }
}
