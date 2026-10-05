//! Deleted photos: they lie in the `.originals` folder beside their folder (`originals`). The
//! filter bar's 🗑 box shows them, `Ctrl+Z` and the menus put them back. Found on a thread when
//! a folder opens and kept up to date by deleting and putting back – never read per frame.
//! While shown they are part of the library the analysis and the loader see (thumbnails,
//! scores); otherwise they cost nothing. A deleted RAW + JPG pair is one photo here too, paired
//! by the names they had (never with a live photo), and goes back together.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use eframe::egui;

use crate::i18n;
use crate::library;
use crate::originals;
use crate::pairs::{self, Pairs};
use crate::view::FilterKind;

use super::CernoApp;
use super::browse::index_of;
use super::gate::Change;
use super::notice::Notice;

/// What a restore worker did: (where it lay, where it came from, where it is now), and what
/// failed; a pair's RAW with the place its JPEG got.
#[derive(Default)]
struct Restored {
    done: Vec<(PathBuf, PathBuf, PathBuf)>,
    failed: Vec<(PathBuf, String)>,
    riders: Vec<Rider>,
}

/// A deleted pair's RAW that went back with its JPEG.
struct Rider {
    aside: PathBuf,
    original: PathBuf,
    to: PathBuf,
    /// Where its JPEG is now.
    primary: PathBuf,
}

/// One photo to put back: where it lies, where it came from, and its RAW if it is a pair.
struct Job {
    aside: PathBuf,
    original: PathBuf,
    rider: Option<(PathBuf, PathBuf)>,
}

#[derive(Default)]
pub(super) struct Deleted {
    /// Where each photo lies now → where it came from.
    from: HashMap<PathBuf, PathBuf>,
    /// Where they lie, in the order of their original names – a pair's RAW not among them.
    paths: Vec<PathBuf>,
    /// The deleted pairs: the RAW lying beside each deleted JPEG.
    pairs: Pairs,
    /// The scan of the open folder's `.originals`.
    scan: Option<mpsc::Receiver<Vec<originals::Deleted>>>,
    /// Waiting to go back: a playing video closes its file first.
    queued: Vec<PathBuf>,
    /// Where to go when the photo put back leaves the view.
    after: Option<PathBuf>,
    restoring: Option<mpsc::Receiver<Restored>>,
}

impl Deleted {
    pub(super) fn contains(&self, path: &Path) -> bool {
        self.from.contains_key(path)
    }

    /// The RAW lying beside a deleted JPEG of a pair.
    pub(super) fn companion(&self, aside: &Path) -> Option<&Path> {
        self.pairs.companion(aside)
    }

    pub(super) fn original_of(&self, path: &Path) -> Option<&Path> {
        self.from.get(path).map(PathBuf::as_path)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    fn set(&mut self, found: Vec<originals::Deleted>, pair: bool) {
        self.from = found.into_iter().map(|d| (d.aside, d.original)).collect();
        self.arrange(pair);
    }

    fn add(&mut self, aside: PathBuf, original: PathBuf, pair: bool) {
        self.from.insert(aside, original);
        self.arrange(pair);
    }

    fn remove(&mut self, aside: &Path) {
        self.from.remove(aside);
        self.paths.retain(|p| p != aside);
        self.pairs.forget(aside);
    }

    /// In the order of their original names; pairs by those names too.
    fn arrange(&mut self, pair: bool) {
        let from = &self.from;
        let mut all: Vec<PathBuf> = from.keys().cloned().collect();
        all.sort_by(|a, b| {
            let name = |p: &PathBuf| from.get(p).unwrap_or(p).to_string_lossy().into_owned();
            library::natural_cmp(&name(a), &name(b)).then_with(|| a.cmp(b))
        });
        (self.paths, self.pairs) = if pair {
            pairs::pair_up(all, |p| {
                from.get(p).cloned().unwrap_or_else(|| p.to_path_buf())
            })
        } else {
            (all, Pairs::default())
        };
    }
}

impl CernoApp {
    /// Opening a folder: its deleted photos are looked for on a thread (a big `.originals`
    /// must not delay the first photo).
    pub(super) fn scan_deleted(&mut self) {
        self.deleted = Deleted::default();
        let Some(dir) = self.dir.clone() else {
            return;
        };
        let (db, subfolders) = (Arc::clone(&self.db), self.subfolders);
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("cerno-deleted".into())
            .spawn(move || {
                let started = std::time::Instant::now();
                let found = originals::deleted_in(&db, &library::folders(&dir, subfolders));
                log::info!(
                    "deleted photos: {} in {} ms",
                    found.len(),
                    started.elapsed().as_millis()
                );
                let _ = tx.send(found);
            });
        match spawned {
            Ok(_) => self.deleted.scan = Some(rx),
            Err(err) => log::warn!("deleted photos: {err}"),
        }
    }

    /// Whether `path` is a deleted photo (in `.originals`).
    pub(super) fn is_deleted(&self, path: &Path) -> bool {
        self.deleted.contains(path)
    }

    /// Whether the open folder has deleted photos – the 🗑 box is greyed out otherwise.
    pub(super) fn has_deleted(&self) -> bool {
        !self.deleted.is_empty()
    }

    /// The photos sorting and filtering look at: the folder, and its deleted photos while the
    /// 🗑 box is ticked. The analysis follows, so the deleted ones get thumbnails and scores.
    pub(super) fn sync_library(&mut self) {
        let shown = self.options.filter.contains(FilterKind::Deleted) && self.has_deleted();
        let library = if shown {
            let mut all = self.all.to_vec();
            all.extend(self.deleted.paths.iter().cloned());
            Arc::new(all)
        } else {
            Arc::clone(&self.all)
        };
        let current = self.view.get(self.current).cloned();
        self.all_index = index_of(&library);
        let index = current
            .and_then(|path| self.all_index.get(&path).copied())
            .unwrap_or(0);
        if *library != *self.library {
            self.analyzer.set_library(Arc::clone(&library), index);
        }
        self.library = library;
    }

    /// A deletion was carried out: the photo now lies at `aside`.
    pub(super) fn add_deleted(&mut self, original: PathBuf, aside: PathBuf) {
        self.deleted.add(aside, original, self.pair_mode);
    }

    /// `Ctrl+Z` or the menu on a deleted photo: it goes back into its folder.
    pub(super) fn restore_current(&mut self) {
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        if !self.is_deleted(&path) {
            return;
        }
        self.deleted.after = self.neighbour(self.current, &[&path]);
        self.restore(vec![path]);
    }

    /// The deleted photos the filter shows go back ("Put back (n photos)").
    pub(super) fn restore_shown(&mut self) {
        let paths: Vec<PathBuf> = self
            .view
            .iter()
            .filter(|path| self.is_deleted(path))
            .cloned()
            .collect();
        self.deleted.after = self
            .view
            .iter()
            .find(|path| !self.is_deleted(path))
            .cloned();
        self.restore(paths);
    }

    /// The deleted photos the view shows.
    pub(super) fn deleted_shown(&self) -> usize {
        self.view
            .iter()
            .filter(|path| self.is_deleted(path))
            .count()
    }

    fn restore(&mut self, mut paths: Vec<PathBuf>) {
        // Like every change to a photo, through the gate (`Change::Restore`).
        paths.retain(|path| self.menu_block(Change::Restore, Some(path)).is_none());
        if paths.is_empty() {
            return;
        }
        // A playing video keeps its file open; it goes back once the file is closed.
        if self
            .current_video()
            .is_some_and(|video| paths.iter().any(|path| path == video))
        {
            self.stop_video();
        }
        for path in paths {
            if !self.deleted.queued.contains(&path) {
                self.deleted.queued.push(path);
            }
        }
    }

    /// The scan's result and finished restores; starts queued ones once no video holds them.
    pub(super) fn poll_deleted(&mut self, ctx: &egui::Context) {
        if let Some(found) = self.deleted.scan.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.deleted.scan = None;
            self.deleted.set(found, self.pair_mode);
            if self.options.filter.contains(FilterKind::Deleted) {
                self.sync_library();
                self.rebuild_view(ctx, None);
            }
        }
        if let Some(rx) = &self.deleted.restoring {
            match rx.try_recv() {
                Ok(done) => {
                    self.deleted.restoring = None;
                    self.apply_restored(ctx, done);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.deleted.restoring = None,
            }
        }
        if self.deleted.restoring.is_none()
            && !self.deleted.queued.is_empty()
            && self.videos_released()
        {
            self.start_restore(ctx);
        } else if !self.deleted.queued.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    /// The queued photos, with where each one goes back to (a pair's RAW with it).
    fn take_queued(&mut self) -> Vec<Job> {
        std::mem::take(&mut self.deleted.queued)
            .into_iter()
            .filter_map(|aside| {
                let original = self.deleted.original_of(&aside)?.to_path_buf();
                let rider = self.deleted.pairs.companion(&aside).and_then(|raw| {
                    let from = self.deleted.original_of(raw)?;
                    Some((raw.to_path_buf(), from.to_path_buf()))
                });
                Some(Job {
                    aside,
                    original,
                    rider,
                })
            })
            .collect()
    }

    fn start_restore(&mut self, ctx: &egui::Context) {
        let jobs = self.take_queued();
        let files = Arc::clone(&self.files);
        let repaint = ctx.clone();
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("cerno-restore".into())
            .spawn(move || {
                let _ = tx.send(run_restores(&files, jobs));
                repaint.request_repaint();
            });
        match spawned {
            Ok(_) => self.deleted.restoring = Some(rx),
            Err(err) => self.notice = Some(Notice::error(err.to_string())),
        }
    }

    /// The photos are back: the index follows, they join the folder again in name order, and
    /// their deletion no longer teaches the taste model.
    fn apply_restored(&mut self, ctx: &egui::Context, restored: Restored) {
        let t = i18n::t();
        let mut all = self.all.to_vec();
        let mut renamed = 0;
        for (aside, original, to) in &restored.done {
            if let Err(err) = self.db.record_restore(
                &aside.to_string_lossy(),
                &to.to_string_lossy(),
                &original.to_string_lossy(),
            ) {
                log::warn!("index: {err:#}");
            }
            if to != original {
                renamed += 1;
            }
            self.deleted.remove(aside);
            self.thumbs.invalidate(aside);
            all.push(to.clone());
        }
        // A pair's RAW is back beside its JPEG and rides along again.
        for rider in &restored.riders {
            if let Err(err) = self.db.record_restore(
                &rider.aside.to_string_lossy(),
                &rider.to.to_string_lossy(),
                &rider.original.to_string_lossy(),
            ) {
                log::warn!("index: {err:#}");
            }
            self.deleted.remove(&rider.aside);
            if self.pair_mode {
                self.pairs.insert(rider.primary.clone(), rider.to.clone());
            }
        }
        if let Some(dir) = &self.dir {
            library::sort(dir, &mut all);
        }
        // A RAW put back alone beside its JPEG pairs with it now; the hint reads the sidecars
        // of the pairs back.
        if self.pair_mode {
            let (paired, found) = pairs::pair_up(all, Path::to_path_buf);
            all = paired;
            self.pairs.extend(found);
        }
        self.all = Arc::new(all);
        if !restored.done.is_empty() {
            self.analyzer.taste_changed();
            self.scan_raw_marks();
        }
        // Nothing deleted is left: the 🗑 box would only show an empty view.
        if self.deleted.is_empty() && self.options.filter.contains(FilterKind::Deleted) {
            self.options.filter.set(FilterKind::Deleted, false);
            self.save_options();
        }
        self.sync_library();
        // The photo back stays the current one where the view still shows it.
        let back = restored.done.first().map(|(_, _, to)| to.clone());
        let after = self.deleted.after.take();
        let view = self.build_view();
        let keep = back
            .filter(|to| restored.done.len() == 1 && view.contains(to))
            .or(after);
        self.set_view(ctx, view, keep);
        self.notice = Some(match restored.failed.first() {
            Some((path, err)) => Notice::error((t.restore_failed)(
                restored.failed.len(),
                &library::file_name_lossy(path),
                err,
            )),
            None => {
                let name = restored
                    .done
                    .first()
                    .map(|(_, _, to)| self.photo_name(to))
                    .unwrap_or_default();
                Notice::hint((t.restored)(restored.done.len(), renamed, &name))
            }
        });
    }

    /// On exit: a restore still running finishes, one still queued is carried out (the video
    /// that held it is stopped by now), and both are recorded.
    pub(super) fn finish_restores(&mut self) {
        let mut finished = Vec::new();
        if let Some(rx) = self.deleted.restoring.take()
            && let Ok(restored) = rx.recv_timeout(std::time::Duration::from_secs(5))
        {
            finished.push(restored);
        }
        let queued = self.take_queued();
        if !queued.is_empty() {
            finished.push(run_restores(&self.files, queued));
        }
        for restored in &finished {
            let riders = restored
                .riders
                .iter()
                .map(|r| (&r.aside, &r.original, &r.to));
            for (aside, original, to) in restored
                .done
                .iter()
                .map(|(a, o, t)| (a, o, t))
                .chain(riders)
            {
                if let Err(err) = self.db.record_restore(
                    &aside.to_string_lossy(),
                    &to.to_string_lossy(),
                    &original.to_string_lossy(),
                ) {
                    log::warn!("index: {err:#}");
                }
            }
        }
    }
}

/// Puts each photo back (`originals::restore`), holding it while it moves. A pair's RAW
/// follows its JPEG under the JPEG's new name (`IMG_1 (2).CR3` beside `IMG_1 (2).jpg`), or the
/// next free one when that is taken.
fn run_restores(files: &crate::filelock::FileLocks, jobs: Vec<Job>) -> Restored {
    let mut out = Restored::default();
    for job in jobs {
        let held = files.hold_write(&job.aside);
        let result = originals::restore(&job.aside, &job.original);
        drop(held);
        let to = match result {
            Ok(to) => to,
            Err(err) => {
                out.failed.push((job.aside, format!("{err:#}")));
                continue;
            }
        };
        if let Some((raw, raw_original)) = job.rider {
            let beside = to.with_extension(raw_original.extension().unwrap_or_default());
            let _held = files.hold_write(&raw);
            match originals::restore(&raw, &beside) {
                Ok(raw_to) => {
                    if raw_to != beside {
                        log::warn!(
                            "RAW back as {}: {} is taken",
                            raw_to.display(),
                            beside.display()
                        );
                    }
                    out.riders.push(Rider {
                        aside: raw,
                        original: raw_original,
                        to: raw_to,
                        primary: to.clone(),
                    });
                }
                Err(err) => out.failed.push((raw, format!("{err:#}"))),
            }
        }
        out.done.push((job.aside, job.original, to));
    }
    out
}
