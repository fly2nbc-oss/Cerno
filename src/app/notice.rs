//! Messages over the photo: hints, errors, "working" notes, the deletion countdown and the
//! copy progress.

use std::time::{Duration, Instant};

use eframe::egui::{self, Rect};

use crate::i18n;
use crate::library;
use crate::transfer::{Mode as TransferMode, Snapshot};
use crate::ui::overlays;

use super::CernoApp;

/// How long a hint stays at least (longer ones a little longer), and how long it fades.
const NOTICE_TIME: Duration = Duration::from_secs(5);
const NOTICE_FADE: Duration = Duration::from_millis(400);

/// A file this large gets its name in the copy progress: it takes a while on its own, and
/// the bar only moves once it is done.
const BIG_FILE: u64 = 100_000_000;

/// `Copying 12 / 340 photos – 1.2 / 8.4 GB`, plus the name of a large file in progress.
fn transfer_text(mode: TransferMode, progress: &Snapshot) -> String {
    let at = (progress.files_done + 1).min(progress.files_total);
    // Until the worker starts (it waits for rating writes) the sizes are not known.
    let sizes = if progress.bytes_total == 0 && progress.files_done == 0 {
        "…".to_owned()
    } else {
        i18n::sizes(progress.bytes_done, progress.bytes_total)
    };
    let file = match &progress.current {
        Some((path, size)) if *size >= BIG_FILE => {
            format!("{} ({})", library::file_name_lossy(path), i18n::size(*size))
        }
        _ => String::new(),
    };
    (i18n::t().transfer_progress)(
        mode == TransferMode::Move,
        at,
        progress.files_total,
        &sizes,
        &file,
    )
}

/// A message over the photo. Hints fade on their own; errors stay until Esc or a click;
/// "working" messages stay until the work is done.
pub(super) struct Notice {
    pub(super) text: String,
    pub(super) error: bool,
    /// When a hint is gone; `None` keeps the message.
    until: Option<Instant>,
}

impl Notice {
    pub(super) fn hint(text: impl Into<String>) -> Self {
        let text = text.into();
        let reading = Duration::from_millis(60 * text.chars().count() as u64);
        Self {
            until: Some(Instant::now() + NOTICE_TIME.max(reading).min(NOTICE_TIME * 2)),
            text,
            error: false,
        }
    }

    pub(super) fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: true,
            until: None,
        }
    }

    pub(super) fn working(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: false,
            until: None,
        }
    }

    /// 1 while shown, fading to 0 at the end; `None` once gone.
    fn opacity(&self, now: Instant) -> Option<f32> {
        let Some(until) = self.until else {
            return Some(1.0);
        };
        let left = until.checked_duration_since(now)?;
        Some((left.as_secs_f32() / NOTICE_FADE.as_secs_f32()).min(1.0))
    }
}

impl CernoApp {
    /// The deletion countdown and the copy progress – stacked above a video's play button –
    /// then the notice, unless a failed rating write is reported, which wins as long as the
    /// writer reports it.
    pub(super) fn draw_messages(&mut self, ui: &egui::Ui, area: Rect) {
        let mut slot = usize::from(self.video_on_screen(area));
        if let Some((count, left)) = self.deletions.countdown(Instant::now()) {
            overlays::delete_countdown(ui, overlays::bottom_slot(area, slot), count, left);
            slot += 1;
        }
        if let Some((mode, progress)) = self.transfers.progress() {
            overlays::transfer_progress(
                ui,
                overlays::bottom_slot(area, slot),
                transfer_text(mode, &progress),
                progress.fraction(),
            );
        }
        let writer_error = self
            .writer
            .status()
            .last_error
            .map(|e| (i18n::t().rating_not_saved)(&e));
        let now = Instant::now();
        let opacity = self.notice.as_ref().and_then(|n| n.opacity(now));
        if opacity.is_none() {
            self.notice = None;
        }
        let message = match (&writer_error, &self.notice, opacity) {
            (Some(err), _, _) => Some((err.as_str(), true, 1.0)),
            (None, Some(notice), Some(opacity)) => {
                Some((notice.text.as_str(), notice.error, opacity))
            }
            _ => None,
        };
        if overlays::notices(ui, area, message) && writer_error.is_none() {
            self.notice = None;
        }
        if let Some(until) = self.notice.as_ref().and_then(|n| n.until) {
            // Wake up for the fade and to remove the hint.
            let left = until.saturating_duration_since(now);
            ui.ctx().request_repaint_after(
                left.saturating_sub(NOTICE_FADE)
                    .max(Duration::from_millis(16)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_fade_and_errors_stay() {
        let now = Instant::now();
        let hint = Notice::hint("3 copied, 0 skipped");
        assert_eq!(hint.opacity(now), Some(1.0));
        assert!(
            hint.opacity(now + NOTICE_TIME - NOTICE_FADE / 2)
                .is_some_and(|o| o < 1.0)
        );
        assert_eq!(hint.opacity(now + NOTICE_TIME * 3), None);
        let error = Notice::error("Could not delete 1 photo");
        assert_eq!(error.opacity(now + NOTICE_TIME * 3), Some(1.0));
        let working = Notice::working("Writing the photo…");
        assert_eq!(working.opacity(now + NOTICE_TIME * 3), Some(1.0));
    }

    /// Tests never switch the language, so the texts are English.
    #[test]
    fn the_copy_progress_names_the_photo_and_a_large_file() {
        let waiting = Snapshot {
            files_total: 340,
            ..Snapshot::default()
        };
        assert_eq!(
            transfer_text(TransferMode::Copy, &waiting),
            "Copying 1 / 340 photos – …"
        );
        let running = Snapshot {
            files_total: 340,
            files_done: 11,
            bytes_total: 8_400_000_000,
            bytes_done: 1_200_000_000,
            current: Some(("D:/100CANON/MVI_0001.MP4".into(), 4_100_000_000)),
        };
        assert_eq!(
            transfer_text(TransferMode::Move, &running),
            "Moving 12 / 340 photos – 1.2 / 8.4 GB – MVI_0001.MP4 (4.1 GB)"
        );
        let small = Snapshot {
            current: Some(("D:/IMG_1.JPG".into(), 5_000_000)),
            ..running
        };
        assert!(!transfer_text(TransferMode::Copy, &small).contains("IMG_1"));
        let done = Snapshot {
            files_done: 340,
            current: None,
            ..small
        };
        assert!(transfer_text(TransferMode::Copy, &done).starts_with("Copying 340 / 340"));
    }
}
