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
    SetRating { path: PathBuf, rating: Rating },
    SetLabel { path: PathBuf, label: Option<Label> },
    Shutdown,
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
}

impl RatingWriter {
    pub fn new(ctx: egui::Context, db: Arc<Db>) -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(WriterStatus::default()));
        let thread_status = Arc::clone(&status);
        let thread = std::thread::Builder::new()
            .name("cerno-rating-writer".into())
            .spawn(move || run(&rx, &thread_status, &ctx, &db))
            .expect("failed to spawn rating writer");
        Self {
            tx,
            thread: Some(thread),
            status,
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

    pub fn status(&self) -> WriterStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
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

fn run(rx: &mpsc::Receiver<Message>, status: &Mutex<WriterStatus>, ctx: &egui::Context, db: &Db) {
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
                let entry = pending.entry(path).or_insert_with(|| Pending {
                    rating: None,
                    label: None,
                    at: Instant::now(),
                });
                entry.rating = Some(rating);
                entry.at = Instant::now();
            }
            Ok(Message::SetLabel { path, label }) => {
                let entry = pending.entry(path).or_insert_with(|| Pending {
                    rating: None,
                    label: None,
                    at: Instant::now(),
                });
                entry.label = Some(label);
                entry.at = Instant::now();
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
            let result = write_marks(&mut exiftool, &path, marks.rating, marks.label);
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
