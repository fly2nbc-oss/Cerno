//! The window. `CernoApp` holds the state; each submodule adds the methods for one part of it:
//!
//! - `browse`: opening a folder, the view (sort, filter), moving through it
//! - `marks`: stars, rejection, colour labels
//! - `describe`: comment and keywords (the details panel's description tab)
//! - `files`: copy, move, delete with the countdown
//! - `deleted`: the deleted photos in `.originals` – shown through 🗑, put back with `Ctrl+Z`
//! - `editing`: straighten, crop, quarter turns, `Ctrl+Z`
//! - `gate`: one action at a time on a photo
//! - `exiftool`: ExifTool – found or not, its download, the offer
//! - `keys`: the keyboard
//! - `photos`: the photo area, compare mode, mouse zoom and pan
//! - `frame`: the layout and the bars around the photo
//! - `menu`: menus, help, models card, confirmations
//! - `panels`: which bars show, the language
//! - `notice`: messages over the photo
//!
//! Drawing of the widgets themselves lives in `ui/`.

mod browse;
mod camera_time;
mod deleted;
mod describe;
mod editing;
mod exiftool;
mod external;
mod faces;
mod files;
mod frame;
mod gate;
mod keys;
mod marks;
mod menu;
mod name_list;
mod notice;
mod pairs;
mod panels;
mod photos;
mod video;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, Rect, ViewportCommand};

use crate::analysis::{Analyzer, ScoreBoard};
use crate::db::Db;
use crate::deletion::{self, DeleteQueue};
use crate::filelock::FileLocks;
use crate::i18n::{self, Lang};
use crate::loader::Loader;
use crate::metadata::{Description, Label, Rating};
use crate::paths;
use crate::rating::RatingWriter;
use crate::theme::tokens;
use crate::thumbs::Thumbs;
use crate::transfer::Queue as TransferQueue;
use crate::ui::details::{DetailsMode, DetailsTab};
use crate::ui::overlays;
use crate::ui::{description, palette, viewer};
use crate::view::{FilterKind, Media, Percentiles, PhotoFilter, SortKey, View, ViewOptions};

use editing::EditSession;
use menu::ConfirmAction;
use notice::Notice;

/// Decode size before the window exists, so the first photo decodes while the GPU starts up.
/// Covers screens up to 4K; once the photo area is known, that photo is decoded again for it.
const START_TARGET: [u32; 2] = [3840, 2160];

/// How long the photo area must keep its size before photos are decoded for it: a window
/// dragged larger, or maximized after the first frame, would otherwise decode them at every
/// step.
const TARGET_SETTLE: Duration = Duration::from_millis(200);

/// Set once the hint about the aesthetics model has been shown. Before 0.10 the same key
/// meant "the download dialog was declined", which also ends the hint.
const CLIP_OFFER_SHOWN: &str = "clip_download_declined";

/// Set once the hint about V2.5 has been shown (or a download was asked for) – CLIP users of
/// releases before the V2.5 download get it once.
const V25_OFFER_SHOWN: &str = "v25_offer_shown";

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
    /// What sorting and filtering look at, and the analysis works on: `all`, plus the deleted
    /// photos while the 🗑 box is ticked (`sync_library`).
    library: Arc<Vec<PathBuf>>,
    /// Index into `library`.
    all_index: HashMap<PathBuf, usize>,
    /// The deleted photos of the folder, lying in `.originals`.
    deleted: deleted::Deleted,
    /// The pasted file-name list (*Filter ▸ By file list …*).
    name_list: name_list::NameList,
    /// Camera clocks set right in the open folder (*Camera time …*).
    camera_time: camera_time::CameraTime,
    /// What is shown, after sorting, filtering and hiding pending deletions.
    view: View,
    current: usize,
    /// Compare mode: the photo pinned on the left. The current photo is shown on the right.
    pinned: Option<PathBuf>,
    /// The four-up view (`Shift+C`): the view index of the first of its four photos.
    quad: Option<usize>,
    options: ViewOptions,
    /// The photo "similar photos" (`M`) is about and its CLIP embedding, while that filter is
    /// on. The embedding stays even if the photo is deleted meanwhile.
    similar_to: Option<(PathBuf, Arc<[f32]>)>,
    /// The photos Top N picked (`view::pick_top`), kept until a filter changes or "Refresh
    /// order": picking again after every mark would slip the next photo into a rejected one's
    /// place unnoticed. Empty while Top N is off.
    top_pick: HashSet<PathBuf>,
    /// The options `top_pick` was made for, sort aside (`ViewOptions::top_key`): another sort
    /// keeps it.
    top_pick_for: Option<ViewOptions>,
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
    /// Comments and keywords given in this session; they win over the file the same way.
    session_descriptions: HashMap<PathBuf, Description>,
    /// Which tab the details panel shows (`B` opens the description, `G` the faces).
    details_tab: DetailsTab,
    /// The current photo's faces (`G`, `Shift+G`).
    faces: faces::Faces,
    /// The comment and keyword being typed in the description tab.
    drafts: description::Drafts,
    /// `0`–`5`, `X` and `6`–`9` also move to the next photo.
    auto_advance: bool,
    /// The open folder includes nested folders.
    subfolders: bool,
    /// RAW + JPG of one name are one photo (*RAW+JPG as one photo*, on by default).
    pair_mode: bool,
    /// The RAWs riding along with the open folder's JPEGs.
    pairs: crate::pairs::Pairs,
    /// What the pairs' RAW sidecars say, for the note where they differ.
    raw_marks: pairs::RawMarks,
    target: Option<[u32; 2]>,
    /// A new decode size and since when the photo area has had it (see `TARGET_SETTLE`).
    pending_target: Option<([u32; 2], Instant)>,
    zoom: viewer::Zoom,
    /// The check overlay over the photos (`O`); not saved.
    overlay: crate::overlay::Mode,
    /// The grid (`F7`) instead of the single photo; not saved.
    grid: bool,
    /// Its cell size, an index into `grid::STEPS`.
    grid_step: usize,
    /// The photo the grid last scrolled to: another current photo scrolls it into view.
    grid_shown: Option<usize>,
    /// Columns and cells per page of the last drawn grid, for `↑`/`↓` and Page Up / Down.
    grid_columns: usize,
    grid_page: usize,
    /// Top bar (`F`), filmstrip (`F6`) and details panel (`Tab`); the info bar always shows.
    show_toolbar: bool,
    show_filmstrip: bool,
    details: DetailsMode,
    /// The stage `Tab` brings back.
    details_last: DetailsMode,
    /// The CLIP attributes are folded out in the details panel (session-wide).
    attributes_open: bool,
    /// Help page over the photos (`H`, `F1`, `?`).
    help_open: bool,
    /// Its tab: the shortcuts or the tips (`←`/`→`); not saved.
    help_page: crate::ui::help::Page,
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
    /// The filter bar showed on its own (a filter hid everything) and the pointer is on it: it
    /// stays until the pointer leaves.
    toolbar_held: bool,
    /// `Tab` presses taken out of egui's input (`true` = with Shift), see `raw_input_hook`.
    tab_presses: Vec<bool>,
    /// When the language was last switched (the flag shows for a moment).
    language_flash: Option<Instant>,
    /// Message over the photo; hints fade, errors wait for Esc or a click.
    notice: Option<Notice>,
    /// ExifTool: found or not, its download (Windows).
    exiftool: exiftool::ExifToolSetup,
    /// Process start, for the start-up log lines.
    started: Instant,
    logged_first_frame: bool,
    logged_first_photo: bool,
    /// ffmpeg was not found at start: videos show a placeholder instead of a frame.
    no_ffmpeg: bool,
    /// The program `E` opens photos in (remembered).
    external_editor: Option<crate::external::Editor>,
    /// The programs the system offers, per file extension (asked once).
    editors: HashMap<String, Vec<crate::external::Editor>>,
    /// Photos opened in another program, watched for saves.
    watched: Vec<external::Watched>,
    /// A program being started (its original kept first).
    launching: Option<std::sync::mpsc::Receiver<external::Launched>>,
    external_checked: Instant,
    /// Straighten or crop, while it is open. The saved zoom comes back on Enter or Esc.
    edit: Option<EditSession>,
    /// Encodes a confirmed edit. Joined on exit so the write is not lost.
    edit_thread: Option<JoinHandle<()>>,
    /// A quarter turn or a re-encode is still in the writer.
    edit_busy: bool,
    /// The video playing (or paused) in the single view (`video`).
    video: Option<video::Session>,
    /// Stopped players, until their files are closed.
    video_releases: Vec<crate::playback::Release>,
    /// Volume 0..=1 and sound off, saved.
    video_volume: f32,
    video_muted: bool,
    /// "Plays without sound" (no sound device) was shown in this run.
    video_silent_told: bool,
    /// The streams of the video the details panel shows, read in the background.
    media_probe: Option<video::Probe>,
}

impl CernoApp {
    /// Runs before the window exists (see `main`): opening the start folder here lets the first
    /// decode overlap with the GPU and window set-up.
    pub fn new(ctx: &egui::Context, start_path: Option<PathBuf>, started: Instant) -> Self {
        let ctx = ctx.clone();
        keys::keep_zoom_keys(&ctx);
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
            similar: false,
            media: db
                .setting("media")
                .and_then(|m| Media::from_id(&m))
                .unwrap_or_default(),
            top: None,
            hide_rejected: db.setting("hide_rejected").as_deref() == Some("1"),
            name_list: false,
        };
        let auto_advance = db.setting("auto_advance").as_deref() == Some("1");
        let subfolders = db.setting("subfolders").as_deref() == Some("1");
        let pair_mode = db.setting(pairs::SETTING).as_deref() != Some("0");
        // Only the photo, the filmstrip and the info bar by default.
        let show_toolbar = db.setting("top_bar").as_deref() == Some("1");
        let show_filmstrip = db.setting("filmstrip").as_deref() != Some("0");
        let external_editor = db
            .setting(external::SETTING)
            .and_then(|text| crate::external::Editor::from_setting(&text));
        let details = db
            .setting("details_mode")
            .and_then(|m| DetailsMode::from_id(&m))
            .unwrap_or(DetailsMode::Off);
        let details_tab = db
            .setting("details_tab")
            .and_then(|id| DetailsTab::from_id(&id))
            .unwrap_or(DetailsTab::Values);
        let (video_volume, video_muted) = video::saved_volume(&db);

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
            deletions: DeleteQueue::new(deletion::set_aside),
            transfers: TransferQueue::new(Arc::clone(&files)),
            db,
            files,
            thumbs,
            board,
            dir: None,
            all: Arc::new(Vec::new()),
            library: Arc::new(Vec::new()),
            all_index: HashMap::new(),
            deleted: deleted::Deleted::default(),
            name_list: name_list::NameList::default(),
            camera_time: camera_time::CameraTime::default(),
            view: View::default(),
            current: 0,
            pinned: None,
            quad: None,
            options,
            similar_to: None,
            top_pick: HashSet::new(),
            top_pick_for: None,
            view_version: 0,
            view_built: Instant::now(),
            percentiles: (u64::MAX, Percentiles::default()),
            session_ratings: HashMap::new(),
            session_labels: HashMap::new(),
            session_descriptions: HashMap::new(),
            details_tab,
            faces: faces::Faces::default(),
            drafts: description::Drafts::default(),
            auto_advance,
            subfolders,
            pair_mode,
            pairs: crate::pairs::Pairs::default(),
            raw_marks: pairs::RawMarks::default(),
            target: None,
            pending_target: None,
            zoom: viewer::Zoom::default(),
            overlay: crate::overlay::Mode::Off,
            grid: false,
            grid_step: crate::ui::grid::DEFAULT_STEP,
            grid_shown: None,
            grid_columns: 1,
            grid_page: 1,
            show_toolbar,
            show_filmstrip,
            details,
            details_last: if details == DetailsMode::Off {
                DetailsMode::On
            } else {
                details
            },
            attributes_open: false,
            help_open: false,
            help_page: crate::ui::help::Page::Keys,
            models_open: false,
            confirm: None,
            palette: None,
            action_menu: None,
            action_anchor: None,
            toolbar_before_actions: None,
            toolbar_held: false,
            tab_presses: Vec::new(),
            language_flash: None,
            notice,
            exiftool: exiftool::ExifToolSetup::new(),
            started,
            logged_first_frame: false,
            logged_first_photo: false,
            no_ffmpeg: crate::video::locate().is_none(),
            external_editor,
            editors: HashMap::new(),
            watched: Vec::new(),
            launching: None,
            external_checked: Instant::now(),
            edit: None,
            edit_thread: None,
            edit_busy: false,
            video: None,
            video_releases: Vec::new(),
            video_volume,
            video_muted,
            video_silent_told: false,
            media_probe: None,
        };
        if let Some(path) = start_path {
            app.open(&ctx, &path);
        }
        app
    }

    /// Decode size: the photo area in physical pixels (`viewer::decode_size`), so a fitted
    /// photo is drawn pixel for pixel. Taken once the area has kept it for [`TARGET_SETTLE`].
    /// The first one also starts the prefetch – after it, so the neighbours are decoded for
    /// the area and not for the start-up guess.
    fn update_target(&mut self, ctx: &egui::Context, areas: &[Rect]) {
        let max_side = ctx.input(|i| i.max_texture_side) as u32;
        let Some(wanted) = viewer::decode_size(areas, ctx.pixels_per_point(), max_side) else {
            return;
        };
        if self.target == Some(wanted) {
            self.pending_target = None;
            return;
        }
        let now = Instant::now();
        let since = match self.pending_target {
            Some((pending, since)) if pending == wanted => since,
            _ => {
                self.pending_target = Some((wanted, now));
                now
            }
        };
        let waited = now - since;
        if waited < TARGET_SETTLE {
            ctx.request_repaint_after(TARGET_SETTLE - waited);
            return;
        }
        self.pending_target = None;
        let first = self.target.replace(wanted).is_none();
        self.loader.set_target(wanted);
        if first {
            self.loader.start_prefetch();
        }
    }

    /// Results from the background: deletions, copy/move, edits, new analysis marks and a
    /// finished "Delete models".
    fn poll_background(&mut self, ctx: &egui::Context) {
        self.process_deletions(ctx);
        self.poll_deleted(ctx);
        self.poll_raw_marks();
        self.poll_faces();
        self.poll_transfer(ctx);
        self.poll_external(ctx);
        self.poll_edits();
        self.poll_exiftool();
        self.refresh_marks(ctx);
        if let Some(removal) = self.analyzer.take_removal() {
            self.notice = Some(match removal {
                Ok(()) => Notice::hint(i18n::t().models_deleted),
                Err(err) => Notice::error(err),
            });
        }
        if let Some(download) = self.analyzer.take_download() {
            let t = i18n::t();
            self.notice = Some(match download {
                Ok(()) => Notice::hint(t.models_downloaded),
                Err(err) => Notice::error((t.download_failed)(&err)),
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
            log::info!(
                "start-up: first frame after {} ms",
                self.started.elapsed().as_millis()
            );
        }
        self.poll_background(&ctx);

        let layout = self.layout(window);
        // The grid shows no photo, so it keeps the decode size.
        if !self.grid {
            let areas = self.photo_areas(layout.area);
            self.update_target(&ctx, &areas);
        }
        // The grid shows no photo: zoom keys go to its cell size, not to a hidden photo. A
        // video has no frame here either: it is not zoomed.
        let frames: Vec<viewer::Frame> = if self.grid {
            Vec::new()
        } else {
            self.slots(layout.area)
                .iter()
                .filter(|slot| !self.slot_is_video(slot))
                .filter_map(|slot| self.frame_of(&ctx, slot))
                .collect()
        };
        self.apply_face_zoom(&frames);
        self.handle_keys(&ctx, &frames);
        self.update_video(&ctx);
        // Again: a key can empty the view (a mark took the last photo the filter showed), and
        // the bars of the old layout would then draw cells of photos that are gone.
        let layout = self.layout(window);

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

    /// `Tab` toggles the details panel. egui would move keyboard focus to the next widget with
    /// it – and `Space` would then click that widget – so it never reaches egui.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // While a comment or keyword is being typed, `Tab` belongs to the field.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
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
        // A running ExifTool download stops; its `.part` waits for the next attempt.
        self.exiftool.cancel();
        // A rating given right before closing must still reach the file, and a deletion that
        // wasn't undone is carried out. A confirmed edit encodes first, then the writer
        // applies it.
        if let Some(thread) = self.edit_thread.take() {
            let _ = thread.join();
        }
        // A comment still in its field is written with the rest.
        self.commit_comment();
        // A playing video keeps its file open: closed before anything moves it.
        self.stop_video();
        self.wait_for_videos(Duration::from_secs(2));
        self.writer.shutdown();
        for outcome in self.transfers.finish_now() {
            self.retarget_moved(&outcome);
        }
        // Photos that were really set aside teach For you, as during the session.
        for (path, aside) in self.deletions.finish_now().deleted {
            self.forget_deleted(&path, &aside);
        }
        self.finish_restores();
    }
}
