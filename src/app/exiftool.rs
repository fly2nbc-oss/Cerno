//! ExifTool from the app's side: whether one is there – marks and edits wait for it – the
//! download with its offer on Windows, and the install command on Linux.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::exiftool::{self as tool, Origin, ToolError};
use crate::i18n;
use crate::tools::EXIFTOOL;
use crate::ui::models::ExifToolRow;

use super::CernoApp;
use super::gate::{Blocked, Tool};
use super::menu::ConfirmAction;
use super::notice::Notice;
use crate::sync::lock;

/// Set once the hint about ExifTool has been shown (Windows, ExifTool missing).
const OFFER_SHOWN: &str = "exiftool_offer_shown";

/// A missing ExifTool is looked for again at most this often when a mark wants it – one
/// installed meanwhile (`apt install`) works without a restart.
const RECHECK: Duration = Duration::from_secs(1);

/// ExifTool starts this long after the first photo shows (`poll_exiftool`).
const PREPARE_AFTER: Duration = Duration::from_millis(1500);

/// Windows downloads ExifTool; Linux installs it from the system's packages.
const CAN_DOWNLOAD: bool = cfg!(windows);

pub(super) struct ExifToolSetup {
    /// Where `exiftool::locate` found one; `None`: missing.
    found: Option<Origin>,
    checked: Instant,
    /// The download card came up in this session; afterwards a hint says where it is.
    offered: bool,
    /// The writer was asked to start ExifTool (once the first photo shows).
    prepared: bool,
    download: Option<Download>,
    /// Linux: the command that installs ExifTool on this distribution.
    install_command: Option<&'static str>,
}

struct Download {
    progress: Arc<Mutex<Progress>>,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
struct Progress {
    received: u64,
    result: Option<Result<(), String>>,
}

impl ExifToolSetup {
    pub(super) fn new() -> Self {
        Self {
            found: tool::locate().map(|found| found.origin),
            checked: Instant::now(),
            offered: false,
            prepared: false,
            download: None,
            install_command: if CAN_DOWNLOAD {
                None
            } else {
                crate::system::exiftool_install_command(&crate::system::os_release())
            },
        }
    }

    /// One on `PATH`, already prepared (tests: marks pass the gate on every machine).
    #[cfg(test)]
    pub(super) fn found() -> Self {
        Self {
            found: Some(Origin::Path),
            checked: Instant::now(),
            offered: false,
            prepared: true,
            download: None,
            install_command: None,
        }
    }

    /// Ends a running download (on exit); its `.part` waits for the next attempt.
    pub(super) fn cancel(&self) {
        if let Some(download) = &self.download {
            download.cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl CernoApp {
    /// Whether ExifTool can write now (`gate`).
    pub(super) fn exiftool_tool(&self) -> Tool {
        if self.exiftool.download.is_some() {
            return Tool::Loading;
        }
        if self.exiftool.found.is_none() {
            return Tool::Missing;
        }
        match self.writer.status().tool_problem {
            Some(ToolError::TooOld(_)) => Tool::TooOld,
            _ => Tool::Ready,
        }
    }

    /// Looks for a missing ExifTool again (at most once a second); `true` once one is there.
    /// The writer starts it right away.
    pub(super) fn recheck_exiftool(&mut self) -> bool {
        if self.exiftool.found.is_some() {
            return true;
        }
        if self.exiftool.checked.elapsed() < RECHECK {
            return false;
        }
        self.exiftool.checked = Instant::now();
        self.exiftool.found = tool::locate().map(|found| found.origin);
        if self.exiftool.found.is_some() {
            self.writer.prepare();
        }
        self.exiftool.found.is_some()
    }

    /// Says why a mark or edit can't be written, with the way out: on Windows the download
    /// card (once a session, a hint afterwards), on Linux the install command.
    pub(super) fn offer_exiftool(&mut self, reason: Blocked) {
        let t = i18n::t();
        let fixable = matches!(reason, Blocked::NoExifTool | Blocked::ExifToolOld);
        if CAN_DOWNLOAD && fixable && !self.exiftool.offered {
            self.exiftool.offered = true;
            self.ask(ConfirmAction::DownloadExifTool, false);
            return;
        }
        let text = if !CAN_DOWNLOAD && reason == Blocked::NoExifTool {
            (t.exiftool_install)(self.exiftool.install_command)
        } else {
            reason.hint().to_owned()
        };
        self.notice = Some(Notice::hint(text));
    }

    /// Downloads ExifTool into the data folder, in the background (Windows).
    pub(super) fn download_exiftool(&mut self, ctx: &egui::Context) {
        if self.exiftool.download.is_some() {
            return;
        }
        let Some(dest) = tool::download_dir() else {
            return;
        };
        let progress = Arc::new(Mutex::new(Progress::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let (shared, stop, ctx) = (Arc::clone(&progress), Arc::clone(&cancel), ctx.clone());
        let spawned = std::thread::Builder::new()
            .name("cerno-exiftool-download".into())
            .spawn(move || {
                let mut last = Instant::now();
                let mut report = |received| {
                    if last.elapsed() > Duration::from_millis(200) {
                        lock(&shared).received = received;
                        ctx.request_repaint();
                        last = Instant::now();
                    }
                };
                let result = crate::tools::install_exiftool(&dest, &mut report, &|| {
                    stop.load(Ordering::Relaxed)
                });
                if let Err(err) = &result {
                    log::error!("ExifTool download: {err:#}");
                }
                lock(&shared).result = Some(result.map(|_| ()).map_err(|err| format!("{err:#}")));
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => self.exiftool.download = Some(Download { progress, cancel }),
            Err(err) => {
                self.notice = Some(Notice::error((i18n::t().exiftool_failed)(&err.to_string())));
            }
        }
    }

    /// The download's end. And a moment after the first photo shows, ExifTool starts ahead of
    /// the first mark: its version is known then (a too old one greys the marks out before one
    /// is set), and the first star needs no Perl start-up. Not sooner: the neighbours' decodes
    /// come first. A mark set before that starts it in the writer.
    pub(super) fn poll_exiftool(&mut self, ctx: &egui::Context) {
        if !self.exiftool.prepared
            && self.exiftool.found.is_some()
            && let Some(shown) = self.first_photo
        {
            let waited = shown.elapsed();
            if waited >= PREPARE_AFTER {
                self.exiftool.prepared = true;
                self.writer.prepare();
            } else {
                ctx.request_repaint_after(PREPARE_AFTER - waited);
            }
        }
        let Some(download) = &self.exiftool.download else {
            return;
        };
        let Some(result) = lock(&download.progress).result.take() else {
            return;
        };
        self.exiftool.download = None;
        let t = i18n::t();
        match result {
            Ok(()) => {
                self.exiftool.found = tool::locate().map(|found| found.origin);
                self.writer.prepare();
                self.notice = Some(Notice::hint(t.exiftool_ready));
            }
            Err(err) => self.notice = Some(Notice::error((t.exiftool_failed)(&err))),
        }
    }

    /// Models & data opens: a missing ExifTool is looked for once more, a found one started,
    /// so the card shows its version.
    pub(super) fn exiftool_card_opens(&mut self) {
        self.exiftool.checked = Instant::now()
            .checked_sub(RECHECK)
            .unwrap_or_else(Instant::now);
        if self.recheck_exiftool() {
            self.writer.prepare();
        }
    }

    /// The ExifTool row in Models & data.
    pub(super) fn exiftool_row(&self) -> ExifToolRow {
        if let Some(download) = &self.exiftool.download {
            let received = lock(&download.progress).received;
            return ExifToolRow::Downloading {
                percent: received as f32 * 100.0 / EXIFTOOL.bytes as f32,
            };
        }
        let status = self.writer.status();
        match (self.exiftool.found, status.tool_problem) {
            (None, _) => ExifToolRow::Missing {
                download: CAN_DOWNLOAD,
                install: (!CAN_DOWNLOAD)
                    .then(|| (i18n::t().exiftool_install)(self.exiftool.install_command)),
            },
            (Some(_), Some(ToolError::TooOld(version))) => ExifToolRow::TooOld {
                version,
                download: CAN_DOWNLOAD,
            },
            (Some(origin), _) => ExifToolRow::Ready {
                version: status.exiftool.unwrap_or_default(),
                downloaded: origin == Origin::Downloaded,
            },
        }
    }

    /// Once per installation, when a folder opens without ExifTool (Windows): where to get it.
    /// `true` when the hint came – the models' hint waits for the next folder.
    pub(super) fn offer_exiftool_once(&mut self) -> bool {
        if !CAN_DOWNLOAD
            || self.exiftool.found.is_some()
            || self.db.setting(OFFER_SHOWN).as_deref() == Some("1")
        {
            return false;
        }
        self.db.put_setting(OFFER_SHOWN, "1");
        let size = i18n::size(EXIFTOOL.bytes);
        self.notice = Some(Notice::hint((i18n::t().exiftool_offer)(&size)));
        true
    }
}
