//! Copy and move of the photos on screen, and deletion with the countdown.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::i18n;
use crate::library;
use crate::transfer::{Mode as TransferMode, Outcome as TransferOutcome};

use super::CernoApp;
use super::browse::index_of;
use super::gate::Change;
use super::notice::Notice;

fn same_folder(open: &Path, dest: &Path) -> bool {
    match (open.canonicalize(), dest.canonicalize()) {
        (Ok(open), Ok(dest)) => open == dest,
        _ => open == dest,
    }
}

impl CernoApp {
    /// Copies or moves every photo the filter currently shows. Choosing the folder is the
    /// confirmation – no extra question. The folder dialog blocks, like opening a folder; the
    /// files themselves move on a background thread once pending rating writes have finished.
    pub(super) fn begin_transfer(&mut self, ctx: &egui::Context, mode: TransferMode) {
        let t = i18n::t();
        if self.view.is_empty() {
            self.notice = Some(Notice::hint(t.no_match));
            return;
        }
        if self.transfers.is_busy() {
            self.notice = Some(Notice::hint(t.transfer_busy));
            return;
        }
        if !self.allowed(Change::Transfer, None) {
            return;
        }
        let title = match mode {
            TransferMode::Copy => t.transfer_copy_cmd,
            TransferMode::Move => t.transfer_move_cmd,
        };
        let mut dialog = rfd::FileDialog::new().set_title(title);
        if let Some(dir) = &self.dir {
            dialog = dialog.set_directory(dir);
        }
        let Some(dest) = dialog.pick_folder() else {
            return;
        };
        if self
            .dir
            .as_deref()
            .is_some_and(|dir| same_folder(dir, &dest))
        {
            self.notice = Some(Notice::hint(t.transfer_same_folder));
            return;
        }
        let sources = self.view.paths.iter().cloned().collect();
        self.transfers.push(mode, sources, dest);
        self.poll_transfer(ctx);
    }

    pub(super) fn poll_transfer(&mut self, ctx: &egui::Context) {
        let writes_pending = self.writer.status().pending > 0
            || self
                .edit_thread
                .as_ref()
                .is_some_and(|thread| !thread.is_finished());
        let repaint = ctx.clone();
        let waiting = self
            .transfers
            .kick(writes_pending, move || repaint.request_repaint());
        if let Some(outcome) = self.transfers.poll() {
            self.finish_transfer(ctx, outcome);
        }
        if waiting {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn finish_transfer(&mut self, ctx: &egui::Context, outcome: TransferOutcome) {
        let t = i18n::t();
        let (name, err) = outcome
            .failed
            .first()
            .map(|(path, err)| (library::file_name_lossy(path), err.clone()))
            .unwrap_or_default();
        let text = (t.transfer_done)(
            outcome.mode == TransferMode::Move,
            outcome.done.len(),
            outcome.skipped.len(),
            &name,
            &err,
        );
        self.notice = Some(if outcome.failed.is_empty() {
            Notice::hint(text)
        } else {
            Notice::error(text)
        });
        if outcome.mode == TransferMode::Move && !outcome.done.is_empty() {
            self.retarget_moved(&outcome);
            let gone: HashSet<&PathBuf> = outcome.done.iter().map(|(src, _)| src).collect();
            for (src, _) in &outcome.done {
                self.session_ratings.remove(src);
                self.session_labels.remove(src);
            }
            if self.pinned.as_ref().is_some_and(|path| gone.contains(path)) {
                self.pinned = None;
            }
            let all: Vec<PathBuf> = self
                .all
                .iter()
                .filter(|path| !gone.contains(*path))
                .cloned()
                .collect();
            self.all_index = index_of(&all);
            self.all = Arc::new(all);
            let current = self
                .view
                .get(self.current)
                .and_then(|path| self.all_index.get(path))
                .copied()
                .unwrap_or(0);
            self.analyzer.set_library(Arc::clone(&self.all), current);
            self.rebuild_view(ctx, None);
        }
    }

    pub(super) fn retarget_moved(&self, outcome: &TransferOutcome) {
        if outcome.mode != TransferMode::Move {
            return;
        }
        for (src, dest) in &outcome.done {
            if let Err(err) = self
                .db
                .retarget_path(&src.to_string_lossy(), &dest.to_string_lossy())
            {
                log::warn!("index: {err:#}");
            }
        }
    }

    /// The photos on screen go to the trash with the usual countdown – no question, `Esc`
    /// brings them all back, like `Delete` for a single photo.
    pub(super) fn delete_selection(&mut self, ctx: &egui::Context) {
        if self.view.is_empty() {
            self.notice = Some(Notice::hint(i18n::t().no_match));
            return;
        }
        if !self.allowed(Change::Delete, None) {
            return;
        }
        let now = Instant::now();
        for path in self.view.paths.iter().cloned() {
            self.deletions.push(path, now);
        }
        self.rebuild_view(ctx, None);
    }

    /// All rejected photos go to the trash – with the usual countdown, `Esc` brings them back.
    pub(super) fn delete_rejected(&mut self, ctx: &egui::Context) {
        if !self.allowed(Change::Delete, None) {
            return;
        }
        let now = Instant::now();
        for path in self.rejected() {
            self.deletions.push(path, now);
        }
        self.rebuild_view(ctx, None);
    }

    /// Hides the photo at once; it goes to the trash when the countdown runs out.
    fn delete(&mut self, ctx: &egui::Context, path: PathBuf, keep: Option<PathBuf>) {
        self.deletions.push(path, Instant::now());
        self.rebuild_view(ctx, keep);
    }

    pub(super) fn delete_current(&mut self, ctx: &egui::Context) {
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        if !self.allowed(Change::Delete, Some(&path)) {
            return;
        }
        let keep = self.neighbour(self.current, &[&path]);
        self.delete(ctx, path, keep);
    }

    /// Esc while the countdown runs: every waiting photo comes back.
    pub(super) fn undo_deletions(&mut self, ctx: &egui::Context) {
        if self.deletions.cancel() > 0 {
            self.rebuild_view(ctx, None);
        }
    }

    /// Starts due deletions and applies finished ones.
    pub(super) fn process_deletions(&mut self, ctx: &egui::Context) {
        let repaint = ctx.clone();
        self.deletions
            .tick(Instant::now(), move || repaint.request_repaint());
        if let Some(done) = self.deletions.poll() {
            if !done.deleted.is_empty() {
                let gone: HashSet<&PathBuf> = done.deleted.iter().collect();
                let all: Vec<PathBuf> = self
                    .all
                    .iter()
                    .filter(|p| !gone.contains(p))
                    .cloned()
                    .collect();
                for path in &done.deleted {
                    self.session_ratings.remove(path);
                    // Deleted photos teach the taste model what the user doesn't like.
                    if let Err(err) = self.db.record_deletion(&path.to_string_lossy()) {
                        log::warn!("index: {err:#}");
                    }
                }
                self.analyzer.taste_changed();
                self.all_index = index_of(&all);
                self.all = Arc::new(all);
                let current = self
                    .view
                    .get(self.current)
                    .and_then(|p| self.all_index.get(p))
                    .copied()
                    .unwrap_or(0);
                self.analyzer.set_library(Arc::clone(&self.all), current);
            }
            if let Some((path, err)) = done.failed.first() {
                self.notice = Some(Notice::error((i18n::t().delete_failed)(
                    done.failed.len(),
                    &library::file_name_lossy(path),
                    err,
                )));
            }
            self.rebuild_view(ctx, None);
        }
        if self.deletions.countdown(Instant::now()).is_some() {
            // Animates the countdown bar and makes sure it fires without input.
            ctx.request_repaint();
        }
    }
}
