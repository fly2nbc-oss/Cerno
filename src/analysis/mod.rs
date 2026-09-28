//! Background analysis of every image in the folder: fingerprint, thumbnail, sharpness,
//! exposure, faces/eyes and aesthetics (LAION, V2.5), stored in the database. The personal
//! taste model and the CLIP attributes are computed from the stored CLIP embeddings.
//!
//! Nearest images first, like the loader. Work pauses while the user navigates, so decoding for
//! the display always wins. Files the database already knows cost one `stat` and one query.
//!
//! A photo the rating writer is about to change, or changed while it was analysed, is put back
//! and taken up again a moment later: its bytes (and the rating read from them) would be stale.

pub mod aesthetic;
pub mod attributes;
pub mod exposure;
pub mod faces;
pub mod sharpness;
pub mod taste;

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use eframe::egui;

use crate::db::{Db, FileStamp, ImageRecord, Scores};
use crate::filelock::FileLocks;
use crate::metadata::{Label, Rating};
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
/// A photo that was being written is tried again after this long …
const RETRY_DELAY: Duration = Duration::from_secs(1);
/// … this many times; a file that keeps changing is left for the next visit.
const MAX_RETRIES: u32 = 10;
/// A model download that gets no data for this long gives up (ureq has no idle timeout).
const DOWNLOAD_STALL: Duration = Duration::from_secs(60);

/// What is known about a file, as far as the UI is concerned.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Known {
    pub rating: Rating,
    pub label: Option<Label>,
    pub taken_ms: Option<i64>,
    /// `None` until the file has been indexed.
    pub fingerprint: Option<u64>,
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
    /// "Delete models": waiting for a running analysis, then the files go.
    Removing,
}

impl ModelState {
    fn usable(&self) -> bool {
        matches!(self, Self::Available | Self::Loading | Self::Ready { .. })
    }

    /// Nothing to load: the file is not there or is about to go.
    fn absent(&self) -> bool {
        matches!(self, Self::Missing | Self::Removing)
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
    /// Put back while a write was pending: when to try again, and how often it was tried.
    deferred: HashMap<usize, (Instant, u32)>,
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
    files: Arc<FileLocks>,
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
    /// The result of "Delete models", until the UI picks it up.
    removal: Mutex<Option<Result<(), String>>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

struct Job {
    generation: u64,
    index: usize,
    path: PathBuf,
}

/// How one analysis ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Done,
    /// The file is being written (or was, meanwhile): try again shortly.
    Retry,
}

/// Which models can run – decides what a complete record needs. A model that failed to load
/// (even the embedded face detector) is not waited for, or every photo would be decoded
/// again on every visit.
#[derive(Debug, Clone, Copy, Default)]
struct Capabilities {
    clip: bool,
    v25: bool,
    faces: bool,
}

/// Scores and thumbnail are current. Capture time is checked separately so a metadata bump
/// does not decode the image again.
fn scores_complete(image: &ImageRecord, caps: Capabilities) -> bool {
    image.has_thumbnail
        && image.scores.sharpness.is_some()
        && image.sharpness_version == sharpness::VERSION
        && image.exposure_version == exposure::VERSION
        && (!caps.faces || image.faces_version == faces::VERSION)
        && (!caps.clip || image.aesthetic_model.as_deref() == Some(aesthetic::MODEL_ID))
        && (!caps.v25 || image.aesthetic25_model.as_deref() == Some(aesthetic::V25_MODEL_ID))
}

/// Everything the current analysis would compute is already stored.
fn is_complete(image: &ImageRecord, caps: Capabilities) -> bool {
    scores_complete(image, caps) && image.metadata_version == metadata::VERSION
}

fn known_from(record: &crate::db::FileRecord) -> Known {
    Known {
        rating: record.rating,
        label: record.label,
        taken_ms: record.image.taken_ms,
        fingerprint: Some(record.fingerprint),
        scores: record.image.scores,
    }
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
        files: Arc<FileLocks>,
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
                deferred: HashMap::new(),
                shutdown: false,
            }),
            wake: Condvar::new(),
            ctx,
            db,
            board,
            thumbs,
            files,
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
            removal: Mutex::new(None),
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
        state.deferred.clear();
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
                self.shared.board.set(path, known_from(&record));
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

    /// The file's pixels changed. Drop this path from the "already done" set so it is
    /// analysed again, and ignore an analysis that is still running on the old bytes.
    pub fn revisit(&self, path: &Path) {
        self.shared
            .embeddings
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .remove(path);
        let mut state = lock(&self.shared.state);
        state.generation += 1;
        state.in_flight.clear();
        if let Some(index) = state.paths.iter().position(|p| p == path) {
            state.done.remove(&index);
        }
        drop(state);
        self.shared.wake.notify_all();
    }

    /// Downloads the CLIP model in the background, then re-runs the analysis to add the
    /// scores.
    /// Clears taste feedback in the index; star ratings in the files stay.
    pub fn reset_taste_learning(&self) {
        if let Err(err) = self.shared.db.reset_taste_learning() {
            log::warn!("taste reset: {err:#}");
            return;
        }
        self.taste_changed();
    }

    /// Removes the downloaded ONNX files, in the background. The states say "removing" at
    /// once, so no analysis starts a model; the files go once a running inference has
    /// finished. The result comes back through [`Self::take_removal`].
    pub fn delete_installed_models(&self) {
        for state in [&self.shared.clip_state, &self.shared.v25_state] {
            *lock(state) = ModelState::Removing;
        }
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name("cerno-models-remove".into())
            .spawn(move || {
                let result = remove_models(&shared).map_err(|err| format!("{err:#}"));
                if let Err(err) = &result {
                    log::error!("delete models: {err}");
                }
                *lock(&shared.removal) = Some(result);
                lock(&shared.state).done.clear();
                shared.wake.notify_all();
                shared.ctx.request_repaint();
            })
            .expect("failed to spawn model removal");
    }

    /// The outcome of the last "Delete models", once.
    pub fn take_removal(&self) -> Option<Result<(), String>> {
        lock(&self.shared.removal).take()
    }

    pub fn clip_model_missing(&self) -> bool {
        matches!(
            *lock(&self.shared.clip_state),
            ModelState::Missing | ModelState::Failed(_)
        )
    }

    pub fn download_model(&self) {
        {
            let mut state = lock(&self.shared.clip_state);
            if matches!(
                *state,
                ModelState::Downloading { .. } | ModelState::Removing
            ) {
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

/// Holds both model slots – waiting for an inference that runs now – so no worker has a
/// session open on the files while they are deleted. Workers take one slot at a time, so
/// taking both here cannot deadlock.
fn remove_models(shared: &Shared) -> Result<()> {
    let mut clip = lock(&shared.clip);
    let mut v25 = lock(&shared.v25);
    *clip = Slot::NotLoaded;
    *v25 = Slot::NotLoaded;
    let mut result = Ok(());
    if let Ok(dir) = paths::models_dir() {
        for name in [
            aesthetic::MODEL_FILE,
            aesthetic::SIGLIP_FILE,
            aesthetic::V25_HEAD_FILE,
        ] {
            let path = dir.join(name);
            if path.is_file()
                && let Err(err) = std::fs::remove_file(&path)
            {
                result = Err(anyhow::Error::from(err))
                    .with_context(|| format!("cannot delete {}", path.display()));
            }
        }
        let _ = std::fs::remove_file(dir.join(format!("{}.part", aesthetic::MODEL_FILE)));
    }
    // What is still there (a file that could not be deleted) stays usable.
    let present = |ok: bool| {
        if ok {
            ModelState::Available
        } else {
            ModelState::Missing
        }
    };
    *lock(&shared.clip_state) = present(clip_path().is_some_and(|p| p.is_file()));
    *lock(&shared.v25_state) = present(
        paths::models_dir()
            .ok()
            .as_deref()
            .is_some_and(V25Model::installed),
    );
    result
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
                let now = Instant::now();
                if let Some(job) = next_job(&mut state, now) {
                    break job;
                }
                // Only put-back photos left: wake up when the first of them is due.
                let due = state.deferred.values().map(|(at, _)| *at).min();
                state = match due {
                    Some(at) => {
                        shared
                            .wake
                            .wait_timeout(state, at.saturating_duration_since(now))
                            .unwrap_or_else(|p| p.into_inner())
                            .0
                    }
                    None => shared.wake.wait(state).unwrap_or_else(|p| p.into_inner()),
                };
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

        let outcome = analyze(shared, &job.path).unwrap_or_else(|err| {
            log::warn!("analysis of {}: {err:#}", job.path.display());
            Outcome::Done
        });

        let mut state = lock(&shared.state);
        if state.generation == job.generation {
            state.in_flight.remove(&job.index);
            finish(&mut state, job.index, outcome, Instant::now());
        }
        drop(state);
        shared.ctx.request_repaint();
    }
}

/// Marks the job done, or puts it back for a later try (a bounded number of times).
fn finish(state: &mut State, index: usize, outcome: Outcome, now: Instant) {
    let tries = state.deferred.remove(&index).map_or(0, |(_, tries)| tries);
    if outcome == Outcome::Retry && tries < MAX_RETRIES {
        state.deferred.insert(index, (now + RETRY_DELAY, tries + 1));
    } else {
        state.done.insert(index);
    }
}

fn next_job(state: &mut State, now: Instant) -> Option<Job> {
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
            let waiting = state.deferred.get(&index).is_some_and(|(at, _)| *at > now);
            if state.done.contains(&index) || waiting || !state.in_flight.insert(index) {
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

/// The file's bytes, read while nobody writes it.
fn read(files: &FileLocks, path: &Path) -> Result<Vec<u8>> {
    let _held = files.hold(path);
    std::fs::read(path).context("cannot read file")
}

fn analyze(shared: &Shared, path: &Path) -> Result<Outcome> {
    // A mark on its way would make the rating read below stale; take the photo up later.
    let files = &*shared.files;
    if files.busy(path) {
        return Ok(Outcome::Retry);
    }
    let generation = files.generation(path);
    // Written meanwhile (the stamp can stay the same: same size, dates put back)?
    let changed = || files.busy(path) || files.generation(path) != generation;
    let stamp = FileStamp::of(path)?;
    let key = path.to_string_lossy();
    let caps = Capabilities {
        clip: lock(&shared.clip_state).usable(),
        v25: lock(&shared.v25_state).usable(),
        faces: lock(&shared.faces_state).usable(),
    };

    // Fast path: nothing to compute.
    if let Some(record) = shared.db.lookup(&key, stamp)? {
        remember_embedding(shared, path, record.image.embedding.as_deref());
        if is_complete(&record.image, caps) {
            shared.board.set(path, known_from(&record));
            return Ok(Outcome::Done);
        }
        // Scores are done; only the capture time (and a fresh read of rating/label) is missing.
        if scores_complete(&record.image, caps) {
            let bytes = read(files, path)?;
            let meta = metadata::read(&bytes);
            if changed() {
                return Ok(Outcome::Retry);
            }
            shared.db.put_file(
                &key,
                stamp,
                record.fingerprint,
                meta.rating.value,
                meta.label.known(),
            )?;
            shared
                .db
                .put_metadata(record.fingerprint, meta.camera.taken_ms, metadata::VERSION)?;
            shared.board.set(
                path,
                Known {
                    rating: meta.rating.value,
                    label: meta.label.known(),
                    taken_ms: meta.camera.taken_ms,
                    fingerprint: Some(record.fingerprint),
                    scores: record.image.scores,
                },
            );
            // A write that slipped in between is repaired by the next try.
            return Ok(if changed() {
                Outcome::Retry
            } else {
                Outcome::Done
            });
        }
    }

    let mut timings = Vec::new();
    let mut stage = Instant::now();
    let mut lap = |name: &'static str, timings: &mut Vec<(&'static str, u128)>| {
        timings.push((name, stage.elapsed().as_millis()));
        stage = Instant::now();
    };

    let format = library::format_of(path).context("unsupported file type")?;
    let bytes = read(files, path)?;
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
    // A mark, straighten or crop rewrote the file while this ran on the old bytes.
    if changed() || FileStamp::of(path).ok() != Some(stamp) {
        log::debug!(
            "file changed during analysis, again later: {}",
            path.display()
        );
        return Ok(Outcome::Retry);
    }
    shared.db.put_file(
        &key,
        stamp,
        fingerprint,
        meta.rating.value,
        meta.label.known(),
    )?;
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
    if caps.faces
        && record.faces_version != faces::VERSION
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
    shared
        .db
        .put_metadata(fingerprint, meta.camera.taken_ms, metadata::VERSION)?;
    log::debug!("analysed {}: {timings:?} ms", path.display());

    shared.board.set(
        path,
        Known {
            rating: meta.rating.value,
            label: meta.label.known(),
            taken_ms: meta.camera.taken_ms,
            fingerprint: Some(fingerprint),
            scores: record.scores,
        },
    );
    // A write that slipped in after the check above: the next try stores its rating.
    Ok(if changed() {
        Outcome::Retry
    } else {
        Outcome::Done
    })
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
        // Deleted, or being deleted, since this analysis looked: don't open the file again.
        if lock(state).absent() {
            return None;
        }
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

/// Streams the model to `<dest>.part`, checks its size, then renames it into place. A failed
/// or cancelled download leaves no `.part` behind.
fn download(shared: &Shared, dest: &Path) -> Result<()> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = dest.with_extension("onnx.part");
    let result = fetch(shared, &part).and_then(|received| {
        if received == aesthetic::MODEL_BYTES {
            Ok(())
        } else {
            bail!(
                "unexpected size {received} bytes (expected {})",
                aesthetic::MODEL_BYTES
            )
        }
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
        return result;
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

fn fetch(shared: &Shared, part: &Path) -> Result<u64> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .build()
        .into();
    let response = agent.get(aesthetic::MODEL_URL).call()?;
    let total = response
        .body()
        .content_length()
        .unwrap_or(aesthetic::MODEL_BYTES);
    let mut reader = response.into_body().into_reader();
    // The socket is read on a helper thread, so a connection that stalls without closing
    // ends the download after `DOWNLOAD_STALL` instead of hanging it forever. The helper
    // ends with its next read once nobody listens any more.
    let (tx, rx) = mpsc::sync_channel(4);
    std::thread::Builder::new()
        .name("cerno-model-read".into())
        .spawn(move || {
            loop {
                let mut buffer = vec![0u8; 1 << 20];
                let chunk = reader.read(&mut buffer).map(|n| {
                    buffer.truncate(n);
                    buffer
                });
                let last = !matches!(&chunk, Ok(bytes) if !bytes.is_empty());
                if tx.send(chunk).is_err() || last {
                    break;
                }
            }
        })?;
    let mut file = File::create(part)?;
    let mut last_report = Instant::now();
    let received = receive(
        &rx,
        &mut file,
        DOWNLOAD_STALL,
        |received| {
            if last_report.elapsed() > Duration::from_millis(200) {
                *lock(&shared.clip_state) = ModelState::Downloading { received, total };
                shared.ctx.request_repaint();
                last_report = Instant::now();
            }
        },
        || lock(&shared.state).shutdown,
    )?;
    file.sync_all()?;
    Ok(received)
}

/// Writes the chunks from `rx` to `out` until the empty one that marks the end. Gives up after
/// `stall` without data, or as soon as `cancelled` says so. Returns the bytes written.
fn receive(
    rx: &mpsc::Receiver<io::Result<Vec<u8>>>,
    out: &mut impl Write,
    stall: Duration,
    mut progress: impl FnMut(u64),
    cancelled: impl Fn() -> bool,
) -> Result<u64> {
    let tick = stall.min(Duration::from_millis(500));
    let mut received = 0u64;
    let mut last_data = Instant::now();
    loop {
        if cancelled() {
            bail!("cancelled");
        }
        let chunk = match rx.recv_timeout(tick) {
            Ok(chunk) => chunk?,
            Err(RecvTimeoutError::Timeout) if last_data.elapsed() >= stall => {
                bail!("no data for {} s", stall.as_secs())
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => bail!("the download ended early"),
        };
        if chunk.is_empty() {
            return Ok(received);
        }
        out.write_all(&chunk)?;
        received += chunk.len() as u64;
        last_data = Instant::now();
        progress(received);
    }
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
            deferred: HashMap::new(),
            shutdown: false,
        };
        let now = Instant::now();
        let order: Vec<usize> =
            std::iter::from_fn(|| next_job(&mut state, now).map(|j| j.index)).collect();
        assert_eq!(order, [2, 3, 4, 0, 5]);
    }

    #[test]
    fn a_photo_being_written_comes_back_later_a_few_times() {
        let mut state = State {
            generation: 0,
            paths: Arc::new(vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg")]),
            current: 0,
            done: HashSet::new(),
            in_flight: HashSet::new(),
            deferred: HashMap::new(),
            shutdown: false,
        };
        let t0 = Instant::now();
        let job = next_job(&mut state, t0).unwrap();
        assert_eq!(job.index, 0);
        state.in_flight.remove(&0);
        finish(&mut state, 0, Outcome::Retry, t0);
        assert!(!state.done.contains(&0));
        // Not yet due: the neighbour goes first, then nothing until the delay has passed.
        assert_eq!(next_job(&mut state, t0).map(|j| j.index), Some(1));
        assert!(next_job(&mut state, t0).is_none());
        let again = next_job(&mut state, t0 + RETRY_DELAY).unwrap();
        assert_eq!(again.index, 0);
        state.in_flight.remove(&0);
        // A file that keeps changing is given up on after `MAX_RETRIES`.
        let mut at = t0;
        for _ in 1..MAX_RETRIES {
            finish(&mut state, 0, Outcome::Retry, at);
            at += RETRY_DELAY;
        }
        assert!(!state.done.contains(&0));
        finish(&mut state, 0, Outcome::Retry, at);
        assert!(state.done.contains(&0));
        assert!(state.deferred.is_empty());
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
            metadata_version: metadata::VERSION,
            ..ImageRecord::default()
        }
    }

    #[test]
    fn a_stalled_download_gives_up_and_a_finished_one_counts_its_bytes() {
        let (tx, rx) = mpsc::sync_channel(4);
        tx.send(Ok(vec![1, 2, 3])).unwrap();
        tx.send(Ok(vec![4])).unwrap();
        tx.send(Ok(Vec::new())).unwrap();
        let mut out = Vec::new();
        let mut seen = Vec::new();
        let received = receive(
            &rx,
            &mut out,
            Duration::from_secs(5),
            |n| seen.push(n),
            || false,
        )
        .unwrap();
        assert_eq!((received, out, seen), (4, vec![1, 2, 3, 4], vec![3, 4]));

        // The sender stays alive but sends nothing: a stalled connection.
        let (tx, rx) = mpsc::sync_channel(4);
        tx.send(Ok(vec![9])).unwrap();
        let started = Instant::now();
        let err = receive(
            &rx,
            &mut Vec::new(),
            Duration::from_millis(60),
            |_| {},
            || false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("no data"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(2));
        drop(tx);

        let (_tx, rx) = mpsc::sync_channel::<io::Result<Vec<u8>>>(1);
        let err = receive(
            &rx,
            &mut Vec::new(),
            Duration::from_secs(5),
            |_| {},
            || true,
        )
        .unwrap_err();
        assert!(err.to_string().contains("cancelled"));
    }

    #[test]
    fn completeness_checks_every_module() {
        let all = Capabilities {
            clip: true,
            v25: true,
            faces: true,
        };
        assert!(is_complete(&complete_record(), all));
        let missing: [fn(&mut ImageRecord); 8] = [
            |r| r.has_thumbnail = false,
            |r| r.scores.sharpness = None,
            |r| r.sharpness_version = 0,
            |r| r.exposure_version = 0,
            |r| r.faces_version = 0,
            |r| r.aesthetic_model = None,
            |r| r.aesthetic25_model = Some("old".into()),
            |r| r.metadata_version = 0,
        ];
        for (i, strip) in missing.iter().enumerate() {
            let mut record = complete_record();
            strip(&mut record);
            assert!(!is_complete(&record, all), "case {i}");
        }
        // Models that can't run are not required – the face detector included.
        let mut record = complete_record();
        record.aesthetic_model = None;
        record.aesthetic25_model = None;
        record.faces_version = 0;
        assert!(is_complete(&record, Capabilities::default()));
    }
}
