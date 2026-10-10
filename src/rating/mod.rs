//! Writes ratings (stars or "rejected"), colour labels, comments and keywords into the original
//! files – in the background, debounced, with the file dates preserved.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use eframe::egui;

use crate::db::{Db, FileStamp};
use crate::exiftool::{ExifTool, ToolError};
use crate::filelock::FileLocks;
use crate::filetimes;
use crate::metadata::{self, Description, Label, LabelInfo, Rating, RatingInfo};

mod args;
mod edits;
mod marks;
mod writer;

use args::*;
use edits::*;
use marks::*;
use writer::*;

/// Pressing 3 and then 4 within this time results in a single write.
const DEBOUNCE: Duration = Duration::from_millis(400);

#[derive(Debug, Default, Clone)]
pub struct WriterStatus {
    pub pending: usize,
    pub last_error: Option<String>,
    /// The version of the ExifTool that runs (once one was started).
    pub exiftool: Option<String>,
    /// Why no ExifTool runs: missing or too old. Not a failed write – the UI blocks the marks
    /// and says why; cleared once one runs.
    pub tool_problem: Option<ToolError>,
}

enum Message {
    SetRating {
        path: PathBuf,
        rating: Rating,
    },
    SetLabel {
        path: PathBuf,
        label: Option<Label>,
    },
    /// Comment and keywords, replacing what the file has.
    SetDescription {
        path: PathBuf,
        description: Description,
    },
    /// Quarter turn, lossless: only the EXIF orientation changes.
    RotateQuarter {
        path: PathBuf,
        clockwise: bool,
    },
    /// New JPEG bytes. Metadata is copied from the current file, then written in place.
    ReplacePixels {
        path: PathBuf,
        jpeg: Vec<u8>,
    },
    EditFailed {
        path: PathBuf,
        message: String,
    },
    /// `Ctrl+Z`: put the newest kept original back.
    Restore {
        path: PathBuf,
    },
    /// Start ExifTool now (it is started on the first write otherwise): its version is known
    /// before a mark is set, and the first star needs no Perl start-up.
    Prepare,
    /// Another program saved the file: the marks it dropped come back (`lost_marks`,
    /// `lost_description`).
    KeepMarks {
        path: PathBuf,
        rating: Rating,
        label: Option<Label>,
        description: Description,
    },
    /// Write what is pending, end ExifTool and hold every further write until `Resume`: its
    /// folder is replaced (`tools::place_exiftool`), and Windows doesn't let a running program's
    /// folder go. `done` says when ExifTool has ended.
    Pause {
        done: mpsc::Sender<()>,
    },
    Resume,
    Shutdown,
}

/// What the UI should do after a straighten or crop has been written (or failed).
pub struct EditOutcome {
    pub path: PathBuf,
    pub error: Option<String>,
    /// Pixels were re-encoded, so the quality notice applies. A quarter turn is lossless.
    pub reencoded: bool,
    /// `Ctrl+Z` put the kept original back.
    pub restored: bool,
    /// Another program saved the file and the marks it dropped are back (`keep_marks`) – not
    /// an edit of Cerno's; the photo is reloaded now.
    pub elsewhere: bool,
}

/// Pauses the writer while ExifTool's folder is replaced – from the download's thread.
pub struct Pauser {
    tx: mpsc::Sender<Message>,
}

impl Pauser {
    /// Writes what is pending, ends ExifTool and holds further writes; `true` once that is
    /// done within `timeout`.
    pub fn pause(&self, timeout: Duration) -> bool {
        let (done, ended) = mpsc::channel();
        self.tx.send(Message::Pause { done }).is_ok() && ended.recv_timeout(timeout).is_ok()
    }

    /// Writes again – with a new ExifTool, started on the first write or `prepare`.
    pub fn resume(&self) {
        let _ = self.tx.send(Message::Resume);
    }
}

/// Sends a finished pixel edit to the writer thread. Cheap to clone into a worker.
#[derive(Clone)]
pub struct EditChannel {
    tx: mpsc::Sender<Message>,
}

impl EditChannel {
    pub fn replace_pixels(&self, path: PathBuf, jpeg: Vec<u8>) {
        let _ = self.tx.send(Message::ReplacePixels { path, jpeg });
    }

    pub fn fail(&self, path: PathBuf, message: String) {
        let _ = self.tx.send(Message::EditFailed { path, message });
    }
}

pub struct RatingWriter {
    tx: mpsc::Sender<Message>,
    thread: Option<JoinHandle<()>>,
    /// A message could not be sent: the thread has ended (it panicked). Marks would be lost
    /// without a word, so the app says so (`stopped`).
    lost: AtomicBool,
    status: Arc<Mutex<WriterStatus>>,
    outcomes: Arc<Mutex<Vec<EditOutcome>>>,
}

impl RatingWriter {
    /// `files`: every write holds its path there, so no one reads or copies a half-written file.
    pub fn new(ctx: egui::Context, db: Arc<Db>, files: Arc<FileLocks>) -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(WriterStatus::default()));
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        let thread_status = Arc::clone(&status);
        let thread_outcomes = Arc::clone(&outcomes);
        let thread = std::thread::Builder::new()
            .name("cerno-rating-writer".into())
            .spawn(move || {
                // Cerno 1.0 kept its copies in the data folder; they move beside the photos.
                let legacy = crate::originals::legacy_dir();
                match legacy.map(|dir| crate::originals::migrate(&db, &dir)) {
                    Ok(Ok(0)) | Err(_) => {}
                    Ok(Ok(n)) => log::info!("moved {n} kept originals into .originals folders"),
                    Ok(Err(err)) => log::warn!("kept originals: {err:#}"),
                }
                run(
                    &rx,
                    &Shared {
                        status: &thread_status,
                        outcomes: &thread_outcomes,
                        ctx: &ctx,
                        db: &db,
                        files: &files,
                    },
                );
            })
            .unwrap_or_else(crate::process::no_thread);
        Self {
            tx,
            thread: Some(thread),
            lost: AtomicBool::new(false),
            status,
            outcomes,
        }
    }

    /// A writer without its thread (tests): marks are sent and never written.
    #[cfg(test)]
    pub fn detached() -> Self {
        let (tx, rx) = mpsc::channel();
        // Kept open, so sending never fails: the messages stay unread.
        std::mem::forget(rx);
        Self {
            tx,
            thread: None,
            lost: AtomicBool::new(false),
            status: Arc::default(),
            outcomes: Arc::default(),
        }
    }

    /// What the writer reports, set by a test (the detached writer has no thread to say it).
    #[cfg(test)]
    pub fn report(&self, status: WriterStatus) {
        if let Ok(mut shown) = self.status.lock() {
            *shown = status;
        }
    }

    fn send(&self, message: Message) {
        if self.tx.send(message).is_err() && !self.lost.swap(true, Ordering::Relaxed) {
            log::error!("the mark writer has stopped: marks are no longer written");
        }
    }

    /// The writer thread has ended before `shutdown` – marks and edits are not written.
    pub fn stopped(&self) -> bool {
        self.lost.load(Ordering::Relaxed)
            || self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    /// `Rating::Unrated` removes the rating.
    pub fn set(&self, path: PathBuf, rating: Rating) {
        self.send(Message::SetRating { path, rating });
    }

    /// `None` removes the colour label.
    pub fn set_label(&self, path: PathBuf, label: Option<Label>) {
        self.send(Message::SetLabel { path, label });
    }

    /// The whole comment and keyword list; empty ones remove them from the file.
    pub fn set_description(&self, path: PathBuf, description: Description) {
        self.send(Message::SetDescription { path, description });
    }

    /// Clockwise or counter-clockwise quarter turn, written as EXIF orientation.
    pub fn rotate_quarter(&self, path: PathBuf, clockwise: bool) {
        self.send(Message::RotateQuarter { path, clockwise });
    }

    /// Puts the newest kept original of `path` back (after any pending mark write).
    pub fn restore(&self, path: PathBuf) {
        self.send(Message::Restore { path });
    }

    /// Another program saved `path`: the rating, label, comment and keywords Cerno knew go
    /// back where the file now has none (Paint, for one, drops all metadata). What that
    /// program set itself stays.
    pub fn keep_marks(
        &self,
        path: PathBuf,
        rating: Rating,
        label: Option<Label>,
        description: Description,
    ) {
        self.send(Message::KeepMarks {
            path,
            rating,
            label,
            description,
        });
    }

    /// Starts ExifTool in the background, or tries again after it was missing; the outcome
    /// shows in [`WriterStatus`].
    pub fn prepare(&self) {
        self.send(Message::Prepare);
    }

    /// Handle that pauses the writer while ExifTool is replaced.
    pub fn pauser(&self) -> Pauser {
        Pauser {
            tx: self.tx.clone(),
        }
    }

    /// Handle for a worker that encodes pixels and then hands the JPEG back here.
    pub fn channel(&self) -> EditChannel {
        EditChannel {
            tx: self.tx.clone(),
        }
    }

    pub fn status(&self) -> WriterStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Finished edits since the last call.
    pub fn poll_edits(&self) -> Vec<EditOutcome> {
        self.outcomes
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default()
    }

    /// Writes everything still pending and waits for it. Must run before the app exits.
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(Message::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for RatingWriter {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests;
