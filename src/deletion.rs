//! Delayed deletion without confirmation.
//!
//! A deleted photo disappears from the view at once and waits in a queue. Every further
//! deletion restarts the countdown; cancelling (Esc) brings all waiting photos back. When the
//! countdown runs out, the whole queue is set aside on a background thread – into the hidden
//! `.originals` folder beside each photo, never the trash or gone (`originals`) – so browsing,
//! rating and deleting go on meanwhile.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const DELAY: Duration = Duration::from_secs(5);

/// Moves one file away and says where it went; `.originals` in the app, a recorder in tests.
pub type Remover = fn(&Path) -> Result<PathBuf, String>;

pub fn set_aside(path: &Path) -> Result<PathBuf, String> {
    crate::originals::set_aside(path).map_err(|e| format!("{e:#}"))
}

/// Result of one finished batch.
#[derive(Debug, Default, PartialEq)]
pub struct Finished {
    /// Where each photo was, and where it lies now (in `.originals`, so it can come back).
    pub deleted: Vec<(PathBuf, PathBuf)>,
    pub failed: Vec<(PathBuf, String)>,
}

pub struct DeleteQueue {
    remove: Remover,
    pending: Vec<PathBuf>,
    /// The RAWs riding along with pending JPEGs (`pairs`): set aside with them, never counted.
    riders: Vec<PathBuf>,
    deadline: Option<Instant>,
    /// Handed to a worker, still hidden until its result arrives.
    in_progress: HashSet<PathBuf>,
    /// `pending` and `riders` again, for `is_hidden` – asked for every photo of the folder in
    /// a frame, and a deletion of hundreds made those lists long.
    waiting: HashSet<PathBuf>,
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
            riders: Vec::new(),
            deadline: None,
            in_progress: HashSet::new(),
            waiting: HashSet::new(),
            tx,
            rx,
            workers: Vec::new(),
        }
    }

    /// Queues `path` and restarts the countdown.
    pub fn push(&mut self, path: PathBuf, now: Instant) {
        if !self.is_hidden(&path) {
            self.waiting.insert(path.clone());
            self.pending.push(path);
        }
        self.deadline = Some(now + DELAY);
    }

    /// Queues the RAW riding along with a queued JPEG.
    pub fn push_rider(&mut self, path: PathBuf) {
        if !self.is_hidden(&path) {
            self.waiting.insert(path.clone());
            self.riders.push(path);
        }
    }

    /// Brings every waiting photo back. Returns how many.
    pub fn cancel(&mut self) -> usize {
        self.deadline = None;
        self.riders.clear();
        self.waiting.clear();
        std::mem::take(&mut self.pending).len()
    }

    pub fn is_hidden(&self, path: &Path) -> bool {
        self.in_progress.contains(path) || self.waiting.contains(path)
    }

    /// Waiting photos and the share of the countdown still left (1.0 → 0.0).
    pub fn countdown(&self, now: Instant) -> Option<(usize, f32)> {
        let deadline = self.deadline?;
        let left = deadline.saturating_duration_since(now).as_secs_f32() / DELAY.as_secs_f32();
        Some((self.pending.len(), left.clamp(0.0, 1.0)))
    }

    /// Starts the worker that sets the photos aside once the countdown has run out. `on_done` runs on the worker
    /// after the batch (e.g. to request a repaint).
    pub fn tick(&mut self, now: Instant, on_done: impl FnOnce() + Send + 'static) -> bool {
        if self.deadline.is_none_or(|d| now < d) || self.pending.is_empty() {
            return false;
        }
        self.deadline = None;
        let mut batch = std::mem::take(&mut self.pending);
        batch.append(&mut self.riders);
        self.waiting.clear();
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
                .map(|(path, _)| path)
                .chain(batch.failed.iter().map(|(path, _)| path))
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
    /// Returns everything that happened, like [`Self::poll`], so the deletions still count.
    pub fn finish_now(&mut self) -> Finished {
        let mut all = if self.pending.is_empty() {
            Finished::default()
        } else {
            let mut batch = std::mem::take(&mut self.pending);
            batch.append(&mut self.riders);
            self.waiting.clear();
            run(self.remove, batch)
        };
        self.deadline = None;
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        while let Ok(batch) = self.rx.try_recv() {
            all.deleted.extend(batch.deleted);
            all.failed.extend(batch.failed);
        }
        self.in_progress.clear();
        all
    }
}

fn run(remove: Remover, batch: Vec<PathBuf>) -> Finished {
    let mut finished = Finished::default();
    for path in batch {
        match remove(&path) {
            Ok(to) => {
                log::info!("deleted (set aside): {}", path.display());
                finished.deleted.push((path, to));
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

    fn aside(path: &Path) -> PathBuf {
        Path::new(".originals").join(path)
    }

    fn ok(path: &Path) -> Result<PathBuf, String> {
        Ok(aside(path))
    }

    fn locked(path: &Path) -> Result<PathBuf, String> {
        if path.ends_with("locked.jpg") {
            Err("in use".into())
        } else {
            Ok(aside(path))
        }
    }

    /// Where the photos were, without where they went.
    fn sources(done: &Finished) -> Vec<PathBuf> {
        done.deleted.iter().map(|(from, _)| from.clone()).collect()
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
            sources(&done),
            [PathBuf::from("a.jpg"), PathBuf::from("b.jpg")]
        );
        assert_eq!(
            done.deleted[0].1,
            aside(Path::new("a.jpg")),
            "where it went"
        );
        assert!(!queue.is_hidden(Path::new("a.jpg")));
        assert!(queue.countdown(t0 + Duration::from_secs(9)).is_none());
    }

    /// The RAW of a pair goes with its JPEG and is never counted; Esc keeps both.
    #[test]
    fn a_rider_goes_along_uncounted() {
        let now = Instant::now();
        let mut queue = DeleteQueue::new(ok);
        queue.push(PathBuf::from("IMG_1.jpg"), now);
        queue.push_rider(PathBuf::from("IMG_1.CR3"));
        assert_eq!(queue.countdown(now).map(|(n, _)| n), Some(1));
        assert!(queue.is_hidden(Path::new("IMG_1.CR3")));
        queue.cancel();
        assert!(!queue.is_hidden(Path::new("IMG_1.CR3")), "Esc keeps both");

        queue.push(PathBuf::from("IMG_1.jpg"), now);
        queue.push_rider(PathBuf::from("IMG_1.CR3"));
        assert!(queue.tick(now + DELAY, || {}));
        let done = wait_for(&mut queue);
        assert_eq!(
            sources(&done),
            [PathBuf::from("IMG_1.jpg"), PathBuf::from("IMG_1.CR3")]
        );
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
        assert_eq!(sources(&done), [PathBuf::from("free.jpg")]);
        assert_eq!(done.failed.len(), 1);
        assert!(!queue.is_hidden(Path::new("locked.jpg")));
    }

    #[test]
    fn pending_deletions_run_on_exit() {
        let mut queue = DeleteQueue::new(ok);
        queue.push("a.jpg".into(), Instant::now());
        let done = queue.finish_now();
        assert_eq!(sources(&done), [PathBuf::from("a.jpg")]);
        assert!(queue.countdown(Instant::now()).is_none());
        assert!(!queue.is_hidden(Path::new("a.jpg")));
    }

    #[test]
    fn a_batch_still_running_on_exit_is_reported_too() {
        let t0 = Instant::now();
        let mut queue = DeleteQueue::new(ok);
        queue.push("running.jpg".into(), t0);
        assert!(queue.tick(t0 + DELAY, || {}));
        queue.push("waiting.jpg".into(), t0 + DELAY);
        let mut done = sources(&queue.finish_now());
        done.sort();
        assert_eq!(
            done,
            [PathBuf::from("running.jpg"), PathBuf::from("waiting.jpg")]
        );
        assert!(!queue.is_hidden(Path::new("running.jpg")));
    }
}
