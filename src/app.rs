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
use crate::db::{Db, FileStamp};
use crate::deletion::{self, DeleteQueue};
use crate::i18n::{self, Lang};
use crate::library::{self, Library};
use crate::loader::{LoadedImage, Loader, Lookup};
use crate::metadata::{Label, Rating};
use crate::paths;
use crate::rating::RatingWriter;
use crate::theme::tokens;
use crate::thumbs::Thumbs;
use crate::ui::bars::Panels;
use crate::ui::details::{self, DetailRow, DetailsMode, all_expanded, set_all_expanded};
use crate::ui::icons::Panel;
use crate::ui::{bars, filmstrip, help, palette, viewer};
use crate::view::{
    self, BLURRY_PERCENTILE, Facts, Percentiles, RatingFilter, SortKey, View, ViewOptions,
};

/// Decode size before the window exists, so the first photo decodes while the GPU starts up.
/// Covers screens up to 4K; the real monitor size replaces it on the first frame (a larger
/// screen re-decodes).
const START_TARGET: [u32; 2] = [3840, 2160];
/// Decode size if the monitor size is unknown (Wayland never reports it).
const FALLBACK_TARGET: [u32; 2] = [2560, 1440];
const STAR_KEYS: [(Key, Rating); 6] = [
    (Key::Num0, Rating::Unrated),
    (Key::Num1, Rating::Stars(1)),
    (Key::Num2, Rating::Stars(2)),
    (Key::Num3, Rating::Stars(3)),
    (Key::Num4, Rating::Stars(4)),
    (Key::Num5, Rating::Stars(5)),
];
/// Lightroom's colour keys. Purple has no digit; it lives in the command palette.
const LABEL_KEYS: [(Key, Label); 4] = [
    (Key::Num6, Label::Red),
    (Key::Num7, Label::Yellow),
    (Key::Num8, Label::Green),
    (Key::Num9, Label::Blue),
];
/// Zoom step for `+`/`-`.
const ZOOM_STEP: f32 = 1.25;
/// Gap between the two photos in compare mode.
const COMPARE_GUTTER: f32 = 4.0;
/// How long the flag stays after switching the language, and how long it fades out.
const LANGUAGE_FLASH: Duration = Duration::from_millis(1400);
const LANGUAGE_FADE: Duration = Duration::from_millis(450);
const CLIP_DOWNLOAD_DECLINED: &str = "clip_download_declined";

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
    view: View,
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
    session_ratings: HashMap<PathBuf, Rating>,
    /// Colour labels given in this session (`None` clears). They win over the file the same way.
    session_labels: HashMap<PathBuf, Option<Label>>,
    /// `0`–`5`, `X` and `6`–`9` also move to the next photo.
    auto_advance: bool,
    /// The open folder includes nested folders.
    subfolders: bool,
    target: Option<[u32; 2]>,
    zoom: viewer::Zoom,
    /// Top bar (`T`), filmstrip (`F6`) and details panel (`Tab`, stages with `I`); the info
    /// bar always shows. Lightroom's keys, so photographers feel at home.
    show_toolbar: bool,
    show_filmstrip: bool,
    details: DetailsMode,
    /// The stage `Tab` brings back.
    details_last: DetailsMode,
    /// Which detail rows show their explanation (session-wide).
    details_expanded: HashSet<DetailRow>,
    /// Automatic CLIP download prompt runs once after the first frame.
    clip_download_offer_done: bool,
    /// Help page over the photos (`H`, `F1`, `?`).
    help_open: bool,
    /// Command palette (`Ctrl+K`) while open.
    palette: Option<palette::State>,
    /// `Tab` presses taken out of egui's input (`true` = with Shift), see `raw_input_hook`.
    tab_presses: Vec<bool>,
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
    rating: Option<Rating>,
    /// `Shift+0…5`: rate and move on.
    rate_and_next: Option<Rating>,
    /// `X` (toggles), `Shift+X` rejects and moves on – like Lightroom.
    reject: bool,
    reject_and_next: bool,
    label: Option<Label>,
    /// `Shift+6…9`: set the colour and move on.
    label_and_next: Option<Label>,
    delete: bool,
    compare: bool,
    keep_left: bool,
    keep_right: bool,
    toggle_fullscreen: bool,
    escape: bool,
    toggle_toolbar: bool,
    toggle_filmstrip: bool,
    cycle_details: bool,
    help: bool,
    language: bool,
    palette: bool,
    toggle_zoom: bool,
    zoom_in: bool,
    zoom_out: bool,
    open: bool,
    is_fullscreen: bool,
}

/// What a palette entry does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Open,
    Sort(SortKey),
    Filter(RatingFilter),
    HideBlurry,
    Refresh,
    EnableAesthetics,
    TopBar,
    Details,
    Explanations,
    Filmstrip,
    AllPanels,
    Compare,
    Zoom,
    Fullscreen,
    First,
    Last,
    Reject,
    DeleteRejected,
    Label(Option<Label>),
    AutoAdvance,
    Subfolders,
    BestOfSeries,
    OnlyDuplicates,
    Language(Lang),
    Help,
}

/// `Shift` plus a physical key. With Shift the typed character depends on the layout
/// (`!`, `"`, `§`, `=` …), so the key itself is what counts.
fn shifted_key<T: Copy>(events: &[egui::Event], keys: &[(Key, T)]) -> Option<T> {
    events.iter().find_map(|event| match event {
        egui::Event::Key {
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } if modifiers.shift_only() => keys.iter().find(|(k, _)| k == key).map(|(_, value)| *value),
        _ => None,
    })
}

/// `Shift+0…5` as stars. Found by the physical key: with Shift the typed character is `!`,
/// `"`, `§` … depending on the keyboard layout.
fn shifted_digit(events: &[egui::Event]) -> Option<Rating> {
    shifted_key(events, &STAR_KEYS)
}

fn shifted_label(events: &[egui::Event]) -> Option<Label> {
    shifted_key(events, &LABEL_KEYS)
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
            label: db
                .setting("label_filter")
                .as_deref()
                .and_then(Label::from_stored),
            best_of_series: db.setting("best_of_series").as_deref() == Some("1"),
            only_duplicates: db.setting("only_duplicates").as_deref() == Some("1"),
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
            view: View::default(),
            current: 0,
            pinned: None,
            options,
            view_version: 0,
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
            clip_download_offer_done: false,
            help_open: false,
            palette: None,
            tab_presses: Vec::new(),
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
        let (library, index) = match Library::open(path, self.subfolders) {
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
        self.view = View::default();
        self.rebuild_view(ctx, start);
    }

    /// Re-applies sorting, filtering and pending deletions, staying on `keep` (or the current
    /// photo) if it is still shown.
    fn rebuild_view(&mut self, ctx: &egui::Context, keep: Option<PathBuf>) {
        let keep = keep.or_else(|| self.view.get(self.current).cloned());
        let view = view::build(
            &self.all,
            self.options,
            |p| self.facts(p),
            &self.session_ratings,
            &self.session_labels,
            |p| self.deletions.is_hidden(p),
        );

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
        self.view = view;
        self.view_version = self.board.version();
        self.loader
            .set_library(Arc::clone(&self.view.paths), self.current, pinned);
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
        self.db.put_setting(
            "label_filter",
            self.options.label.map(|l| l.id()).unwrap_or(""),
        );
        self.db.put_setting(
            "best_of_series",
            if self.options.best_of_series {
                "1"
            } else {
                "0"
            },
        );
        self.db.put_setting(
            "only_duplicates",
            if self.options.only_duplicates {
                "1"
            } else {
                "0"
            },
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

    fn confirm_model_download(&mut self) {
        let t = i18n::t();
        let answer = rfd::MessageDialog::new()
            .set_title(t.download_title)
            .set_description((t.download_text)(
                crate::analysis::aesthetic::MODEL_BYTES as f64 / 1e9,
            ))
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if answer == rfd::MessageDialogResult::Yes {
            self.db.put_setting(CLIP_DOWNLOAD_DECLINED, "0");
            self.analyzer.download_model();
        } else {
            self.db.put_setting(CLIP_DOWNLOAD_DECLINED, "1");
        }
    }

    fn should_offer_clip_download(&self) -> bool {
        self.analyzer.clip_model_missing()
            && self.db.setting(CLIP_DOWNLOAD_DECLINED).as_deref() != Some("1")
    }

    fn offer_clip_download_if_needed(&mut self) {
        if self.should_offer_clip_download() {
            self.confirm_model_download();
        }
    }

    fn confirm_reset_taste(&mut self) {
        let t = i18n::t();
        let answer = rfd::MessageDialog::new()
            .set_title(t.confirm_reset_taste_title)
            .set_description(t.confirm_reset_taste_text)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if answer == rfd::MessageDialogResult::Yes {
            self.analyzer.reset_taste_learning();
        }
    }

    fn confirm_delete_models(&mut self) {
        let t = i18n::t();
        let answer = rfd::MessageDialog::new()
            .set_title(t.confirm_delete_models_title)
            .set_description(t.confirm_delete_models_text)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if answer == rfd::MessageDialogResult::Yes {
            match self.analyzer.delete_installed_models() {
                Ok(()) => {
                    self.db.put_setting(CLIP_DOWNLOAD_DECLINED, "0");
                    self.offer_clip_download_if_needed();
                }
                Err(err) => {
                    log::error!("delete models: {err:#}");
                    self.notice = Some(format!("{err:#}"));
                }
            }
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
            let (view, current) = (Arc::clone(&self.view.paths), self.current);
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
            Some(path) => format!(
                "{} – Cerno",
                self.dir
                    .as_ref()
                    .map(|dir| library::display_name(dir, path))
                    .unwrap_or_else(|| library::file_name_lossy(path))
            ),
            None => "Cerno".to_owned(),
        };
        ctx.send_viewport_cmd(ViewportCommand::Title(title));
    }

    /// Rates the current photo. `advance` moves on afterwards, unless the photo itself
    /// dropped out of the view (a filter or "best of series") – that already lands on the next one.
    fn set_rating(&mut self, ctx: &egui::Context, rating: Rating, advance: bool) {
        if let Some(path) = self.view.get(self.current).cloned() {
            self.rate(ctx, path, rating, advance);
        }
    }

    fn rate(&mut self, ctx: &egui::Context, path: PathBuf, rating: Rating, advance: bool) {
        let next = self.next_path();
        if let Ok(stamp) = FileStamp::of(&path)
            && let Ok(Some(record)) = self.db.lookup(&path.to_string_lossy(), stamp)
            && let Err(err) = self.db.allow_taste_for(record.fingerprint)
        {
            log::warn!("taste allow: {err:#}");
        }
        self.session_ratings.insert(path.clone(), rating);
        self.writer.set(path.clone(), rating);
        self.analyzer.taste_changed();
        self.finish_mark(ctx, &path, next, advance, self.rating_affects_view());
    }

    fn set_label(&mut self, ctx: &egui::Context, label: Option<Label>, advance: bool) {
        if let Some(path) = self.view.get(self.current).cloned() {
            self.apply_label(ctx, path, label, advance);
        }
    }

    /// Sets the colour, or removes it when the photo already has that colour.
    fn toggle_label(&mut self, ctx: &egui::Context, label: Label, advance: bool) {
        if let Some(path) = self.view.get(self.current).cloned() {
            let image = match self.loader.get(self.current) {
                Lookup::Ready(image) => Some(image),
                _ => None,
            };
            let next = if self.label_of(&path, image.as_deref()) == Some(label) {
                None
            } else {
                Some(label)
            };
            self.apply_label(ctx, path, next, advance);
        }
    }

    fn apply_label(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        label: Option<Label>,
        advance: bool,
    ) {
        let next = self.next_path();
        self.session_labels.insert(path.clone(), label);
        self.writer.set_label(path.clone(), label);
        self.finish_mark(ctx, &path, next, advance, self.options.label.is_some());
    }

    /// The photo after the current one, remembered before a rebuild moves things around.
    fn next_path(&self) -> Option<PathBuf> {
        self.view.get(self.current.saturating_add(1)).cloned()
    }

    /// Rebuilds when the mark changes which photos are shown or in which order, then optionally
    /// steps to `next` (the photo that followed this one before the rebuild).
    fn finish_mark(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        next: Option<PathBuf>,
        advance: bool,
        rebuild: bool,
    ) {
        if rebuild {
            self.rebuild_view(ctx, Some(path.to_path_buf()));
        }
        let still_here = self.view.get(self.current).is_some_and(|p| p == path);
        if still_here && !advance {
            return;
        }
        if let Some(next) = next.as_ref()
            && let Some(index) = self.view.iter().position(|p| p == next)
        {
            self.go_to(ctx, index, 1);
        } else if !still_here && next.is_none() && !self.view.is_empty() {
            // It was the last photo and left the view: stay at the new end.
            self.go_to(ctx, self.view.len() - 1, -1);
        }
    }

    fn rating_affects_view(&self) -> bool {
        self.options.best_of_series
            || self.options.filter != RatingFilter::All
            || matches!(self.options.sort, SortKey::Rating | SortKey::Taken)
    }

    /// `X`: rejects the current photo, or takes the rejection back.
    fn toggle_reject(&mut self, ctx: &egui::Context, advance: bool) {
        if let Some(path) = self.view.get(self.current).cloned() {
            let image = match self.loader.get(self.current) {
                Lookup::Ready(image) => Some(image),
                _ => None,
            };
            let rating = match self.rating_of(&path, image.as_deref()) {
                Rating::Rejected => Rating::Unrated,
                _ => Rating::Rejected,
            };
            self.rate(ctx, path, rating, advance);
        }
    }

    fn rating_of(&self, path: &Path, image: Option<&LoadedImage>) -> Rating {
        if let Some(rating) = self.session_ratings.get(path) {
            return *rating;
        }
        image
            .map(|i| i.rating.value)
            .or_else(|| self.board.get(path).map(|k| k.rating))
            .unwrap_or_default()
    }

    /// Rejected photos of the folder (for "delete rejected photos").
    fn rejected(&self) -> Vec<PathBuf> {
        self.all
            .iter()
            .filter(|p| !self.deletions.is_hidden(p))
            .filter(|p| {
                let rating = match self.session_ratings.get(*p) {
                    Some(rating) => *rating,
                    None => self.board.get(p).map(|k| k.rating).unwrap_or_default(),
                };
                rating == Rating::Rejected
            })
            .cloned()
            .collect()
    }

    /// All rejected photos go to the trash – with the usual countdown, `Esc` brings them back.
    fn delete_rejected(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        for path in self.rejected() {
            self.deletions.push(path, now);
        }
        self.rebuild_view(ctx, None);
    }

    /// What sorting, filtering and the UI know about a photo.
    fn facts(&self, path: &Path) -> Option<Facts> {
        let known = self.board.get(path)?;
        Some(Facts {
            rating: known.rating,
            label: known.label,
            taken_ms: known.taken_ms,
            fingerprint: known.fingerprint,
            scores: known.scores,
            personal: self.analyzer.personal(path),
        })
    }

    fn label_of(&self, path: &Path, image: Option<&LoadedImage>) -> Option<Label> {
        if let Some(label) = self.session_labels.get(path) {
            return *label;
        }
        if let Some(known) = self.board.get(path) {
            return known.label;
        }
        image.and_then(|image| image.label.known())
    }

    fn photo_name(&self, path: &Path) -> String {
        self.dir
            .as_ref()
            .map(|dir| library::display_name(dir, path))
            .unwrap_or_else(|| library::file_name_lossy(path))
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

    /// `A`: the left photo wins, the right one is rejected and the next photo moves in.
    fn keep_left(&mut self, ctx: &egui::Context) {
        if self.pinned.is_none() {
            return;
        }
        let Some(right) = self.view.get(self.current).cloned() else {
            return;
        };
        let next = self.neighbour(self.current, &[&right]);
        self.rate(ctx, right, Rating::Rejected, false);
        self.rebuild_view(ctx, next);
    }

    /// `D`: the right photo wins and moves to the left, the left one is rejected.
    fn keep_right(&mut self, ctx: &egui::Context) {
        let (Some(left), Some(right)) = (self.pinned.clone(), self.view.get(self.current).cloned())
        else {
            return;
        };
        let next = self.neighbour(self.current, &[&left, &right]);
        self.rate(ctx, left, Rating::Rejected, false);
        self.pinned = Some(right);
        self.rebuild_view(ctx, next);
    }

    fn panels(&self) -> Panels {
        Panels {
            toolbar: self.show_toolbar,
            details: self.details != DetailsMode::Off,
            filmstrip: self.show_filmstrip,
        }
    }

    fn save_panels(&self) {
        let flag = |shown: bool| if shown { "1" } else { "0" };
        self.db.put_setting("top_bar", flag(self.show_toolbar));
        self.db.put_setting("filmstrip", flag(self.show_filmstrip));
        self.db.put_setting("details_mode", self.details.id());
    }

    fn set_details(&mut self, mode: DetailsMode) {
        self.details = mode;
        if mode != DetailsMode::Off {
            self.details_last = mode;
        }
    }

    /// Buttons and `T` / `Tab` / `F6`. The details panel comes back at the stage it had.
    fn toggle_panel(&mut self, panel: Panel) {
        match panel {
            Panel::Top => self.show_toolbar = !self.show_toolbar,
            Panel::Bottom => self.show_filmstrip = !self.show_filmstrip,
            Panel::Right if self.details == DetailsMode::Off => self.details = self.details_last,
            Panel::Right => self.details = DetailsMode::Off,
        }
        self.save_panels();
    }

    /// `I`: open the panel or expand/collapse all explanations.
    fn toggle_explanations(&mut self) {
        if self.details == DetailsMode::Off {
            self.set_details(DetailsMode::On);
            set_all_expanded(&mut self.details_expanded, true);
        } else if all_expanded(&self.details_expanded) {
            set_all_expanded(&mut self.details_expanded, false);
        } else {
            set_all_expanded(&mut self.details_expanded, true);
        }
        self.save_panels();
    }

    /// `Shift+Tab`: hides top bar, details and filmstrip together – or shows all three if none
    /// is. The info bar stays either way.
    fn toggle_all_panels(&mut self) {
        let Panels {
            toolbar,
            details,
            filmstrip,
        } = self.panels();
        let show = !(toolbar || details || filmstrip);
        self.show_toolbar = show;
        self.show_filmstrip = show;
        self.details = if show {
            self.details_last
        } else {
            DetailsMode::Off
        };
        self.save_panels();
    }

    fn switch_language(&mut self, ctx: &egui::Context) {
        self.set_language(ctx, i18n::current().next());
    }

    fn set_language(&mut self, ctx: &egui::Context, lang: Lang) {
        i18n::set(lang);
        self.db.put_setting("language", lang.code());
        self.language_flash = Some(Instant::now());
        ctx.request_repaint();
    }

    /// Everything the palette offers, labelled in the current language.
    fn commands(&self) -> Vec<palette::Command<Action>> {
        use palette::Command;
        let t = i18n::t();
        let key = |k: &str| Some(k.to_owned());
        let mut list = vec![Command::new(
            Action::Open,
            t.open_folder,
            Some(i18n::with_ctrl("O")),
        )];
        if !self.all.is_empty() {
            for sort in SortKey::ALL {
                list.push(
                    Command::new(Action::Sort(sort), (t.sort)(sort.label()), None)
                        .checked(self.options.sort == sort),
                );
            }
            for filter in RatingFilter::ALL {
                list.push(
                    Command::new(Action::Filter(filter), (t.show)(&filter.label()), None)
                        .checked(self.options.filter == filter),
                );
            }
            list.push(
                Command::new(Action::HideBlurry, t.hide_blurry, None)
                    .checked(self.options.hide_blurry),
            );
            if self.options.depends_on_scores() && self.board.version() != self.view_version {
                list.push(Command::new(Action::Refresh, t.refresh_order, None));
            }
            list.extend([
                Command::new(Action::TopBar, t.button_toolbar, key("T")).checked(self.show_toolbar),
                Command::new(Action::Details, t.button_details, key("Tab"))
                    .checked(self.details != DetailsMode::Off),
                Command::new(Action::Explanations, t.cmd_explanations, key("I")).checked(
                    self.details != DetailsMode::Off && all_expanded(&self.details_expanded),
                ),
                Command::new(Action::Filmstrip, t.button_filmstrip, key("F6"))
                    .checked(self.show_filmstrip),
                Command::new(
                    Action::AllPanels,
                    t.cmd_all_panels,
                    Some(i18n::with_shift("Tab")),
                ),
                Command::new(Action::Compare, t.cmd_compare, key("C"))
                    .checked(self.pinned.is_some()),
                Command::new(Action::Zoom, t.cmd_zoom, key("Z")).checked(self.zoom.is_zoomed()),
                Command::new(Action::Fullscreen, t.cmd_fullscreen, key("F11")),
                Command::new(Action::First, t.cmd_first, None),
                Command::new(Action::Last, t.cmd_last, None),
                Command::new(Action::Reject, t.cmd_reject, key("X")),
                Command::new(Action::AutoAdvance, t.cmd_auto_advance, None)
                    .checked(self.auto_advance),
                Command::new(Action::Subfolders, t.cmd_subfolders, None).checked(self.subfolders),
                Command::new(Action::BestOfSeries, t.cmd_best_of_series, None)
                    .checked(self.options.best_of_series),
                Command::new(Action::OnlyDuplicates, t.cmd_only_duplicates, None)
                    .checked(self.options.only_duplicates),
            ]);
            let current_label = self.view.get(self.current).and_then(|path| {
                let image = match self.loader.get(self.current) {
                    Lookup::Ready(image) => Some(image),
                    _ => None,
                };
                self.label_of(path, image.as_deref())
            });
            for (label, shortcut) in [
                (Label::Red, Some("6")),
                (Label::Yellow, Some("7")),
                (Label::Green, Some("8")),
                (Label::Blue, Some("9")),
                (Label::Purple, None),
            ] {
                list.push(
                    Command::new(
                        Action::Label(Some(label)),
                        (t.cmd_label)(i18n::label_name(label)),
                        shortcut.map(str::to_owned),
                    )
                    .checked(current_label == Some(label)),
                );
            }
            let rejected = self.rejected().len();
            if rejected > 0 {
                list.push(Command::new(
                    Action::DeleteRejected,
                    (t.cmd_delete_rejected)(rejected),
                    None,
                ));
            }
        }
        if self.analyzer.status().aesthetics == crate::analysis::ModelState::Missing {
            list.push(Command::new(
                Action::EnableAesthetics,
                t.enable_aesthetics,
                None,
            ));
        }
        for lang in Lang::ALL {
            list.push(
                Command::new(Action::Language(lang), (t.cmd_language)(lang.name()), None)
                    .checked(i18n::current() == lang),
            );
        }
        list.push(Command::new(Action::Help, t.help_title, key("H")));
        list
    }

    fn run(&mut self, ctx: &egui::Context, action: Action, frames: &[viewer::Frame]) {
        match action {
            Action::Open => self.pick_folder(ctx),
            Action::Sort(sort) => self.change_options(ctx, |o| o.sort = sort),
            Action::Filter(filter) => self.change_options(ctx, |o| o.filter = filter),
            Action::HideBlurry => self.change_options(ctx, |o| o.hide_blurry = !o.hide_blurry),
            Action::Refresh => self.rebuild_view(ctx, None),
            Action::EnableAesthetics => self.confirm_model_download(),
            Action::TopBar => self.toggle_panel(Panel::Top),
            Action::Details => self.toggle_panel(Panel::Right),
            Action::Explanations => self.toggle_explanations(),
            Action::Filmstrip => self.toggle_panel(Panel::Bottom),
            Action::AllPanels => self.toggle_all_panels(),
            Action::Compare => self.toggle_compare(ctx),
            Action::Zoom => {
                if let Some(frame) = frames.last() {
                    self.zoom.toggle(frame, None);
                }
            }
            Action::Fullscreen => {
                let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
            }
            Action::First => self.go_to(ctx, 0, 1),
            Action::Last => self.go_to(ctx, usize::MAX, -1),
            Action::Reject => self.toggle_reject(ctx, false),
            Action::DeleteRejected => self.delete_rejected(ctx),
            Action::Label(label) => match label {
                Some(label) => self.toggle_label(ctx, label, false),
                None => self.set_label(ctx, None, false),
            },
            Action::AutoAdvance => {
                self.auto_advance = !self.auto_advance;
                self.db
                    .put_setting("auto_advance", if self.auto_advance { "1" } else { "0" });
            }
            Action::Subfolders => {
                self.subfolders = !self.subfolders;
                self.db
                    .put_setting("subfolders", if self.subfolders { "1" } else { "0" });
                if let Some(dir) = self.dir.clone() {
                    self.open(ctx, &dir);
                }
            }
            Action::BestOfSeries => {
                self.change_options(ctx, |o| o.best_of_series = !o.best_of_series);
            }
            Action::OnlyDuplicates => {
                self.change_options(ctx, |o| o.only_duplicates = !o.only_duplicates);
            }
            Action::Language(lang) => self.set_language(ctx, lang),
            Action::Help => self.help_open = !self.all.is_empty(),
        }
    }

    fn change_options(&mut self, ctx: &egui::Context, change: impl FnOnce(&mut ViewOptions)) {
        change(&mut self.options);
        self.save_options();
        self.rebuild_view(ctx, None);
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

    /// Lightroom's keys where Cerno has the same function (see the help page for all).
    fn handle_keys(&mut self, ctx: &egui::Context, frames: &[viewer::Frame]) {
        if let Some(path) =
            ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
        {
            self.open(ctx, &path);
        }
        let tabs = std::mem::take(&mut self.tab_presses);

        let keys = ctx.input(|i| {
            let plain = i.modifiers.is_none();
            let rate_and_next = shifted_digit(&i.events);
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
                rate_and_next,
                reject: plain && i.key_pressed(Key::X),
                reject_and_next: i.modifiers.shift_only() && i.key_pressed(Key::X),
                label: LABEL_KEYS
                    .iter()
                    .find(|(k, _)| plain && i.key_pressed(*k))
                    .map(|(_, label)| *label),
                label_and_next: shifted_label(&i.events),
                delete: plain && i.key_pressed(Key::Delete),
                compare: plain && i.key_pressed(Key::C),
                keep_left: plain && i.key_pressed(Key::A),
                keep_right: plain && i.key_pressed(Key::D),
                toggle_fullscreen: i.key_pressed(Key::F11) || (plain && i.key_pressed(Key::F)),
                escape: i.key_pressed(Key::Escape),
                toggle_toolbar: plain && i.key_pressed(Key::T),
                toggle_filmstrip: i.key_pressed(Key::F6),
                cycle_details: plain && i.key_pressed(Key::I),
                help: i.key_pressed(Key::F1)
                    || i.key_pressed(Key::Questionmark)
                    || (plain && i.key_pressed(Key::H)),
                language: i.modifiers.command && i.key_pressed(Key::L),
                palette: i.modifiers.command && i.key_pressed(Key::K),
                toggle_zoom: plain && i.key_pressed(Key::Z),
                // German layouts type "=" for Shift+0, which is "rate 0 and next" here.
                zoom_in: !i.modifiers.command
                    && rate_and_next.is_none()
                    && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)),
                zoom_out: !i.modifiers.command && i.key_pressed(Key::Minus),
                open: i.modifiers.command && i.key_pressed(Key::O),
                is_fullscreen: i.viewport().fullscreen.unwrap_or(false),
            }
        });

        if keys.language {
            self.switch_language(ctx);
        }
        // The palette handles its own keys (typing, arrows, Enter, Esc).
        if self.palette.is_some() {
            if keys.palette {
                self.palette = None;
            }
            return;
        }
        if keys.palette {
            self.help_open = false;
            self.palette = Some(palette::State::default());
            return;
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
            self.set_rating(ctx, stars, self.auto_advance);
        }
        if let Some(stars) = keys.rate_and_next {
            self.set_rating(ctx, stars, true);
        }
        if keys.reject {
            self.toggle_reject(ctx, self.auto_advance);
        }
        if keys.reject_and_next {
            self.set_rating(ctx, Rating::Rejected, true);
        }
        if let Some(label) = keys.label {
            self.toggle_label(ctx, label, self.auto_advance);
        }
        if let Some(label) = keys.label_and_next {
            self.set_label(ctx, Some(label), true);
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
        for shift in tabs {
            if shift {
                self.toggle_all_panels();
            } else {
                self.toggle_panel(Panel::Right);
            }
        }
        if keys.toggle_toolbar {
            self.toggle_panel(Panel::Top);
        }
        if keys.toggle_filmstrip {
            self.toggle_panel(Panel::Bottom);
        }
        if keys.cycle_details {
            self.toggle_explanations();
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
        } else if !self.clip_download_offer_done {
            self.clip_download_offer_done = true;
            self.offer_clip_download_if_needed();
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
        let details_rect = (info_rect.is_some() && self.details != DetailsMode::Off).then(|| {
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
            let paths = Arc::clone(&self.view.paths);
            let series = Arc::clone(&self.view.series);
            let duplicates = Arc::clone(&self.view.duplicate_of);
            let grouped = self.view.grouped;
            let pinned = self.pinned_index();
            let current_series = series
                .get(self.current)
                .and_then(|place| *place)
                .map(|p| p.id);
            // Percentiles need `&mut self`; take them before borrowing `self` in the closure.
            let percentiles = self.percentiles().clone();
            let strip =
                filmstrip::draw(ui, rect, &paths, self.current, &self.thumbs, grouped, |i| {
                    let path = &paths[i];
                    let known = self.board.get(path);
                    let blurry = known
                        .and_then(|k| percentiles.subject(&k.scores))
                        .filter(|(p, _)| *p < BLURRY_PERCENTILE)
                        .map(|(p, eyes)| (i18n::t().blurry_tooltip)(eyes, p * 100.0));
                    let rating = match self.session_ratings.get(path) {
                        Some(rating) => *rating,
                        None => known.map(|k| k.rating).unwrap_or_default(),
                    };
                    let place = series.get(i).and_then(|place| *place);
                    let more = (self.options.best_of_series && place.is_some_and(|p| p.len > 1))
                        .then(|| place.map(|p| p.len - 1))
                        .flatten();
                    filmstrip::CellInfo {
                        rating,
                        blurry,
                        pinned: pinned == Some(i),
                        label: self.label_of(path, None),
                        series_id: place.map(|p| p.id),
                        in_current_series: place.is_some_and(|p| Some(p.id) == current_series),
                        more,
                        duplicate: duplicates.get(i).is_some_and(|p| p.is_some()),
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
            let name = self.photo_name(&path);
            let series = self
                .view
                .series
                .get(self.current)
                .and_then(|place| *place)
                .map(|place| (place.index, place.len));
            let duplicate_of = self
                .view
                .duplicate_of
                .get(self.current)
                .and_then(|p| p.as_ref())
                .map(|original| self.photo_name(original));
            let bar = bars::InfoBar {
                name: &name,
                position: (self.current + 1, self.view.len()),
                image: image.as_deref(),
                rating: self.rating_of(&path, image.as_deref()),
                label: self.label_of(&path, image.as_deref()),
                series,
                duplicate_of,
                auto_advance: self.auto_advance,
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
                self.set_rating(&ctx, stars, false);
            }
            if let Some(panel) = out.toggle {
                self.toggle_panel(panel);
            }
            if out.help {
                self.help_open = true;
            }
            if let Some(url) = out.open_map {
                ctx.open_url(OpenUrl::new_tab(url));
            }
            if let Some(rect) = details_rect {
                let status = self.analyzer.status();
                let detail_out = details::draw(
                    ui,
                    rect,
                    &details::Details {
                        scores,
                        personal,
                        frame_percentile: scores.and_then(|s| percentiles.frame(&s)),
                        eyes_percentile: scores.and_then(|s| percentiles.eyes(&s)),
                        attributes: self.analyzer.attributes(&path),
                        histogram: image.as_deref().map(|i| &i.histogram),
                        status: &status,
                    },
                    &mut self.details_expanded,
                );
                if detail_out.reset_taste {
                    self.confirm_reset_taste();
                }
                if detail_out.delete_models {
                    self.confirm_delete_models();
                }
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
        if let Some(mut state) = self.palette.take() {
            let commands = self.commands();
            let out = palette::show(&ctx, window, &mut state, &commands);
            if !out.close && out.run.is_none() {
                self.palette = Some(state);
            }
            if let Some(action) = out.run {
                self.run(&ctx, action, &frames);
            }
        }
        self.draw_language_flash(&ctx, if self.all.is_empty() { window } else { area });
        bars::drop_hint(ui, window);
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
        // wasn't undone is carried out.
        self.writer.shutdown();
        self.deletions.finish_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, Modifiers};

    fn key(physical: Key, logical: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key: logical,
            physical_key: Some(physical),
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn shift_digits_rate_by_physical_key() {
        // German layout: Shift+3 types "§", Shift+0 types "=".
        let shift = Modifiers::SHIFT;
        assert_eq!(
            shifted_digit(&[key(Key::Num3, Key::Num3, shift)]),
            Some(Rating::Stars(3))
        );
        assert_eq!(
            shifted_digit(&[key(Key::Num0, Key::Equals, shift)]),
            Some(Rating::Unrated)
        );
        assert_eq!(
            shifted_digit(&[key(Key::Num3, Key::Num3, Modifiers::NONE)]),
            None
        );
        assert_eq!(shifted_digit(&[key(Key::Num7, Key::Slash, shift)]), None);
    }
}
