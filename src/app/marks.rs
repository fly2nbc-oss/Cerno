//! Stars, rejection and colour labels – shown at once, written by the rating writer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui;

use crate::db::Db;
use crate::loader::{LoadedImage, Lookup};
use crate::metadata::{Description, Label, Rating};
use crate::view::SortKey;

use super::CernoApp;
use super::gate::Change;
use super::undo::{Entry, Journal};

/// The marks of this session: until the writer has flushed, they win over what the file says.
pub(super) struct Marks {
    /// Ratings given in this session; they win over the value read from the file, whose
    /// write may still be pending.
    pub(super) ratings: HashMap<PathBuf, Rating>,
    /// Colour labels given in this session (`None` clears). They win over the file the same way.
    pub(super) labels: HashMap<PathBuf, Option<Label>>,
    /// Comments and keywords given in this session; they win over the file the same way.
    pub(super) descriptions: HashMap<PathBuf, Description>,
    /// What Ctrl+Z takes back: the session's marks and edits.
    pub(super) journal: Journal,
    /// `0`–`5`, `X` and `6`–`9` also move to the next photo.
    pub(super) auto_advance: bool,
}

impl Marks {
    /// Nothing marked yet; auto-advance as saved.
    pub(super) fn restore(db: &Db) -> Self {
        Self {
            ratings: HashMap::new(),
            labels: HashMap::new(),
            descriptions: HashMap::new(),
            journal: Journal::default(),
            auto_advance: db.setting("auto_advance").as_deref() == Some("1"),
        }
    }
}

impl CernoApp {
    /// Rates the current photo. `advance` moves on afterwards, unless the photo itself
    /// dropped out of the view (a filter) – that already lands on the next one.
    pub(super) fn set_rating(&mut self, ctx: &egui::Context, rating: Rating, advance: bool) {
        if let Some(path) = self.view.get(self.current).cloned() {
            self.rate(ctx, path, rating, advance);
        }
    }

    /// Rates `path` and records it for `Ctrl+Z`.
    pub(super) fn rate(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        rating: Rating,
        advance: bool,
    ) {
        let before = self.rating_of(&path, self.loaded(&path).as_deref());
        let raw = self.companion_marks(&path).map(|(raw, _)| raw);
        if self.apply_rating(ctx, path.clone(), rating, advance) && before != rating {
            self.marks.journal.push(Entry::Rating {
                path,
                before,
                after: rating,
                raw,
            });
        }
    }

    /// Rates `path` without recording it (`Ctrl+Z` itself). False when the photo takes no
    /// mark right now.
    pub(super) fn apply_rating(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        rating: Rating,
        advance: bool,
    ) -> bool {
        if !self.allowed(Change::Mark, Some(&path)) {
            return false;
        }
        let next = self.next_path();
        self.marks.ratings.insert(path.clone(), rating);
        self.writer.set(path.clone(), rating);
        self.rate_companion(&path, rating);
        self.analyzer.taste_changed();
        self.finish_mark(ctx, &path, next, advance, self.rating_affects_view());
        true
    }

    /// The loaded image of `path`, when the loader has it.
    fn loaded(&self, path: &Path) -> Option<Arc<LoadedImage>> {
        let index = if self
            .view
            .get(self.current)
            .is_some_and(|shown| shown == path)
        {
            self.current
        } else {
            self.view.iter().position(|shown| shown == path)?
        };
        match self.loader.get(index) {
            Lookup::Ready(image) => Some(image),
            _ => None,
        }
    }

    pub(super) fn set_label(&mut self, ctx: &egui::Context, label: Option<Label>, advance: bool) {
        if let Some(path) = self.view.get(self.current).cloned() {
            self.label(ctx, path, label, advance);
        }
    }

    /// Sets the colour, or removes it when the photo already has that colour.
    pub(super) fn toggle_label(&mut self, ctx: &egui::Context, label: Label, advance: bool) {
        if let Some(path) = self.view.get(self.current).cloned() {
            let image = match self.loader.get(self.current) {
                Lookup::Ready(image) => Some(image),
                _ => None,
            };
            let next = if self.label_of(&path, image.as_deref()) == Some(label) {
                None
            } else {
                Some(label)
            };
            self.label(ctx, path, next, advance);
        }
    }

    /// Sets or clears the colour of `path` and records it for `Ctrl+Z`.
    fn label(&mut self, ctx: &egui::Context, path: PathBuf, label: Option<Label>, advance: bool) {
        let before = self.label_of(&path, self.loaded(&path).as_deref());
        let raw = self.companion_marks(&path).map(|(_, raw)| raw);
        if self.apply_label(ctx, path.clone(), label, advance) && before != label {
            self.marks.journal.push(Entry::Label { path, before, raw });
        }
    }

    /// Sets the colour without recording it (`Ctrl+Z` itself). False when the photo takes no
    /// mark right now.
    pub(super) fn apply_label(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        label: Option<Label>,
        advance: bool,
    ) -> bool {
        if !self.allowed(Change::Mark, Some(&path)) {
            return false;
        }
        let next = self.next_path();
        self.marks.labels.insert(path.clone(), label);
        self.writer.set_label(path.clone(), label);
        self.label_companion(&path, label);
        self.finish_mark(ctx, &path, next, advance, self.options.filter.has_colour());
        true
    }

    /// The photo after the current one, remembered before a rebuild moves things around.
    fn next_path(&self) -> Option<PathBuf> {
        self.view.get(self.current.saturating_add(1)).cloned()
    }

    /// Rebuilds when the mark changes which photos are shown or in which order, then optionally
    /// steps to `next` (the photo that followed this one before the rebuild).
    fn finish_mark(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        next: Option<PathBuf>,
        advance: bool,
        rebuild: bool,
    ) {
        if rebuild {
            self.rebuild_view(ctx, Some(path.to_path_buf()));
        }
        let still_here = self.view.get(self.current).is_some_and(|p| p == path);
        if still_here && !advance {
            return;
        }
        if let Some(next) = next.as_ref()
            && let Some(index) = self.view.iter().position(|p| p == next)
        {
            self.go_to(ctx, index, 1);
        } else if !still_here && next.is_none() && !self.view.is_empty() {
            // It was the last photo and left the view: stay at the new end.
            self.go_to(ctx, self.view.len() - 1, -1);
        }
    }

    fn rating_affects_view(&self) -> bool {
        !self.options.filter.is_all()
            || self.options.hide_rejected
            || matches!(self.options.sort, SortKey::Rating | SortKey::Taken)
    }

    /// `X`: rejects the current photo, or takes the rejection back.
    pub(super) fn toggle_reject(&mut self, ctx: &egui::Context, advance: bool) {
        if let Some(path) = self.view.get(self.current).cloned() {
            let image = match self.loader.get(self.current) {
                Lookup::Ready(image) => Some(image),
                _ => None,
            };
            let rating = match self.rating_of(&path, image.as_deref()) {
                Rating::Rejected => Rating::Unrated,
                _ => Rating::Rejected,
            };
            self.rate(ctx, path, rating, advance);
        }
    }

    pub(super) fn rating_of(&self, path: &Path, image: Option<&LoadedImage>) -> Rating {
        if let Some(rating) = self.marks.ratings.get(path) {
            return *rating;
        }
        image
            .map(|i| i.rating.value)
            .or_else(|| self.board.get(path).map(|k| k.rating))
            .unwrap_or_default()
    }

    /// Rejected photos of the folder (for "delete rejected photos").
    pub(super) fn rejected(&self) -> Vec<PathBuf> {
        self.all
            .iter()
            .filter(|p| !self.deletions.is_hidden(p))
            .filter(|p| {
                let rating = match self.marks.ratings.get(*p) {
                    Some(rating) => *rating,
                    None => self.board.get(p).map(|k| k.rating).unwrap_or_default(),
                };
                rating == Rating::Rejected
            })
            .cloned()
            .collect()
    }

    pub(super) fn label_of(&self, path: &Path, image: Option<&LoadedImage>) -> Option<Label> {
        if let Some(label) = self.marks.labels.get(path) {
            return *label;
        }
        if let Some(known) = self.board.get(path) {
            return known.label;
        }
        image.and_then(|image| image.label.known())
    }
}
