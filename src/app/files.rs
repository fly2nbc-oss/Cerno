//! Copy and move of the photos on screen, and deletion with the countdown.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::i18n;
use crate::library;
use crate::originals;
use crate::transfer::{Mode as TransferMode, Outcome as TransferOutcome};

use super::CernoApp;
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
        // Deleted photos (the 🗑 box) stay where they are.
        let sources: Vec<PathBuf> = self
            .view
            .paths
            .iter()
            .filter(|path| !self.is_deleted(path))
            .cloned()
            .collect();
        if sources.is_empty() {
            self.notice = Some(Notice::hint(t.no_match));
            return;
        }
        // A pair's RAW rides along with its JPEG.
        let riders = sources
            .iter()
            .filter_map(|jpeg| Some((jpeg.clone(), self.pairs.companion(jpeg)?.to_path_buf())))
            .collect();
        // A playing video keeps its file open; the job waits until it is closed.
        self.stop_video();
        self.transfers.push(mode, sources, riders, dest);
        self.poll_transfer(ctx);
    }

    pub(super) fn poll_transfer(&mut self, ctx: &egui::Context) {
        let videos_open = !self.videos_released();
        let writes_pending = self.writer.status().pending > 0
            || videos_open
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
        // While it waits for rating writes, and while it runs: the progress moves on its own.
        if waiting || self.transfers.is_busy() {
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
                self.session_descriptions.remove(src);
                self.pairs.forget(src);
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
            self.all = Arc::new(all);
            self.sync_library();
            self.rebuild_view(ctx, None);
        }
    }

    pub(super) fn retarget_moved(&mut self, outcome: &TransferOutcome) {
        if outcome.mode != TransferMode::Move {
            return;
        }
        self.listed_follow(&outcome.done);
        for (src, dest) in outcome.done.iter().chain(&outcome.riders) {
            // The kept original follows into `.originals` at the destination first.
            if let Err(err) = originals::follow(&self.db, src, dest) {
                log::warn!("kept original of {}: {err:#}", src.display());
            }
            if let Err(err) = self
                .db
                .retarget_path(&src.to_string_lossy(), &dest.to_string_lossy())
            {
                log::warn!("index: {err:#}");
            }
            // Moved into a folder that is shown too (subfolders): still among the best.
            if self.top_pick.remove(src) {
                self.top_pick.insert(dest.clone());
            }
        }
    }

    /// The photos on screen are set aside into `.originals` with the usual countdown – no question, `Esc`
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
        let shown: Vec<PathBuf> = self
            .view
            .iter()
            .filter(|path| !self.is_deleted(path))
            .cloned()
            .collect();
        for path in shown {
            self.queue_deletion(path, now);
        }
        self.rebuild_view(ctx, None);
    }

    /// Queues a photo for the countdown – a pair's RAW rides along, uncounted.
    fn queue_deletion(&mut self, path: PathBuf, now: Instant) {
        if let Some(raw) = self.pairs.companion(&path) {
            self.deletions.push_rider(raw.to_path_buf());
        }
        self.deletions.push(path, now);
    }

    /// All rejected photos are set aside – with the usual countdown, `Esc` brings them back.
    pub(super) fn delete_rejected(&mut self, ctx: &egui::Context) {
        if !self.allowed(Change::Delete, None) {
            return;
        }
        let now = Instant::now();
        for path in self.rejected() {
            self.queue_deletion(path, now);
        }
        self.rebuild_view(ctx, None);
    }

    /// Hides the photo at once; it moves into `.originals` when the countdown runs out.
    fn delete(&mut self, ctx: &egui::Context, path: PathBuf, keep: Option<PathBuf>) {
        self.queue_deletion(path, Instant::now());
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

    /// A photo was really set aside at `aside`: it teaches the taste model what the user
    /// doesn't like. Its first original stays in `.originals`, and so does the index row
    /// pointing at it; its own row moves along, so it can come back (`deleted`).
    pub(super) fn forget_deleted(&self, path: &Path, aside: &Path) {
        if let Err(err) = self
            .db
            .record_deletion(&path.to_string_lossy(), &aside.to_string_lossy())
        {
            log::warn!("index: {err:#}");
        }
    }

    /// Starts due deletions and applies finished ones.
    pub(super) fn process_deletions(&mut self, ctx: &egui::Context) {
        let repaint = ctx.clone();
        // A stopped video's file must be closed before it is set aside (Windows refuses to
        // move an open file); the countdown is long enough that this hardly ever waits.
        if self.videos_released() {
            self.deletions
                .tick(Instant::now(), move || repaint.request_repaint());
        } else {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        if let Some(done) = self.deletions.poll() {
            if !done.deleted.is_empty() {
                let gone: HashSet<&PathBuf> = done.deleted.iter().map(|(from, _)| from).collect();
                let all: Vec<PathBuf> = self
                    .all
                    .iter()
                    .filter(|p| !gone.contains(p))
                    .cloned()
                    .collect();
                for (path, aside) in &done.deleted {
                    self.pairs.forget(path);
                    self.session_ratings.remove(path);
                    self.session_labels.remove(path);
                    self.session_descriptions.remove(path);
                    self.forget_deleted(path, aside);
                    self.add_deleted(path.clone(), aside.clone());
                }
                self.analyzer.taste_changed();
                self.all = Arc::new(all);
                self.sync_library();
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
