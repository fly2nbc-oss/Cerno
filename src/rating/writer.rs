//! The writer thread: debounced marks per file, edits, what it reports to the app. Marks that
//! could not be written are kept and tried again (`unsaved`).

use std::collections::VecDeque;

use super::unsaved::{Marks, Unsaved};
use super::*;

pub(super) struct Pending {
    marks: Marks,
    at: Instant,
}

impl Pending {
    /// The entry for `path`, restarted: the debounce counts from the newest change.
    fn of(pending: &mut HashMap<PathBuf, Pending>, path: PathBuf) -> &mut Pending {
        let entry = pending.entry(path).or_insert_with(|| Pending {
            marks: Marks::default(),
            at: Instant::now(),
        });
        entry.at = Instant::now();
        entry
    }
}

/// What the writer thread works with.
pub(super) struct Shared<'a> {
    pub(super) status: &'a Mutex<WriterStatus>,
    pub(super) outcomes: &'a Mutex<Vec<EditOutcome>>,
    pub(super) ctx: &'a egui::Context,
    pub(super) db: &'a Db,
    pub(super) files: &'a FileLocks,
}

/// The writer's marks: waiting for their debounce, or not written and kept for another try.
struct Work<'a> {
    status: &'a Mutex<WriterStatus>,
    db: &'a Db,
    files: &'a FileLocks,
    exiftool: Option<ExifTool>,
    pending: HashMap<PathBuf, Pending>,
    unsaved: Unsaved,
}

pub(super) fn run(rx: &mpsc::Receiver<Message>, shared: &Shared<'_>) {
    let Shared {
        status,
        outcomes,
        ctx,
        db,
        files,
    } = *shared;
    let mut work = Work {
        status,
        db,
        files,
        exiftool: None,
        pending: HashMap::new(),
        unsaved: Unsaved::default(),
    };
    let mut shutting_down = false;
    // While ExifTool's folder is replaced: marks wait in `pending`, everything else that needs
    // ExifTool here, in order.
    let mut paused = false;
    let mut held: VecDeque<Message> = VecDeque::new();

    while !shutting_down {
        let timeout = if paused {
            Duration::from_secs(3600)
        } else if !work.pending.is_empty() {
            Duration::from_millis(50)
        } else {
            // A kept mark's next try wakes the thread; nothing polls in between.
            work.unsaved
                .next_wake(Instant::now())
                .unwrap_or(Duration::from_secs(3600))
                .min(Duration::from_secs(3600))
        };
        let message = match held.pop_front() {
            Some(message) if !paused => Ok(message),
            Some(message) => {
                held.push_front(message);
                rx.recv_timeout(timeout)
            }
            None => rx.recv_timeout(timeout),
        };
        let message = match message {
            Ok(message) if paused && needs_exiftool(&message) => {
                held.push_back(message);
                continue;
            }
            other => other,
        };
        let mut retries_done = 0;
        match message {
            Ok(Message::SetRating { path, rating }) => {
                allow_taste(db, &path);
                work.queue(path).rating = Some(rating);
            }
            Ok(Message::SetLabel { path, label }) => {
                work.queue(path).label = Some(label);
            }
            Ok(Message::SetDescription { path, description }) => {
                work.queue(path).description = Some(description);
            }
            // Every edit keeps the original first; without that copy it does not happen.
            Ok(Message::RotateQuarter { path, clockwise }) => {
                let held = files.hold_write(&path);
                let result = work
                    .flush(&path)
                    .and_then(|()| crate::originals::keep(db, &path))
                    .and_then(|()| apply_quarter_turn(&mut work.exiftool, &path, clockwise, db));
                drop(held);
                note_tool(status, &result);
                push_outcome(outcomes, path, result, Done::Rotated);
            }
            Ok(Message::ReplacePixels { path, jpeg }) => {
                let held = files.hold_write(&path);
                let result = work
                    .flush(&path)
                    .and_then(|()| crate::originals::keep(db, &path))
                    .and_then(|()| apply_pixels(&mut work.exiftool, &path, &jpeg, db));
                drop(held);
                note_tool(status, &result);
                push_outcome(outcomes, path, result, Done::Reencoded);
            }
            Ok(Message::EditFailed { path, message }) => {
                push_outcome(
                    outcomes,
                    path,
                    Err(anyhow::anyhow!("{message}")),
                    Done::Rotated,
                );
            }
            Ok(Message::Restore { path }) => {
                let held = files.hold_write(&path);
                let result = work
                    .flush(&path)
                    .and_then(|()| restore_original(&mut work.exiftool, &path, db));
                drop(held);
                note_tool(status, &result);
                push_outcome(outcomes, path, result, Done::Restored);
            }
            Ok(Message::KeepMarks {
                path,
                rating,
                label,
                description,
            }) => {
                let held = files.hold_write(&path);
                work.keep_marks(&path, rating, label, description);
                drop(held);
                // A failed write is kept for another try; the photo is reloaded either way.
                push_outcome(outcomes, path, Ok(()), Done::Elsewhere);
            }
            Ok(Message::Retry { paths }) => {
                work.retry_now(paths.as_deref());
                retries_done += 1;
            }
            Ok(Message::Moved { from, to }) => work.moved(&from, to),
            Ok(Message::Forget { path }) => work.forget(&path),
            Ok(Message::Prepare) => {
                if work.exiftool.is_none() {
                    let started = ExifTool::spawn().map(|tool| work.exiftool = Some(tool));
                    if let Err(err) = &started
                        && tool_problem(err).is_none()
                    {
                        log::warn!("ExifTool: {err:#}");
                    }
                    note_tool(status, &started);
                }
            }
            Ok(Message::Pause { done }) => {
                work.write_due(true);
                // Dropping it ends the stay-open process.
                work.exiftool = None;
                paused = true;
                let _ = done.send(());
            }
            Ok(Message::Resume) => paused = false,
            Ok(Message::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                // Whatever was held is written now, with whichever ExifTool is there.
                paused = false;
                shutting_down = true;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }

        if !paused {
            work.write_due(shutting_down);
            // Kept marks are tried on their own only during the session: at exit the app
            // has asked already (`app::unsaved`), and another round would delay the end.
            if !shutting_down {
                work.retry_due(Instant::now());
            }
        }

        if let Ok(mut status) = status.lock() {
            status.pending = work.pending.len();
            status.unsaved = work.unsaved.report();
            status.retry_done += retries_done;
            if let Some(tool) = &work.exiftool {
                status.exiftool = Some(tool.version().to_owned());
                status.tool_problem = None;
            }
        }
        ctx.request_repaint();
    }
    // Held edits of a shutdown during a pause: their files stay as they are, the log says so.
    for message in held {
        log::warn!("not written at exit: {}", describe(&message));
    }
    for unsaved in work.unsaved.report() {
        log::warn!(
            "marks not written at exit: {}: {}",
            unsaved.path.display(),
            unsaved.error
        );
    }
    // Dropping `work.exiftool` ends the stay-open process.
}

/// Messages that run ExifTool when they come; a pause holds them. Marks wait in `pending`.
fn needs_exiftool(message: &Message) -> bool {
    matches!(
        message,
        Message::RotateQuarter { .. }
            | Message::ReplacePixels { .. }
            | Message::Restore { .. }
            | Message::KeepMarks { .. }
            | Message::Retry { .. }
            | Message::Prepare
    )
}

fn describe(message: &Message) -> String {
    match message {
        Message::RotateQuarter { path, .. }
        | Message::ReplacePixels { path, .. }
        | Message::Restore { path }
        | Message::KeepMarks { path, .. } => path.display().to_string(),
        Message::Retry { .. } => "another try of unsaved marks".to_owned(),
        _ => "ExifTool start".to_owned(),
    }
}

impl Work<'_> {
    /// The marks waiting for `path`, restarted – with the marks kept from a failed write
    /// underneath, so the next write carries them too.
    fn queue(&mut self, path: PathBuf) -> &mut Marks {
        self.files.set_queued(&path, true);
        let kept = self.unsaved.take(&path);
        let entry = Pending::of(&mut self.pending, path);
        if let Some(kept) = kept {
            entry.marks = std::mem::take(&mut entry.marks).over(kept.marks);
        }
        &mut entry.marks
    }

    /// Writes the marks whose debounce ran out – every one when `all`.
    fn write_due(&mut self, all: bool) {
        let mut due: Vec<PathBuf> = self
            .pending
            .iter()
            .filter(|(_, pending)| all || pending.at.elapsed() >= DEBOUNCE)
            .map(|(path, _)| path.clone())
            .collect();
        due.sort();
        for path in due {
            if let Some(pending) = self.pending.remove(&path) {
                self.write(&path, pending.marks, 0);
            }
        }
    }

    /// Kept marks whose next try is due.
    fn retry_due(&mut self, now: Instant) {
        for path in self.unsaved.due(now) {
            if let Some(failed) = self.unsaved.take(&path) {
                self.write(&path, failed.marks, failed.failures);
            }
        }
    }

    /// The app asks: what waits for `paths` – or for every file – is written now, past its
    /// debounce, and kept marks are tried again (a failure starts their backoff over).
    fn retry_now(&mut self, paths: Option<&[PathBuf]>) {
        let mut waiting: Vec<PathBuf> = self
            .pending
            .keys()
            .filter(|path| paths.is_none_or(|paths| paths.contains(path)))
            .cloned()
            .collect();
        waiting.sort();
        for path in waiting {
            if let Some(pending) = self.pending.remove(&path) {
                self.write(&path, pending.marks, 0);
            }
        }
        for path in self.unsaved.matching(paths) {
            if let Some(failed) = self.unsaved.take(&path) {
                self.write(&path, failed.marks, 0);
            }
        }
    }

    /// One write of `marks` into `path`, after `failures` failed ones; a failure keeps them.
    fn write(&mut self, path: &Path, marks: Marks, failures: usize) {
        let held = self.files.hold_write(path);
        let result = write_marks(
            &mut self.exiftool,
            path,
            marks.rating,
            marks.label,
            marks.description.as_ref(),
        );
        self.files.set_queued(path, false);
        // A failed write left the file as it was: no new generation, or every retry would
        // look like Cerno's own save to the watch of *Edit elsewhere*.
        if result.is_err() {
            held.unchanged();
        } else {
            drop(held);
        }
        self.settle(path, marks, failures, result);
    }

    /// After a marks write: the index follows the file; a failure keeps the marks for the
    /// next try, with its error for the app.
    fn settle(
        &mut self,
        path: &Path,
        marks: Marks,
        failures: usize,
        result: Result<Option<Written>>,
    ) {
        match result {
            Ok(written) => {
                if let Some(written) = written {
                    update_index(self.db, path, &written);
                }
            }
            Err(err) => self.keep(path, marks, failures, &err),
        }
    }

    /// Keeps the marks of a failed write for the next try.
    fn keep(&mut self, path: &Path, marks: Marks, failures: usize, err: &anyhow::Error) {
        if let Some(problem) = tool_problem(err) {
            // No ExifTool: the app blocks new marks and says why; these wait for one.
            log::warn!("marks for {}: {err:#}", path.display());
            if let Ok(mut status) = self.status.lock() {
                status.tool_problem = Some(problem);
            }
        } else {
            log::error!("marks for {}: {err:#}", path.display());
        }
        self.unsaved.fail(
            path.to_path_buf(),
            marks,
            format!("{err:#}"),
            failures,
            Instant::now(),
        );
    }

    /// Writes the marks waiting for `path` – and those kept from a failed write – before a
    /// pixel edit, so the copied metadata includes them. The index row is not updated: the
    /// edit deletes it afterwards. On failure the marks are kept and the edit does not happen.
    /// The caller holds `path` for writing.
    fn flush(&mut self, path: &Path) -> Result<()> {
        let waiting = self.pending.remove(path).map(|pending| pending.marks);
        let kept = self.unsaved.take(path);
        let failures = kept.as_ref().map_or(0, |kept| kept.failures);
        let marks = match (waiting, kept) {
            (None, None) => return Ok(()),
            (Some(waiting), None) => waiting,
            (None, Some(kept)) => kept.marks,
            (Some(waiting), Some(kept)) => waiting.over(kept.marks),
        };
        let result = write_marks(
            &mut self.exiftool,
            path,
            marks.rating,
            marks.label,
            marks.description.as_ref(),
        );
        self.files.set_queued(path, false);
        match result {
            Ok(_) => Ok(()),
            Err(err) => {
                self.keep(path, marks, failures, &err);
                Err(err)
            }
        }
    }

    /// Another program saved `path`: the marks it dropped come back (`lost_marks`,
    /// `lost_description`). The caller holds `path` for writing.
    fn keep_marks(
        &mut self,
        path: &Path,
        rating: Rating,
        label: Option<Label>,
        description: Description,
    ) {
        let lost = self.flush(path).and_then(|()| {
            // Where the marks live: a RAW's sidecar is not what the program saved.
            let meta = marks_on_disk(path, crate::sidecar::applies(path))?;
            let (rating, label) = lost_marks(meta.rating.value, meta.label, rating, label);
            let description = lost_description(&meta.description, &description);
            Ok(Marks {
                rating,
                label,
                description,
            })
        });
        let marks = match lost {
            Ok(marks) => marks,
            Err(err) => {
                // What the save dropped is unknown: Cerno's own marks are kept for the next
                // try, so none is lost.
                let known = Marks {
                    rating: (rating != Rating::Unrated).then_some(rating),
                    label: label.map(Some),
                    description: (!description.comment.trim().is_empty()
                        || !description.keywords.is_empty())
                    .then_some(description),
                };
                if known != Marks::default() {
                    self.keep(path, known, 0, &err);
                }
                return;
            }
        };
        if marks == Marks::default() {
            return;
        }
        let result = write_marks(
            &mut self.exiftool,
            path,
            marks.rating,
            marks.label,
            marks.description.as_ref(),
        );
        self.settle(path, marks, 0, result);
    }

    /// Cerno moved `from` to `to`: what waits or is kept for it goes along.
    fn moved(&mut self, from: &Path, to: PathBuf) {
        if let Some(waiting) = self.pending.remove(from) {
            self.files.set_queued(from, false);
            self.files.set_queued(&to, true);
            let entry = Pending::of(&mut self.pending, to.clone());
            entry.marks = waiting.marks.over(std::mem::take(&mut entry.marks));
        }
        self.unsaved.moved(from, to);
    }

    /// Cerno set `path` aside (deleted): its marks are not written any more – a deleted photo
    /// takes no marks, and the session forgets its marks too.
    fn forget(&mut self, path: &Path) {
        let waiting = self.pending.remove(path).is_some();
        if waiting {
            self.files.set_queued(path, false);
        }
        if self.unsaved.forget(path) || waiting {
            log::warn!(
                "marks not written, the photo was deleted: {}",
                path.display()
            );
        }
    }
}

/// The size changed, the mtime didn't: keep the index valid without rehashing.
fn update_index(db: &Db, path: &Path, written: &Written) {
    let updated = FileStamp::of(path)
        .map_err(anyhow::Error::from)
        .and_then(|stamp| {
            db.update_after_write(
                &path.to_string_lossy(),
                stamp,
                written.rating,
                written.label,
            )
        });
    if let Err(err) = updated {
        log::warn!("index update for {}: {err:#}", path.display());
    }
}

/// A new rating or rejection counts again for the taste model (`Db::allow_taste_for`). Here,
/// not on the UI thread: a stat, a lookup and a write per star.
pub(super) fn allow_taste(db: &Db, path: &Path) {
    if let Ok(stamp) = crate::db::FileStamp::of(path)
        && let Ok(Some(record)) = db.lookup(&path.to_string_lossy(), stamp)
        && let Err(err) = db.allow_taste_for(record.fingerprint)
    {
        log::warn!("taste allow: {err:#}");
    }
}

/// A missing or too old ExifTool, wherever in the error it sits.
pub(super) fn tool_problem(err: &anyhow::Error) -> Option<ToolError> {
    err.chain()
        .find_map(|cause| cause.downcast_ref::<ToolError>())
        .cloned()
}

/// Remembers why ExifTool can't run, for the UI.
pub(super) fn note_tool<T>(status: &Mutex<WriterStatus>, result: &Result<T>) {
    if let Err(err) = result
        && let Some(problem) = tool_problem(err)
        && let Ok(mut status) = status.lock()
    {
        status.tool_problem = Some(problem);
    }
}

/// The marks another program's save dropped: Cerno's rating where the file has none, its
/// colour where the file has no label at all (an unknown label text stays).
pub(super) fn lost_marks(
    have: Rating,
    have_label: LabelInfo,
    rating: Rating,
    label: Option<Label>,
) -> (Option<Rating>, Option<Option<Label>>) {
    let rating = (have == Rating::Unrated && rating != Rating::Unrated).then_some(rating);
    let label = match (have_label, label) {
        (LabelInfo::None, Some(label)) => Some(Some(label)),
        _ => None,
    };
    (rating, label)
}

/// Cerno's comment and keywords, when the save dropped both (a program that keeps one of
/// them handles the description itself).
pub(super) fn lost_description(have: &Description, known: &Description) -> Option<Description> {
    let empty = |d: &Description| d.comment.trim().is_empty() && d.keywords.is_empty();
    (empty(have) && !empty(known)).then(|| known.clone())
}

/// What an edit message did, for the notice afterwards.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Done {
    Rotated,
    Reencoded,
    Restored,
    Elsewhere,
}

pub(super) fn push_outcome(
    outcomes: &Mutex<Vec<EditOutcome>>,
    path: PathBuf,
    result: Result<()>,
    done: Done,
) {
    let error = match &result {
        Ok(()) => None,
        Err(err) => {
            log::error!("edit of {}: {err:#}", path.display());
            Some(format!("{err:#}"))
        }
    };
    if let Ok(mut queue) = outcomes.lock() {
        queue.push(EditOutcome {
            path,
            error,
            reencoded: done == Done::Reencoded,
            restored: done == Done::Restored,
            elsewhere: done == Done::Elsewhere,
        });
    }
}
