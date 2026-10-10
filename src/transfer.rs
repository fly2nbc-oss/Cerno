//! Copy or move the photos the current filter shows into another folder.
//!
//! The work runs on a background thread. A move prefers `rename` and falls back to copy plus
//! delete when the destination is on another volume. An existing file of the same name is
//! left alone. Each file is held in [`FileLocks`] while it is copied or moved, so a rating
//! written meanwhile never meets a half-copied file. [`Progress`] tells the UI how far it is.
//! The RAW of a RAW + JPG pair rides along with its JPEG (`pairs`): both go, or neither.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;

use crate::filelock::FileLocks;
use crate::filetimes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Copy,
    Move,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub mode: Mode,
    pub done: Vec<(PathBuf, PathBuf)>,
    pub skipped: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
    /// The RAWs that went along with a JPEG of `done` – not counted as photos of their own.
    pub riders: Vec<(PathBuf, PathBuf)>,
}

/// How far a copy or move is. The worker counts, the UI reads a [`Snapshot`]. Files count
/// when they are done – also skipped and failed ones, so the bar reaches the end.
#[derive(Default)]
pub struct Progress {
    files_total: AtomicUsize,
    files_done: AtomicUsize,
    bytes_total: AtomicU64,
    bytes_done: AtomicU64,
    /// The file being copied or moved right now, and its size.
    current: Mutex<Option<(PathBuf, u64)>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub files_total: usize,
    pub files_done: usize,
    pub bytes_total: u64,
    pub bytes_done: u64,
    pub current: Option<(PathBuf, u64)>,
}

impl Snapshot {
    /// 0..=1: by bytes once their total is known, else by files.
    pub fn fraction(&self) -> f32 {
        let (done, total) = if self.bytes_total > 0 {
            (self.bytes_done as f64, self.bytes_total as f64)
        } else {
            (self.files_done as f64, self.files_total as f64)
        };
        if total > 0.0 {
            (done / total).clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    }
}

impl Progress {
    fn new(files_total: usize) -> Self {
        let progress = Self::default();
        progress.files_total.store(files_total, Ordering::Relaxed);
        progress
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            files_total: self.files_total.load(Ordering::Relaxed),
            files_done: self.files_done.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
            bytes_done: self.bytes_done.load(Ordering::Relaxed),
            current: self
                .current
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
        }
    }

    fn start(&self, files: usize, bytes: u64) {
        self.files_total.store(files, Ordering::Relaxed);
        self.bytes_total.store(bytes, Ordering::Relaxed);
    }

    fn begin(&self, path: &Path, size: u64) {
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = Some((path.to_owned(), size));
    }

    fn finish(&self, size: u64) {
        self.bytes_done.fetch_add(size, Ordering::Relaxed);
        self.files_done.fetch_add(1, Ordering::Relaxed);
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

struct Job {
    mode: Mode,
    sources: Vec<PathBuf>,
    /// The RAW riding along with a source JPEG.
    riders: HashMap<PathBuf, PathBuf>,
    dest: PathBuf,
}

pub struct Queue {
    files: Arc<FileLocks>,
    pending: Option<Job>,
    inflight: bool,
    /// The photos of the job that is waiting or running, until its outcome is collected.
    involved: Option<(Mode, HashSet<PathBuf>)>,
    /// How far the job that is waiting or running is.
    progress: Arc<Progress>,
    tx: mpsc::Sender<Outcome>,
    rx: mpsc::Receiver<Outcome>,
    workers: Vec<JoinHandle<()>>,
}

impl Queue {
    pub fn new(files: Arc<FileLocks>) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            files,
            pending: None,
            inflight: false,
            involved: None,
            progress: Arc::default(),
            tx,
            rx,
            workers: Vec::new(),
        }
    }

    pub fn is_busy(&self) -> bool {
        self.pending.is_some() || self.inflight
    }

    /// Whether `path` belongs to the job that is waiting or running, and how.
    pub fn involves(&self, path: &Path) -> Option<Mode> {
        let (mode, paths) = self.involved.as_ref()?;
        paths.contains(path).then_some(*mode)
    }

    /// The mode of the job that is waiting or running.
    pub fn mode(&self) -> Option<Mode> {
        self.involved.as_ref().map(|(mode, _)| *mode)
    }

    /// The job that is waiting or running and how far it is.
    pub fn progress(&self) -> Option<(Mode, Snapshot)> {
        self.mode().map(|mode| (mode, self.progress.snapshot()))
    }

    /// Remembers the job until [`Self::kick`] sees that rating writes have finished.
    pub fn push(
        &mut self,
        mode: Mode,
        sources: Vec<PathBuf>,
        riders: HashMap<PathBuf, PathBuf>,
        dest: PathBuf,
    ) -> bool {
        if self.is_busy() {
            return false;
        }
        self.progress = Arc::new(Progress::new(sources.len()));
        let involved = sources.iter().chain(riders.values()).cloned().collect();
        self.involved = Some((mode, involved));
        self.pending = Some(Job {
            mode,
            sources,
            riders,
            dest,
        });
        true
    }

    /// Starts the worker once `writes_pending` is false. Returns whether it is still waiting
    /// for those writes. `on_done` runs on the worker when the batch finishes.
    pub fn kick(&mut self, writes_pending: bool, on_done: impl FnOnce() + Send + 'static) -> bool {
        if writes_pending || self.pending.is_none() {
            return self.pending.is_some();
        }
        let Some(job) = self.pending.take() else {
            return false;
        };
        self.inflight = true;
        let (tx, files) = (self.tx.clone(), Arc::clone(&self.files));
        let progress = Arc::clone(&self.progress);
        self.workers.push(
            std::thread::Builder::new()
                .name("cerno-transfer".into())
                .spawn(move || {
                    let outcome = run(
                        job.mode,
                        &job.sources,
                        &job.riders,
                        &job.dest,
                        &files,
                        &progress,
                    );
                    let _ = tx.send(outcome);
                    on_done();
                })
                .unwrap_or_else(crate::process::no_thread),
        );
        false
    }

    pub fn poll(&mut self) -> Option<Outcome> {
        match self.rx.try_recv() {
            Ok(outcome) => {
                self.inflight = false;
                self.involved = None;
                self.workers.retain(|worker| !worker.is_finished());
                Some(outcome)
            }
            Err(_) => None,
        }
    }

    /// Finishes a job that was still waiting, then waits for a running worker. Used on exit,
    /// after rating writes have been flushed.
    pub fn finish_now(&mut self) -> Vec<Outcome> {
        let mut outcomes = Vec::new();
        if let Some(job) = self.pending.take() {
            outcomes.push(run(
                job.mode,
                &job.sources,
                &job.riders,
                &job.dest,
                &self.files,
                &self.progress,
            ));
        }
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        while let Ok(outcome) = self.rx.try_recv() {
            self.inflight = false;
            outcomes.push(outcome);
        }
        self.involved = None;
        outcomes
    }
}

pub fn run(
    mode: Mode,
    sources: &[PathBuf],
    riders: &HashMap<PathBuf, PathBuf>,
    dest_dir: &Path,
    files: &FileLocks,
    progress: &Progress,
) -> Outcome {
    let mut outcome = Outcome {
        mode,
        done: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
        riders: Vec::new(),
    };
    let size_of = |path: &Path| fs::metadata(path).map_or(0, |meta| meta.len());
    // A pair counts as one photo with the bytes of both.
    let sizes: Vec<u64> = sources
        .iter()
        .map(|src| size_of(src) + riders.get(src).map_or(0, |raw| size_of(raw)))
        .collect();
    progress.start(sources.len(), sizes.iter().sum());
    for (src, &size) in sources.iter().zip(&sizes) {
        progress.begin(src, size);
        let rider = riders.get(src);
        // No half pairs: a RAW whose name is taken there keeps its JPEG here too.
        let rider_blocked = rider
            .and_then(|raw| raw.file_name())
            .is_some_and(|name| dest_dir.join(name).exists());
        if rider_blocked {
            outcome.skipped.push(src.to_owned());
        } else if transfer_one(mode, src, dest_dir, files, &mut outcome).is_some()
            && let Some(raw) = rider
        {
            let mut along = Outcome {
                mode,
                done: Vec::new(),
                skipped: Vec::new(),
                failed: Vec::new(),
                riders: Vec::new(),
            };
            transfer_one(mode, raw, dest_dir, files, &mut along);
            outcome.riders.extend(along.done);
            outcome.failed.extend(along.failed);
        }
        progress.finish(size);
    }
    outcome
}

/// Copies or moves one file; where it went, when it did.
fn transfer_one(
    mode: Mode,
    src: &Path,
    dest_dir: &Path,
    files: &FileLocks,
    outcome: &mut Outcome,
) -> Option<PathBuf> {
    let Some(name) = src.file_name() else {
        outcome
            .failed
            .push((src.to_owned(), "missing file name".to_owned()));
        return None;
    };
    let dest = dest_dir.join(name);
    if dest.exists() {
        outcome.skipped.push(src.to_owned());
        return None;
    }
    // A move makes the file disappear, which counts as a write for everyone else.
    let held = match mode {
        Mode::Copy => files.hold(src),
        Mode::Move => files.hold_write(src),
    };
    let result = apply(mode, src, &dest);
    drop(held);
    match result {
        Ok(()) => {
            log::info!("{} {} → {}", verb(mode), src.display(), dest.display());
            carry_sidecar(mode, src, &dest);
            outcome.done.push((src.to_owned(), dest.clone()));
            Some(dest)
        }
        Err(err) => {
            log::warn!("could not {} {}: {err}", verb(mode), src.display());
            outcome.failed.push((src.to_owned(), err));
            None
        }
    }
}

/// A RAW's or video's marks live in its XMP sidecar: it goes (or is copied) along – under the
/// name the destination's folder asks for, so it never lands on a RAW's of the same name
/// there. One already at the destination is left as it is.
fn carry_sidecar(mode: Mode, src: &Path, dest: &Path) {
    if !crate::sidecar::applies(src) {
        return;
    }
    let from = crate::sidecar::path_of(src);
    if !from.is_file() {
        return;
    }
    let to = crate::sidecar::moved(src, &from, dest);
    if to.exists() {
        log::warn!("sidecar {} exists already – kept", to.display());
        return;
    }
    if let Err(err) = apply(mode, &from, &to) {
        log::warn!("could not {} {}: {err}", verb(mode), from.display());
    }
}

fn verb(mode: Mode) -> &'static str {
    match mode {
        Mode::Copy => "copy",
        Mode::Move => "move",
    }
}

fn apply(mode: Mode, src: &Path, dest: &Path) -> Result<(), String> {
    match mode {
        Mode::Copy => copy_keeping_times(src, dest),
        Mode::Move => move_file(src, dest),
    }
}

fn copy_keeping_times(src: &Path, dest: &Path) -> Result<(), String> {
    let times = filetimes::Snapshot::capture(src).map_err(|err| err.to_string())?;
    fs::copy(src, dest).map_err(|err| err.to_string())?;
    times.restore(dest).map_err(|err| err.to_string())?;
    Ok(())
}

fn move_file(src: &Path, dest: &Path) -> Result<(), String> {
    match fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(err) if different_volume(&err) => {
            copy_keeping_times(src, dest)?;
            // On the disk before the only other copy goes (a pulled card, a power cut). Write
            // access, not a write: Windows flushes only a handle that may write, and the dates
            // stay.
            fs::OpenOptions::new()
                .write(true)
                .open(dest)
                .and_then(|file| file.sync_all())
                .map_err(|err| err.to_string())?;
            fs::remove_file(src).map_err(|err| err.to_string())
        }
        Err(err) => Err(err.to_string()),
    }
}

/// Unix `EXDEV` is 18, Windows `ERROR_NOT_SAME_DEVICE` is 17.
fn different_volume(err: &io::Error) -> bool {
    matches!(err.raw_os_error(), Some(17) | Some(18))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{Duration, SystemTime};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cerno-transfer-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_old(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        let old = SystemTime::now() - Duration::from_secs(86_400 * 30);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(old))
            .unwrap();
        path
    }

    #[test]
    fn copy_keeps_the_original_and_its_modified_time() {
        let src_dir = temp_dir("copy-src");
        let dest_dir = temp_dir("copy-dest");
        let src = write_old(&src_dir, "a.jpg", b"photo");
        let modified = fs::metadata(&src).unwrap().modified().unwrap();

        let outcome = run(
            Mode::Copy,
            std::slice::from_ref(&src),
            &HashMap::new(),
            &dest_dir,
            &FileLocks::default(),
            &Progress::default(),
        );
        assert_eq!(outcome.done.len(), 1);
        assert!(outcome.skipped.is_empty() && outcome.failed.is_empty());
        assert_eq!(fs::read(&src).unwrap(), b"photo");
        let dest = &outcome.done[0].1;
        assert_eq!(fs::read(dest).unwrap(), b"photo");
        assert_eq!(fs::metadata(dest).unwrap().modified().unwrap(), modified);

        fs::remove_dir_all(&src_dir).unwrap();
        fs::remove_dir_all(&dest_dir).unwrap();
    }

    #[test]
    fn move_removes_the_source() {
        let src_dir = temp_dir("move-src");
        let dest_dir = temp_dir("move-dest");
        let src = write_old(&src_dir, "b.jpg", b"moved");

        let outcome = run(
            Mode::Move,
            std::slice::from_ref(&src),
            &HashMap::new(),
            &dest_dir,
            &FileLocks::default(),
            &Progress::default(),
        );
        assert_eq!(outcome.done.len(), 1);
        assert!(!src.exists());
        assert_eq!(fs::read(&outcome.done[0].1).unwrap(), b"moved");

        fs::remove_dir_all(&src_dir).unwrap();
        fs::remove_dir_all(&dest_dir).unwrap();
    }

    /// A RAW + JPG pair moves as one photo, the RAW's sidecar with it; when the RAW's name is
    /// taken at the destination, neither moves.
    #[test]
    fn a_pair_moves_whole_or_not_at_all() {
        let src_dir = temp_dir("pair-src");
        let dest_dir = temp_dir("pair-dest");
        let jpeg = write_old(&src_dir, "IMG_1.JPG", b"jpeg");
        let raw = write_old(&src_dir, "IMG_1.CR3", b"raw");
        write_old(&src_dir, "IMG_1.xmp", b"marks");
        let blocked_jpeg = write_old(&src_dir, "IMG_2.JPG", b"jpeg");
        let blocked_raw = write_old(&src_dir, "IMG_2.CR3", b"raw");
        write_old(&dest_dir, "IMG_2.CR3", b"someone else's");
        let riders = HashMap::from([
            (jpeg.clone(), raw.clone()),
            (blocked_jpeg.clone(), blocked_raw.clone()),
        ]);

        let outcome = run(
            Mode::Move,
            &[jpeg, blocked_jpeg.clone()],
            &riders,
            &dest_dir,
            &FileLocks::default(),
            &Progress::default(),
        );
        assert_eq!(outcome.done.len(), 1, "a pair is one photo");
        assert_eq!(outcome.riders.len(), 1);
        assert!(dest_dir.join("IMG_1.CR3").exists() && dest_dir.join("IMG_1.xmp").exists());
        assert!(!raw.exists());
        assert_eq!(outcome.skipped, std::slice::from_ref(&blocked_jpeg));
        assert!(
            blocked_jpeg.exists() && blocked_raw.exists(),
            "neither moved"
        );

        fs::remove_dir_all(&src_dir).unwrap();
        fs::remove_dir_all(&dest_dir).unwrap();
    }

    /// A RAW's marks are in its sidecar: moving it takes `IMG_7.xmp` along; a JPEG's name twin
    /// is not its sidecar and stays.
    #[test]
    fn a_raw_takes_its_sidecar_along() {
        let src_dir = temp_dir("side-src");
        let dest_dir = temp_dir("side-dest");
        let raw = write_old(&src_dir, "IMG_7.CR2", b"raw");
        write_old(&src_dir, "IMG_7.xmp", b"<x:xmpmeta xmp:Rating='4'/>");
        let jpeg = write_old(&src_dir, "IMG_8.JPG", b"jpeg");
        write_old(&src_dir, "IMG_8.xmp", b"someone else's");

        let outcome = run(
            Mode::Move,
            &[raw, jpeg],
            &HashMap::new(),
            &dest_dir,
            &FileLocks::default(),
            &Progress::default(),
        );
        assert_eq!(outcome.done.len(), 2);
        assert_eq!(
            fs::read(dest_dir.join("IMG_7.xmp")).unwrap(),
            b"<x:xmpmeta xmp:Rating='4'/>"
        );
        assert!(!src_dir.join("IMG_7.xmp").exists());
        assert!(src_dir.join("IMG_8.xmp").exists(), "not the JPEG's");
        assert!(!dest_dir.join("IMG_8.xmp").exists());

        fs::remove_dir_all(&src_dir).unwrap();
        fs::remove_dir_all(&dest_dir).unwrap();
    }

    /// A video alone has `IMG_1.xmp`; where it lands beside a RAW of its name, that name is the
    /// RAW's – the video's marks become `IMG_1.MOV.xmp` and the RAW's stay as they were. A long
    /// name moves as one, in either order.
    #[test]
    fn a_video_never_lands_on_a_raw_sidecar() {
        let src_dir = temp_dir("video-src");
        let dest_dir = temp_dir("video-dest");
        let video = write_old(&src_dir, "IMG_1.MOV", b"video");
        write_old(&src_dir, "IMG_1.xmp", b"video's");
        write_old(&dest_dir, "IMG_1.CR3", b"raw");
        write_old(&dest_dir, "IMG_1.xmp", b"raw's");
        let both = [
            write_old(&src_dir, "IMG_2.MOV", b"video"),
            write_old(&src_dir, "IMG_2.CR3", b"raw"),
        ];
        write_old(&src_dir, "IMG_2.MOV.xmp", b"video 2's");
        write_old(&src_dir, "IMG_2.xmp", b"raw 2's");

        let outcome = run(
            Mode::Move,
            &[video, both[0].clone(), both[1].clone()],
            &HashMap::new(),
            &dest_dir,
            &FileLocks::default(),
            &Progress::default(),
        );
        assert_eq!(outcome.done.len(), 3);
        let read = |name: &str| fs::read(dest_dir.join(name)).unwrap();
        assert_eq!(read("IMG_1.xmp"), b"raw's", "untouched");
        assert_eq!(read("IMG_1.MOV.xmp"), b"video's");
        assert_eq!(read("IMG_2.MOV.xmp"), b"video 2's");
        assert_eq!(read("IMG_2.xmp"), b"raw 2's");
        assert_eq!(fs::read_dir(&src_dir).unwrap().count(), 0, "all moved");

        fs::remove_dir_all(&src_dir).unwrap();
        fs::remove_dir_all(&dest_dir).unwrap();
    }

    #[test]
    fn the_queue_knows_its_photos_until_the_outcome_is_collected() {
        let src_dir = temp_dir("involved-src");
        let dest_dir = temp_dir("involved-dest");
        let src = write_old(&src_dir, "d.jpg", b"photo");
        let mut queue = Queue::new(Arc::new(FileLocks::default()));
        assert!(queue.push(
            Mode::Move,
            vec![src.clone()],
            HashMap::new(),
            dest_dir.clone()
        ));
        assert_eq!(queue.involves(&src), Some(Mode::Move));
        let (mode, waiting) = queue.progress().unwrap();
        assert_eq!(
            (mode, waiting.files_total, waiting.files_done),
            (Mode::Move, 1, 0)
        );
        assert_eq!(queue.involves(&src_dir.join("other.jpg")), None);
        // Still waiting for rating writes: nothing runs, the photo stays involved.
        assert!(queue.kick(true, || {}));
        assert_eq!(queue.mode(), Some(Mode::Move));
        assert!(!queue.kick(false, || {}));
        let outcome = loop {
            if let Some(outcome) = queue.poll() {
                break outcome;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(outcome.done.len(), 1);
        assert_eq!(queue.involves(&src), None);
        assert_eq!(queue.progress(), None);
        assert!(!queue.is_busy());

        fs::remove_dir_all(&src_dir).unwrap();
        fs::remove_dir_all(&dest_dir).unwrap();
    }

    /// Every file counts once it is done – a skipped one too – so the bar reaches the end.
    #[test]
    fn progress_counts_files_and_bytes() {
        let src_dir = temp_dir("progress-src");
        let dest_dir = temp_dir("progress-dest");
        let a = write_old(&src_dir, "a.jpg", b"12345");
        let b = write_old(&src_dir, "b.jpg", b"123");
        fs::write(dest_dir.join("b.jpg"), b"taken").unwrap();
        let progress = Progress::new(2);
        assert_eq!(progress.snapshot().files_total, 2);
        assert_eq!(progress.snapshot().fraction(), 0.0);

        let outcome = run(
            Mode::Copy,
            &[a, b],
            &HashMap::new(),
            &dest_dir,
            &FileLocks::default(),
            &progress,
        );
        assert_eq!((outcome.done.len(), outcome.skipped.len()), (1, 1));
        let done = progress.snapshot();
        assert_eq!((done.files_done, done.files_total), (2, 2));
        assert_eq!((done.bytes_done, done.bytes_total), (8, 8));
        assert_eq!(done.current, None);
        assert_eq!(done.fraction(), 1.0);

        fs::remove_dir_all(&src_dir).unwrap();
        fs::remove_dir_all(&dest_dir).unwrap();
    }

    #[test]
    fn the_fraction_falls_back_to_files_without_bytes() {
        let snapshot = Snapshot {
            files_total: 4,
            files_done: 1,
            ..Snapshot::default()
        };
        assert_eq!(snapshot.fraction(), 0.25);
        assert_eq!(Snapshot::default().fraction(), 0.0);
    }

    #[test]
    fn an_existing_name_is_skipped() {
        let src_dir = temp_dir("skip-src");
        let dest_dir = temp_dir("skip-dest");
        let src = write_old(&src_dir, "c.jpg", b"new");
        let dest = dest_dir.join("c.jpg");
        fs::File::create(&dest).unwrap().write_all(b"old").unwrap();

        let outcome = run(
            Mode::Copy,
            std::slice::from_ref(&src),
            &HashMap::new(),
            &dest_dir,
            &FileLocks::default(),
            &Progress::default(),
        );
        assert!(outcome.done.is_empty());
        assert_eq!(outcome.skipped, vec![src]);
        assert_eq!(fs::read(&dest).unwrap(), b"old");

        fs::remove_dir_all(&src_dir).unwrap();
        fs::remove_dir_all(&dest_dir).unwrap();
    }
}
