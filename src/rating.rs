//! Writes ratings (stars or "rejected") into the original files – in the background, debounced, with the file
//! dates preserved.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use eframe::egui;

use crate::db::{Db, FileStamp};
use crate::exiftool::ExifTool;
use crate::filelock::FileLocks;
use crate::filetimes;
use crate::metadata::{self, Label, LabelInfo, Rating, RatingInfo};

/// Pressing 3 and then 4 within this time results in a single write.
const DEBOUNCE: Duration = Duration::from_millis(400);

#[derive(Debug, Default, Clone)]
pub struct WriterStatus {
    pub pending: usize,
    pub last_error: Option<String>,
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
    at: Instant,
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
                match crate::backup::prune(&db) {
                    Ok(0) => {}
                    Ok(n) => log::info!("deleted {n} originals older than 30 days"),
                    Err(err) => log::warn!("backups: {err:#}"),
                }
                let backups = crate::backup::dir().ok();
                run(
                    &rx,
                    &Shared {
                        status: &thread_status,
                        outcomes: &thread_outcomes,
                        ctx: &ctx,
                        db: &db,
                        files: &files,
                        backups: backups.as_deref(),
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

    /// Clockwise or counter-clockwise quarter turn, written as EXIF orientation.
    pub fn rotate_quarter(&self, path: PathBuf, clockwise: bool) {
        let _ = self.tx.send(Message::RotateQuarter { path, clockwise });
    }

    /// Puts the newest kept original of `path` back (after any pending mark write).
    pub fn restore(&self, path: PathBuf) {
        let _ = self.tx.send(Message::Restore { path });
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
    backups: Option<&'a Path>,
}

fn run(rx: &mpsc::Receiver<Message>, shared: &Shared<'_>) {
    let Shared {
        status,
        outcomes,
        ctx,
        db,
        files,
        backups,
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
                let entry = pending.entry(path).or_insert_with(|| Pending {
                    rating: None,
                    label: None,
                    at: Instant::now(),
                });
                entry.rating = Some(rating);
                entry.at = Instant::now();
            }
            Ok(Message::SetLabel { path, label }) => {
                files.set_queued(&path, true);
                let entry = pending.entry(path).or_insert_with(|| Pending {
                    rating: None,
                    label: None,
                    at: Instant::now(),
                });
                entry.label = Some(label);
                entry.at = Instant::now();
            }
            // Every edit keeps the original first; without that copy it does not happen.
            Ok(Message::RotateQuarter { path, clockwise }) => {
                let held = files.hold_write(&path);
                let result = flush_pending(&mut pending, &mut exiftool, &path, status, files)
                    .and_then(|()| keep_original(db, backups, &path))
                    .and_then(|()| apply_quarter_turn(&mut exiftool, &path, clockwise, db));
                drop(held);
                push_outcome(outcomes, path, result, Done::Rotated);
            }
            Ok(Message::ReplacePixels { path, jpeg }) => {
                let held = files.hold_write(&path);
                let result = flush_pending(&mut pending, &mut exiftool, &path, status, files)
                    .and_then(|()| keep_original(db, backups, &path))
                    .and_then(|()| apply_pixels(&mut exiftool, &path, &jpeg, db));
                drop(held);
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
                push_outcome(outcomes, path, result, Done::Restored);
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
            let result = write_marks(&mut exiftool, &path, marks.rating, marks.label);
            files.set_queued(&path, false);
            drop(held);
            if let Ok(Some(written)) = &result {
                // The size changed, the mtime didn't: keep the index valid without rehashing.
                let updated = FileStamp::of(&path)
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
                    Err(err) => {
                        log::error!("rating for {}: {err:#}", path.display());
                        status.last_error = Some(format!(
                            "{}: {err:#}",
                            crate::library::file_name_lossy(&path)
                        ));
                    }
                }
            }
        }

        if let Ok(mut status) = status.lock() {
            status.pending = pending.len();
        }
        ctx.request_repaint();
    }
    // Dropping `exiftool` ends the stay-open process.
}

struct Written {
    rating: Rating,
    label: Option<Label>,
}

/// Writes the rating and/or colour label that changed. `Ok(None)` when the file already
/// matches, so the dates are not touched.
fn write_marks(
    exiftool: &mut Option<ExifTool>,
    path: &Path,
    rating: Option<Rating>,
    label: Option<Option<Label>>,
) -> Result<Option<Written>> {
    let path_str = path.to_str().context("path is not valid Unicode")?;
    // Read what is in the file right now: skips no-op writes and tells which extra rating tags
    // (Windows Explorer's) need to be kept in sync.
    let bytes = std::fs::read(path).context("cannot read file")?;
    let meta = metadata::read(&bytes);
    drop(bytes);

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
    if args.is_empty() {
        return Ok(None);
    }

    let snapshot = filetimes::Snapshot::capture(path).context("cannot read file times")?;
    let tool = match exiftool {
        Some(tool) => tool,
        None => exiftool.insert(ExifTool::spawn()?),
    };
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
        .restore(path)
        .context("cannot restore file times")?
    {
        log::debug!("restored exact file times of {}", path.display());
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
        path.display()
    );
    Ok(Some(Written {
        rating: written_rating,
        label: written_label,
    }))
}

fn label_needs_write(on_disk: LabelInfo, wanted: Option<Label>) -> bool {
    match (on_disk, wanted) {
        (LabelInfo::Known(have), Some(want)) => have != want,
        (LabelInfo::None, None) => false,
        _ => true,
    }
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
    let result = write_marks(exiftool, path, marks.rating, marks.label);
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
        });
    }
}

/// Copies the file into the backup folder before an edit (see `backup`).
fn keep_original(db: &Db, backups: Option<&Path>, path: &Path) -> Result<()> {
    let dir = backups.context("no folder for the kept originals")?;
    crate::backup::keep(db, dir, path)?;
    Ok(())
}

/// `Ctrl+Z`: writes the newest kept original back into the file – in place, so the file keeps
/// its identity and dates – then the rating and colour label the file has now, so marks set
/// after the edit stay. The copy is used up.
fn restore_original(exiftool: &mut Option<ExifTool>, path: &Path, db: &Db) -> Result<()> {
    let key = path.to_string_lossy();
    let (id, copy) = db
        .latest_backup(&key)?
        .context("no original kept for this photo")?;
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
    write_marks(exiftool, path, Some(now.rating.value), label)?;
    db.forget_file(&key).context("cannot drop the index row")?;
    db.drop_backup(id)?;
    if let Err(err) = std::fs::remove_file(&copy) {
        log::warn!("kept original {copy}: {err}");
    }
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
        )
        .unwrap();
        assert_eq!(on_disk(), Rating::Stars(4));
        assert_eq!(on_label(), LabelInfo::Known(Label::Red));
        write_marks(&mut exiftool, &path, Some(Rating::Stars(4)), None).unwrap(); // no-op
        write_marks(&mut exiftool, &path, Some(Rating::Rejected), None).unwrap();
        assert_eq!(on_disk(), Rating::Rejected);
        assert_eq!(on_label(), LabelInfo::Known(Label::Red));
        write_marks(&mut exiftool, &path, Some(Rating::Unrated), Some(None)).unwrap();
        assert_eq!(on_disk(), Rating::Unrated);
        assert_eq!(on_label(), LabelInfo::None);

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

        keep_original(&db, Some(&dir.join("backups")), &path).unwrap();
        apply_quarter_turn(&mut exiftool, &path, true, &db).unwrap();
        assert_ne!(read().orientation, before);
        write_marks(&mut exiftool, &path, Some(Rating::Stars(4)), None).unwrap();

        restore_original(&mut exiftool, &path, &db).unwrap();
        let after = read();
        assert_eq!(after.orientation, before);
        assert_eq!(after.rating.value, Rating::Stars(4));
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old);
        assert!(db.latest_backup(&path.to_string_lossy()).unwrap().is_none());
        assert!(
            restore_original(&mut exiftool, &path, &db).is_err(),
            "used up"
        );
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
