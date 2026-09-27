//! Background analysis of every image in the folder: fingerprint, thumbnail, sharpness,
//! exposure, faces/eyes and aesthetics (LAION, V2.5), stored in the database. The personal
//! taste model and the CLIP attributes are computed from the stored CLIP embeddings.
//!
//! Nearest images first, like the loader. Work pauses while the user navigates, so decoding for
//! the display always wins. Files the database already knows cost one `stat` and one query.

pub mod aesthetic;
pub mod attributes;
pub mod exposure;
pub mod faces;
pub mod sharpness;
pub mod taste;

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

use crate::db::{Db, FileStamp, ImageRecord, Scores};
use crate::metadata::Rating;
use crate::{decode, library, metadata, paths, thumbs};
use aesthetic::{AestheticModel, V25Model};
use faces::FaceDetector;
use taste::TasteModel;

/// Long side of the image the analysis works on.
const ANALYSIS_SIZE: u32 = 2048;
/// Analysis waits this long after the last navigation.
const NAVIGATION_PAUSE: Duration = Duration::from_millis(900);
/// The taste model retrains this long after the last rating or deletion.
const TASTE_DELAY: Duration = Duration::from_secs(2);
const WORKERS: usize = 2;

/// What is known about a file, as far as the UI is concerned.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Known {
    pub rating: Rating,
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

    /// Only a real change bumps the version (the analysis re-confirms known values).
    fn set(&self, path: &Path, known: Known) {
        let previous = self
            .entries
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(path.to_path_buf(), known);
        if previous != Some(known) {
            self.bump();
        }
    }

    /// Something derived changed (e.g. the taste model was retrained).
    fn bump(&self) {
        self.version.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelState {
    /// Model files not present.
    Missing,
    Downloading {
        received: u64,
        total: u64,
    },
    /// Present, loaded on first use.
    Available,
    Loading,
    Ready {
        backend: &'static str,
    },
    Failed(String),
}

impl ModelState {
    fn usable(&self) -> bool {
        matches!(self, Self::Available | Self::Loading | Self::Ready { .. })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TasteStatus {
    /// Rated or deleted photos with an embedding.
    pub examples: usize,
    pub model: Option<(usize, f32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub done: usize,
    pub total: usize,
    /// CLIP + LAION.
    pub aesthetics: ModelState,
    /// SigLIP + Aesthetic Predictor V2.5.
    pub v25: ModelState,
    pub faces: ModelState,
    pub taste: TasteStatus,
}

struct State {
    generation: u64,
    paths: Arc<Vec<PathBuf>>,
    current: usize,
    done: HashSet<usize>,
    in_flight: HashSet<usize>,
    shutdown: bool,
}

enum Slot<M> {
    NotLoaded,
    Loaded(Box<M>),
    Unusable,
}

struct TasteState {
    dirty_since: Option<Instant>,
    shutdown: bool,
    status: TasteStatus,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    ctx: egui::Context,
    db: Arc<Db>,
    board: Arc<ScoreBoard>,
    thumbs: Arc<thumbs::Thumbs>,
    last_navigation: Mutex<Instant>,
    clip: Mutex<Slot<AestheticModel>>,
    clip_state: Mutex<ModelState>,
    v25: Mutex<Slot<V25Model>>,
    v25_state: Mutex<ModelState>,
    faces: Mutex<Slot<FaceDetector>>,
    faces_state: Mutex<ModelState>,
    /// CLIP embeddings of the files seen in this session (personal model, attributes).
    embeddings: RwLock<HashMap<PathBuf, Arc<[f32]>>>,
    taste: RwLock<Option<Arc<TasteModel>>>,
    taste_state: Mutex<TasteState>,
    taste_wake: Condvar,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

struct Job {
    generation: u64,
    index: usize,
    path: PathBuf,
}

/// Which optional models can run – decides what a complete record needs.
#[derive(Debug, Clone, Copy, Default)]
struct Capabilities {
    clip: bool,
    v25: bool,
}

/// Everything the current analysis would compute is already stored.
fn is_complete(image: &ImageRecord, caps: Capabilities) -> bool {
    image.has_thumbnail
        && image.scores.sharpness.is_some()
        && image.sharpness_version == sharpness::VERSION
        && image.exposure_version == exposure::VERSION
        && image.faces_version == faces::VERSION
        && (!caps.clip || image.aesthetic_model.as_deref() == Some(aesthetic::MODEL_ID))
        && (!caps.v25 || image.aesthetic25_model.as_deref() == Some(aesthetic::V25_MODEL_ID))
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
        let present = |ok: bool| {
            if ok {
                ModelState::Available
            } else {
                ModelState::Missing
            }
        };
        let models = paths::models_dir().ok();
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
            clip: Mutex::new(Slot::NotLoaded),
            clip_state: Mutex::new(present(clip_path().is_some_and(|p| p.is_file()))),
            v25: Mutex::new(Slot::NotLoaded),
            v25_state: Mutex::new(present(models.as_deref().is_some_and(V25Model::installed))),
            faces: Mutex::new(Slot::NotLoaded),
            faces_state: Mutex::new(ModelState::Available),
            embeddings: RwLock::default(),
            taste: RwLock::default(),
            taste_state: Mutex::new(TasteState {
                // Train once at start-up.
                dirty_since: Some(Instant::now() - TASTE_DELAY),
                shutdown: false,
                status: TasteStatus {
                    examples: 0,
                    model: None,
                },
            }),
            taste_wake: Condvar::new(),
        });
        let mut workers: Vec<JoinHandle<()>> = (0..WORKERS)
            .map(|i| {
                let shared = Arc::clone(&shared);
                std::thread::Builder::new()
                    .name(format!("cerno-analysis-{i}"))
                    .spawn(move || worker(&shared))
                    .expect("failed to spawn analysis worker")
            })
            .collect();
        let trainer_shared = Arc::clone(&shared);
        workers.push(
            std::thread::Builder::new()
                .name("cerno-taste".into())
                .spawn(move || taste_trainer(&trainer_shared))
                .expect("failed to spawn taste trainer"),
        );
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
                remember_embedding(&self.shared, path, record.image.embedding.as_deref());
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
            aesthetics: lock(&self.shared.clip_state).clone(),
            v25: lock(&self.shared.v25_state).clone(),
            faces: lock(&self.shared.faces_state).clone(),
            taste: lock(&self.shared.taste_state).status.clone(),
        }
    }

    /// Predicted stars from the personal taste model, 0..=5.
    pub fn personal(&self, path: &Path) -> Option<f32> {
        let embedding = self.embedding(path)?;
        let model = self
            .shared
            .taste
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()?;
        model.predict(&embedding)
    }

    /// CLIP zero-shot attributes (see `attributes::NAMES`).
    pub fn attributes(&self, path: &Path) -> Option<[f32; 6]> {
        attributes::scores(&self.embedding(path)?)
    }

    fn embedding(&self, path: &Path) -> Option<Arc<[f32]>> {
        self.shared
            .embeddings
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(path)
            .cloned()
    }

    /// A rating or deletion happened: retrain the taste model shortly.
    pub fn taste_changed(&self) {
        lock(&self.shared.taste_state).dirty_since = Some(Instant::now());
        self.shared.taste_wake.notify_all();
    }

    /// Downloads the CLIP model in the background, then re-runs the analysis to add the
    /// scores.
    pub fn download_model(&self) {
        {
            let mut state = lock(&self.shared.clip_state);
            if matches!(*state, ModelState::Downloading { .. }) {
                return;
            }
            *state = ModelState::Downloading {
                received: 0,
                total: aesthetic::MODEL_BYTES,
            };
        }
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name("cerno-model-download".into())
            .spawn(move || {
                let result = clip_path()
                    .context("no data directory")
                    .and_then(|dest| download(&shared, &dest));
                let state = match result {
                    Ok(()) => {
                        *lock(&shared.clip) = Slot::NotLoaded;
                        ModelState::Available
                    }
                    Err(err) => {
                        log::error!("model download failed: {err:#}");
                        ModelState::Failed(format!("Download failed: {err:#}"))
                    }
                };
                *lock(&shared.clip_state) = state;
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
        lock(&self.shared.taste_state).shutdown = true;
        self.shared.wake.notify_all();
        self.shared.taste_wake.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn clip_path() -> Option<PathBuf> {
    paths::models_dir()
        .ok()
        .map(|dir| dir.join(aesthetic::MODEL_FILE))
}

fn remember_embedding(shared: &Shared, path: &Path, embedding: Option<&[f32]>) {
    if let Some(embedding) = embedding {
        shared
            .embeddings
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(path.to_path_buf(), Arc::from(embedding));
    }
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
    let caps = Capabilities {
        clip: lock(&shared.clip_state).usable(),
        v25: lock(&shared.v25_state).usable(),
    };

    // Fast path: nothing to compute.
    if let Some(record) = shared.db.lookup(&key, stamp)? {
        remember_embedding(shared, path, record.image.embedding.as_deref());
        if is_complete(&record.image, caps) {
            shared.board.set(
                path,
                Known {
                    rating: record.rating,
                    scores: record.image.scores,
                },
            );
            return Ok(());
        }
    }

    let mut timings = Vec::new();
    let mut stage = Instant::now();
    let mut lap = |name: &'static str, timings: &mut Vec<(&'static str, u128)>| {
        timings.push((name, stage.elapsed().as_millis()));
        stage = Instant::now();
    };

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
    let (rgb, w, h) = (&image.rgb, image.width, image.height);
    lap("decode", &mut timings);

    let (tw, th, thumb) = thumbs::downscale(rgb, w, h)?;
    let fingerprint = fingerprint(&thumb, image.original_size);
    let mut record = shared.db.image(fingerprint)?;
    shared
        .db
        .put_file(&key, stamp, fingerprint, meta.rating.value)?;
    if !record.has_thumbnail {
        shared
            .db
            .put_thumbnail(fingerprint, &thumbs::encode_jpeg(tw, th, &thumb)?)?;
    }
    if !shared.thumbs.contains(path) {
        shared.thumbs.insert(path, tw, th, &thumb);
    }

    if record.scores.sharpness.is_none() || record.sharpness_version != sharpness::VERSION {
        let value = sharpness::measure(rgb, w, h);
        shared
            .db
            .put_sharpness(fingerprint, value, sharpness::VERSION)?;
        record.scores.sharpness = Some(value);
        lap("sharpness", &mut timings);
    }
    if record.exposure_version != exposure::VERSION {
        let (highlights, shadows) = exposure::measure(rgb);
        shared
            .db
            .put_exposure(fingerprint, highlights, shadows, exposure::VERSION)?;
        record.scores.highlights = Some(highlights);
        record.scores.shadows = Some(shadows);
    }
    if record.faces_version != faces::VERSION
        && let Some(found) = run_model(
            &shared.faces,
            &shared.faces_state,
            &shared.ctx,
            FaceDetector::load,
            |_| "CPU",
            |detector| detector.detect(rgb, w, h),
        )
    {
        let eyes = faces::eye_sharpness(rgb, w, h, &found);
        let count = found.len().min(255) as u8;
        shared
            .db
            .put_faces(fingerprint, eyes, count, faces::VERSION)?;
        record.scores.eyes = eyes;
        record.scores.faces = Some(count);
        lap("faces", &mut timings);
    }
    if caps.clip
        && record.aesthetic_model.as_deref() != Some(aesthetic::MODEL_ID)
        && let Some((value, embedding)) = run_model(
            &shared.clip,
            &shared.clip_state,
            &shared.ctx,
            || AestheticModel::load(&clip_path().context("no data directory")?),
            |m| m.backend,
            |model| model.score(rgb, w, h),
        )
    {
        shared
            .db
            .put_aesthetic(fingerprint, value, aesthetic::MODEL_ID, &embedding)?;
        record.scores.aesthetic = Some(value);
        record.embedding = Some(embedding);
        lap("clip", &mut timings);
    }
    remember_embedding(shared, path, record.embedding.as_deref());
    if caps.v25
        && record.aesthetic25_model.as_deref() != Some(aesthetic::V25_MODEL_ID)
        && let Some(value) = run_model(
            &shared.v25,
            &shared.v25_state,
            &shared.ctx,
            || V25Model::load(&paths::models_dir()?),
            |m| m.backend,
            |model| model.score(rgb, w, h),
        )
    {
        shared
            .db
            .put_aesthetic25(fingerprint, value, aesthetic::V25_MODEL_ID)?;
        record.scores.aesthetic25 = Some(value);
        lap("v2.5", &mut timings);
    }
    log::debug!("analysed {}: {timings:?} ms", path.display());

    shared.board.set(
        path,
        Known {
            rating: meta.rating.value,
            scores: record.scores,
        },
    );
    Ok(())
}

/// Loads a model on first use (one instance shared by all workers), reports its state and
/// runs it. `None` if it can't be loaded or the run fails.
fn run_model<M, R>(
    slot: &Mutex<Slot<M>>,
    state: &Mutex<ModelState>,
    ctx: &egui::Context,
    load: impl FnOnce() -> Result<M>,
    backend: impl Fn(&M) -> &'static str,
    run: impl FnOnce(&mut M) -> Result<R>,
) -> Option<R> {
    let mut slot = lock(slot);
    if matches!(*slot, Slot::NotLoaded) {
        *lock(state) = ModelState::Loading;
        ctx.request_repaint();
        *slot = match load() {
            Ok(model) => {
                *lock(state) = ModelState::Ready {
                    backend: backend(&model),
                };
                Slot::Loaded(Box::new(model))
            }
            Err(err) => {
                log::error!("model: {err:#}");
                *lock(state) = ModelState::Failed(format!("{err:#}"));
                Slot::Unusable
            }
        };
        ctx.request_repaint();
    }
    let Slot::Loaded(model) = &mut *slot else {
        return None;
    };
    match run(model) {
        Ok(result) => Some(result),
        Err(err) => {
            log::warn!("model run failed: {err:#}");
            None
        }
    }
}

/// Retrains the taste model a little after ratings or deletions change.
fn taste_trainer(shared: &Shared) {
    loop {
        {
            let mut state = lock(&shared.taste_state);
            loop {
                if state.shutdown {
                    return;
                }
                match state.dirty_since {
                    Some(since) if since.elapsed() >= TASTE_DELAY => break,
                    Some(since) => {
                        let wait = TASTE_DELAY - since.elapsed();
                        state = shared
                            .taste_wake
                            .wait_timeout(state, wait)
                            .unwrap_or_else(|p| p.into_inner())
                            .0;
                    }
                    None => {
                        state = shared
                            .taste_wake
                            .wait(state)
                            .unwrap_or_else(|p| p.into_inner());
                    }
                }
            }
            state.dirty_since = None;
        }

        let started = Instant::now();
        let examples = match shared.db.taste_examples() {
            Ok(examples) => examples,
            Err(err) => {
                log::warn!("taste examples: {err:#}");
                continue;
            }
        };
        let model = TasteModel::train(&examples).map(Arc::new);
        match &model {
            Some(m) => log::info!(
                "taste model: {} examples, error {:.2} stars, {} ms",
                m.examples,
                m.error,
                started.elapsed().as_millis()
            ),
            None => log::info!("taste model: {} examples – not enough yet", examples.len()),
        }
        lock(&shared.taste_state).status = TasteStatus {
            examples: examples.len(),
            model: model.as_ref().map(|m| (m.examples, m.error)),
        };
        *shared.taste.write().unwrap_or_else(|p| p.into_inner()) = model;
        shared.board.bump();
        shared.ctx.request_repaint();
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
            *lock(&shared.clip_state) = ModelState::Downloading { received, total };
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

    fn complete_record() -> ImageRecord {
        ImageRecord {
            scores: Scores {
                sharpness: Some(1.0),
                ..Scores::default()
            },
            sharpness_version: sharpness::VERSION,
            aesthetic_model: Some(aesthetic::MODEL_ID.into()),
            aesthetic25_model: Some(aesthetic::V25_MODEL_ID.into()),
            exposure_version: exposure::VERSION,
            faces_version: faces::VERSION,
            has_thumbnail: true,
            embedding: None,
        }
    }

    #[test]
    fn completeness_checks_every_module() {
        let all = Capabilities {
            clip: true,
            v25: true,
        };
        assert!(is_complete(&complete_record(), all));
        let missing: [fn(&mut ImageRecord); 7] = [
            |r| r.has_thumbnail = false,
            |r| r.scores.sharpness = None,
            |r| r.sharpness_version = 0,
            |r| r.exposure_version = 0,
            |r| r.faces_version = 0,
            |r| r.aesthetic_model = None,
            |r| r.aesthetic25_model = Some("old".into()),
        ];
        for (i, strip) in missing.iter().enumerate() {
            let mut record = complete_record();
            strip(&mut record);
            assert!(!is_complete(&record, all), "case {i}");
        }
        // Models that can't run are not required.
        let mut record = complete_record();
        record.aesthetic_model = None;
        record.aesthetic25_model = None;
        assert!(is_complete(&record, Capabilities::default()));
    }
}
