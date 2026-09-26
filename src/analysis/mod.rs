//! Background analysis of every image in the folder: fingerprint, thumbnail, sharpness and
//! aesthetics, stored in the database.
//!
//! Nearest images first, like the loader. Work pauses while the user navigates, so decoding for
//! the display always wins. Files the database already knows cost one `stat` and one query.

pub mod aesthetic;
pub mod sharpness;

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use eframe::egui;

use crate::db::{Db, FileStamp, Scores};
use crate::{decode, library, metadata, paths, thumbs};
use aesthetic::AestheticModel;

/// Long side of the image the analysis works on.
const ANALYSIS_SIZE: u32 = 2048;
/// Analysis waits this long after the last navigation.
const NAVIGATION_PAUSE: Duration = Duration::from_millis(900);
const WORKERS: usize = 2;

/// What is known about a file, as far as the UI is concerned.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Known {
    pub rating: Option<u8>,
    pub scores: Scores,
}

/// Results shared with the UI. `version` changes whenever an entry does.
#[derive(Default)]
pub struct ScoreBoard {
    entries: RwLock<HashMap<PathBuf, Known>>,
    version: AtomicU64,
}

impl ScoreBoard {
    pub fn get(&self, path: &Path) -> Option<Known> {
        self.entries
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(path)
            .copied()
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    /// Sharpness values of the given paths, sorted – for percentiles within the folder.
    pub fn sorted_sharpness(&self, paths: &[PathBuf]) -> Vec<f32> {
        let entries = self.entries.read().unwrap_or_else(|p| p.into_inner());
        let mut values: Vec<f32> = paths
            .iter()
            .filter_map(|p| entries.get(p)?.scores.sharpness)
            .collect();
        values.sort_by(f32::total_cmp);
        values
    }

    /// Only a real change bumps the version (the analysis re-confirms known values).
    fn set(&self, path: &Path, known: Known) {
        let previous = self
            .entries
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(path.to_path_buf(), known);
        if previous != Some(known) {
            self.version.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AestheticsState {
    /// Model file not downloaded.
    Missing,
    Downloading {
        received: u64,
        total: u64,
    },
    /// Downloaded, loaded on first use.
    Available,
    Loading,
    Ready {
        backend: &'static str,
    },
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub done: usize,
    pub total: usize,
    pub aesthetics: AestheticsState,
}

struct State {
    generation: u64,
    paths: Arc<Vec<PathBuf>>,
    current: usize,
    done: HashSet<usize>,
    in_flight: HashSet<usize>,
    shutdown: bool,
}

enum Model {
    NotLoaded,
    Loaded(Box<AestheticModel>),
    Unusable,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    ctx: egui::Context,
    db: Arc<Db>,
    board: Arc<ScoreBoard>,
    thumbs: Arc<thumbs::Thumbs>,
    last_navigation: Mutex<Instant>,
    model: Mutex<Model>,
    aesthetics: Mutex<AestheticsState>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

struct Job {
    generation: u64,
    index: usize,
    path: PathBuf,
}

pub struct Analyzer {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}

impl Analyzer {
    pub fn new(
        ctx: egui::Context,
        db: Arc<Db>,
        board: Arc<ScoreBoard>,
        thumbs: Arc<thumbs::Thumbs>,
    ) -> Self {
        let aesthetics = if model_path().is_some_and(|p| p.is_file()) {
            AestheticsState::Available
        } else {
            AestheticsState::Missing
        };
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                generation: 0,
                paths: Arc::new(Vec::new()),
                current: 0,
                done: HashSet::new(),
                in_flight: HashSet::new(),
                shutdown: false,
            }),
            wake: Condvar::new(),
            ctx,
            db,
            board,
            thumbs,
            last_navigation: Mutex::new(Instant::now()),
            model: Mutex::new(Model::NotLoaded),
            aesthetics: Mutex::new(aesthetics),
        });
        let workers = (0..WORKERS)
            .map(|i| {
                let shared = Arc::clone(&shared);
                std::thread::Builder::new()
                    .name(format!("cerno-analysis-{i}"))
                    .spawn(move || worker(&shared))
                    .expect("failed to spawn analysis worker")
            })
            .collect();
        Self { shared, workers }
    }

    pub fn set_library(&self, paths: Arc<Vec<PathBuf>>, current: usize) {
        let mut state = lock(&self.shared.state);
        state.generation += 1;
        state.paths = paths;
        state.current = current;
        state.done.clear();
        state.in_flight.clear();
        drop(state);
        self.shared.wake.notify_all();
    }

    /// Fills the score board from the index for unchanged files, so a saved sort order or
    /// filter applies immediately when an analysed folder is reopened.
    pub fn preload(&self, paths: &[PathBuf]) {
        let started = Instant::now();
        for path in paths {
            let Ok(stamp) = FileStamp::of(path) else {
                continue;
            };
            if let Ok(Some(record)) = self.shared.db.lookup(&path.to_string_lossy(), stamp) {
                self.shared.board.set(
                    path,
                    Known {
                        rating: record.rating,
                        scores: record.image.scores,
                    },
                );
            }
        }
        log::info!(
            "preloaded {} files from the index in {} ms",
            paths.len(),
            started.elapsed().as_millis()
        );
    }

    /// `current` indexes the full folder list; also pauses the analysis for a moment.
    pub fn set_current(&self, current: usize) {
        *lock(&self.shared.last_navigation) = Instant::now();
        lock(&self.shared.state).current = current;
    }

    pub fn status(&self) -> Status {
        let state = lock(&self.shared.state);
        Status {
            done: state.done.len(),
            total: state.paths.len(),
            aesthetics: lock(&self.shared.aesthetics).clone(),
        }
    }

    /// Downloads the aesthetics model in the background, then re-runs the analysis to add
    /// the scores.
    pub fn download_model(&self) {
        {
            let mut aesthetics = lock(&self.shared.aesthetics);
            if matches!(*aesthetics, AestheticsState::Downloading { .. }) {
                return;
            }
            *aesthetics = AestheticsState::Downloading {
                received: 0,
                total: aesthetic::MODEL_BYTES,
            };
        }
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name("cerno-model-download".into())
            .spawn(move || {
                let result = model_path()
                    .context("no data directory")
                    .and_then(|dest| download(&shared, &dest));
                let state = match result {
                    Ok(()) => {
                        *lock(&shared.model) = Model::NotLoaded;
                        AestheticsState::Available
                    }
                    Err(err) => {
                        log::error!("model download failed: {err:#}");
                        AestheticsState::Failed(format!("Download failed: {err:#}"))
                    }
                };
                *lock(&shared.aesthetics) = state;
                // Revisit every image so the new scores get added.
                lock(&shared.state).done.clear();
                shared.wake.notify_all();
                shared.ctx.request_repaint();
            })
            .expect("failed to spawn download thread");
    }
}

impl Drop for Analyzer {
    fn drop(&mut self) {
        lock(&self.shared.state).shutdown = true;
        self.shared.wake.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn model_path() -> Option<PathBuf> {
    paths::models_dir()
        .ok()
        .map(|dir| dir.join(aesthetic::MODEL_FILE))
}

fn worker(shared: &Shared) {
    loop {
        let job = {
            let mut state = lock(&shared.state);
            loop {
                if state.shutdown {
                    return;
                }
                if let Some(job) = next_job(&mut state) {
                    break job;
                }
                state = shared.wake.wait(state).unwrap_or_else(|p| p.into_inner());
            }
        };

        // Let the display decode first while the user is browsing.
        loop {
            let since = lock(&shared.last_navigation).elapsed();
            if since >= NAVIGATION_PAUSE {
                break;
            }
            std::thread::sleep(NAVIGATION_PAUSE - since);
        }

        if let Err(err) = analyze(shared, &job.path) {
            log::warn!("analysis of {}: {err:#}", job.path.display());
        }

        let mut state = lock(&shared.state);
        if state.generation == job.generation {
            state.in_flight.remove(&job.index);
            state.done.insert(job.index);
        }
        drop(state);
        shared.ctx.request_repaint();
    }
}

fn next_job(state: &mut State) -> Option<Job> {
    let len = state.paths.len();
    for distance in 0..len {
        for index in [
            state.current.checked_add(distance),
            state.current.checked_sub(distance),
        ]
        .into_iter()
        .flatten()
        .filter(|&i| i < len)
        {
            if state.done.contains(&index) || !state.in_flight.insert(index) {
                continue;
            }
            return Some(Job {
                generation: state.generation,
                index,
                path: state.paths[index].clone(),
            });
        }
    }
    None
}

fn analyze(shared: &Shared, path: &Path) -> Result<()> {
    let stamp = FileStamp::of(path)?;
    let key = path.to_string_lossy();
    let want_aesthetics = aesthetics_wanted(shared);

    // Fast path: nothing to compute.
    if let Some(record) = shared.db.lookup(&key, stamp)? {
        let image = &record.image;
        let complete = image.scores.sharpness.is_some()
            && image.sharpness_version == sharpness::VERSION
            && image.has_thumbnail
            && (!want_aesthetics || image.aesthetic_model.as_deref() == Some(aesthetic::MODEL_ID));
        if complete {
            shared.board.set(
                path,
                Known {
                    rating: record.rating,
                    scores: image.scores,
                },
            );
            return Ok(());
        }
    }

    let format = library::format_of(path).context("unsupported file type")?;
    let bytes = std::fs::read(path).context("cannot read file")?;
    let meta = metadata::read(&bytes);
    let image = decode::decode_for_display(
        &bytes,
        format,
        meta.orientation,
        [ANALYSIS_SIZE, ANALYSIS_SIZE],
    )?;
    drop(bytes);

    let (tw, th, thumb) = thumbs::downscale(&image.rgb, image.width, image.height)?;
    let fingerprint = fingerprint(&thumb, image.original_size);
    let mut record = shared.db.image(fingerprint)?;
    shared
        .db
        .put_file(&key, stamp, fingerprint, meta.rating.stars)?;

    if !record.has_thumbnail {
        shared
            .db
            .put_thumbnail(fingerprint, &thumbs::encode_jpeg(tw, th, &thumb)?)?;
    }
    if !shared.thumbs.contains(path) {
        shared.thumbs.insert(path, tw, th, &thumb);
    }
    if record.scores.sharpness.is_none() || record.sharpness_version != sharpness::VERSION {
        let value = sharpness::measure(&image.rgb, image.width, image.height);
        shared
            .db
            .put_sharpness(fingerprint, value, sharpness::VERSION)?;
        record.scores.sharpness = Some(value);
    }
    if want_aesthetics
        && record.aesthetic_model.as_deref() != Some(aesthetic::MODEL_ID)
        && let Some((value, embedding)) =
            score_aesthetics(shared, &image.rgb, image.width, image.height)
    {
        shared
            .db
            .put_aesthetic(fingerprint, value, aesthetic::MODEL_ID, &embedding)?;
        record.scores.aesthetic = Some(value);
    }

    shared.board.set(
        path,
        Known {
            rating: meta.rating.stars,
            scores: record.scores,
        },
    );
    Ok(())
}

fn aesthetics_wanted(shared: &Shared) -> bool {
    matches!(
        *lock(&shared.aesthetics),
        AestheticsState::Available | AestheticsState::Loading | AestheticsState::Ready { .. }
    )
}

/// Loads the model on first use (one session for all workers) and scores the image.
fn score_aesthetics(
    shared: &Shared,
    rgb: &[u8],
    width: u32,
    height: u32,
) -> Option<(f32, Vec<f32>)> {
    let mut model = lock(&shared.model);
    if matches!(*model, Model::NotLoaded) {
        *lock(&shared.aesthetics) = AestheticsState::Loading;
        shared.ctx.request_repaint();
        let loaded = model_path()
            .context("no data directory")
            .and_then(|p| AestheticModel::load(&p));
        *model = match loaded {
            Ok(loaded) => {
                *lock(&shared.aesthetics) = AestheticsState::Ready {
                    backend: loaded.backend,
                };
                Model::Loaded(Box::new(loaded))
            }
            Err(err) => {
                log::error!("aesthetics model: {err:#}");
                *lock(&shared.aesthetics) = AestheticsState::Failed(format!("{err:#}"));
                Model::Unusable
            }
        };
        shared.ctx.request_repaint();
    }
    let Model::Loaded(model) = &mut *model else {
        return None;
    };
    match model.score(rgb, width, height) {
        Ok(value) => Some(value),
        Err(err) => {
            log::warn!("aesthetics scoring failed: {err:#}");
            None
        }
    }
}

/// FNV-1a over the thumbnail pixels and the full size: survives metadata edits and renames,
/// changes when the pixels do. (Not `DefaultHasher`: its algorithm may change between Rust
/// versions.) A decoder or resizer upgrade changes fingerprints too – that only means a
/// re-analysis, no data loss.
pub fn fingerprint(thumb_rgb: &[u8], original_size: [u32; 2]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let size = original_size.map(u32::to_le_bytes);
    for byte in size.iter().flatten().chain(thumb_rgb) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Streams the model to `<dest>.part`, checks its size, then renames it into place.
fn download(shared: &Shared, dest: &Path) -> Result<()> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = dest.with_extension("onnx.part");
    let mut response = ureq::get(aesthetic::MODEL_URL).call()?;
    let total = response
        .body()
        .content_length()
        .unwrap_or(aesthetic::MODEL_BYTES);
    let mut reader = response.body_mut().as_reader();
    let mut file = File::create(&part)?;
    let mut buffer = vec![0u8; 1 << 20];
    let mut received = 0u64;
    let mut last_report = Instant::now();
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])?;
        received += n as u64;
        if last_report.elapsed() > Duration::from_millis(200) {
            *lock(&shared.aesthetics) = AestheticsState::Downloading { received, total };
            shared.ctx.request_repaint();
            last_report = Instant::now();
        }
        if lock(&shared.state).shutdown {
            bail!("cancelled");
        }
    }
    file.sync_all()?;
    drop(file);
    if received != aesthetic::MODEL_BYTES {
        let _ = std::fs::remove_file(&part);
        bail!(
            "unexpected size {received} bytes (expected {})",
            aesthetic::MODEL_BYTES
        );
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_depends_on_pixels_and_size() {
        let a = fingerprint(&[1, 2, 3], [100, 50]);
        assert_eq!(a, fingerprint(&[1, 2, 3], [100, 50]));
        assert_ne!(a, fingerprint(&[1, 2, 4], [100, 50]));
        assert_ne!(a, fingerprint(&[1, 2, 3], [50, 100]));
    }

    #[test]
    fn jobs_start_at_the_current_image_and_spread_out() {
        let mut state = State {
            generation: 0,
            paths: Arc::new((0..6).map(|i| PathBuf::from(format!("{i}.jpg"))).collect()),
            current: 2,
            done: HashSet::from([1]),
            in_flight: HashSet::new(),
            shutdown: false,
        };
        let order: Vec<usize> =
            std::iter::from_fn(|| next_job(&mut state).map(|j| j.index)).collect();
        assert_eq!(order, [2, 3, 4, 0, 5]);
    }
}
