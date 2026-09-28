//! Messages over the photo: hints, errors, "working" notes and the deletion countdown.

use std::time::{Duration, Instant};

use eframe::egui::{self, Rect};

use crate::i18n;
use crate::ui::bars;

use super::CernoApp;

/// How long a hint stays at least (longer ones a little longer), and how long it fades.
const NOTICE_TIME: Duration = Duration::from_secs(5);
const NOTICE_FADE: Duration = Duration::from_millis(400);

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
    /// The deletion countdown, then the notice – unless a failed rating write is reported,
    /// which wins as long as the writer reports it.
    pub(super) fn draw_messages(&mut self, ui: &egui::Ui, area: Rect) {
        if let Some((count, left)) = self.deletions.countdown(Instant::now()) {
            bars::delete_countdown(ui, area, count, left);
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
        if bars::notices(ui, area, message) && writer_error.is_none() {
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
}
