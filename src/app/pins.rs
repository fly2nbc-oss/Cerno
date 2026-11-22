//! Marking photos in Cerno alone (`V`, CER-74): a mark that is never written into a file –
//! a pre-selection while culling, beside the stars and colours that are. Kept in the index
//! (`db::pins`), so it survives a restart and follows a photo moved, renamed, deleted or put
//! back; the pin box of the filter bar shows the marked photos.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use eframe::egui;

use crate::db::Db;
use crate::i18n;
use crate::view::FilterKind;

use super::CernoApp;
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
        let rebuild = self.options.filter.contains(FilterKind::Pinned);
        self.finish_mark(ctx, &path, next, advance, rebuild);
        true
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
