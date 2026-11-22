//! Marking photos in Cerno alone (`V`, CER-74): a mark that is never written into a file –
//! a pre-selection while culling, beside the stars and colours that are. Kept in the index
//! (`db::pins`), so it survives a restart and follows a photo moved, renamed, deleted or put
//! back; the pin box of the filter bar shows the marked photos, and *Pinned photos of every
//! folder* opens them all as one collection.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use eframe::egui;

use crate::db::Db;
use crate::i18n;
use crate::library::{self, Format};
use crate::view::FilterKind;

use super::CernoApp;
use super::browse::Taken;
use super::gate::Change;
use super::menu::ConfirmAction;
use super::notice::Notice;
use super::undo::Entry;

/// Every marked photo, as the index has them.
pub(super) fn load(db: &Db) -> HashSet<PathBuf> {
    db.pins()
        .inspect_err(|err| log::warn!("pins: {err:#}"))
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

impl CernoApp {
    pub(super) fn is_pinned(&self, path: &Path) -> bool {
        self.pins.contains(path)
    }

    /// `V` marks the current photo or takes the mark away; `Shift+V` marks and moves on.
    pub(super) fn toggle_pin(&mut self, ctx: &egui::Context, advance: bool, only_on: bool) {
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        let on = only_on || !self.is_pinned(&path);
        if on == self.is_pinned(&path) {
            if advance {
                self.go_to(ctx, self.current.saturating_add(1), 1);
            }
            return;
        }
        if self.apply_pin(ctx, path.clone(), on, advance) {
            self.marks.journal.push(Entry::Pin { path, before: !on });
        }
    }

    /// Marks `path` or takes the mark away, without recording it (`Ctrl+Z` itself).
    pub(super) fn apply_pin(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        on: bool,
        advance: bool,
    ) -> bool {
        if !self.allowed(Change::Pin, Some(&path)) {
            return false;
        }
        if let Err(err) = self.db.set_pin(&path.to_string_lossy(), on) {
            self.notice = Some(Notice::error(format!("{err:#}")));
            return false;
        }
        let next = self.view.get(self.current.saturating_add(1)).cloned();
        if on {
            self.pins.insert(path.clone());
        } else {
            self.pins.remove(&path);
        }
        let t = i18n::t();
        self.notice = Some(Notice::hint(if on { t.pin_set } else { t.pin_cleared }));
        // In the collection a photo unpinned leaves it, one pinned again (`Ctrl+Z`) comes back.
        if self.folder.collection {
            self.collect_member(&path, on);
        }
        let rebuild = self.options.filter.contains(FilterKind::Pinned) || self.folder.collection;
        self.finish_mark(ctx, &path, next, advance, rebuild);
        true
    }

    /// The collection takes `path` in or lets it go, with its rider.
    fn collect_member(&mut self, path: &Path, on: bool) {
        let mut all = self.folder.all.to_vec();
        let present = all.iter().any(|p| p == path);
        if on && !present {
            all.push(path.to_path_buf());
            if let Some(dir) = &self.folder.dir {
                library::sort(dir, &mut all);
            }
        } else if !on && present {
            all.retain(|p| p != path);
        } else {
            return;
        }
        self.folder.all = Arc::new(all);
        self.sync_library();
    }

    /// *Visible photos ▸ Pinned photos of every folder*: on, the collection opens; off again,
    /// the folder it was opened from comes back, at the photo shown then.
    pub(super) fn open_pins(&mut self, ctx: &egui::Context) {
        if self.folder.collection {
            if let Some(back) = self.folder.before_pins.take() {
                self.open(ctx, &back);
            }
            return;
        }
        self.folder.before_pins = self
            .view
            .get(self.current)
            .filter(|path| !self.is_deleted(path))
            .cloned()
            .or_else(|| self.folder.dir.clone());
        self.gather_pins(ctx);
    }

    /// Every pinned photo still there, as one collection – shown, marked, copied, moved like a
    /// folder's photos. Their RAWs and Live Photo clips are looked for beside them, so they
    /// ride along again.
    pub(super) fn gather_pins(&mut self, ctx: &egui::Context) {
        let started = Instant::now();
        let pinned: HashSet<PathBuf> = self
            .pins
            .iter()
            .filter(|path| {
                !crate::originals::is_inside(path)
                    && library::format_of(path).is_some()
                    && path.is_file()
            })
            .cloned()
            .collect();
        let mut candidates: Vec<PathBuf> = pinned.iter().cloned().collect();
        candidates.extend(companions(&pinned));
        let rules = self.folder.modes().rules();
        let (paths, pairs) = if rules.any() {
            crate::pairs::pair_up(candidates.clone(), Path::to_path_buf, &rules)
        } else {
            (candidates.clone(), crate::pairs::Pairs::default())
        };
        // A rider found beside a pinned photo that pairs with nothing is no member; a photo
        // whose rider was pinned is.
        let primaries: HashSet<&Path> = pinned
            .iter()
            .filter_map(|path| pairs.primary_of(path))
            .collect();
        let dir = common_dir(&pinned);
        let mut paths: Vec<PathBuf> = paths
            .into_iter()
            .filter(|path| pinned.contains(path) || primaries.contains(path.as_path()))
            .collect();
        if let Some(dir) = &dir {
            library::sort(dir, &mut paths);
            library::sort(dir, &mut candidates);
        }
        let start = self
            .view
            .get(self.current)
            .filter(|path| paths.contains(path))
            .cloned();
        let index = start
            .as_ref()
            .and_then(|start| candidates.iter().position(|p| p == start))
            .unwrap_or(0);
        let stamps = vec![None; candidates.len()];
        self.take_library(
            ctx,
            Taken {
                dir,
                collection: true,
                scanned: Arc::new(candidates),
                stamps,
                paths,
                pairs,
                start,
                index,
                started,
            },
        );
    }

    /// The index moved marks along with photos (moved, renamed, deleted, put back).
    pub(super) fn reload_pins(&mut self) {
        self.pins = load(&self.db);
    }

    /// *Visible photos ▸ Clear marks (n)*: asks first.
    pub(super) fn ask_clear_pins(&mut self) {
        self.ask(ConfirmAction::ClearPins, false);
    }

    /// Every mark goes.
    pub(super) fn clear_pins(&mut self, ctx: &egui::Context) {
        if let Err(err) = self.db.clear_pins() {
            self.notice = Some(Notice::error(format!("{err:#}")));
            return;
        }
        self.pins.clear();
        self.notice = Some(Notice::hint(i18n::t().pins_cleared));
        if self.options.filter.contains(FilterKind::Pinned) {
            self.rebuild_view(ctx, None);
        }
    }
}

/// The RAWs and clips of the same name beside the pinned photos (one listing per folder).
fn companions(pinned: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let mut stems: HashMap<&Path, HashSet<String>> = HashMap::new();
    for path in pinned {
        if let (Some(dir), Some(stem)) = (path.parent(), path.file_stem()) {
            stems
                .entry(dir)
                .or_default()
                .insert(stem.to_string_lossy().to_lowercase());
        }
    }
    let mut out = Vec::new();
    for (dir, stems) in stems {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let rider = library::format_of(&path)
                .is_some_and(|format| format.is_raw() || format == Format::Video);
            let named = path
                .file_stem()
                .is_some_and(|stem| stems.contains(&stem.to_string_lossy().to_lowercase()));
            if rider && named && !pinned.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

/// The deepest folder holding every photo – `None` when they lie on different drives.
fn common_dir(paths: &HashSet<PathBuf>) -> Option<PathBuf> {
    let mut dirs = paths.iter().filter_map(|path| path.parent());
    let first = dirs.next()?.to_path_buf();
    dirs.try_fold(first, |common, dir| {
        let shared: PathBuf = common
            .components()
            .zip(dir.components())
            .take_while(|(a, b)| a == b)
            .map(|(a, _)| a.as_os_str())
            .collect();
        (!shared.as_os_str().is_empty()).then_some(shared)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The collection's folder is the deepest one they share; on different drives none.
    #[test]
    fn the_common_folder_is_the_deepest_shared_one() {
        let set = |paths: &[&str]| paths.iter().map(PathBuf::from).collect::<HashSet<_>>();
        let root = if cfg!(windows) { "D:\\" } else { "/" };
        let p = |rest: &str| format!("{root}{rest}");
        assert_eq!(
            common_dir(&set(&[
                &p("Pictures/2026/a/1.jpg"),
                &p("Pictures/2026/b/2.jpg")
            ])),
            Some(PathBuf::from(p("Pictures/2026")))
        );
        assert_eq!(
            common_dir(&set(&[&p("Pictures/a/1.jpg")])),
            Some(PathBuf::from(p("Pictures/a")))
        );
        if cfg!(windows) {
            assert_eq!(common_dir(&set(&["C:\\a\\1.jpg", "D:\\a\\2.jpg"])), None);
        }
    }
}
