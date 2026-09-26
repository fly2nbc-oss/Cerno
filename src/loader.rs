//! Background decoding with prefetch around the current image.
//!
//! Workers don't consume a queue: each one picks the most urgent image that is neither cached
//! nor being decoded, relative to the *current* index at the moment it looks. Jumping around
//! therefore re-prioritises automatically.
//!
//! Display images are decoded at monitor resolution. For zooming, the current image can also
//! be loaded at full resolution, split into tiles that fit the GPU's texture limit.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::{Context as _, Result};
use eframe::egui::{self, ColorImage, TextureFilter, TextureHandle, TextureOptions};

use crate::metadata::{self, CameraInfo, RatingInfo};
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
    /// Full image size after orientation.
    pub original_size: [u32; 2],
    /// Rating as stored in the file when it was decoded.
    pub rating: RatingInfo,
    pub camera: CameraInfo,
    pub load_ms: u128,
}

/// The current image at 100 %, as tiles in image pixel coordinates.
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
    /// Decode size in physical pixels (monitor size, clamped to the max texture side).
    target: [u32; 2],
    cache: HashMap<usize, Slot>,
    in_flight: HashSet<usize>,
    /// Full resolution wanted for this index (set while zoomed in).
    want_full: Option<usize>,
    full: Option<(usize, Option<Arc<FullImage>>)>,
    full_in_flight: bool,
    shutdown: bool,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    ctx: egui::Context,
    thumbs: Arc<Thumbs>,
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
    pub fn new(ctx: egui::Context, target: [u32; 2], thumbs: Arc<Thumbs>) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                generation: 0,
                paths: Arc::new(Vec::new()),
                current: 0,
                target,
                cache: HashMap::new(),
                in_flight: HashSet::new(),
                want_full: None,
                full: None,
                full_in_flight: false,
                shutdown: false,
            }),
            wake: Condvar::new(),
            ctx,
            thumbs,
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

    /// Switches to a new list. Images that are in both lists stay cached (re-sorting or
    /// filtering doesn't decode anything again).
    pub fn set_library(&self, paths: Arc<Vec<PathBuf>>, current: usize) {
        let mut guard = self.shared.lock();
        let state = &mut *guard;
        let old_paths = std::mem::replace(&mut state.paths, paths);
        let positions: HashMap<&PathBuf, usize> = state
            .paths
            .iter()
            .enumerate()
            .map(|(i, p)| (p, i))
            .collect();
        for (old, slot) in std::mem::take(&mut state.cache) {
            if let Some(&new) = old_paths.get(old).and_then(|p| positions.get(p))
                && new.abs_diff(current) <= KEEP_RADIUS
            {
                state.cache.insert(new, slot);
            }
        }
        state.full = state
            .full
            .take()
            .and_then(|(old, full)| Some((*positions.get(old_paths.get(old)?)?, full)))
            .filter(|(index, _)| *index == current);
        state.generation += 1;
        state.current = current;
        state.in_flight.clear();
        state.want_full = None;
        state.full_in_flight = false;
        drop(guard);
        self.shared.wake.notify_all();
    }

    pub fn set_current(&self, current: usize) {
        let mut state = self.shared.lock();
        if state.current == current {
            return;
        }
        state.current = current;
        state
            .cache
            .retain(|&i, _| i.abs_diff(current) <= KEEP_RADIUS);
        if state.full.as_ref().is_some_and(|(i, _)| *i != current) {
            state.full = None;
        }
        if state.want_full != Some(current) {
            state.want_full = None;
        }
        drop(state);
        self.shared.wake.notify_all();
    }

    /// Applies to new decodes; images already cached keep their size until evicted.
    pub fn set_target(&self, target: [u32; 2]) {
        self.shared.lock().target = target;
    }

    pub fn get(&self, index: usize) -> Lookup {
        match self.shared.lock().cache.get(&index) {
            Some(Slot::Ready(image)) => Lookup::Ready(Arc::clone(image)),
            Some(Slot::Failed(message)) => Lookup::Failed(message.clone()),
            None => Lookup::Pending,
        }
    }

    /// Asks for the full-resolution version of `index` (only kept for the current image).
    pub fn request_full(&self, index: usize) {
        let mut state = self.shared.lock();
        if state.want_full != Some(index) {
            state.want_full = Some(index);
            drop(state);
            self.shared.wake.notify_all();
        }
    }

    pub fn full(&self, index: usize) -> Option<Arc<FullImage>> {
        match &self.shared.lock().full {
            Some((i, Some(full))) if *i == index => Some(Arc::clone(full)),
            _ => None,
        }
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
                let result = load_display(shared, &job);
                let mut state = shared.lock();
                if state.generation != job.generation {
                    continue;
                }
                state.in_flight.remove(&job.index);
                let slot = match result {
                    Ok(image) => Slot::Ready(Arc::new(image)),
                    Err(err) => {
                        log::warn!("{}: {err:#}", job.path.display());
                        Slot::Failed(format!("{err:#}"))
                    }
                };
                if job.index.abs_diff(state.current) <= KEEP_RADIUS {
                    state.cache.insert(job.index, slot);
                }
            }
            Kind::Full => {
                let result = load_full(&shared.ctx, &job);
                let mut state = shared.lock();
                if state.generation != job.generation {
                    continue;
                }
                state.full_in_flight = false;
                if job.index == state.current {
                    let full = result
                        .map_err(|err| log::warn!("full size {}: {err:#}", job.path.display()))
                        .ok()
                        .map(Arc::new);
                    state.full = Some((job.index, full));
                }
            }
        }
        shared.ctx.request_repaint();
    }
}

fn next_job(state: &mut State) -> Option<Job> {
    let len = state.paths.len();
    let job = |state: &State, kind, index: usize| Job {
        kind,
        generation: state.generation,
        index,
        path: state.paths[index].clone(),
        target: state.target,
    };
    for (n, offset) in PREFETCH_ORDER.into_iter().enumerate() {
        // Right after the current image itself: its full-resolution version, if zoomed.
        if n == 1
            && state.want_full == Some(state.current)
            && !state.full_in_flight
            && state.full.as_ref().is_none_or(|(i, _)| *i != state.current)
        {
            state.full_in_flight = true;
            return Some(job(state, Kind::Full, state.current));
        }
        let Some(index) = state
            .current
            .checked_add_signed(offset)
            .filter(|&i| i < len)
        else {
            continue;
        };
        if state.cache.contains_key(&index) || !state.in_flight.insert(index) {
            continue;
        }
        return Some(job(state, Kind::Display, index));
    }
    None
}

fn load_display(shared: &Shared, job: &Job) -> Result<LoadedImage> {
    let started = Instant::now();
    let format = library::format_of(&job.path).context("unsupported file type")?;
    let bytes = std::fs::read(&job.path).context("cannot read file")?;
    let meta = metadata::read(&bytes);
    let decoded = decode::decode_for_display(&bytes, format, meta.orientation, job.target)?;

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
        original_size: decoded.original_size,
        rating: meta.rating,
        camera: meta.camera,
        load_ms: started.elapsed().as_millis(),
    })
}

fn load_full(ctx: &egui::Context, job: &Job) -> Result<FullImage> {
    let format = library::format_of(&job.path).context("unsupported file type")?;
    let bytes = std::fs::read(&job.path).context("cannot read file")?;
    let meta = metadata::read(&bytes);
    let decoded = decode::decode_for_display(&bytes, format, meta.orientation, [u32::MAX; 2])?;
    drop(bytes);

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
