//! The writer thread: debounced marks per file, edits, what it reports to the app.

use super::*;

pub(super) struct Pending {
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

/// What the writer thread works with.
pub(super) struct Shared<'a> {
    pub(super) status: &'a Mutex<WriterStatus>,
    pub(super) outcomes: &'a Mutex<Vec<EditOutcome>>,
    pub(super) ctx: &'a egui::Context,
    pub(super) db: &'a Db,
    pub(super) files: &'a FileLocks,
}

pub(super) fn run(rx: &mpsc::Receiver<Message>, shared: &Shared<'_>) {
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
                allow_taste(db, &path);
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

pub(super) fn note_marks(
    db: &Db,
    status: &Mutex<WriterStatus>,
    path: &Path,
    result: Result<Option<Written>>,
) {
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

/// Writes a pending rating for `path` before a pixel edit, so the copied metadata includes it.
/// The index row is not updated: the edit deletes it afterwards.
/// The caller holds `path` for writing.
pub(super) fn flush_pending(
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
