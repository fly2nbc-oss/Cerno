//! Background decoding with prefetch around the current image.
//!
//! Workers don't consume a queue: each one picks the most urgent image that is neither cached
//! nor being decoded, relative to the *current* index at the moment it looks. Jumping around
//! therefore re-prioritises automatically.
//!
//! Display images are decoded at the size of the photo area, so a fitted photo is drawn pixel
//! for pixel; when the area changes, cached ones are decoded again. For zooming, the current
//! image (and the other photos on screen in compare mode and the four-up view) can also be
//! loaded at full resolution,
//! split into tiles that fit the GPU's texture limit.
//!
//! While the check overlay is on (`O`), the photos on screen and the current one's neighbours
//! also get it, computed from the same decode – or from a new one, since a texture can't be
//! read back.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::{Context as _, Result};
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};

use crate::filelock::FileLocks;
use crate::histogram::{self, RgbHistogram};
use crate::metadata::{self, CameraInfo, Description, LabelInfo, RatingInfo};
use crate::overlay::{self, Mode};
use crate::thumbs::{self, Thumbs};
use crate::{decode, library};

/// Offsets from the current index in order of urgency: the image itself, then mostly forward.
const PREFETCH_ORDER: [isize; 7] = [0, 1, -1, 2, 3, -2, -3];
/// Cached images further away than this from the current one are dropped.
const KEEP_RADIUS: usize = 4;
/// Upper bound for full-resolution tiles, below every GPU's limit we care about.
const MAX_TILE: u32 = 4096;

pub struct LoadedImage {
    /// Fitted into the decode size it was made for, so a fitted photo is drawn pixel for pixel.
    pub texture: TextureHandle,
    /// The decode size (`Loader::set_target`) this image was made for.
    pub decoded_for: [u32; 2],
    /// RGB histogram of the display decode.
    pub histogram: RgbHistogram,
    /// Full image size after orientation.
    pub original_size: [u32; 2],
    /// Rating as stored in the file when it was decoded.
    pub rating: RatingInfo,
    /// Colour label as stored in the file when it was decoded.
    pub label: LabelInfo,
    pub camera: CameraInfo,
    /// Comment and keywords as stored in the file when it was decoded.
    pub description: Description,
    pub load_ms: u128,
    /// The file's size in bytes (Details › File).
    pub file_bytes: u64,
    /// A JPEG's estimated quality and its chroma subsampling.
    pub jpeg: Option<crate::jpeg_info::JpegInfo>,
}

/// What the file itself says, besides its pixels: its size and, for a JPEG, its compression.
#[derive(Default)]
struct FileFacts {
    bytes: u64,
    jpeg: Option<crate::jpeg_info::JpegInfo>,
}

/// An image at 100 %, as tiles in image pixel coordinates.
pub struct FullImage {
    pub size: [u32; 2],
    pub tiles: Vec<Tile>,
}

pub struct Tile {
    pub texture: TextureHandle,
    pub origin: [u32; 2],
    pub size: [u32; 2],
}

#[derive(Clone)]
pub enum Lookup {
    Ready(Arc<LoadedImage>),
    Failed(String),
    Pending,
}

enum Slot {
    Ready(Arc<LoadedImage>),
    Failed(String),
}

struct State {
    /// Bumped whenever the list changes; results of older generations are discarded.
    generation: u64,
    paths: Arc<Vec<PathBuf>>,
    current: usize,
    /// The other photos on screen, kept decoded however far away they are: the left photo in
    /// compare mode, the other three of the four-up view.
    shown: Vec<usize>,
    /// Decode size in physical pixels: the photo area, clamped to the max texture side (a 4K
    /// guess until the window has one).
    target: [u32; 2],
    cache: HashMap<usize, Slot>,
    in_flight: HashSet<usize>,
    /// Full resolution wanted for these indices (the photos on screen, while zoomed in).
    want_full: HashSet<usize>,
    full: HashMap<usize, Option<Arc<FullImage>>>,
    full_in_flight: HashSet<usize>,
    /// Off until the first frame: before that only the current photo decodes, so the
    /// neighbours don't compete with the GPU and window set-up.
    prefetch: bool,
    /// What the check overlay shows; `Off` computes nothing.
    overlay: Mode,
    /// Bumped when the overlay changes; overlays computed for an older one are discarded.
    overlay_generation: u64,
    /// Overlays of the display images. `None`: this photo has none (a video, a failed decode).
    overlays: HashMap<usize, Option<TextureHandle>>,
    overlay_in_flight: HashSet<usize>,
    /// Overlays of the full-resolution images, tile for tile like `full`.
    full_overlays: HashMap<usize, Option<Arc<Vec<Tile>>>>,
    full_overlay_in_flight: HashSet<usize>,
    shutdown: bool,
}

impl State {
    fn keeps(&self, index: usize) -> bool {
        index.abs_diff(self.current) <= KEEP_RADIUS || self.shown.contains(&index)
    }

    /// Only the photos on screen get full resolution.
    fn on_screen(&self, index: usize) -> bool {
        index == self.current || self.shown.contains(&index)
    }

    /// The overlay is made for the photos on screen and the current one's neighbours, so
    /// stepping on shows it at once; further ones would only cost memory.
    fn wants_overlay(&self, index: usize) -> bool {
        self.overlay != Mode::Off
            && (index.abs_diff(self.current) <= 1 || self.shown.contains(&index))
    }

    /// See [`Loader::set_overlay`]. Whether anything changed.
    fn set_overlay(&mut self, mode: Mode) -> bool {
        if self.overlay == mode {
            return false;
        }
        self.overlay = mode;
        self.overlay_generation += 1;
        self.overlays.clear();
        self.full_overlays.clear();
        self.overlay_in_flight.clear();
        self.full_overlay_in_flight.clear();
        true
    }

    fn prune(&mut self) {
        let (current, shown) = (self.current, std::mem::take(&mut self.shown));
        let on_screen = |i: usize| i == current || shown.contains(&i);
        let overlay_on = self.overlay != Mode::Off;
        self.cache
            .retain(|&i, _| i.abs_diff(current) <= KEEP_RADIUS || shown.contains(&i));
        self.full.retain(|&i, _| on_screen(i));
        self.want_full.retain(|&i| on_screen(i));
        self.overlays
            .retain(|&i, _| overlay_on && (i.abs_diff(current) <= 1 || shown.contains(&i)));
        self.full_overlays.retain(|&i, _| on_screen(i));
        self.shown = shown;
    }

    /// See [`Loader::set_library`]. The same list again (compare mode on or off) keeps the
    /// decodes in flight: their indices still mean the same photos.
    fn switch(&mut self, paths: Arc<Vec<PathBuf>>, current: usize, shown: Vec<usize>) {
        if *paths == *self.paths {
            self.paths = paths;
            self.current = current;
            self.shown = shown;
            self.prune();
            return;
        }
        let old_paths = std::mem::replace(&mut self.paths, paths);
        let positions: HashMap<&PathBuf, usize> =
            self.paths.iter().enumerate().map(|(i, p)| (p, i)).collect();
        let remap = |old: usize| old_paths.get(old).and_then(|p| positions.get(p)).copied();
        remap_keys(&mut self.cache, &remap);
        remap_keys(&mut self.full, &remap);
        remap_keys(&mut self.overlays, &remap);
        remap_keys(&mut self.full_overlays, &remap);
        drop(positions);
        self.generation += 1;
        self.current = current;
        self.shown = shown;
        self.in_flight.clear();
        self.want_full.clear();
        self.full_in_flight.clear();
        self.overlay_in_flight.clear();
        self.full_overlay_in_flight.clear();
        self.prune();
    }
}

/// Moves the entries of `map` to the photos' places in a new list; photos no longer in it go.
fn remap_keys<T>(map: &mut HashMap<usize, T>, remap: &impl Fn(usize) -> Option<usize>) {
    *map = std::mem::take(map)
        .into_iter()
        .filter_map(|(old, value)| Some((remap(old)?, value)))
        .collect();
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    ctx: egui::Context,
    thumbs: Arc<Thumbs>,
    files: Arc<FileLocks>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Display,
    Full,
    /// The overlay of a photo whose display image is already cached (decoded again).
    Overlay,
    /// The overlay tiles of a full-resolution image already loaded (decoded again).
    FullOverlay,
}

struct Job {
    kind: Kind,
    generation: u64,
    index: usize,
    path: PathBuf,
    target: [u32; 2],
    /// The overlay to compute along with it; `Off` for none.
    overlay: Mode,
    overlay_generation: u64,
}

pub struct Loader {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}

impl Loader {
    pub fn new(
        ctx: egui::Context,
        target: [u32; 2],
        thumbs: Arc<Thumbs>,
        files: Arc<FileLocks>,
    ) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                generation: 0,
                paths: Arc::new(Vec::new()),
                current: 0,
                shown: Vec::new(),
                target,
                cache: HashMap::new(),
                in_flight: HashSet::new(),
                want_full: HashSet::new(),
                full: HashMap::new(),
                full_in_flight: HashSet::new(),
                prefetch: false,
                overlay: Mode::Off,
                overlay_generation: 0,
                overlays: HashMap::new(),
                overlay_in_flight: HashSet::new(),
                full_overlays: HashMap::new(),
                full_overlay_in_flight: HashSet::new(),
                shutdown: false,
            }),
            wake: Condvar::new(),
            ctx,
            thumbs,
            files,
        });
        // Each decode is single-threaded; a few in parallel keep the prefetch window full.
        let count = std::thread::available_parallelism()
            .map_or(2, |n| n.get().saturating_sub(1))
            .clamp(2, 4);
        let workers = (0..count)
            .map(|i| {
                let shared = Arc::clone(&shared);
                std::thread::Builder::new()
                    .name(format!("cerno-decode-{i}"))
                    .spawn(move || worker(&shared))
                    .expect("failed to spawn decode worker")
            })
            .collect();
        Self { shared, workers }
    }

    /// Switches to a new list. Images that are in both lists stay cached (re-sorting,
    /// filtering or deleting doesn't decode anything again).
    pub fn set_library(&self, paths: Arc<Vec<PathBuf>>, current: usize, shown: Vec<usize>) {
        self.shared.lock().switch(paths, current, shown);
        self.shared.wake.notify_all();
    }

    pub fn set_current(&self, current: usize) {
        let mut state = self.shared.lock();
        if state.current == current {
            return;
        }
        state.current = current;
        state.prune();
        drop(state);
        self.shared.wake.notify_all();
    }

    /// The other photos on screen besides the current one (see `State::shown`).
    pub fn set_shown(&self, shown: Vec<usize>) {
        let mut state = self.shared.lock();
        if state.shown == shown {
            return;
        }
        state.shown = shown;
        state.prune();
        drop(state);
        self.shared.wake.notify_all();
    }

    /// The decode size: the photo area in physical pixels. Cached images that would come out
    /// at another size are decoded again – the current photo first – and stay on screen until
    /// the new one is there; scaled by the GPU they look soft.
    pub fn set_target(&self, target: [u32; 2]) {
        let mut state = self.shared.lock();
        if state.target != target {
            state.target = target;
            drop(state);
            self.shared.wake.notify_all();
        }
    }

    /// Switches the check overlay. Overlays of the previous one are dropped; the photos on
    /// screen get the new one first.
    pub fn set_overlay(&self, mode: Mode) {
        if self.shared.lock().set_overlay(mode) {
            self.shared.wake.notify_all();
        }
    }

    /// Drops one photo so the next frame decodes it again. In-flight decodes are discarded too:
    /// a worker may be reading the file while it is rewritten.
    pub fn invalidate(&self, index: usize) {
        let mut state = self.shared.lock();
        state.cache.remove(&index);
        state.full.remove(&index);
        state.want_full.remove(&index);
        state.overlays.remove(&index);
        state.full_overlays.remove(&index);
        state.generation += 1;
        state.in_flight.clear();
        state.full_in_flight.clear();
        state.overlay_in_flight.clear();
        state.full_overlay_in_flight.clear();
        drop(state);
        self.shared.wake.notify_all();
    }

    pub fn get(&self, index: usize) -> Lookup {
        match self.shared.lock().cache.get(&index) {
            Some(Slot::Ready(image)) => Lookup::Ready(Arc::clone(image)),
            Some(Slot::Failed(message)) => Lookup::Failed(message.clone()),
            None => Lookup::Pending,
        }
    }

    /// Asks for the full-resolution version of `index` (kept only while it is on screen).
    pub fn request_full(&self, index: usize) {
        let mut state = self.shared.lock();
        if state.on_screen(index) && state.want_full.insert(index) {
            drop(state);
            self.shared.wake.notify_all();
        }
    }

    /// Called once the window is up: decode the neighbours too.
    pub fn start_prefetch(&self) {
        self.set_prefetch(true);
    }

    /// Whether the neighbours are decoded too. Off while the grid shows: only its cursor photo
    /// is decoded then, not photos at full-window size that nobody sees.
    pub fn set_prefetch(&self, on: bool) {
        let mut state = self.shared.lock();
        if state.prefetch != on {
            state.prefetch = on;
            drop(state);
            self.shared.wake.notify_all();
        }
    }

    pub fn full(&self, index: usize) -> Option<Arc<FullImage>> {
        self.shared.lock().full.get(&index).cloned().flatten()
    }

    /// The check overlay over the display image, once it is computed.
    pub fn overlay(&self, index: usize) -> Option<TextureHandle> {
        self.shared.lock().overlays.get(&index).cloned().flatten()
    }

    /// The check overlay's tiles over the full resolution, once they are computed.
    pub fn full_overlay(&self, index: usize) -> Option<Arc<Vec<Tile>>> {
        self.shared
            .lock()
            .full_overlays
            .get(&index)
            .cloned()
            .flatten()
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.shared.lock().shutdown = true;
        self.shared.wake.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn worker(shared: &Shared) {
    loop {
        let job = {
            let mut state = shared.lock();
            loop {
                if state.shutdown {
                    return;
                }
                if let Some(job) = next_job(&mut state) {
                    break job;
                }
                state = shared
                    .wake
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        };

        match job.kind {
            Kind::Display => {
                let result = decode::catch_panic(|| load_display(shared, &job));
                let mut state = shared.lock();
                if state.generation != job.generation {
                    continue;
                }
                state.in_flight.remove(&job.index);
                // Made for a decode size that changed meanwhile, it still goes into the cache:
                // better than the image there, if any, and `display_job` decodes it again.
                let (slot, overlay) = match result {
                    Ok((image, overlay)) => (Slot::Ready(Arc::new(image)), overlay),
                    Err(err) => {
                        log::warn!("{}: {err:#}", job.path.display());
                        (Slot::Failed(format!("{err:#}")), None)
                    }
                };
                if state.keeps(job.index) {
                    state.cache.insert(job.index, slot);
                }
                if overlay_still_wanted(&state, &job) {
                    state.overlays.insert(job.index, overlay);
                }
            }
            Kind::Full => {
                let result = decode::catch_panic(|| load_full(shared, &job));
                let mut state = shared.lock();
                if state.generation != job.generation {
                    continue;
                }
                state.full_in_flight.remove(&job.index);
                if state.on_screen(job.index) {
                    let (full, overlay) = match result {
                        Ok((full, overlay)) => (Some(Arc::new(full)), overlay),
                        Err(err) => {
                            log::warn!("full size {}: {err:#}", job.path.display());
                            (None, None)
                        }
                    };
                    state.full.insert(job.index, full);
                    if overlay_still_wanted(&state, &job) {
                        state.full_overlays.insert(job.index, overlay.map(Arc::new));
                    }
                }
            }
            Kind::Overlay => {
                let result = decode::catch_panic(|| load_overlay(shared, &job));
                let mut state = shared.lock();
                if state.generation != job.generation
                    || state.overlay_generation != job.overlay_generation
                {
                    continue;
                }
                state.overlay_in_flight.remove(&job.index);
                if state.wants_overlay(job.index) {
                    let overlay = result
                        .map_err(|err| log::warn!("overlay {}: {err:#}", job.path.display()))
                        .ok()
                        .flatten();
                    state.overlays.insert(job.index, overlay);
                }
            }
            Kind::FullOverlay => {
                let result = decode::catch_panic(|| load_full_overlay(shared, &job));
                let mut state = shared.lock();
                if state.generation != job.generation
                    || state.overlay_generation != job.overlay_generation
                {
                    continue;
                }
                state.full_overlay_in_flight.remove(&job.index);
                if state.on_screen(job.index) {
                    let overlay = result
                        .map_err(|err| log::warn!("overlay {}: {err:#}", job.path.display()))
                        .ok()
                        .flatten();
                    state.full_overlays.insert(job.index, overlay.map(Arc::new));
                }
            }
        }
        shared.ctx.request_repaint();
    }
}

/// A decode that computed the overlay along with the picture: it still counts if the overlay
/// has not changed meanwhile and the photo still needs one.
fn overlay_still_wanted(state: &State, job: &Job) -> bool {
    job.overlay != Mode::Off
        && job.overlay_generation == state.overlay_generation
        && state.wants_overlay(job.index)
}

fn job(state: &State, kind: Kind, index: usize) -> Job {
    Job {
        kind,
        generation: state.generation,
        index,
        path: state.paths[index].clone(),
        target: state.target,
        overlay: if state.wants_overlay(index) {
            state.overlay
        } else {
            Mode::Off
        },
        overlay_generation: state.overlay_generation,
    }
}

/// A photo not decoded yet, or decoded for another size (see [`stale`]).
fn display_job(state: &mut State, index: usize) -> Option<Job> {
    let target = state.target;
    if index >= state.paths.len()
        || state
            .cache
            .get(&index)
            .is_some_and(|slot| !stale(slot, target))
        || !state.in_flight.insert(index)
    {
        return None;
    }
    Some(job(state, Kind::Display, index))
}

fn full_job(state: &mut State, index: usize) -> Option<Job> {
    if !state.want_full.contains(&index)
        || state.full.contains_key(&index)
        || !state.full_in_flight.insert(index)
    {
        return None;
    }
    Some(job(state, Kind::Full, index))
}

/// The overlay of a photo whose display image came without one (the overlay was off then, or
/// the photo too far away).
fn overlay_job(state: &mut State, index: usize) -> Option<Job> {
    if !state.wants_overlay(index)
        || !matches!(state.cache.get(&index), Some(Slot::Ready(_)))
        || state.overlays.contains_key(&index)
        || !state.overlay_in_flight.insert(index)
    {
        return None;
    }
    Some(job(state, Kind::Overlay, index))
}

/// The overlay tiles of a full-resolution image that came without them.
fn full_overlay_job(state: &mut State, index: usize) -> Option<Job> {
    if state.overlay == Mode::Off
        || !matches!(state.full.get(&index), Some(Some(_)))
        || state.full_overlays.contains_key(&index)
        || state.full_in_flight.contains(&index)
        || !state.full_overlay_in_flight.insert(index)
    {
        return None;
    }
    Some(job(state, Kind::FullOverlay, index))
}

/// Current photo, its overlay and full resolution, the other photos on screen likewise (all
/// their display images first), then the neighbours (and the overlay of the next and previous
/// one).
fn next_job(state: &mut State) -> Option<Job> {
    let current = state.current;
    if let Some(job) = display_job(state, current) {
        return Some(job);
    }
    if !state.prefetch {
        return None;
    }
    if let Some(job) = on_screen_job(state, current) {
        return Some(job);
    }
    let shown = state.shown.clone();
    if let Some(job) = shown.iter().find_map(|&index| display_job(state, index)) {
        return Some(job);
    }
    if let Some(job) = shown.iter().find_map(|&index| on_screen_job(state, index)) {
        return Some(job);
    }
    PREFETCH_ORDER.into_iter().skip(1).find_map(|offset| {
        let index = current.checked_add_signed(offset)?;
        display_job(state, index).or_else(|| overlay_job(state, index))
    })
}

/// What a photo on screen may still need after its display image.
fn on_screen_job(state: &mut State, index: usize) -> Option<Job> {
    overlay_job(state, index)
        .or_else(|| full_job(state, index))
        .or_else(|| full_overlay_job(state, index))
}

/// A display image made for another decode size that would now come out at another size.
/// Asking for both keeps a photo from decoding over and over if its size ever disagrees with
/// [`decode::fit_within`] (a video's placeholder, say). Failures are not retried.
fn stale(slot: &Slot, target: [u32; 2]) -> bool {
    let Slot::Ready(image) = slot else {
        return false;
    };
    let [w, h] = decode::fit_within(image.original_size, target);
    image.decoded_for != target && image.texture.size() != [w as usize, h as usize]
}

/// The picture of `path`, fitted into `target`, and its metadata: a photo from its bytes
/// (marks from the sidecar where they live there), a video from one frame – or a placeholder
/// without ffmpeg – and its sidecar. The flag is false for that placeholder: its play sign is
/// no thumbnail, the filmstrip paints its own over every video.
fn picture(
    shared: &Shared,
    path: &Path,
    target: [u32; 2],
) -> Result<(
    metadata::FileMetadata,
    decode::DecodedImage,
    bool,
    FileFacts,
)> {
    let format = library::format_of(path).context("unsupported file type")?;
    if format == library::Format::Video {
        let meta = metadata::read_sidecar(path);
        // Never read into memory: its size from the file system.
        let facts = FileFacts {
            bytes: std::fs::metadata(path).map_or(0, |m| m.len()),
            jpeg: None,
        };
        let (decoded, framed) = match crate::video::poster(path) {
            Ok(jpeg) => (
                decode::decode_for_screen(&jpeg, library::Format::Jpeg, 1, target)?,
                true,
            ),
            Err(err) => {
                log::info!("no frame of {}: {err:#}", path.display());
                (crate::video::placeholder(target), false)
            }
        };
        return Ok((meta, decoded, framed, facts));
    }
    let bytes = read(&shared.files, path)?;
    let meta = metadata::read_for(path, &bytes);
    let decoded = decode::decode_for_screen(&bytes, format, meta.orientation, target)?;
    let facts = FileFacts {
        bytes: bytes.len() as u64,
        jpeg: (format == library::Format::Jpeg)
            .then(|| crate::jpeg_info::read(&bytes))
            .flatten(),
    };
    Ok((meta, decoded, true, facts))
}

/// The display image and, when the job asks for it, its overlay.
fn load_display(shared: &Shared, job: &Job) -> Result<(LoadedImage, Option<TextureHandle>)> {
    let started = Instant::now();
    let (meta, decoded, framed, facts) = picture(shared, &job.path, job.target)?;

    // The neighbourhood's filmstrip thumbnails come almost for free from here.
    if framed
        && !shared.thumbs.contains(&job.path)
        && let Ok((w, h, rgb)) = thumbs::downscale(&decoded.rgb, decoded.width, decoded.height)
    {
        shared.thumbs.insert(&job.path, w, h, &rgb);
    }

    let image = ColorImage::from_rgb(
        [decoded.width as usize, decoded.height as usize],
        &decoded.rgb,
    );
    // `Context` is `Send + Sync`: uploading here keeps the UI thread free.
    let texture = shared.ctx.load_texture(
        library::file_name_lossy(&job.path),
        image,
        TextureOptions::LINEAR,
    );
    let overlay = display_overlay(shared, job, &decoded);

    let image = LoadedImage {
        texture,
        decoded_for: job.target,
        histogram: histogram::compute(&decoded.rgb),
        original_size: decoded.original_size,
        rating: meta.rating,
        label: meta.label,
        camera: meta.camera,
        description: meta.description,
        load_ms: started.elapsed().as_millis(),
        file_bytes: facts.bytes,
        jpeg: facts.jpeg,
    };
    Ok((image, overlay))
}

/// Videos get no overlay: their frame is only a preview.
fn takes_overlay(job: &Job) -> bool {
    job.overlay != Mode::Off && library::format_of(&job.path) != Some(library::Format::Video)
}

/// The overlay over a display decode, when the job asks for one.
fn display_overlay(
    shared: &Shared,
    job: &Job,
    decoded: &decode::DecodedImage,
) -> Option<TextureHandle> {
    if !takes_overlay(job) {
        return None;
    }
    let (w, h) = (decoded.width, decoded.height);
    let threshold = overlay::threshold(job.overlay, &decoded.rgb, w, h);
    let image = overlay::render(&decoded.rgb, w, h, [0, 0, w, h], job.overlay, threshold)?;
    let name = format!("overlay:{}", job.path.display());
    Some(shared.ctx.load_texture(name, image, TextureOptions::LINEAR))
}

/// The overlay of a photo already cached: its display image once more, for the pixels.
fn load_overlay(shared: &Shared, job: &Job) -> Result<Option<TextureHandle>> {
    if !takes_overlay(job) {
        return Ok(None);
    }
    let (_, decoded, _, _) = picture(shared, &job.path, job.target)?;
    Ok(display_overlay(shared, job, &decoded))
}

/// The file's bytes, never while the rating writer is halfway through rewriting it.
fn read(files: &FileLocks, path: &Path) -> Result<Vec<u8>> {
    let _held = files.hold(path);
    std::fs::read(path).context("cannot read file")
}

/// The full resolution as tiles and, when the job asks for it, the overlay's tiles.
fn load_full(shared: &Shared, job: &Job) -> Result<(FullImage, Option<Vec<Tile>>)> {
    let ctx = &shared.ctx;
    let (_, decoded, _, _) = picture(shared, &job.path, [u32::MAX; 2])?;
    let (width, height) = (decoded.width, decoded.height);
    let tiles = tile_regions(ctx, width, height)
        .into_iter()
        .map(|[x0, y0, tw, th]| {
            let mut rgb = Vec::with_capacity((tw * th * 3) as usize);
            for y in y0..y0 + th {
                let start = ((y * width + x0) * 3) as usize;
                rgb.extend_from_slice(&decoded.rgb[start..start + (tw * 3) as usize]);
            }
            let image = ColorImage::from_rgb([tw as usize, th as usize], &rgb);
            let texture = ctx.load_texture(
                format!("full:{}:{x0}:{y0}", job.path.display()),
                image,
                TextureOptions::LINEAR,
            );
            Tile {
                texture,
                origin: [x0, y0],
                size: [tw, th],
            }
        })
        .collect();
    let full = FullImage {
        size: [width, height],
        tiles,
    };
    Ok((full, full_overlay_tiles(shared, job, &decoded)))
}

/// The overlay tiles of a full-resolution image already on screen: decoded once more.
fn load_full_overlay(shared: &Shared, job: &Job) -> Result<Option<Vec<Tile>>> {
    if !takes_overlay(job) {
        return Ok(None);
    }
    let (_, decoded, _, _) = picture(shared, &job.path, [u32::MAX; 2])?;
    Ok(full_overlay_tiles(shared, job, &decoded))
}

/// The overlay over a full-resolution decode, in the same tiles as the picture. One threshold
/// for the whole photo, so the tiles mark alike.
fn full_overlay_tiles(
    shared: &Shared,
    job: &Job,
    decoded: &decode::DecodedImage,
) -> Option<Vec<Tile>> {
    if !takes_overlay(job) {
        return None;
    }
    let (width, height) = (decoded.width, decoded.height);
    let threshold = overlay::threshold(job.overlay, &decoded.rgb, width, height);
    tile_regions(&shared.ctx, width, height)
        .into_iter()
        .map(|region| {
            let [x0, y0, tw, th] = region;
            let image =
                overlay::render(&decoded.rgb, width, height, region, job.overlay, threshold)?;
            let texture = shared.ctx.load_texture(
                format!("overlay:{}:{x0}:{y0}", job.path.display()),
                image,
                TextureOptions::LINEAR,
            );
            Some(Tile {
                texture,
                origin: [x0, y0],
                size: [tw, th],
            })
        })
        .collect()
}

/// The tiles a full-resolution image is cut into: as large as the GPU allows, at most
/// [`MAX_TILE`].
fn tile_regions(ctx: &egui::Context, width: u32, height: u32) -> Vec<overlay::Region> {
    let max_side = ctx.input(|i| i.max_texture_side) as u32;
    let side = max_side.min(MAX_TILE);
    let mut regions = Vec::new();
    for y0 in (0..height).step_by(side as usize) {
        for x0 in (0..width).step_by(side as usize) {
            regions.push([x0, y0, side.min(width - x0), side.min(height - y0)]);
        }
    }
    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(len: usize, current: usize, pinned: Option<usize>) -> State {
        State {
            generation: 0,
            paths: Arc::new(
                (0..len)
                    .map(|i| PathBuf::from(format!("{i}.jpg")))
                    .collect(),
            ),
            current,
            shown: pinned.into_iter().collect(),
            target: [100, 100],
            cache: HashMap::new(),
            in_flight: HashSet::new(),
            want_full: HashSet::new(),
            full: HashMap::new(),
            full_in_flight: HashSet::new(),
            prefetch: true,
            overlay: Mode::Off,
            overlay_generation: 0,
            overlays: HashMap::new(),
            overlay_in_flight: HashSet::new(),
            full_overlays: HashMap::new(),
            full_overlay_in_flight: HashSet::new(),
            shutdown: false,
        }
    }

    fn order(state: &mut State) -> Vec<(usize, bool)> {
        std::iter::from_fn(|| next_job(state).map(|j| (j.index, j.kind == Kind::Full))).collect()
    }

    /// Every job `next_job` hands out, with its kind and the overlay it computes.
    fn jobs(state: &mut State) -> Vec<(usize, Kind, Mode)> {
        std::iter::from_fn(|| next_job(state).map(|j| (j.index, j.kind, j.overlay))).collect()
    }

    /// A decoded display image, as the cache holds it.
    fn ready(ctx: &egui::Context) -> Slot {
        decoded(ctx, [1, 1], [1, 1], [100, 100])
    }

    /// A display image of `size` for a photo of `original` size, made for decode size `target`.
    fn decoded(
        ctx: &egui::Context,
        size: [usize; 2],
        original: [u32; 2],
        target: [u32; 2],
    ) -> Slot {
        let pixels = ColorImage::new(size, vec![egui::Color32::BLACK; size[0] * size[1]]);
        Slot::Ready(Arc::new(LoadedImage {
            texture: ctx.load_texture("photo", pixels, TextureOptions::LINEAR),
            decoded_for: target,
            histogram: [[0; 256]; 3],
            original_size: original,
            rating: RatingInfo::default(),
            label: LabelInfo::None,
            camera: CameraInfo::default(),
            description: Description::default(),
            load_ms: 0,
            file_bytes: 0,
            jpeg: None,
        }))
    }

    #[test]
    fn overlays_come_with_the_decodes_on_screen_and_next_to_it() {
        let mut s = state(20, 10, Some(2));
        s.set_overlay(Mode::Sharpness);
        let sharp = Mode::Sharpness;
        assert_eq!(
            jobs(&mut s),
            [
                (10, Kind::Display, sharp),
                (2, Kind::Display, sharp),
                (11, Kind::Display, sharp),
                (9, Kind::Display, sharp),
                // Further away only the picture: an overlay there would only cost memory.
                (12, Kind::Display, Mode::Off),
                (13, Kind::Display, Mode::Off),
                (8, Kind::Display, Mode::Off),
                (7, Kind::Display, Mode::Off),
            ]
        );
    }

    #[test]
    fn cached_photos_get_their_overlay_decoded_again() {
        let ctx = egui::Context::default();
        let mut s = state(20, 10, None);
        for index in [9, 10, 11, 12] {
            s.cache.insert(index, ready(&ctx));
        }
        assert!(
            jobs(&mut s)
                .iter()
                .all(|&(_, kind, _)| kind == Kind::Display),
            "without the overlay nothing is decoded again"
        );
        let mut s = state(20, 10, None);
        for index in [9, 10, 11, 12] {
            s.cache.insert(index, ready(&ctx));
        }
        s.set_overlay(Mode::Exposure);
        let exposure = Mode::Exposure;
        assert_eq!(
            jobs(&mut s),
            [
                (10, Kind::Overlay, exposure),
                (11, Kind::Overlay, exposure),
                (9, Kind::Overlay, exposure),
                (13, Kind::Display, Mode::Off),
                (8, Kind::Display, Mode::Off),
                (7, Kind::Display, Mode::Off),
            ]
        );
    }

    #[test]
    fn a_new_overlay_drops_the_old_ones_and_their_decodes() {
        let ctx = egui::Context::default();
        let mut s = state(20, 10, None);
        s.set_overlay(Mode::Sharpness);
        s.cache.insert(10, ready(&ctx));
        s.overlays.insert(10, None);
        s.overlay_in_flight.insert(11);
        let old = job(&s, Kind::Overlay, 11);
        assert!(
            !s.set_overlay(Mode::Sharpness),
            "the same mode changes nothing"
        );
        assert!(s.set_overlay(Mode::Exposure));
        assert!(s.overlays.is_empty() && s.overlay_in_flight.is_empty());
        assert_ne!(old.overlay_generation, s.overlay_generation);
        assert!(
            !overlay_still_wanted(&s, &old),
            "a result for the old mode is dropped"
        );
        assert_eq!(
            jobs(&mut s).first(),
            Some(&(10, Kind::Overlay, Mode::Exposure))
        );
    }

    #[test]
    fn overlays_of_photos_moved_away_from_are_dropped() {
        let mut s = state(20, 10, Some(2));
        s.set_overlay(Mode::Sharpness);
        for index in [2, 9, 10, 11, 12] {
            s.overlays.insert(index, None);
        }
        s.current = 11;
        s.prune();
        let mut kept: Vec<usize> = s.overlays.keys().copied().collect();
        kept.sort_unstable();
        assert_eq!(kept, [2, 10, 11, 12]);
        s.set_overlay(Mode::Off);
        s.overlays.insert(11, None);
        s.prune();
        assert!(s.overlays.is_empty(), "no overlay is kept while it is off");
    }

    #[test]
    fn full_resolution_overlays_wait_for_their_image() {
        let mut s = state(20, 10, None);
        s.set_overlay(Mode::Sharpness);
        s.want_full.insert(10);
        s.overlays.insert(10, None);
        s.in_flight.insert(10);
        // The full decode computes the overlay tiles along with it.
        assert_eq!(
            jobs(&mut s).first(),
            Some(&(10, Kind::Full, Mode::Sharpness))
        );
        // Loaded without them (the overlay was off then): decoded once more for the overlay.
        let mut s = state(20, 10, None);
        s.in_flight.insert(10);
        s.full.insert(10, None);
        s.set_overlay(Mode::Sharpness);
        s.overlays.insert(10, None);
        assert!(
            !jobs(&mut s)
                .iter()
                .any(|&(_, kind, _)| kind == Kind::FullOverlay),
            "a failed full decode gets no overlay"
        );
        let mut s = state(20, 10, None);
        s.in_flight.insert(10);
        let loaded = FullImage {
            size: [1, 1],
            tiles: Vec::new(),
        };
        s.full.insert(10, Some(Arc::new(loaded)));
        s.set_overlay(Mode::Sharpness);
        s.overlays.insert(10, None);
        assert_eq!(
            jobs(&mut s).first(),
            Some(&(10, Kind::FullOverlay, Mode::Sharpness))
        );
    }

    #[test]
    fn the_same_list_keeps_decodes_in_flight_a_new_one_drops_them() {
        let mut s = state(20, 3, None);
        s.in_flight.extend([3, 4]);
        let same = Arc::clone(&s.paths);
        s.switch(Arc::new(same.to_vec()), 4, vec![3]);
        assert_eq!(
            (s.generation, s.current, s.shown.as_slice()),
            (0, 4, &[3][..])
        );
        assert_eq!(
            s.in_flight,
            HashSet::from([3, 4]),
            "compare mode keeps them"
        );
        let fewer: Vec<PathBuf> = same.iter().skip(1).cloned().collect();
        s.switch(Arc::new(fewer), 2, Vec::new());
        assert_eq!(s.generation, 1);
        assert!(s.in_flight.is_empty(), "indices mean other photos now");
    }

    #[test]
    fn current_first_then_pinned_then_neighbours() {
        let mut s = state(20, 10, Some(2));
        s.want_full.extend([10, 2]);
        assert_eq!(
            order(&mut s),
            [
                (10, false),
                (10, true),
                (2, false),
                (2, true),
                (11, false),
                (9, false),
                (12, false),
                (13, false),
                (8, false),
                (7, false),
            ]
        );
    }

    /// The four-up view: the four display images first, then full resolution for all of them.
    #[test]
    fn four_photos_on_screen_come_before_the_neighbours() {
        let mut s = state(20, 10, None);
        s.shown = vec![8, 9, 11];
        s.want_full.extend([8, 9, 10, 11]);
        let first: Vec<(usize, bool)> = order(&mut s).into_iter().take(8).collect();
        assert_eq!(
            first,
            [
                (10, false),
                (10, true),
                (8, false),
                (9, false),
                (11, false),
                (8, true),
                (9, true),
                (11, true),
            ]
        );
    }

    /// After the photo area changed, a photo is decoded again – the current one first – unless
    /// it would come out at the same size. The old one is shown until then.
    #[test]
    fn photos_decoded_for_another_size_are_decoded_again() {
        let ctx = egui::Context::default();
        let mut s = state(20, 10, None);
        s.target = [256, 116];
        // Made for the start-up guess: 288 × 216, now it would be 155 × 116.
        s.cache
            .insert(10, decoded(&ctx, [288, 216], [400, 300], [384, 216]));
        s.cache
            .insert(11, decoded(&ctx, [155, 116], [400, 300], [256, 116]));
        // Smaller than either size: it comes out the same.
        s.cache
            .insert(9, decoded(&ctx, [80, 60], [80, 60], [384, 216]));
        // Made for this size, even if its size disagrees: decoding again would never end.
        s.cache
            .insert(12, decoded(&ctx, [10, 10], [400, 300], [256, 116]));
        let display: Vec<usize> = jobs(&mut s)
            .into_iter()
            .filter(|&(_, kind, _)| kind == Kind::Display)
            .map(|(index, _, _)| index)
            .collect();
        assert_eq!(display, [10, 13, 8, 7]);
        assert!(
            matches!(s.cache.get(&10), Some(Slot::Ready(_))),
            "the old picture stays until the new one is there"
        );
    }

    #[test]
    fn only_the_current_photo_before_the_first_frame() {
        let mut s = state(20, 10, None);
        s.prefetch = false;
        assert_eq!(order(&mut s), [(10, false)]);
    }
}
