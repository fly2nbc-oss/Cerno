//! Delayed deletion without confirmation.
//!
//! A deleted photo disappears from the view at once and waits in a queue. Every further
//! deletion restarts the countdown; cancelling (Esc) brings all waiting photos back. When the
//! countdown runs out, the whole queue is moved to the trash on a background thread, so
//! browsing, rating and deleting go on meanwhile.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const DELAY: Duration = Duration::from_secs(5);

/// Moves one file away; the trash in the app, a recorder in tests.
pub type Remover = fn(&Path) -> Result<(), String>;

pub fn move_to_trash(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| e.to_string())
}

/// Result of one finished batch.
#[derive(Debug, Default, PartialEq)]
pub struct Finished {
    pub deleted: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
}

pub struct DeleteQueue {
    remove: Remover,
    pending: Vec<PathBuf>,
    deadline: Option<Instant>,
    /// Handed to a worker, still hidden until its result arrives.
    in_progress: HashSet<PathBuf>,
    tx: mpsc::Sender<Finished>,
    rx: mpsc::Receiver<Finished>,
    workers: Vec<JoinHandle<()>>,
}

impl DeleteQueue {
    pub fn new(remove: Remover) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            remove,
            pending: Vec::new(),
            deadline: None,
            in_progress: HashSet::new(),
            tx,
            rx,
            workers: Vec::new(),
        }
    }

    /// Queues `path` and restarts the countdown.
    pub fn push(&mut self, path: PathBuf, now: Instant) {
        if !self.is_hidden(&path) {
            self.pending.push(path);
        }
        self.deadline = Some(now + DELAY);
    }

    /// Brings every waiting photo back. Returns how many.
    pub fn cancel(&mut self) -> usize {
        self.deadline = None;
        std::mem::take(&mut self.pending).len()
    }

    pub fn is_hidden(&self, path: &Path) -> bool {
        self.in_progress.contains(path) || self.pending.iter().any(|p| p == path)
    }

    /// Waiting photos and the share of the countdown still left (1.0 → 0.0).
    pub fn countdown(&self, now: Instant) -> Option<(usize, f32)> {
        let deadline = self.deadline?;
        let left = deadline.saturating_duration_since(now).as_secs_f32() / DELAY.as_secs_f32();
        Some((self.pending.len(), left.clamp(0.0, 1.0)))
    }

    /// Starts the trash worker once the countdown has run out. `on_done` runs on the worker
    /// after the batch (e.g. to request a repaint).
    pub fn tick(&mut self, now: Instant, on_done: impl FnOnce() + Send + 'static) -> bool {
        if self.deadline.is_none_or(|d| now < d) || self.pending.is_empty() {
            return false;
        }
        self.deadline = None;
        let batch = std::mem::take(&mut self.pending);
        self.in_progress.extend(batch.iter().cloned());
        let (remove, tx) = (self.remove, self.tx.clone());
        self.workers.push(
            std::thread::Builder::new()
                .name("cerno-delete".into())
                .spawn(move || {
                    let _ = tx.send(run(remove, batch));
                    on_done();
                })
                .expect("failed to spawn delete worker"),
        );
        true
    }

    /// Collects finished batches; their photos are no longer hidden afterwards (the deleted
    /// ones are gone, the failed ones reappear).
    pub fn poll(&mut self) -> Option<Finished> {
        let mut all = Finished::default();
        let mut any = false;
        while let Ok(batch) = self.rx.try_recv() {
            any = true;
            for path in batch
                .deleted
                .iter()
                .chain(batch.failed.iter().map(|(p, _)| p))
            {
                self.in_progress.remove(path);
            }
            all.deleted.extend(batch.deleted);
            all.failed.extend(batch.failed);
        }
        self.workers.retain(|w| !w.is_finished());
        any.then_some(all)
    }

    /// On exit: a deletion that wasn't cancelled is carried out, and running batches finish.
    pub fn finish_now(&mut self) {
        if !self.pending.is_empty() {
            let batch = std::mem::take(&mut self.pending);
            for (path, err) in run(self.remove, batch).failed {
                log::error!("could not delete {}: {err}", path.display());
            }
        }
        self.deadline = None;
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn run(remove: Remover, batch: Vec<PathBuf>) -> Finished {
    let mut finished = Finished::default();
    for path in batch {
        match remove(&path) {
            Ok(()) => {
                log::info!("moved to trash: {}", path.display());
                finished.deleted.push(path);
            }
            Err(err) => {
                log::warn!("could not delete {}: {err}", path.display());
                finished.failed.push((path, err));
            }
        }
    }
    finished
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(_: &Path) -> Result<(), String> {
        Ok(())
    }

    fn locked(path: &Path) -> Result<(), String> {
        if path.ends_with("locked.jpg") {
            Err("in use".into())
        } else {
            Ok(())
        }
    }

    fn wait_for(queue: &mut DeleteQueue) -> Finished {
        for _ in 0..200 {
            if let Some(done) = queue.poll() {
                return done;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("delete worker did not finish");
    }

    #[test]
    fn every_deletion_restarts_the_countdown() {
        let t0 = Instant::now();
        let mut queue = DeleteQueue::new(ok);
        queue.push("a.jpg".into(), t0);
        queue.push("b.jpg".into(), t0 + Duration::from_secs(4));
        // 5 s after the first, but only 1 s after the second deletion: nothing happens yet.
        assert!(!queue.tick(t0 + Duration::from_secs(5), || {}));
        let (count, left) = queue.countdown(t0 + Duration::from_secs(5)).unwrap();
        assert_eq!(count, 2);
        assert!((left - 0.8).abs() < 1e-3);

        assert!(queue.tick(t0 + Duration::from_secs(9), || {}));
        assert!(
            queue.is_hidden(Path::new("a.jpg")),
            "hidden while the worker runs"
        );
        let done = wait_for(&mut queue);
        assert_eq!(
            done.deleted,
            [PathBuf::from("a.jpg"), PathBuf::from("b.jpg")]
        );
        assert!(!queue.is_hidden(Path::new("a.jpg")));
        assert!(queue.countdown(t0 + Duration::from_secs(9)).is_none());
    }

    #[test]
    fn cancel_brings_everything_back() {
        let t0 = Instant::now();
        let mut queue = DeleteQueue::new(ok);
        queue.push("a.jpg".into(), t0);
        queue.push("b.jpg".into(), t0);
        assert!(queue.is_hidden(Path::new("b.jpg")));
        assert_eq!(queue.cancel(), 2);
        assert!(!queue.is_hidden(Path::new("b.jpg")));
        assert!(!queue.tick(t0 + Duration::from_secs(60), || {}));
    }

    #[test]
    fn failures_are_reported_and_reappear() {
        let t0 = Instant::now();
        let mut queue = DeleteQueue::new(locked);
        queue.push("locked.jpg".into(), t0);
        queue.push("free.jpg".into(), t0);
        assert!(queue.tick(t0 + DELAY, || {}));
        let done = wait_for(&mut queue);
        assert_eq!(done.deleted, [PathBuf::from("free.jpg")]);
        assert_eq!(done.failed.len(), 1);
        assert!(!queue.is_hidden(Path::new("locked.jpg")));
    }

    #[test]
    fn pending_deletions_run_on_exit() {
        let mut queue = DeleteQueue::new(ok);
        queue.push("a.jpg".into(), Instant::now());
        queue.finish_now();
        assert!(queue.countdown(Instant::now()).is_none());
        assert!(!queue.is_hidden(Path::new("a.jpg")));
    }
}
