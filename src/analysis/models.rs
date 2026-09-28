//! The model files: downloading the CLIP model, removing the downloaded ones, and running a
//! model that is loaded on first use.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use eframe::egui;
use sha2::{Digest as _, Sha256};

use crate::paths;

use super::aesthetic::{self, V25Model};
use super::{Analyzer, Shared, lock};

/// A model download that gets no data for this long gives up (ureq has no idle timeout).
const DOWNLOAD_STALL: Duration = Duration::from_secs(60);

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

/// Streams the model to `<dest>.part`, checks its size and SHA-256, then renames it into
/// place. A failed or cancelled download leaves no `.part` behind.
fn download(shared: &Shared, dest: &Path) -> Result<()> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = dest.with_extension("onnx.part");
    let result = fetch(shared, &part).and_then(|(received, sha256)| verify(received, &sha256));
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
        return result;
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

/// The file is the one the pinned commit names: ONNX Runtime parses it next, so nothing
/// else may get through.
fn verify(received: u64, sha256: &str) -> Result<()> {
    if received != aesthetic::MODEL_BYTES {
        bail!(
            "unexpected size {received} bytes (expected {})",
            aesthetic::MODEL_BYTES
        );
    }
    if sha256 != aesthetic::MODEL_SHA256 {
        bail!(
            "unexpected SHA-256 {sha256} (expected {})",
            aesthetic::MODEL_SHA256
        );
    }
    Ok(())
}

/// Passes the bytes on and hashes them on the way, so the 1.2 GB are not read a second time.
struct Hashing<W> {
    out: W,
    sha256: Sha256,
}

impl<W> Hashing<W> {
    fn new(out: W) -> Self {
        Self {
            out,
            sha256: Sha256::new(),
        }
    }

    /// The writer back, and the SHA-256 in lower-case hex (as Hugging Face lists it).
    fn finish(self) -> (W, String) {
        let digest = self.sha256.finalize();
        let hex = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        (self.out, hex)
    }
}

impl<W: Write> Write for Hashing<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.out.write(buf)?;
        self.sha256.update(&buf[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

/// The bytes received and their SHA-256.
fn fetch(shared: &Shared, part: &Path) -> Result<(u64, String)> {
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
    let mut file = Hashing::new(File::create(part)?);
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
    let (file, sha256) = file.finish();
    file.sync_all()?;
    Ok((received, sha256))
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

impl Analyzer {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_download_is_hashed_on_the_way_and_checked() {
        let mut out = Hashing::new(Vec::new());
        out.write_all(b"ab").unwrap();
        out.write_all(b"c").unwrap();
        let (bytes, sha256) = out.finish();
        assert_eq!(bytes, b"abc");
        assert_eq!(
            sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        let size = aesthetic::MODEL_BYTES;
        assert!(verify(size, aesthetic::MODEL_SHA256).is_ok());
        let err = verify(size, &sha256).unwrap_err();
        assert!(err.to_string().contains("SHA-256"), "{err}");
        let err = verify(size - 1, aesthetic::MODEL_SHA256).unwrap_err();
        assert!(err.to_string().contains("size"), "{err}");
    }

    #[test]
    fn the_model_url_names_a_commit() {
        let url = aesthetic::MODEL_URL;
        assert!(!url.contains(char::is_whitespace), "{url}");
        assert!(
            url.contains("/resolve/c307790166907339eed5a9a53a249af534102536/onnx/"),
            "{url}"
        );
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
}
