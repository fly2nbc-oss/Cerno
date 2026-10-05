//! Writes ratings (stars or "rejected"), colour labels, comments and keywords into the original
//! files – in the background, debounced, with the file dates preserved.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

struct Pending {
    rating: Option<Rating>,
    /// `Some` means the user set a label (`None` inside clears it).
    label: Option<Option<Label>>,
    description: Option<Description>,
    at: Instant,
}

impl Pending {
    /// The entry for `path`, restarted: the debounce counts from the newest change.
    fn of(pending: &mut HashMap<PathBuf, Pending>, path: PathBuf) -> &mut Pending {
        let entry = pending.entry(path).or_insert_with(|| Pending {
            rating: None,
            label: None,
            description: None,
            at: Instant::now(),
        });
        entry.at = Instant::now();
        entry
    }
}

pub struct RatingWriter {
    tx: mpsc::Sender<Message>,
    thread: Option<JoinHandle<()>>,
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
            .expect("failed to spawn rating writer");
        Self {
            tx,
            thread: Some(thread),
            status,
            outcomes,
        }
    }

    /// `Rating::Unrated` removes the rating.
    pub fn set(&self, path: PathBuf, rating: Rating) {
        let _ = self.tx.send(Message::SetRating { path, rating });
    }

    /// `None` removes the colour label.
    pub fn set_label(&self, path: PathBuf, label: Option<Label>) {
        let _ = self.tx.send(Message::SetLabel { path, label });
    }

    /// The whole comment and keyword list; empty ones remove them from the file.
    pub fn set_description(&self, path: PathBuf, description: Description) {
        let _ = self.tx.send(Message::SetDescription { path, description });
    }

    /// Clockwise or counter-clockwise quarter turn, written as EXIF orientation.
    pub fn rotate_quarter(&self, path: PathBuf, clockwise: bool) {
        let _ = self.tx.send(Message::RotateQuarter { path, clockwise });
    }

    /// Puts the newest kept original of `path` back (after any pending mark write).
    pub fn restore(&self, path: PathBuf) {
        let _ = self.tx.send(Message::Restore { path });
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
        let _ = self.tx.send(Message::KeepMarks {
            path,
            rating,
            label,
            description,
        });
    }

    /// Starts ExifTool in the background, or tries again after it was missing; the outcome
    /// shows in [`WriterStatus`].
    pub fn prepare(&self) {
        let _ = self.tx.send(Message::Prepare);
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

/// What the writer thread works with.
struct Shared<'a> {
    status: &'a Mutex<WriterStatus>,
    outcomes: &'a Mutex<Vec<EditOutcome>>,
    ctx: &'a egui::Context,
    db: &'a Db,
    files: &'a FileLocks,
}

fn run(rx: &mpsc::Receiver<Message>, shared: &Shared<'_>) {
    let Shared {
        status,
        outcomes,
        ctx,
        db,
        files,
    } = *shared;
    let mut exiftool: Option<ExifTool> = None;
    let mut pending: HashMap<PathBuf, Pending> = HashMap::new();
    let mut shutting_down = false;

    while !shutting_down {
        let timeout = if pending.is_empty() {
            Duration::from_secs(3600)
        } else {
            Duration::from_millis(50)
        };
        match rx.recv_timeout(timeout) {
            Ok(Message::SetRating { path, rating }) => {
                files.set_queued(&path, true);
                Pending::of(&mut pending, path).rating = Some(rating);
            }
            Ok(Message::SetLabel { path, label }) => {
                files.set_queued(&path, true);
                Pending::of(&mut pending, path).label = Some(label);
            }
            Ok(Message::SetDescription { path, description }) => {
                files.set_queued(&path, true);
                Pending::of(&mut pending, path).description = Some(description);
            }
            // Every edit keeps the original first; without that copy it does not happen.
            Ok(Message::RotateQuarter { path, clockwise }) => {
                let held = files.hold_write(&path);
                let result = flush_pending(&mut pending, &mut exiftool, &path, status, files)
                    .and_then(|()| crate::originals::keep(db, &path))
                    .and_then(|()| apply_quarter_turn(&mut exiftool, &path, clockwise, db));
                drop(held);
                note_tool(status, &result);
                push_outcome(outcomes, path, result, Done::Rotated);
            }
            Ok(Message::ReplacePixels { path, jpeg }) => {
                let held = files.hold_write(&path);
                let result = flush_pending(&mut pending, &mut exiftool, &path, status, files)
                    .and_then(|()| crate::originals::keep(db, &path))
                    .and_then(|()| apply_pixels(&mut exiftool, &path, &jpeg, db));
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
                let result = flush_pending(&mut pending, &mut exiftool, &path, status, files)
                    .and_then(|()| restore_original(&mut exiftool, &path, db));
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
                let result = flush_pending(&mut pending, &mut exiftool, &path, status, files)
                    .and_then(|()| {
                        // Where the marks live: a RAW's sidecar is not what the program saved.
                        let meta = marks_on_disk(&path, crate::sidecar::applies(&path))?;
                        let (rating, label) =
                            lost_marks(meta.rating.value, meta.label, rating, label);
                        let description = lost_description(&meta.description, &description);
                        if rating.is_none() && label.is_none() && description.is_none() {
                            return Ok(None);
                        }
                        write_marks(&mut exiftool, &path, rating, label, description.as_ref())
                    });
                drop(held);
                note_marks(db, status, &path, result);
                // A failed write shows in the status; the photo is reloaded either way.
                push_outcome(outcomes, path, Ok(()), Done::Elsewhere);
            }
            Ok(Message::Prepare) => {
                if exiftool.is_none() {
                    let started = ExifTool::spawn().map(|tool| exiftool = Some(tool));
                    if let Err(err) = &started
                        && tool_problem(err).is_none()
                    {
                        log::warn!("ExifTool: {err:#}");
                    }
                    note_tool(status, &started);
                }
            }
            Ok(Message::Shutdown) | Err(RecvTimeoutError::Disconnected) => shutting_down = true,
            Err(RecvTimeoutError::Timeout) => {}
        }

        let due: Vec<PathBuf> = pending
            .iter()
            .filter(|(_, pending)| shutting_down || pending.at.elapsed() >= DEBOUNCE)
            .map(|(path, _)| path.clone())
            .collect();
        for path in due {
            let Some(marks) = pending.remove(&path) else {
                continue;
            };
            let held = files.hold_write(&path);
            let result = write_marks(
                &mut exiftool,
                &path,
                marks.rating,
                marks.label,
                marks.description.as_ref(),
            );
            files.set_queued(&path, false);
            drop(held);
            note_marks(db, status, &path, result);
        }

        if let Ok(mut status) = status.lock() {
            status.pending = pending.len();
            if let Some(tool) = &exiftool {
                status.exiftool = Some(tool.version().to_owned());
                status.tool_problem = None;
            }
        }
        ctx.request_repaint();
    }
    // Dropping `exiftool` ends the stay-open process.
}

/// After a marks write: the index follows the file, the status shows the error.
fn note_marks(db: &Db, status: &Mutex<WriterStatus>, path: &Path, result: Result<Option<Written>>) {
    if let Ok(Some(written)) = &result {
        // The size changed, the mtime didn't: keep the index valid without rehashing.
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
    if let Ok(mut status) = status.lock() {
        match result {
            Ok(_) => status.last_error = None,
            Err(err) if tool_problem(&err).is_some() => {
                log::warn!("rating for {}: {err:#}", path.display());
                status.tool_problem = tool_problem(&err);
            }
            Err(err) => {
                log::error!("rating for {}: {err:#}", path.display());
                status.last_error = Some(format!(
                    "{}: {err:#}",
                    crate::library::file_name_lossy(path)
                ));
            }
        }
    }
}

/// A missing or too old ExifTool, wherever in the error it sits.
fn tool_problem(err: &anyhow::Error) -> Option<ToolError> {
    err.chain()
        .find_map(|cause| cause.downcast_ref::<ToolError>())
        .cloned()
}

/// Remembers why ExifTool can't run, for the UI.
fn note_tool<T>(status: &Mutex<WriterStatus>, result: &Result<T>) {
    if let Err(err) = result
        && let Some(problem) = tool_problem(err)
        && let Ok(mut status) = status.lock()
    {
        status.tool_problem = Some(problem);
    }
}

/// The marks another program's save dropped: Cerno's rating where the file has none, its
/// colour where the file has no label at all (an unknown label text stays).
fn lost_marks(
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
fn lost_description(have: &Description, known: &Description) -> Option<Description> {
    let empty = |d: &Description| d.comment.trim().is_empty() && d.keywords.is_empty();
    (empty(have) && !empty(known)).then(|| known.clone())
}

struct Written {
    rating: Rating,
    label: Option<Label>,
}

/// Writes the rating, colour label, comment and keywords – whatever changed – in one ExifTool
/// call. `Ok(None)` when the file already matches, so the dates are not touched.
fn write_marks(
    exiftool: &mut Option<ExifTool>,
    path: &Path,
    rating: Option<Rating>,
    label: Option<Option<Label>>,
    description: Option<&Description>,
) -> Result<Option<Written>> {
    // Where the marks live: in the file, or in its XMP sidecar (RAW, BMP, video).
    let sidecar = crate::sidecar::applies(path);
    let target = if sidecar {
        crate::sidecar::path_of(path)
    } else {
        path.to_path_buf()
    };
    let path_str = target.to_str().context("path is not valid Unicode")?;
    // Read what is there right now: skips no-op writes and tells which extra rating tags
    // (Windows Explorer's) need to be kept in sync.
    let on_disk = marks_on_disk(path, sidecar)?;
    // A new sidecar starts empty and from then on replaces the file's own marks, so the first
    // write into it carries all of them – a colour set on a RAW keeps its in-camera stars.
    // Whether anything changes at all is still judged against the file.
    let fresh = sidecar && !target.is_file();
    let (meta, rating, label, description) = if fresh {
        let changes = rating.is_some_and(|r| r != on_disk.rating.value)
            || label.is_some_and(|l| label_needs_write(on_disk.label, l))
            || description.is_some_and(|d| !description_args(d, &on_disk.description).is_empty());
        if !changes {
            return Ok(None);
        }
        let carried = description.map_or_else(|| on_disk.description.clone(), Clone::clone);
        (
            metadata::read(&[]),
            Some(rating.unwrap_or(on_disk.rating.value)),
            Some(label.unwrap_or(on_disk.label.known())),
            Some(carried),
        )
    } else {
        (on_disk, rating, label, description.cloned())
    };

    let mut args = Vec::new();
    let mut written_rating = meta.rating.value;
    let mut written_label = meta.label.known();
    if let Some(rating) = rating
        && rating != meta.rating.value
    {
        args.extend(rating_args(rating, &meta.rating));
        written_rating = rating;
    }
    if let Some(label) = label
        && label_needs_write(meta.label, label)
    {
        args.push(label_arg(label));
        written_label = label;
    }
    if let Some(description) = &description {
        args.extend(description_args(description, &meta.description));
    }
    if args.is_empty() {
        if !fresh {
            return Ok(None);
        }
        // All the file's marks are cleared: an empty sidecar says so.
        crate::sidecar::ensure(path)?;
        return Ok(Some(Written {
            rating: written_rating,
            label: written_label,
        }));
    }
    // ExifTool first: without it an empty sidecar would be left behind, and from then on it
    // would hide the RAW's own (in-camera) marks.
    let tool = match exiftool {
        Some(tool) => tool,
        None => exiftool.insert(ExifTool::spawn()?),
    };
    if sidecar {
        crate::sidecar::ensure(path)?;
    }

    let snapshot = filetimes::Snapshot::capture(&target).context("cannot read file times")?;
    let mut command: Vec<&str> = args.iter().map(String::as_str).collect();
    command.push(path_str);
    let output = match tool.execute(&command) {
        Ok(output) => output,
        Err(err) => {
            // The process is gone or out of sync – start a fresh one next time.
            *exiftool = None;
            return Err(err);
        }
    };

    // ExifTool's `-P` goes through Perl floats and shifts the times by a few microseconds;
    // this puts back the exact values.
    if snapshot
        .restore(&target)
        .context("cannot restore file times")?
    {
        log::debug!("restored exact file times of {}", target.display());
    }
    let updated = ["1 image files updated", "1 image files unchanged"]
        .iter()
        .any(|ok| output.stdout.contains(ok));
    if !updated {
        let message = output
            .stderr
            .lines()
            .chain(output.stdout.lines())
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("ExifTool did not update the file");
        bail!("{message}");
    }
    log::info!(
        "marks {:?} {:?} written to {}",
        written_rating,
        written_label,
        target.display()
    );
    Ok(Some(Written {
        rating: written_rating,
        label: written_label,
    }))
}

/// The marks as stored now. With a sidecar: only it (a video is never read whole). Before the
/// first write a RAW or BMP shows its own marks – a rating given in the camera – so those are
/// compared with.
fn marks_on_disk(path: &Path, sidecar: bool) -> Result<metadata::FileMetadata> {
    let video = crate::library::format_of(path) == Some(crate::library::Format::Video);
    if sidecar && (video || crate::sidecar::path_of(path).is_file()) {
        return Ok(metadata::read_sidecar(path));
    }
    let bytes = std::fs::read(path).context("cannot read file")?;
    Ok(if sidecar {
        metadata::read_for(path, &bytes)
    } else {
        metadata::read(&bytes)
    })
}

fn label_needs_write(on_disk: LabelInfo, wanted: Option<Label>) -> bool {
    match (on_disk, wanted) {
        (LabelInfo::Known(have), Some(want)) => have != want,
        (LabelInfo::None, None) => false,
        _ => true,
    }
}

/// Only the parts that changed. ExifTool's MWG tags write IPTC and XMP together (and the EXIF
/// description): `MWG:Keywords` = IPTC Keywords + XMP `dc:subject`, `MWG:Description` = IPTC
/// Caption-Abstract + XMP `dc:description` + EXIF ImageDescription. IPTC is marked and written
/// as UTF-8, or umlauts would come out as Latin-1. Repeating `-MWG:Keywords=` builds the new
/// list; one empty value removes them all.
fn description_args(wanted: &Description, on_disk: &Description) -> Vec<String> {
    let mut args = Vec::new();
    if wanted.keywords != on_disk.keywords {
        if wanted.keywords.is_empty() {
            args.push("-MWG:Keywords=".to_owned());
        }
        for keyword in &wanted.keywords {
            // One line each: keywords never hold line breaks.
            let keyword = keyword.replace(['\r', '\n'], " ");
            args.push(format!("-MWG:Keywords={}", keyword.trim()));
        }
    }
    if wanted.comment.trim() != on_disk.comment {
        args.push(format!("-MWG:Description={}", wanted.comment.trim()));
    }
    if !args.is_empty() {
        let options = [
            "-use",
            "MWG",
            "-charset",
            "iptc=UTF8",
            "-IPTC:CodedCharacterSet=UTF8",
        ];
        args.splice(0..0, options.map(str::to_owned));
    }
    args
}

fn label_arg(label: Option<Label>) -> String {
    match label {
        Some(label) => format!("-XMP-xmp:Label={}", label.xmp_name()),
        None => "-XMP-xmp:Label=".to_owned(),
    }
}

/// An empty value makes ExifTool delete the tag. Windows' own tags know no "rejected"; they are
/// cleared then, so Explorer shows no stars.
fn rating_args(rating: Rating, on_disk: &RatingInfo) -> Vec<String> {
    let value = rating.value().map(|v| v.to_string()).unwrap_or_default();
    let stars = rating.stars();
    let windows = stars.map(|s| s.to_string()).unwrap_or_default();
    let percent = stars.map(|s| percent(s).to_string()).unwrap_or_default();
    let mut args = vec![format!("-XMP-xmp:Rating={value}")];
    if on_disk.has_exif_rating {
        args.push(format!("-EXIF:Rating={windows}"));
        args.push(format!("-EXIF:RatingPercent={percent}"));
    }
    if on_disk.has_ms_photo_rating {
        args.push(format!("-XMP-microsoft:RatingPercent={percent}"));
    }
    args
}

/// Windows' mapping of stars to its percentage rating.
fn percent(stars: u8) -> u8 {
    match stars {
        1 => 1,
        2 => 25,
        3 => 50,
        4 => 75,
        _ => 99,
    }
}

/// Writes a pending rating for `path` before a pixel edit, so the copied metadata includes it.
/// The index row is not updated: the edit deletes it afterwards.
/// The caller holds `path` for writing.
fn flush_pending(
    pending: &mut HashMap<PathBuf, Pending>,
    exiftool: &mut Option<ExifTool>,
    path: &Path,
    status: &Mutex<WriterStatus>,
    files: &FileLocks,
) -> Result<()> {
    let Some(marks) = pending.remove(path) else {
        return Ok(());
    };
    let result = write_marks(
        exiftool,
        path,
        marks.rating,
        marks.label,
        marks.description.as_ref(),
    );
    files.set_queued(path, false);
    if let Ok(mut status) = status.lock() {
        match &result {
            Ok(_) => status.last_error = None,
            Err(err) => {
                status.last_error = Some(format!(
                    "{}: {err:#}",
                    crate::library::file_name_lossy(path)
                ));
            }
        }
    }
    result.map(|_| ())
}

/// What an edit message did, for the notice afterwards.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Done {
    Rotated,
    Reencoded,
    Restored,
    Elsewhere,
}

fn push_outcome(outcomes: &Mutex<Vec<EditOutcome>>, path: PathBuf, result: Result<()>, done: Done) {
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

/// `Ctrl+Z`: writes the first original (`.originals`) back into the file – in place, so the
/// file keeps its identity and dates – then the rating and colour label the file has now, so
/// marks set after the edit stay. The original stays where it is.
fn restore_original(exiftool: &mut Option<ExifTool>, path: &Path, db: &Db) -> Result<()> {
    let key = path.to_string_lossy();
    let copy = crate::originals::original(db, path)?.context("no original kept for this photo")?;
    let original = std::fs::read(&copy).context("cannot read the kept original")?;
    let now = metadata::read(&std::fs::read(path).context("cannot read file")?);
    let label = match now.label {
        LabelInfo::Known(label) => Some(Some(label)),
        LabelInfo::None => Some(None),
        // Text Cerno does not know is left as the original had it.
        LabelInfo::Other => None,
    };
    let snapshot = filetimes::Snapshot::capture(path).context("cannot read file times")?;
    write_in_place(path, &original)?;
    snapshot
        .restore(path)
        .context("cannot restore file times")?;
    write_marks(
        exiftool,
        path,
        Some(now.rating.value),
        label,
        Some(&now.description),
    )?;
    db.forget_file(&key).context("cannot drop the index row")?;
    log::info!("original restored: {}", path.display());
    Ok(())
}

fn apply_quarter_turn(
    exiftool: &mut Option<ExifTool>,
    path: &Path,
    clockwise: bool,
    db: &Db,
) -> Result<()> {
    let path_str = path.to_str().context("path is not valid Unicode")?;
    let bytes = std::fs::read(path).context("cannot read file")?;
    let current = metadata::read(&bytes).orientation;
    drop(bytes);
    let next = crate::edit::rotate_orientation(current, if clockwise { 1 } else { -1 });
    if next == current {
        return Ok(());
    }
    let snapshot = filetimes::Snapshot::capture(path).context("cannot read file times")?;
    run_exiftool(
        exiftool,
        &[format!("-Orientation#={next}"), path_str.to_owned()],
    )?;
    if snapshot
        .restore(path)
        .context("cannot restore file times")?
    {
        log::debug!("restored exact file times of {}", path.display());
    }
    db.forget_file(&path.to_string_lossy())
        .context("cannot drop the index row")?;
    log::info!("orientation {current} → {next} on {}", path.display());
    Ok(())
}

fn apply_pixels(exiftool: &mut Option<ExifTool>, path: &Path, jpeg: &[u8], db: &Db) -> Result<()> {
    let path_str = path.to_str().context("path is not valid Unicode")?;
    let snapshot = filetimes::Snapshot::capture(path).context("cannot read file times")?;
    let temp = TempJpeg::write(jpeg)?;
    let temp_str = temp
        .path
        .to_str()
        .context("temp path is not valid Unicode")?
        .to_owned();
    run_exiftool(
        exiftool,
        &[
            "-TagsFromFile".to_owned(),
            path_str.to_owned(),
            "-all:all".to_owned(),
            "-unsafe".to_owned(),
            // Not part of `-all:all`: the pixels stay in the file's colour space
            // (`decode::decode_for_edit`), so its profile must come along.
            "-ICC_Profile".to_owned(),
            "-Orientation#=1".to_owned(),
            "-ThumbnailImage=".to_owned(),
            "-PreviewImage=".to_owned(),
            "-MPF:all=".to_owned(),
            "-IFD1:all=".to_owned(),
            "-EXIF:ImageWidth=".to_owned(),
            "-EXIF:ImageHeight=".to_owned(),
            "-EXIF:ExifImageWidth=".to_owned(),
            "-EXIF:ExifImageHeight=".to_owned(),
            temp_str,
        ],
    )?;
    let prepared = std::fs::read(&temp.path).context("cannot read prepared JPEG")?;
    write_in_place(path, &prepared)?;
    if snapshot
        .restore(path)
        .context("cannot restore file times")?
    {
        log::debug!("restored exact file times of {}", path.display());
    }
    db.forget_file(&path.to_string_lossy())
        .context("cannot drop the index row")?;
    log::info!("pixels rewritten in {}", path.display());
    Ok(())
}

fn run_exiftool(exiftool: &mut Option<ExifTool>, args: &[String]) -> Result<()> {
    if args.iter().any(|arg| arg.contains(['\n', '\r'])) {
        bail!("argument contains a line break");
    }
    let tool = match exiftool {
        Some(tool) => tool,
        None => exiftool.insert(ExifTool::spawn()?),
    };
    let command: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = match tool.execute(&command) {
        Ok(output) => output,
        Err(err) => {
            *exiftool = None;
            return Err(err);
        }
    };
    let updated = ["1 image files updated", "1 image files unchanged"]
        .iter()
        .any(|ok| output.stdout.contains(ok));
    if !updated {
        let message = output
            .stderr
            .lines()
            .chain(output.stdout.lines())
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("ExifTool did not update the file");
        bail!("{message}");
    }
    Ok(())
}

fn write_in_place(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .context("cannot open file")?;
    file.write_all(bytes).context("cannot write file")?;
    file.set_len(bytes.len() as u64)
        .context("cannot resize file")?;
    Ok(())
}

/// Removed on drop, including when the metadata copy fails.
struct TempJpeg {
    path: PathBuf,
}

impl TempJpeg {
    /// A new file with `bytes` in the temp folder. `create_new` fails when the name is taken
    /// instead of following a link another user planted there (a shared `/tmp` on Linux).
    fn write(bytes: &[u8]) -> Result<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("cerno-edit-{}-{nanos}.jpg", std::process::id()));
        Self::write_at(path, bytes)
    }

    fn write_at(path: PathBuf, bytes: &[u8]) -> Result<Self> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .context("cannot create temporary JPEG")?;
        // Ours from here on: removed on drop, also when the write fails.
        let temp = Self { path };
        file.write_all(bytes)
            .context("cannot write temporary JPEG")?;
        Ok(temp)
    }
}

impl Drop for TempJpeg {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The temp JPEG never reuses a name: whatever is there already stays untouched – it is
    /// neither written through nor deleted on drop.
    #[test]
    fn temp_jpeg_refuses_a_taken_name() {
        use std::fs;
        let dir = std::env::temp_dir().join(format!("cerno-temp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let taken = dir.join("taken.jpg");
        fs::write(&taken, b"someone else's").unwrap();
        assert!(TempJpeg::write_at(taken.clone(), b"edit").is_err());
        assert_eq!(fs::read(&taken).unwrap(), b"someone else's");

        let fresh = dir.join("fresh.jpg");
        let temp = TempJpeg::write_at(fresh.clone(), b"edit").unwrap();
        assert_eq!(fs::read(&fresh).unwrap(), b"edit");
        drop(temp);
        assert!(!fresh.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_marks_a_save_dropped_come_back() {
        let red = Some(Label::Red);
        // Paint and the like drop all metadata: both come back.
        assert_eq!(
            lost_marks(Rating::Unrated, LabelInfo::None, Rating::Stars(4), red),
            (Some(Rating::Stars(4)), Some(red))
        );
        assert_eq!(
            lost_marks(Rating::Unrated, LabelInfo::None, Rating::Rejected, None),
            (Some(Rating::Rejected), None)
        );
        // What the other program set stays, an unknown label text too.
        let blue = LabelInfo::Known(Label::Blue);
        assert_eq!(
            lost_marks(Rating::Stars(2), blue, Rating::Stars(4), red),
            (None, None)
        );
        assert_eq!(
            lost_marks(Rating::Unrated, LabelInfo::Other, Rating::Unrated, red),
            (None, None)
        );
        // Nothing known, nothing to write.
        assert_eq!(
            lost_marks(Rating::Unrated, LabelInfo::None, Rating::Unrated, None),
            (None, None)
        );
    }

    #[test]
    fn only_xmp_unless_microsoft_tags_exist() {
        assert_eq!(
            rating_args(Rating::Stars(4), &RatingInfo::default()),
            ["-XMP-xmp:Rating=4"]
        );
        let windows = RatingInfo {
            value: Rating::Stars(2),
            has_exif_rating: true,
            has_ms_photo_rating: true,
        };
        assert_eq!(
            rating_args(Rating::Stars(5), &windows),
            [
                "-XMP-xmp:Rating=5",
                "-EXIF:Rating=5",
                "-EXIF:RatingPercent=99",
                "-XMP-microsoft:RatingPercent=99",
            ]
        );
    }

    #[test]
    fn clearing_deletes_the_tags() {
        let windows = RatingInfo {
            value: Rating::Stars(3),
            has_exif_rating: true,
            has_ms_photo_rating: false,
        };
        assert_eq!(
            rating_args(Rating::Unrated, &windows),
            ["-XMP-xmp:Rating=", "-EXIF:Rating=", "-EXIF:RatingPercent="]
        );
    }

    #[test]
    fn rejected_is_minus_one_and_clears_the_windows_stars() {
        let windows = RatingInfo {
            value: Rating::Stars(3),
            has_exif_rating: true,
            has_ms_photo_rating: true,
        };
        assert_eq!(
            rating_args(Rating::Rejected, &windows),
            [
                "-XMP-xmp:Rating=-1",
                "-EXIF:Rating=",
                "-EXIF:RatingPercent=",
                "-XMP-microsoft:RatingPercent=",
            ]
        );
    }

    /// A RAW's marks go into `IMG_9.xmp` beside it – created on the first write – and the RAW
    /// itself is never touched. Through a real ExifTool; skipped without one.
    #[test]
    fn raw_marks_go_into_the_sidecar() {
        if crate::exiftool::locate().is_none() {
            eprintln!("ExifTool not found – skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!("cerno-sidecar-w-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let raw = dir.join("Blüte IMG_9.NEF");
        let untouched = b"II*\0 proprietary raw data".to_vec();
        std::fs::write(&raw, &untouched).unwrap();
        let mut exiftool = None;

        write_marks(
            &mut exiftool,
            &raw,
            Some(Rating::Stars(3)),
            Some(Some(Label::Green)),
            None,
        )
        .unwrap();
        assert_eq!(std::fs::read(&raw).unwrap(), untouched, "RAW untouched");
        let meta = metadata::read_for(&raw, &untouched);
        assert_eq!(meta.rating.value, Rating::Stars(3));
        assert_eq!(meta.label, LabelInfo::Known(Label::Green));
        // Back to no stars: the sidecar says so, not the camera.
        write_marks(&mut exiftool, &raw, Some(Rating::Unrated), None, None).unwrap();
        assert_eq!(metadata::read_sidecar(&raw).rating.value, Rating::Unrated);
        drop(exiftool);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The first write into a new sidecar carries the RAW's own marks: a colour set on a RAW
    /// with in-camera stars and a keyword keeps both, and a comment is read back from the
    /// sidecar. Clearing a RAW's only mark leaves an empty sidecar that says so.
    #[test]
    fn a_new_sidecar_keeps_the_raws_own_marks() {
        if crate::exiftool::locate().is_none() {
            eprintln!("ExifTool not found – skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!("cerno-sidecar-c-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let in_camera = "II*\0<x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF \
            xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'><rdf:Description \
            rdf:about='' xmlns:xmp='http://ns.adobe.com/xap/1.0/' \
            xmlns:dc='http://purl.org/dc/elements/1.1/' xmp:Rating='3'><dc:subject><rdf:Bag>\
            <rdf:li>Berg</rdf:li></rdf:Bag></dc:subject></rdf:Description></rdf:RDF>\
            </x:xmpmeta>";
        let raw = dir.join("IMG_7.NEF");
        std::fs::write(&raw, in_camera).unwrap();
        let mut exiftool = None;

        write_marks(&mut exiftool, &raw, None, Some(Some(Label::Green)), None).unwrap();
        let side = metadata::read_sidecar(&raw);
        assert_eq!(
            side.rating.value,
            Rating::Stars(3),
            "in-camera stars carried"
        );
        assert_eq!(side.label, LabelInfo::Known(Label::Green));
        assert_eq!(
            side.description.keywords,
            ["Berg"],
            "in-camera keyword carried"
        );
        let comment = Description {
            comment: "Gipfel".into(),
            keywords: vec!["Berg".into()],
        };
        write_marks(&mut exiftool, &raw, None, None, Some(&comment)).unwrap();
        let meta = metadata::read_for(&raw, in_camera.as_bytes());
        assert_eq!(
            meta.description, comment,
            "the description comes from the sidecar"
        );
        assert_eq!(
            std::fs::read(&raw).unwrap(),
            in_camera.as_bytes(),
            "RAW untouched"
        );

        // A RAW with only in-camera stars, cleared: nothing to write, but the sidecar must say
        // "no stars" from now on.
        let stars_only = in_camera.replace(
            "<dc:subject><rdf:Bag><rdf:li>Berg</rdf:li></rdf:Bag></dc:subject>",
            "",
        );
        let cleared = dir.join("IMG_8.NEF");
        std::fs::write(&cleared, &stars_only).unwrap();
        assert_eq!(
            metadata::read_for(&cleared, stars_only.as_bytes())
                .rating
                .value,
            Rating::Stars(3)
        );
        write_marks(&mut exiftool, &cleared, Some(Rating::Unrated), None, None).unwrap();
        assert!(
            crate::sidecar::path_of(&cleared).is_file(),
            "the empty sidecar is written"
        );
        let meta = metadata::read_for(&cleared, stars_only.as_bytes());
        assert_eq!(meta.rating.value, Rating::Unrated);
        drop(exiftool);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A sidecar another program wrote – Lightroom's development settings, a mask, a namespace
    /// Cerno doesn't know, keywords – keeps all of it when Cerno sets stars, a colour and a
    /// comment: only those tags change. ExifTool writes the packet anew (attributes may become
    /// elements), so the test looks for names and values, not bytes. Skipped without ExifTool.
    #[test]
    fn a_lightroom_sidecar_keeps_its_development() {
        if crate::exiftool::locate().is_none() {
            eprintln!("ExifTool not found – skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!("cerno-lr-xmp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let raw = dir.join("IMG_3.CR3");
        let untouched = b"proprietary raw data".to_vec();
        std::fs::write(&raw, &untouched).unwrap();
        let lightroom = r#"<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core 7.0">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:foo="http://example.com/foo/1.0/"
    xmp:Rating="2"
    crs:Version="16.0"
    crs:ProcessVersion="15.4"
    crs:Exposure2012="+0.50"
    crs:Contrast2012="+12"
    crs:HasSettings="True"
    foo:Kept="keep me">
   <crs:MaskGroupBasedCorrections>
    <rdf:Seq>
     <rdf:li crs:What="Correction" crs:LocalExposure2012="-0.35"/>
    </rdf:Seq>
   </crs:MaskGroupBasedCorrections>
   <dc:subject>
    <rdf:Bag>
     <rdf:li>Berg</rdf:li>
    </rdf:Bag>
   </dc:subject>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end='w'?>
"#;
        let sidecar = crate::sidecar::path_of(&raw);
        std::fs::write(&sidecar, lightroom).unwrap();
        let mut exiftool = None;
        let comment = Description {
            comment: "Gipfel".into(),
            keywords: vec!["Berg".into()],
        };
        write_marks(
            &mut exiftool,
            &raw,
            Some(Rating::Stars(5)),
            Some(Some(Label::Red)),
            Some(&comment),
        )
        .unwrap();
        drop(exiftool);
        let written = std::fs::read_to_string(&sidecar).unwrap();
        for kept in [
            "Exposure2012",
            "+0.50",
            "Contrast2012",
            "ProcessVersion",
            "MaskGroupBasedCorrections",
            "LocalExposure2012",
            "-0.35",
            "keep me",
            "Berg",
        ] {
            assert!(written.contains(kept), "{kept} lost:\n{written}");
        }
        let meta = metadata::read_sidecar(&raw);
        assert_eq!(meta.rating.value, Rating::Stars(5));
        assert_eq!(meta.label, LabelInfo::Known(Label::Red));
        assert_eq!(meta.description.comment, "Gipfel");
        assert_eq!(std::fs::read(&raw).unwrap(), untouched, "RAW untouched");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_description_comes_back_only_when_the_save_dropped_it() {
        let known = Description {
            comment: "Gipfel".into(),
            keywords: vec!["Berg".into()],
        };
        let none = Description::default();
        assert_eq!(lost_description(&none, &known), Some(known.clone()));
        // The program kept (or set) something of its own: that stays.
        let own = Description {
            comment: String::new(),
            keywords: vec!["Tal".into()],
        };
        assert_eq!(lost_description(&own, &known), None);
        // Nothing known, nothing to write.
        assert_eq!(lost_description(&none, &none), None);
    }

    /// End-to-end through a real ExifTool; skipped when ExifTool isn't installed.
    #[test]
    fn writes_stars_and_keeps_file_dates() {
        // Non-ASCII on purpose: needs `-charset filename=UTF8` on Windows.
        round_trip(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/tiny.jpg"
            )),
            "Überprüfung ä.jpg",
        );
    }

    /// The same on a real HEIC (XMP lives in a metadata item there, not in APP1):
    /// `CERNO_TEST_HEIC=<file.heic> cargo test -- --ignored heic`
    #[test]
    #[ignore = "needs a HEIC sample in CERNO_TEST_HEIC"]
    fn heic_rating_round_trip() {
        let source = std::env::var_os("CERNO_TEST_HEIC").expect("set CERNO_TEST_HEIC");
        round_trip(Path::new(&source), "Überprüfung ä.heic");
    }

    fn round_trip(source: &Path, name: &str) {
        use std::fs::{self, File, FileTimes};
        use std::time::SystemTime;

        if crate::exiftool::locate().is_none() {
            eprintln!("ExifTool not found – skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!(
            "cerno-rating-{}-{}",
            std::process::id(),
            name.len()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::copy(source, &path).unwrap();
        let old = SystemTime::now() - Duration::from_secs(86_400 * 400);
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(old))
            .unwrap();
        let before = fs::metadata(&path).unwrap();
        let mut exiftool = None;

        let read = || metadata::read(&fs::read(&path).unwrap());
        let on_disk = || read().rating.value;
        let on_label = || read().label;
        write_marks(
            &mut exiftool,
            &path,
            Some(Rating::Stars(4)),
            Some(Some(Label::Red)),
            None,
        )
        .unwrap();
        assert_eq!(on_disk(), Rating::Stars(4));
        assert_eq!(on_label(), LabelInfo::Known(Label::Red));
        write_marks(&mut exiftool, &path, Some(Rating::Stars(4)), None, None).unwrap(); // no-op
        write_marks(&mut exiftool, &path, Some(Rating::Rejected), None, None).unwrap();
        assert_eq!(on_disk(), Rating::Rejected);
        assert_eq!(on_label(), LabelInfo::Known(Label::Red));
        write_marks(
            &mut exiftool,
            &path,
            Some(Rating::Unrated),
            Some(None),
            None,
        )
        .unwrap();
        assert_eq!(on_disk(), Rating::Unrated);
        assert_eq!(on_label(), LabelInfo::None);

        // Comment (two lines, through ExifTool's C-string arguments) and umlaut keywords.
        let description = Description {
            comment: "Grüße aus Köln\nzweite Zeile".to_owned(),
            keywords: vec!["Straße".to_owned(), "Äpfel".to_owned()],
        };
        write_marks(&mut exiftool, &path, None, None, Some(&description)).unwrap();
        assert_eq!(read().description, description);
        assert_eq!(on_label(), LabelInfo::None, "other marks untouched");
        if name.ends_with(".jpg") {
            // IPTC too (HEIC has no IPTC block), as UTF-8.
            assert_eq!(
                metadata::iptc_description(&fs::read(&path).unwrap()),
                description
            );
        }
        let unchanged = fs::read(&path).unwrap();
        write_marks(&mut exiftool, &path, None, None, Some(&description)).unwrap();
        assert_eq!(fs::read(&path).unwrap(), unchanged, "same values: no write");
        write_marks(
            &mut exiftool,
            &path,
            None,
            None,
            Some(&Description::default()),
        )
        .unwrap();
        assert_eq!(read().description, Description::default());

        let after = fs::metadata(&path).unwrap();
        assert_eq!(after.modified().unwrap(), before.modified().unwrap());
        #[cfg(windows)]
        assert_eq!(after.created().unwrap(), before.created().unwrap());
        drop(exiftool);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A quarter turn with its original kept, a rating given afterwards, then Ctrl+Z: the
    /// orientation comes back, the rating stays, the dates never move.
    #[test]
    fn restore_brings_back_the_original_and_keeps_later_marks() {
        use std::fs::{self, File, FileTimes};
        use std::time::SystemTime;

        if crate::exiftool::locate().is_none() {
            eprintln!("ExifTool not found – skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!("cerno-restore-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Ärger.jpg");
        fs::copy(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/tiny.jpg"
            )),
            &path,
        )
        .unwrap();
        let old = SystemTime::now() - Duration::from_secs(86_400 * 400);
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(old))
            .unwrap();
        let db = Db::open_in_memory().unwrap();
        let mut exiftool = None;
        let read = || metadata::read(&fs::read(&path).unwrap());
        let before = read().orientation;

        let untouched = fs::read(&path).unwrap();
        // Two turns: only the first original is kept.
        for _ in 0..2 {
            crate::originals::keep(&db, &path).unwrap();
            apply_quarter_turn(&mut exiftool, &path, true, &db).unwrap();
        }
        assert_ne!(read().orientation, before);
        let kept = dir.join(crate::originals::FOLDER).join("Ärger.jpg");
        assert_eq!(fs::read(&kept).unwrap(), untouched);
        write_marks(&mut exiftool, &path, Some(Rating::Stars(4)), None, None).unwrap();

        restore_original(&mut exiftool, &path, &db).unwrap();
        let after = read();
        assert_eq!(after.orientation, before);
        assert_eq!(after.rating.value, Rating::Stars(4));
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old);
        assert_eq!(fs::read(&kept).unwrap(), untouched, "the original stays");
        restore_original(&mut exiftool, &path, &db).expect("again: same result");
        assert_eq!(read().orientation, before);
        drop(exiftool);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A re-encode keeps the ICC profile and the stored colours: an Adobe RGB photo must not
    /// come back as untagged sRGB.
    #[test]
    fn reencode_keeps_the_colour_profile() {
        use crate::decode::fixtures;
        use zune_core::bytestream::ZCursor;
        use zune_jpeg::JpegDecoder;

        if crate::exiftool::locate().is_none() {
            eprintln!("ExifTool not found – skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!("cerno-icc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("adobe.jpg");
        let rgb: Vec<u8> = (0..64 * 48).flat_map(|_| [30, 200, 60]).collect();
        let mut jpeg = fixtures::jpeg(64, 48, &rgb);
        let profile = fixtures::adobe_rgb_profile();
        fixtures::insert_icc(&mut jpeg, &profile);
        std::fs::write(&path, &jpeg).unwrap();
        let before = crate::decode::decode_for_edit(&jpeg, 1).unwrap();

        let db = Db::open_in_memory().unwrap();
        let mut exiftool = None;
        let rect = crate::edit::PixelRect {
            x: 8,
            y: 8,
            w: 40,
            h: 30,
        };
        let cropped = crate::edit::render_crop(&path, rect, &FileLocks::default()).unwrap();
        apply_pixels(&mut exiftool, &path, &cropped, &db).unwrap();

        let after = std::fs::read(&path).unwrap();
        let mut decoder = JpegDecoder::new(ZCursor::new(&after));
        decoder.decode_headers().unwrap();
        assert_eq!(decoder.icc_profile().as_deref(), Some(profile.as_slice()));
        let pixels = crate::decode::decode_for_edit(&after, 1).unwrap();
        let (centre_before, centre_after) = (
            &before.rgb[((24 * 64 + 32) * 3)..][..3],
            &pixels.rgb[((15 * 40 + 20) * 3)..][..3],
        );
        for (a, b) in centre_before.iter().zip(centre_after) {
            assert!(a.abs_diff(*b) <= 3, "{centre_before:?} → {centre_after:?}");
        }
        drop(exiftool);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Quarter turn via the orientation tag, then a real re-encode. Dates stay put either way.
    #[test]
    fn quarter_turn_and_reencode_keep_file_dates() {
        use std::fs::{self, File, FileTimes};
        use std::time::SystemTime;

        if crate::exiftool::locate().is_none() {
            eprintln!("ExifTool not found – skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!("cerno-edit-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tiny.jpg");
        fs::copy(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/tiny.jpg"
            )),
            &path,
        )
        .unwrap();
        let old = SystemTime::now() - Duration::from_secs(86_400 * 400);
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(old))
            .unwrap();
        let created = fs::metadata(&path).unwrap().created().ok();
        let db = Db::open_in_memory().unwrap();
        let mut exiftool = None;

        let orientation = || metadata::read(&fs::read(&path).unwrap()).orientation;
        let before = orientation();
        apply_quarter_turn(&mut exiftool, &path, true, &db).unwrap();
        assert_eq!(orientation(), crate::edit::rotate_orientation(before, 1));
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old);

        let bytes = fs::read(&path).unwrap();
        let meta = metadata::read(&bytes);
        let oriented = crate::decode::decode_for_display(
            &bytes,
            crate::library::Format::Jpeg,
            meta.orientation,
            [u32::MAX; 2],
        )
        .unwrap();
        let jpeg = crate::edit::render_rotation(&path, 2.0_f64.to_radians(), &FileLocks::default())
            .unwrap();
        apply_pixels(&mut exiftool, &path, &jpeg, &db).unwrap();
        let after = fs::read(&path).unwrap();
        let after_meta = metadata::read(&after);
        assert_eq!(after_meta.orientation, 1);
        let decoded = crate::decode::decode_for_display(
            &after,
            crate::library::Format::Jpeg,
            after_meta.orientation,
            [u32::MAX; 2],
        )
        .unwrap();
        assert_eq!(
            (decoded.width, decoded.height),
            (oriented.width, oriented.height)
        );
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old);
        #[cfg(windows)]
        assert_eq!(fs::metadata(&path).unwrap().created().ok(), created);
        #[cfg(not(windows))]
        let _ = created;

        drop(exiftool);
        fs::remove_dir_all(&dir).unwrap();
    }
}
