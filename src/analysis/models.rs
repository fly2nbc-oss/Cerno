//! The model files: downloading what the manifest names, removing the downloaded ones, and
//! running a model that is loaded on first use.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use eframe::egui;

use crate::download::{download_file, part_of};
use crate::paths;

use super::aesthetic;
use super::manifest::Pack;
use super::{Analyzer, Shared, Status, lock};

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
    /// `Available` when the files are there, else `Missing`.
    pub(super) fn present(installed: bool) -> Self {
        if installed {
            Self::Available
        } else {
            Self::Missing
        }
    }

    pub(super) fn usable(&self) -> bool {
        matches!(self, Self::Available | Self::Loading | Self::Ready { .. })
    }

    /// Nothing to load: the file is not there or is about to go.
    fn absent(&self) -> bool {
        matches!(self, Self::Missing | Self::Removing)
    }
}

pub(super) enum Slot<M> {
    NotLoaded,
    Loaded(Box<M>),
    Unusable,
}

pub(super) fn clip_path() -> Option<PathBuf> {
    paths::models_dir()
        .ok()
        .map(|dir| dir.join(aesthetic::MODEL_FILE))
}

/// Every file of `pack` is in `dir`.
pub(super) fn installed(dir: &Path, pack: Pack) -> bool {
    pack.files()
        .iter()
        .all(|file| dir.join(file.name).is_file())
}

fn state_of(shared: &Shared, pack: Pack) -> &Mutex<ModelState> {
    match pack {
        Pack::Clip => &shared.clip_state,
        Pack::V25 => &shared.v25_state,
    }
}

/// Holds both model slots – waiting for an inference that runs now – so no worker has a
/// session open on the files while they are deleted. Workers take one slot at a time, so
/// taking both here cannot deadlock.
fn remove_models(shared: &Shared) -> Result<()> {
    let mut clip = lock(&shared.clip);
    let mut v25 = lock(&shared.v25);
    *clip = Slot::NotLoaded;
    *v25 = Slot::NotLoaded;
    let dir = paths::models_dir();
    let result = dir.as_deref().map_or(Ok(()), remove_files);
    // What is still there (a file that could not be deleted) stays usable.
    for pack in Pack::ALL {
        *lock(state_of(shared, pack)) =
            ModelState::present(dir.as_deref().is_ok_and(|dir| installed(dir, pack)));
    }
    result
}

/// Deletes every model file and every unfinished download in `dir`. A file that can't be
/// deleted is reported; the others still go.
fn remove_files(dir: &Path) -> Result<()> {
    let mut result = Ok(());
    for file in Pack::ALL.iter().flat_map(|pack| pack.files()) {
        for path in [dir.join(file.name), part_of(dir, file)] {
            if path.is_file()
                && let Err(err) = std::fs::remove_file(&path)
            {
                result = Err(anyhow::Error::from(err))
                    .with_context(|| format!("cannot delete {}", path.display()));
            }
        }
    }
    result
}

/// Loads a model on first use (one instance shared by all workers), reports its state and
/// runs it. `None` if it can't be loaded or the run fails.
pub(super) fn run_model<M, R>(
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

/// Fetches what is missing of `pack` into `dir`, file after file. `progress` gets the bytes of
/// the pack done so far.
fn download_pack(
    dir: &Path,
    pack: Pack,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    let mut before = 0;
    for file in pack.files() {
        download_file(
            dir,
            file,
            &mut |received| progress(before + received),
            cancelled,
        )?;
        before += file.bytes;
    }
    Ok(())
}

impl Analyzer {
    /// Removes the downloaded ONNX files, in the background. The states say "removing" at
    /// once, so no analysis starts a model; the files go once a running inference has
    /// finished. The result comes back through [`Self::take_removal`].
    pub fn delete_installed_models(&self) {
        // Not under a running download: it would write into a folder that is being emptied.
        if self.status().busy() {
            return;
        }
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

    /// Downloads every model that is missing, one after the other, in the background. The
    /// result comes back through [`Self::take_download`].
    pub fn download_missing(&self) {
        let shared = &self.shared;
        let status = self.status();
        let packs = status.missing();
        if packs.is_empty() {
            return;
        }
        for &pack in &packs {
            *lock(state_of(shared, pack)) = ModelState::Downloading {
                received: 0,
                total: pack.bytes(),
            };
        }
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name("cerno-model-download".into())
            .spawn(move || {
                let dir = paths::models_dir();
                let mut result = Ok(());
                for pack in packs {
                    let state = state_of(&shared, pack);
                    if result.is_err() {
                        // An earlier model failed: this one waits for the next attempt.
                        *lock(state) = ModelState::Missing;
                        continue;
                    }
                    let total = pack.bytes();
                    let mut last_report = Instant::now();
                    let mut progress = |received: u64| {
                        if last_report.elapsed() > Duration::from_millis(200) {
                            *lock(state) = ModelState::Downloading { received, total };
                            shared.ctx.request_repaint();
                            last_report = Instant::now();
                        }
                    };
                    let cancelled = || lock(&shared.state).shutdown;
                    let outcome = match &dir {
                        Ok(dir) => download_pack(dir, pack, &mut progress, &cancelled),
                        Err(err) => Err(anyhow::anyhow!("no data directory: {err:#}")),
                    };
                    match outcome {
                        Ok(()) => {
                            match pack {
                                Pack::Clip => *lock(&shared.clip) = Slot::NotLoaded,
                                Pack::V25 => *lock(&shared.v25) = Slot::NotLoaded,
                            }
                            *lock(state) = ModelState::Available;
                            // Revisit every image so the new scores get added.
                            lock(&shared.state).done.clear();
                            shared.wake.notify_all();
                        }
                        Err(err) => {
                            log::error!("model download failed: {err:#}");
                            // The button offers it again; a `.part` continues from there.
                            *lock(state) = ModelState::Missing;
                            result = Err(format!("{err:#}"));
                        }
                    }
                    shared.ctx.request_repaint();
                }
                *lock(&shared.download) = Some(result);
                shared.ctx.request_repaint();
            })
            .expect("failed to spawn download thread");
    }

    /// The outcome of the last download, once.
    pub fn take_download(&self) -> Option<Result<(), String>> {
        lock(&self.shared.download).take()
    }
}

impl Status {
    fn state_of(&self, pack: Pack) -> &ModelState {
        match pack {
            Pack::Clip => &self.aesthetics,
            Pack::V25 => &self.v25,
        }
    }

    /// The models that can be downloaded now (none while a download or removal runs).
    pub fn missing(&self) -> Vec<Pack> {
        if self.busy() {
            return Vec::new();
        }
        Pack::ALL
            .into_iter()
            .filter(|&pack| *self.state_of(pack) == ModelState::Missing)
            .collect()
    }

    /// Received and total bytes of the model downloading now. The models come one after the
    /// other in `Pack::ALL` order, so the first one still downloading is the one running; the
    /// percentage then starts again for the next model ("Downloading model 57 %").
    pub fn downloading(&self) -> Option<(u64, u64)> {
        Pack::ALL
            .into_iter()
            .find_map(|pack| match self.state_of(pack) {
                ModelState::Downloading { received, total } => Some((*received, *total)),
                _ => None,
            })
    }

    /// A download or "Delete models" is running.
    pub fn busy(&self) -> bool {
        Pack::ALL.into_iter().any(|pack| {
            matches!(
                self.state_of(pack),
                ModelState::Downloading { .. } | ModelState::Removing
            )
        })
    }

    /// The size of the models on disk, for "Delete models".
    pub fn installed_bytes(&self) -> u64 {
        Pack::ALL
            .into_iter()
            .filter(|&pack| *self.state_of(pack) != ModelState::Missing)
            .map(Pack::bytes)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::TasteStatus;
    use crate::analysis::manifest::ModelFile;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cerno-models-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// "Delete models" takes every model file and every unfinished download, nothing else.
    #[test]
    fn removing_takes_the_models_and_their_parts() {
        let dir = temp_dir("remove");
        let files: Vec<&ModelFile> = Pack::ALL.iter().flat_map(|p| p.files()).collect();
        for file in &files {
            std::fs::write(dir.join(file.name), b"x").unwrap();
            std::fs::write(part_of(&dir, file), b"x").unwrap();
        }
        std::fs::write(dir.join("notes.txt"), b"mine").unwrap();
        assert!(Pack::ALL.iter().all(|&pack| installed(&dir, pack)));
        remove_files(&dir).unwrap();
        for file in &files {
            assert!(!dir.join(file.name).exists() && !part_of(&dir, file).exists());
        }
        assert!(dir.join("notes.txt").exists());
        assert!(!installed(&dir, Pack::V25));
        // The head's part is named after the head, not after an ONNX file.
        assert!(
            part_of(&dir, &Pack::V25.files()[0])
                .ends_with("aesthetic-predictor-v2.5-head.bin.part")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn status(aesthetics: ModelState, v25: ModelState) -> Status {
        Status {
            done: 0,
            total: 0,
            aesthetics,
            v25,
            faces: ModelState::Available,
            taste: TasteStatus {
                examples: 0,
                sources: Default::default(),
                model: None,
            },
        }
    }

    #[test]
    fn the_status_says_what_can_be_downloaded() {
        let none = status(ModelState::Missing, ModelState::Missing);
        assert_eq!(none.missing(), vec![Pack::Clip, Pack::V25]);
        assert_eq!(none.installed_bytes(), 0);
        let v25 = status(ModelState::Ready { backend: "CPU" }, ModelState::Missing);
        assert_eq!(v25.missing(), vec![Pack::V25]);
        assert_eq!(v25.installed_bytes(), Pack::Clip.bytes());
        // While CLIP downloads, V2.5 waits for its turn: nothing more to ask for.
        let running = status(
            ModelState::Downloading {
                received: 10,
                total: 100,
            },
            ModelState::Downloading {
                received: 0,
                total: 50,
            },
        );
        assert!(running.busy() && running.missing().is_empty());
        assert_eq!(running.downloading(), Some((10, 100)));
        let removing = status(ModelState::Removing, ModelState::Missing);
        assert!(removing.busy() && removing.missing().is_empty());
        assert_eq!(removing.downloading(), None);
    }

    /// Against the real hosts – run after the `models-1` release exists:
    /// `CERNO_TEST_DOWNLOAD=1 cargo test --features heic -- --ignored the_published_files`
    /// fetches the V2.5 head and asks every file for its last byte (the range survives
    /// GitHub's redirect); `=full` downloads both models into a temp folder, breaks each off
    /// after 50 MB and continues it, and checks size and SHA-256. The live models folder is
    /// never touched.
    #[test]
    #[ignore = "network; set CERNO_TEST_DOWNLOAD"]
    fn the_published_files_download() {
        let Ok(mode) = std::env::var("CERNO_TEST_DOWNLOAD") else {
            return;
        };
        let dir = temp_dir("download");
        for file in Pack::ALL.iter().flat_map(|pack| pack.files()) {
            let from = file.bytes - 1;
            let response = crate::download::open(file, from).unwrap();
            let range = response
                .headers()
                .get("content-range")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            assert_eq!(response.status().as_u16(), 206, "{}", file.url);
            assert_eq!(
                range.as_deref(),
                Some(format!("bytes {from}-{from}/{}", file.bytes).as_str()),
                "{}",
                file.url
            );
        }
        let head = &Pack::V25.files()[0];
        download_file(&dir, head, &mut |_| {}, &|| false).unwrap();
        assert!(dir.join(head.name).is_file());
        if mode == "full" {
            for pack in Pack::ALL {
                let received = std::cell::Cell::new(0);
                let broken = download_pack(&dir, pack, &mut |n| received.set(n), &|| {
                    received.get() > 50_000_000
                });
                assert!(broken.is_err(), "{pack:?} broke off");
                download_pack(&dir, pack, &mut |_| {}, &|| false).unwrap();
                assert!(installed(&dir, pack), "{pack:?}");
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
