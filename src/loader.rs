//! Background decoding with prefetch around the current image.
//!
//! Workers don't consume a queue: each one picks the most urgent image that is neither cached
//! nor being decoded, relative to the *current* index at the moment it looks. Jumping around
//! therefore re-prioritises automatically.
//!
//! Display images are decoded at monitor resolution. For zooming, the current image (and the
//! pinned left image in compare mode) can also be loaded at full resolution, split into tiles
//! that fit the GPU's texture limit.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::{Context as _, Result};
use eframe::egui::{self, ColorImage, TextureFilter, TextureHandle, TextureOptions};

use crate::filelock::FileLocks;
use crate::histogram::{self, RgbHistogram};
use crate::metadata::{self, CameraInfo, Description, LabelInfo, RatingInfo};
use crate::thumbs::{self, Thumbs};
use crate::{decode, library};

/// Offsets from the current index in order of urgency: the image itself, then mostly forward.
const PREFETCH_ORDER: [isize; 7] = [0, 1, -1, 2, 3, -2, -3];
/// Cached images further away than this from the current one are dropped.
const KEEP_RADIUS: usize = 4;
/// Upper bound for full-resolution tiles, below every GPU's limit we care about.
const MAX_TILE: u32 = 4096;

pub struct LoadedImage {
    pub texture: TextureHandle,
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
    /// Kept decoded however far away it is (the left photo in compare mode).
    pinned: Option<usize>,
    /// Decode size in physical pixels (monitor size, clamped to the max texture side).
    target: [u32; 2],
    cache: HashMap<usize, Slot>,
    in_flight: HashSet<usize>,
    /// Full resolution wanted for these indices (current and pinned, while zoomed in).
    want_full: HashSet<usize>,
    full: HashMap<usize, Option<Arc<FullImage>>>,
    full_in_flight: HashSet<usize>,
    /// Off until the first frame: before that only the current photo decodes, so the
    /// neighbours don't compete with the GPU and window set-up.
    prefetch: bool,
    shutdown: bool,
}

impl State {
    fn keeps(&self, index: usize) -> bool {
        index.abs_diff(self.current) <= KEEP_RADIUS || self.pinned == Some(index)
    }

    /// Only the photos on screen get full resolution.
    fn on_screen(&self, index: usize) -> bool {
        index == self.current || self.pinned == Some(index)
    }

    fn prune(&mut self) {
        let (current, pinned) = (self.current, self.pinned);
        let on_screen = |i: usize| i == current || pinned == Some(i);
        self.cache
            .retain(|&i, _| i.abs_diff(current) <= KEEP_RADIUS || pinned == Some(i));
        self.full.retain(|&i, _| on_screen(i));
        self.want_full.retain(|&i| on_screen(i));
    }

    /// See [`Loader::set_library`]. The same list again (compare mode on or off) keeps the
    /// decodes in flight: their indices still mean the same photos.
    fn switch(&mut self, paths: Arc<Vec<PathBuf>>, current: usize, pinned: Option<usize>) {
        if *paths == *self.paths {
            self.paths = paths;
            self.current = current;
            self.pinned = pinned;
            self.prune();
            return;
        }
        let old_paths = std::mem::replace(&mut self.paths, paths);
        let positions: HashMap<&PathBuf, usize> =
            self.paths.iter().enumerate().map(|(i, p)| (p, i)).collect();
        let remap = |old: usize| old_paths.get(old).and_then(|p| positions.get(p)).copied();
        let cache: Vec<_> = std::mem::take(&mut self.cache).into_iter().collect();
        let full: Vec<_> = std::mem::take(&mut self.full).into_iter().collect();
        self.cache = cache
            .into_iter()
            .filter_map(|(old, slot)| Some((remap(old)?, slot)))
            .collect();
        self.full = full
            .into_iter()
            .filter_map(|(old, image)| Some((remap(old)?, image)))
            .collect();
        drop(positions);
        self.generation += 1;
        self.current = current;
        self.pinned = pinned;
        self.in_flight.clear();
        self.want_full.clear();
        self.full_in_flight.clear();
        self.prune();
    }
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Display,
    Full,
}

struct Job {
    kind: Kind,
    generation: u64,
    index: usize,
    path: PathBuf,
    target: [u32; 2],
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
                pinned: None,
                target,
                cache: HashMap::new(),
                in_flight: HashSet::new(),
                want_full: HashSet::new(),
                full: HashMap::new(),
                full_in_flight: HashSet::new(),
                prefetch: false,
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
    pub fn set_library(&self, paths: Arc<Vec<PathBuf>>, current: usize, pinned: Option<usize>) {
        self.shared.lock().switch(paths, current, pinned);
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

    pub fn set_pinned(&self, pinned: Option<usize>) {
        let mut state = self.shared.lock();
        if state.pinned == pinned {
            return;
        }
        state.pinned = pinned;
        state.prune();
        drop(state);
        self.shared.wake.notify_all();
    }

    /// Applies to new decodes. A smaller target keeps the cache; a larger one (screen bigger
    /// than the start-up guess) drops images that would now look soft, so they decode again.
    pub fn set_target(&self, target: [u32; 2]) {
        let mut state = self.shared.lock();
        let grew = target[0] > state.target[0] || target[1] > state.target[1];
        state.target = target;
        if grew {
            state.cache.retain(|_, slot| match slot {
                Slot::Ready(image) => !too_small(image, target),
                Slot::Failed(_) => true,
            });
            drop(state);
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
        state.generation += 1;
        state.in_flight.clear();
        state.full_in_flight.clear();
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
        let mut state = self.shared.lock();
        if !state.prefetch {
            state.prefetch = true;
            drop(state);
            self.shared.wake.notify_all();
        }
    }

    pub fn full(&self, index: usize) -> Option<Arc<FullImage>> {
        self.shared.lock().full.get(&index).cloned().flatten()
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
                // Decoded for a smaller screen than we now know we have: decode again.
                if result
                    .as_ref()
                    .is_ok_and(|image| too_small(image, state.target))
                {
                    drop(state);
                    shared.wake.notify_all();
                    continue;
                }
                let slot = match result {
                    Ok(image) => Slot::Ready(Arc::new(image)),
                    Err(err) => {
                        log::warn!("{}: {err:#}", job.path.display());
                        Slot::Failed(format!("{err:#}"))
                    }
                };
                if state.keeps(job.index) {
                    state.cache.insert(job.index, slot);
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
                    let full = result
                        .map_err(|err| log::warn!("full size {}: {err:#}", job.path.display()))
                        .ok()
                        .map(Arc::new);
                    state.full.insert(job.index, full);
                }
            }
        }
        shared.ctx.request_repaint();
    }
}

fn job(state: &State, kind: Kind, index: usize) -> Job {
    Job {
        kind,
        generation: state.generation,
        index,
        path: state.paths[index].clone(),
        target: state.target,
    }
}

fn display_job(state: &mut State, index: usize) -> Option<Job> {
    if index >= state.paths.len()
        || state.cache.contains_key(&index)
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

/// Current photo, its full resolution, the pinned photo and its full resolution, then the
/// neighbours.
fn next_job(state: &mut State) -> Option<Job> {
    let current = state.current;
    if let Some(job) = display_job(state, current) {
        return Some(job);
    }
    if !state.prefetch {
        return None;
    }
    if let Some(job) = full_job(state, current) {
        return Some(job);
    }
    if let Some(pinned) = state.pinned
        && let Some(job) = display_job(state, pinned).or_else(|| full_job(state, pinned))
    {
        return Some(job);
    }
    PREFETCH_ORDER.into_iter().skip(1).find_map(|offset| {
        let index = current.checked_add_signed(offset)?;
        display_job(state, index)
    })
}

/// Whether `image` has fewer pixels than a decode for `target` would produce.
fn too_small(image: &LoadedImage, target: [u32; 2]) -> bool {
    let wanted = decode::fit_within(image.original_size, target);
    let have = image.texture.size();
    (have[0] as u32) < wanted[0] || (have[1] as u32) < wanted[1]
}

/// The picture of `path`, fitted into `target`, and its metadata: a photo from its bytes
/// (marks from the sidecar where they live there), a video from one frame – or a placeholder
/// without ffmpeg – and its sidecar.
fn picture(
    shared: &Shared,
    path: &Path,
    target: [u32; 2],
) -> Result<(metadata::FileMetadata, decode::DecodedImage)> {
    let format = library::format_of(path).context("unsupported file type")?;
    if format == library::Format::Video {
        let meta = metadata::read_sidecar(path);
        let decoded = match crate::video::poster(path) {
            Ok(jpeg) => decode::decode_for_display(&jpeg, library::Format::Jpeg, 1, target)?,
            Err(err) => {
                log::info!("no frame of {}: {err:#}", path.display());
                crate::video::placeholder(target)
            }
        };
        return Ok((meta, decoded));
    }
    let bytes = read(&shared.files, path)?;
    let meta = metadata::read_for(path, &bytes);
    let decoded = decode::decode_for_display(&bytes, format, meta.orientation, target)?;
    Ok((meta, decoded))
}

fn load_display(shared: &Shared, job: &Job) -> Result<LoadedImage> {
    let started = Instant::now();
    let (meta, decoded) = picture(shared, &job.path, job.target)?;

    // The neighbourhood's filmstrip thumbnails come almost for free from here.
    if !shared.thumbs.contains(&job.path)
        && let Ok((w, h, rgb)) = thumbs::downscale(&decoded.rgb, decoded.width, decoded.height)
    {
        shared.thumbs.insert(&job.path, w, h, &rgb);
    }

    let image = ColorImage::from_rgb(
        [decoded.width as usize, decoded.height as usize],
        &decoded.rgb,
    );
    // Mipmaps keep the picture crisp when the window is much smaller than the monitor.
    let options = TextureOptions {
        mipmap_mode: Some(TextureFilter::Linear),
        ..TextureOptions::LINEAR
    };
    // `Context` is `Send + Sync`: uploading here keeps the UI thread free.
    let texture = shared
        .ctx
        .load_texture(library::file_name_lossy(&job.path), image, options);

    Ok(LoadedImage {
        texture,
        histogram: histogram::compute(&decoded.rgb),
        original_size: decoded.original_size,
        rating: meta.rating,
        label: meta.label,
        camera: meta.camera,
        description: meta.description,
        load_ms: started.elapsed().as_millis(),
    })
}

/// The file's bytes, never while the rating writer is halfway through rewriting it.
fn read(files: &FileLocks, path: &Path) -> Result<Vec<u8>> {
    let _held = files.hold(path);
    std::fs::read(path).context("cannot read file")
}

fn load_full(shared: &Shared, job: &Job) -> Result<FullImage> {
    let ctx = &shared.ctx;
    let (_, decoded) = picture(shared, &job.path, [u32::MAX; 2])?;

    let max_side = ctx.input(|i| i.max_texture_side) as u32;
    let tile_side = max_side.min(MAX_TILE);
    let (width, height) = (decoded.width, decoded.height);
    let mut tiles = Vec::new();
    for y0 in (0..height).step_by(tile_side as usize) {
        for x0 in (0..width).step_by(tile_side as usize) {
            let (tw, th) = (tile_side.min(width - x0), tile_side.min(height - y0));
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
            tiles.push(Tile {
                texture,
                origin: [x0, y0],
                size: [tw, th],
            });
        }
    }
    Ok(FullImage {
        size: [width, height],
        tiles,
    })
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
            pinned,
            target: [100, 100],
            cache: HashMap::new(),
            in_flight: HashSet::new(),
            want_full: HashSet::new(),
            full: HashMap::new(),
            full_in_flight: HashSet::new(),
            prefetch: true,
            shutdown: false,
        }
    }

    fn order(state: &mut State) -> Vec<(usize, bool)> {
        std::iter::from_fn(|| next_job(state).map(|j| (j.index, j.kind == Kind::Full))).collect()
    }

    #[test]
    fn the_same_list_keeps_decodes_in_flight_a_new_one_drops_them() {
        let mut s = state(20, 3, None);
        s.in_flight.extend([3, 4]);
        let same = Arc::clone(&s.paths);
        s.switch(Arc::new(same.to_vec()), 4, Some(3));
        assert_eq!((s.generation, s.current, s.pinned), (0, 4, Some(3)));
        assert_eq!(
            s.in_flight,
            HashSet::from([3, 4]),
            "compare mode keeps them"
        );
        let fewer: Vec<PathBuf> = same.iter().skip(1).cloned().collect();
        s.switch(Arc::new(fewer), 2, None);
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

    #[test]
    fn only_the_current_photo_before_the_first_frame() {
        let mut s = state(20, 10, None);
        s.prefetch = false;
        assert_eq!(order(&mut s), [(10, false)]);
    }
}
