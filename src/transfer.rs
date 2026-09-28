//! Copy or move the photos the current filter shows into another folder.
//!
//! The work runs on a background thread. A move prefers `rename` and falls back to copy plus
//! delete when the destination is on another volume. An existing file of the same name is
//! left alone. Each file is held in [`FileLocks`] while it is copied or moved, so a rating
//! written meanwhile never meets a half-copied file.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
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
}

struct Job {
    mode: Mode,
    sources: Vec<PathBuf>,
    dest: PathBuf,
}

pub struct Queue {
    files: Arc<FileLocks>,
    pending: Option<Job>,
    inflight: bool,
    /// The photos of the job that is waiting or running, until its outcome is collected.
    involved: Option<(Mode, HashSet<PathBuf>)>,
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

    /// Remembers the job until [`Self::kick`] sees that rating writes have finished.
    pub fn push(&mut self, mode: Mode, sources: Vec<PathBuf>, dest: PathBuf) -> bool {
        if self.is_busy() {
            return false;
        }
        self.involved = Some((mode, sources.iter().cloned().collect()));
        self.pending = Some(Job {
            mode,
            sources,
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
        self.workers.push(
            std::thread::Builder::new()
                .name("cerno-transfer".into())
                .spawn(move || {
                    let _ = tx.send(run(job.mode, &job.sources, &job.dest, &files));
                    on_done();
                })
                .expect("failed to spawn transfer worker"),
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
            outcomes.push(run(job.mode, &job.sources, &job.dest, &self.files));
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

pub fn run(mode: Mode, sources: &[PathBuf], dest_dir: &Path, files: &FileLocks) -> Outcome {
    let mut outcome = Outcome {
        mode,
        done: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
    };
    for src in sources {
        let Some(name) = src.file_name() else {
            outcome
                .failed
                .push((src.clone(), "missing file name".to_owned()));
            continue;
        };
        let dest = dest_dir.join(name);
        if dest.exists() {
            outcome.skipped.push(src.clone());
            continue;
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
                outcome.done.push((src.clone(), dest));
            }
            Err(err) => {
                log::warn!("could not {} {}: {err}", verb(mode), src.display());
                outcome.failed.push((src.clone(), err));
            }
        }
    }
    outcome
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
            &dest_dir,
            &FileLocks::default(),
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
            &dest_dir,
            &FileLocks::default(),
        );
        assert_eq!(outcome.done.len(), 1);
        assert!(!src.exists());
        assert_eq!(fs::read(&outcome.done[0].1).unwrap(), b"moved");

        fs::remove_dir_all(&src_dir).unwrap();
        fs::remove_dir_all(&dest_dir).unwrap();
    }

    #[test]
    fn the_queue_knows_its_photos_until_the_outcome_is_collected() {
        let src_dir = temp_dir("involved-src");
        let dest_dir = temp_dir("involved-dest");
        let src = write_old(&src_dir, "d.jpg", b"photo");
        let mut queue = Queue::new(Arc::new(FileLocks::default()));
        assert!(queue.push(Mode::Move, vec![src.clone()], dest_dir.clone()));
        assert_eq!(queue.involves(&src), Some(Mode::Move));
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
        assert!(!queue.is_busy());

        fs::remove_dir_all(&src_dir).unwrap();
        fs::remove_dir_all(&dest_dir).unwrap();
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
            &dest_dir,
            &FileLocks::default(),
        );
        assert!(outcome.done.is_empty());
        assert_eq!(outcome.skipped, vec![src]);
        assert_eq!(fs::read(&dest).unwrap(), b"old");

        fs::remove_dir_all(&src_dir).unwrap();
        fs::remove_dir_all(&dest_dir).unwrap();
    }
}
