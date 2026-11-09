//! Marks the writer could not write (ticket CER-57): the info bar counts them, a card names the
//! files and why and tries again, and closing Cerno first tries once more – with marks still
//! not written it asks before they are lost. The writer keeps them and tries again on its own
//! (`rating::unsaved`).

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Rect, ViewportCommand};

use crate::i18n;
use crate::library;
use crate::rating::WriterStatus;
use crate::ui::unsaved as card;

use super::CernoApp;
use super::layer::Layer;
use super::notice::Notice;

/// How long closing waits for the writer before the card shows (a write on a network drive
/// that hangs): the card then says it is still trying, and *Quit anyway* works.
const EXIT_WAIT: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(super) struct Unsaved {
    /// The files the hint has named already: a hint for each new one.
    told: HashSet<PathBuf>,
    /// The card asked for a try: once it has run, the card closes when nothing is left.
    card_retry: bool,
    /// Where closing Cerno stands.
    pub(super) exit: Exit,
}

/// Where closing Cerno stands.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum Exit {
    /// Not closing.
    #[default]
    Open,
    /// The window's close was held: the writer writes what waits and tries again.
    Writing { since: Instant },
    /// The exit card asks: try again, quit anyway or stay.
    Asking,
    /// Quitting: the next close goes through.
    Confirmed,
}

/// What the writer has to do, for the exit decision.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Writes {
    /// Marks waiting for their debounce (or a comment just committed).
    pub(super) pending: usize,
    /// Files whose marks could not be written.
    pub(super) unsaved: usize,
    /// A try the app asked for has not run yet.
    pub(super) retrying: bool,
    /// The writer thread has ended: nothing will be written any more.
    pub(super) stopped: bool,
}

impl Writes {
    fn of(status: &WriterStatus, stopped: bool) -> Self {
        Self {
            pending: status.pending,
            unsaved: status.unsaved.len(),
            retrying: status.retrying(),
            stopped,
        }
    }

    /// Nothing is being written.
    fn idle(self) -> bool {
        self.pending == 0 && !self.retrying
    }
}

/// What one frame does about closing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExitStep {
    /// Hold the window's close (`ViewportCommand::CancelClose`).
    pub(super) cancel: bool,
    /// Ask the writer to write everything now and try the unsaved marks again.
    pub(super) retry: bool,
    /// Show the exit card.
    pub(super) ask: bool,
    /// Close the window now.
    pub(super) close: bool,
}

/// The exit decision, pure: `close_requested` is this frame's close of the window. Exit stays
/// as fast as before when nothing waits or failed: the close goes through at once.
pub(super) fn exit_step(
    close_requested: bool,
    exit: Exit,
    writes: Writes,
    now: Instant,
) -> (ExitStep, Exit) {
    let step = ExitStep::default();
    if writes.stopped || exit == Exit::Confirmed {
        // Nothing will be written any more, or the user said quit.
        return (
            step,
            if close_requested {
                Exit::Confirmed
            } else {
                exit
            },
        );
    }
    match exit {
        Exit::Open if !close_requested => (step, exit),
        Exit::Open if writes.idle() && writes.unsaved == 0 => (step, Exit::Confirmed),
        // A mark set just before closing, or one not written: written now, while the window
        // still answers – `on_exit` could not ask any more.
        Exit::Open => (
            ExitStep {
                cancel: true,
                retry: true,
                ..step
            },
            Exit::Writing { since: now },
        ),
        Exit::Writing { since } => {
            let step = ExitStep {
                cancel: close_requested,
                ..step
            };
            if writes.idle() && writes.unsaved == 0 {
                (
                    ExitStep {
                        close: true,
                        ..step
                    },
                    Exit::Confirmed,
                )
            } else if writes.idle() || now.duration_since(since) >= EXIT_WAIT {
                (ExitStep { ask: true, ..step }, Exit::Asking)
            } else {
                (step, exit)
            }
        }
        Exit::Asking => {
            let step = ExitStep {
                cancel: close_requested,
                ..step
            };
            if writes.idle() && writes.unsaved == 0 {
                // The card's try wrote everything: Cerno closes as asked.
                (
                    ExitStep {
                        close: true,
                        ..step
                    },
                    Exit::Confirmed,
                )
            } else {
                (step, exit)
            }
        }
        Exit::Confirmed => (step, exit),
    }
}

impl CernoApp {
    /// Every frame: a hint for each newly failed file, the window's close, the card closing
    /// itself once everything is written.
    pub(super) fn poll_unsaved(&mut self, ctx: &egui::Context) {
        let status = self.writer.status();
        self.tell_new_failures(&status);

        let close_requested = ctx.input(|i| i.viewport().close_requested());
        let mut writes = Writes::of(&status, self.writer.stopped());
        if close_requested && self.unsaved.exit == Exit::Open {
            // A comment still in its field is written with the rest – before deciding.
            if self.drafts.comment_changed() {
                writes.pending += 1;
            }
            self.commit_comment();
        }
        // The exit card went away some other way: closing starts over.
        if self.unsaved.exit == Exit::Asking && !self.layer.is_unsaved() {
            self.unsaved.exit = Exit::Open;
        }
        let (step, exit) = exit_step(close_requested, self.unsaved.exit, writes, Instant::now());
        self.unsaved.exit = exit;
        if step.cancel {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        }
        if step.retry {
            self.writer.retry(None);
        }
        if step.ask {
            self.layer = Layer::Unsaved { exiting: true };
        }
        if step.close {
            self.layer.close_if(Layer::is_unsaved);
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if matches!(exit, Exit::Writing { .. }) || status.retrying() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        // The card closes on its own once nothing is left.
        if matches!(self.layer, Layer::Unsaved { exiting: false })
            && writes.idle()
            && status.unsaved.is_empty()
        {
            self.layer = Layer::None;
            if self.unsaved.card_retry {
                self.notice = Some(Notice::hint(i18n::t().unsaved_all_saved));
            }
        }
        if self.unsaved.card_retry && writes.idle() {
            self.unsaved.card_retry = false;
        }
    }

    /// A hint names a file whose marks newly failed – once; the info bar keeps counting.
    fn tell_new_failures(&mut self, status: &WriterStatus) {
        let new = status
            .unsaved
            .iter()
            .find(|unsaved| !self.unsaved.told.contains(&unsaved.path));
        if let Some(new) = new
            && !self.notice.as_ref().is_some_and(|notice| notice.error)
        {
            self.notice = Some(Notice::hint((i18n::t().rating_not_saved)(
                &library::file_name_lossy(&new.path),
            )));
        }
        // Written since: a new failure of the same file is news again.
        self.unsaved.told = status
            .unsaved
            .iter()
            .map(|unsaved| unsaved.path.clone())
            .collect();
    }

    /// How many files have marks that are not written – for the info bar.
    pub(super) fn unsaved_count(&self) -> usize {
        self.writer.status().unsaved.len()
    }

    /// Opens the card of the marks not written.
    pub(super) fn show_unsaved(&mut self) {
        self.layer = Layer::Unsaved { exiting: false };
    }

    /// Writes the marks for `paths` now, before a copy, move or deletion takes the files –
    /// only when one of them has marks not written.
    pub(super) fn retry_before(&self, paths: &[PathBuf]) {
        let status = self.writer.status();
        let involved: Vec<PathBuf> = paths
            .iter()
            .filter(|path| status.is_unsaved(path))
            .cloned()
            .collect();
        if !involved.is_empty() {
            self.writer.retry(Some(involved));
        }
    }

    /// The card: the files and why; *Try again*, *Close* – and *Quit anyway* while closing.
    pub(super) fn draw_unsaved_card(&mut self, ctx: &egui::Context, window: Rect) {
        let Layer::Unsaved { exiting } = self.layer else {
            return;
        };
        let status = self.writer.status();
        let files: Vec<card::File> = status
            .unsaved
            .iter()
            .map(|unsaved| card::File {
                name: library::file_name_lossy(&unsaved.path),
                path: unsaved.path.display().to_string(),
                error: unsaved.error.clone(),
            })
            .collect();
        let out = card::show(
            ctx,
            window,
            &card::Card {
                files: &files,
                exiting,
                retrying: status.retrying(),
            },
        );
        if out.retry {
            self.writer.retry(None);
            self.unsaved.card_retry = true;
        }
        if out.quit {
            self.unsaved.exit = Exit::Confirmed;
            self.layer = Layer::None;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else if out.close {
            self.layer = Layer::None;
            if exiting {
                self.unsaved.exit = Exit::Open;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn writes(pending: usize, unsaved: usize, retrying: bool) -> Writes {
        Writes {
            pending,
            unsaved,
            retrying,
            stopped: false,
        }
    }

    /// Nothing waits and nothing failed: the close goes through at once, as fast as before.
    #[test]
    fn a_clean_exit_is_not_held() {
        let now = Instant::now();
        let (step, exit) = exit_step(true, Exit::Open, writes(0, 0, false), now);
        assert_eq!(step, ExitStep::default());
        assert_eq!(exit, Exit::Confirmed);
        // No close, nothing happens.
        let (step, exit) = exit_step(false, Exit::Open, writes(0, 3, false), now);
        assert_eq!((step, exit), (ExitStep::default(), Exit::Open));
    }

    /// A mark set just before closing, or one not written: the close is held and everything
    /// written now; all written – the window closes on its own.
    #[test]
    fn waiting_marks_are_written_before_the_window_closes() {
        let now = Instant::now();
        for waiting in [writes(1, 0, false), writes(0, 2, false), writes(0, 0, true)] {
            let (step, exit) = exit_step(true, Exit::Open, waiting, now);
            assert!(step.cancel && step.retry && !step.ask && !step.close);
            assert_eq!(exit, Exit::Writing { since: now });
        }
        let writing = Exit::Writing { since: now };
        let later = now + Duration::from_millis(300);
        let (step, exit) = exit_step(false, writing, writes(0, 0, true), later);
        assert_eq!(
            (step, exit),
            (ExitStep::default(), writing),
            "still writing"
        );
        let (step, exit) = exit_step(false, writing, writes(0, 0, false), later);
        assert!(step.close);
        assert_eq!(exit, Exit::Confirmed);
        // The close that follows goes through.
        let (step, _) = exit_step(true, exit, writes(0, 0, false), later);
        assert!(!step.cancel);
    }

    /// Marks still not written after the try: the card asks. Its try writing everything closes
    /// the window; *Quit anyway* lets the next close through.
    #[test]
    fn marks_still_not_written_make_the_card_ask() {
        let now = Instant::now();
        let writing = Exit::Writing { since: now };
        let (step, exit) = exit_step(false, writing, writes(0, 2, false), now);
        assert!(step.ask && !step.close);
        assert_eq!(exit, Exit::Asking);
        // Another click on the window's ×: still held.
        let (step, exit) = exit_step(true, exit, writes(0, 2, false), now);
        assert!(step.cancel && !step.close && !step.retry);
        assert_eq!(exit, Exit::Asking);
        let (step, exit) = exit_step(false, exit, writes(0, 0, false), now);
        assert!(step.close);
        assert_eq!(exit, Exit::Confirmed);
        let (step, exit) = exit_step(true, Exit::Confirmed, writes(0, 2, false), now);
        assert_eq!((step, exit), (ExitStep::default(), Exit::Confirmed));
    }

    /// A write that hangs (a network drive) holds the exit only for `EXIT_WAIT`: then the card
    /// shows, still trying, with *Quit anyway*. A stopped writer never holds it.
    #[test]
    fn a_hanging_or_stopped_writer_does_not_keep_cerno_open() {
        let now = Instant::now();
        let writing = Exit::Writing { since: now };
        let (step, exit) = exit_step(false, writing, writes(0, 1, true), now + EXIT_WAIT);
        assert!(step.ask);
        assert_eq!(exit, Exit::Asking);
        let stopped = Writes {
            stopped: true,
            ..writes(2, 3, true)
        };
        let (step, exit) = exit_step(true, Exit::Open, stopped, now);
        assert_eq!((step, exit), (ExitStep::default(), Exit::Confirmed));
    }
}
