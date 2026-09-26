//! Writes star ratings into the original files – in the background, debounced, with the file
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
use crate::metadata::{self, RatingInfo};

/// Pressing 3 and then 4 within this time results in a single write.
const DEBOUNCE: Duration = Duration::from_millis(400);

#[derive(Debug, Default, Clone)]
pub struct WriterStatus {
    pub pending: usize,
    pub last_error: Option<String>,
}

enum Message {
    Set { path: PathBuf, stars: Option<u8> },
    Shutdown,
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

    /// `None` removes the rating.
    pub fn set(&self, path: PathBuf, stars: Option<u8>) {
        let _ = self.tx.send(Message::Set { path, stars });
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
    let mut pending: HashMap<PathBuf, (Option<u8>, Instant)> = HashMap::new();
    let mut shutting_down = false;

    while !shutting_down {
        let timeout = if pending.is_empty() {
            Duration::from_secs(3600)
        } else {
            Duration::from_millis(50)
        };
        match rx.recv_timeout(timeout) {
            Ok(Message::Set { path, stars }) => {
                pending.insert(path, (stars, Instant::now()));
            }
            Ok(Message::Shutdown) | Err(RecvTimeoutError::Disconnected) => shutting_down = true,
            Err(RecvTimeoutError::Timeout) => {}
        }

        let due: Vec<PathBuf> = pending
            .iter()
            .filter(|(_, (_, changed))| shutting_down || changed.elapsed() >= DEBOUNCE)
            .map(|(path, _)| path.clone())
            .collect();
        for path in due {
            let Some((stars, _)) = pending.remove(&path) else {
                continue;
            };
            let result = write_rating(&mut exiftool, &path, stars);
            if result.is_ok() {
                // The size changed, the mtime didn't: keep the index valid without rehashing.
                let updated = FileStamp::of(&path)
                    .map_err(anyhow::Error::from)
                    .and_then(|stamp| {
                        db.update_after_rating_write(&path.to_string_lossy(), stamp, stars)
                    });
                if let Err(err) = updated {
                    log::warn!("index update for {}: {err:#}", path.display());
                }
            }
            if let Ok(mut status) = status.lock() {
                match result {
                    Ok(()) => status.last_error = None,
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

fn write_rating(exiftool: &mut Option<ExifTool>, path: &Path, stars: Option<u8>) -> Result<()> {
    let path_str = path.to_str().context("path is not valid Unicode")?;
    // Read what is in the file right now: skips no-op writes and tells which extra rating tags
    // (Windows Explorer's) need to be kept in sync.
    let bytes = std::fs::read(path).context("cannot read file")?;
    let on_disk = metadata::read(&bytes).rating;
    drop(bytes);
    if on_disk.stars == stars {
        return Ok(());
    }

    let snapshot = filetimes::Snapshot::capture(path).context("cannot read file times")?;
    let tool = match exiftool {
        Some(tool) => tool,
        None => exiftool.insert(ExifTool::spawn()?),
    };
    let args = rating_args(stars, &on_disk);
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
    log::info!("rating {:?} written to {}", stars, path.display());
    Ok(())
}

/// An empty value makes ExifTool delete the tag.
fn rating_args(stars: Option<u8>, on_disk: &RatingInfo) -> Vec<String> {
    let value = stars.map(|s| s.to_string()).unwrap_or_default();
    let percent = stars.map(|s| percent(s).to_string()).unwrap_or_default();
    let mut args = vec![format!("-XMP-xmp:Rating={value}")];
    if on_disk.has_exif_rating {
        args.push(format!("-EXIF:Rating={value}"));
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
            rating_args(Some(4), &RatingInfo::default()),
            ["-XMP-xmp:Rating=4"]
        );
        let windows = RatingInfo {
            stars: Some(2),
            has_exif_rating: true,
            has_ms_photo_rating: true,
        };
        assert_eq!(
            rating_args(Some(5), &windows),
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
            stars: Some(3),
            has_exif_rating: true,
            has_ms_photo_rating: false,
        };
        assert_eq!(
            rating_args(None, &windows),
            ["-XMP-xmp:Rating=", "-EXIF:Rating=", "-EXIF:RatingPercent="]
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
        let stars_on_disk = || metadata::read(&fs::read(&path).unwrap()).rating.stars;
        let mut exiftool = None;

        write_rating(&mut exiftool, &path, Some(4)).unwrap();
        assert_eq!(stars_on_disk(), Some(4));
        write_rating(&mut exiftool, &path, Some(4)).unwrap(); // no-op
        write_rating(&mut exiftool, &path, None).unwrap();
        assert_eq!(stars_on_disk(), None);

        let after = fs::metadata(&path).unwrap();
        assert_eq!(after.modified().unwrap(), before.modified().unwrap());
        #[cfg(windows)]
        assert_eq!(after.created().unwrap(), before.created().unwrap());
        drop(exiftool);
        fs::remove_dir_all(&dir).unwrap();
    }
}
