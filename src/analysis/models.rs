//! The model files: downloading what the manifest names, removing the downloaded ones, and
//! running a model that is loaded on first use.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use eframe::egui;
use sha2::{Digest as _, Sha256};

use crate::paths;

use super::aesthetic;
use super::manifest::{ModelFile, Pack};
use super::{Analyzer, Shared, Status, lock};

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

/// Where an unfinished download of `file` waits to be continued.
fn part_of(dir: &Path, file: &ModelFile) -> PathBuf {
    dir.join(format!("{}.part", file.name))
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

/// Fetches `file` into `dir` unless it is there. A `.part` left by an earlier attempt is
/// continued. Only bytes of the manifest's size and SHA-256 are renamed into place; wrong
/// bytes are deleted, an interrupted download keeps its `.part` for the next attempt.
fn download_file(
    dir: &Path,
    file: &ModelFile,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    let dest = dir.join(file.name);
    if dest.is_file() {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    let part = part_of(dir, file);
    let (received, sha256) = fetch(file, &part, progress, cancelled)?;
    if let Err(err) = verify(file, received, &sha256) {
        let _ = std::fs::remove_file(&part);
        return Err(err);
    }
    std::fs::rename(&part, &dest)?;
    Ok(())
}

/// The file is the one the manifest names: ONNX Runtime parses it next, so nothing else may
/// get through.
fn verify(file: &ModelFile, received: u64, sha256: &str) -> Result<()> {
    if received != file.bytes {
        bail!(
            "{}: unexpected size {received} bytes (expected {})",
            file.name,
            file.bytes
        );
    }
    if sha256 != file.sha256 {
        bail!(
            "{}: unexpected SHA-256 {sha256} (expected {})",
            file.name,
            file.sha256
        );
    }
    Ok(())
}

/// What to do with the bytes an earlier attempt left in the `.part`.
#[derive(Debug, PartialEq, Eq)]
enum Resume {
    /// The server sends exactly the rest.
    Append,
    /// The server sends the whole file.
    Restart,
}

/// An earlier attempt's bytes are kept only when the server answers the range request with
/// the rest (206 starting where the `.part` ends); a 200 is the whole file again. A 206 for
/// another range fits neither.
fn resume_plan(have: u64, status: u16, content_range: Option<&str>) -> Result<Resume> {
    if status != 206 {
        return Ok(Resume::Restart);
    }
    let start = content_range
        .and_then(|range| range.strip_prefix("bytes "))
        .and_then(|range| range.split('-').next())
        .and_then(|start| start.trim().parse::<u64>().ok());
    if have > 0 && start == Some(have) {
        Ok(Resume::Append)
    } else {
        bail!("the server sent another part ({content_range:?}, asked from byte {have})")
    }
}

/// Passes the bytes on and hashes them on the way, so the 1.7 GB are not read a second time.
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

    /// Feeds bytes that are in the output already (an earlier attempt's) into the hash only.
    fn absorb(&mut self, mut reader: impl Read) -> io::Result<u64> {
        let mut buffer = vec![0u8; 1 << 20];
        let mut total = 0;
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                return Ok(total);
            }
            self.sha256.update(&buffer[..n]);
            total += n as u64;
        }
    }

    /// The writer back, and the SHA-256 in lower-case hex (as the manifest lists it).
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

/// Asks for `url` from byte `from` on (the whole file for 0). Redirects keep the `Range`
/// header (ureq drops only credentials and cookies), so GitHub's hop to its storage host
/// still asks for the rest.
///
/// Certificates are checked against the system's store, like a browser does: an antivirus
/// that scans HTTPS (Kaspersky did for huggingface.co) or a company proxy presents its own
/// root, which Mozilla's built-in list does not know. The SHA-256 check guards the bytes.
fn request(url: &str, from: u64) -> Result<ureq::http::Response<ureq::Body>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into();
    let mut request = agent.get(url);
    if from > 0 {
        request = request.header("Range", &format!("bytes={from}-"));
    }
    Ok(request.call()?)
}

/// The manifest's URL, else its mirror once.
fn open(file: &ModelFile, from: u64) -> Result<ureq::http::Response<ureq::Body>> {
    request(file.url, from).or_else(|err| match file.mirror {
        Some(mirror) => {
            log::warn!(
                "model download from {}: {err:#} – trying the mirror",
                file.url
            );
            request(mirror, from)
        }
        None => Err(err),
    })
}

/// The bytes the `.part` holds in the end and their SHA-256.
fn fetch(
    file: &ModelFile,
    part: &Path,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<(u64, String)> {
    let mut have = std::fs::metadata(part).map_or(0, |meta| meta.len());
    if have > file.bytes {
        std::fs::remove_file(part)?;
        have = 0;
    }
    if have == file.bytes && have > 0 {
        // Complete already: an earlier attempt ended before its check.
        let mut hashing = Hashing::new(io::sink());
        hashing.absorb(File::open(part)?)?;
        progress(have);
        return Ok((have, hashing.finish().1));
    }
    let response = match open(file, have) {
        // The `.part` does not fit what the server has: start over.
        Err(err)
            if have > 0
                && matches!(
                    err.downcast_ref::<ureq::Error>(),
                    Some(ureq::Error::StatusCode(416))
                ) =>
        {
            have = 0;
            open(file, 0)?
        }
        other => other?,
    };
    let content_range = response
        .headers()
        .get("content-range")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let resume = resume_plan(have, response.status().as_u16(), content_range.as_deref())
        .inspect_err(|_| {
            let _ = std::fs::remove_file(part);
        })?;
    let mut out = match resume {
        Resume::Append => {
            let mut out = Hashing::new(OpenOptions::new().append(true).open(part)?);
            // The bytes from before go into the hash first, read once more from disk.
            out.absorb(File::open(part)?.take(have))?;
            log::info!("model download: {} continues at byte {have}", file.name);
            out
        }
        Resume::Restart => {
            have = 0;
            Hashing::new(File::create(part)?)
        }
    };
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
    let received = receive(
        &rx,
        &mut out,
        DOWNLOAD_STALL,
        |received| progress(have + received),
        cancelled,
    );
    // What arrived stays on disk for the next attempt, even when the connection broke.
    let (part_file, sha256) = out.finish();
    part_file.sync_all()?;
    Ok((have + received?, sha256))
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

    /// "abc" and its SHA-256.
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

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

    /// A file no test ever asks a server for: its `.part` is complete already.
    fn local(name: &'static str, sha256: &'static str) -> ModelFile {
        ModelFile {
            name,
            url: "http://127.0.0.1:9/never",
            mirror: None,
            bytes: 3,
            sha256,
        }
    }

    #[test]
    fn the_download_is_hashed_on_the_way_and_checked() {
        let mut out = Hashing::new(Vec::new());
        out.write_all(b"ab").unwrap();
        out.write_all(b"c").unwrap();
        let (bytes, sha256) = out.finish();
        assert_eq!(bytes, b"abc");
        assert_eq!(sha256, ABC);

        let clip = &Pack::Clip.files()[0];
        assert!(verify(clip, clip.bytes, clip.sha256).is_ok());
        let err = verify(clip, clip.bytes, &sha256).unwrap_err();
        assert!(err.to_string().contains("SHA-256"), "{err}");
        let err = verify(clip, clip.bytes - 1, clip.sha256).unwrap_err();
        assert!(err.to_string().contains("size"), "{err}");
    }

    /// A continued download hashes the bytes from before first, then the rest: the same hash
    /// as one piece.
    #[test]
    fn a_continued_download_hashes_like_one_piece() {
        let mut out = Hashing::new(b"ab".to_vec());
        assert_eq!(out.absorb(&b"ab"[..]).unwrap(), 2);
        out.write_all(b"c").unwrap();
        let (bytes, sha256) = out.finish();
        assert_eq!((bytes.as_slice(), sha256.as_str()), (&b"abc"[..], ABC));
    }

    #[test]
    fn only_the_rest_continues_a_part() {
        let rest = Some("bytes 100-199/200");
        assert_eq!(resume_plan(100, 206, rest).unwrap(), Resume::Append);
        // The whole file again (the server ignores ranges, or there was nothing yet).
        assert_eq!(resume_plan(100, 200, None).unwrap(), Resume::Restart);
        assert_eq!(resume_plan(0, 200, None).unwrap(), Resume::Restart);
        // Another part than the one asked for fits nothing.
        assert!(resume_plan(50, 206, rest).is_err());
        assert!(resume_plan(100, 206, None).is_err());
        assert!(resume_plan(0, 206, Some("bytes 0-9/200")).is_err());
    }

    /// A `.part` an earlier attempt finished is checked and renamed without asking a server;
    /// one with wrong bytes is deleted.
    #[test]
    fn a_complete_part_is_checked_and_wrong_bytes_go() {
        let dir = temp_dir("part");
        let good = local("good.bin", ABC);
        std::fs::write(part_of(&dir, &good), b"abc").unwrap();
        let mut seen = 0;
        download_file(&dir, &good, &mut |n| seen = n, &|| false).unwrap();
        assert_eq!(std::fs::read(dir.join("good.bin")).unwrap(), b"abc");
        assert!(!part_of(&dir, &good).exists());
        assert_eq!(seen, 3);
        // There already: nothing to do.
        download_file(&dir, &good, &mut |_| {}, &|| false).unwrap();

        let bad = local("bad.bin", ABC);
        std::fs::write(part_of(&dir, &bad), b"abd").unwrap();
        let err = download_file(&dir, &bad, &mut |_| {}, &|| false).unwrap_err();
        assert!(err.to_string().contains("SHA-256"), "{err}");
        assert!(!part_of(&dir, &bad).exists() && !dir.join("bad.bin").exists());
        std::fs::remove_dir_all(&dir).unwrap();
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
            let response = open(file, from).unwrap();
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
