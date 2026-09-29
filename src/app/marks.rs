//! Stars, rejection and colour labels – shown at once, written by the rating writer.

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::db::FileStamp;
use crate::loader::{LoadedImage, Lookup};
use crate::metadata::{Label, Rating};
use crate::view::SortKey;

use super::CernoApp;
use super::gate::Change;

impl CernoApp {
    /// Rates the current photo. `advance` moves on afterwards, unless the photo itself
    /// dropped out of the view (a filter) – that already lands on the next one.
    pub(super) fn set_rating(&mut self, ctx: &egui::Context, rating: Rating, advance: bool) {
        if let Some(path) = self.view.get(self.current).cloned() {
            self.rate(ctx, path, rating, advance);
        }
    }

    pub(super) fn rate(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        rating: Rating,
        advance: bool,
    ) {
        if !self.allowed(Change::Mark, Some(&path)) {
            return;
        }
        let next = self.next_path();
        if let Ok(stamp) = FileStamp::of(&path)
            && let Ok(Some(record)) = self.db.lookup(&path.to_string_lossy(), stamp)
            && let Err(err) = self.db.allow_taste_for(record.fingerprint)
        {
            log::warn!("taste allow: {err:#}");
        }
        self.session_ratings.insert(path.clone(), rating);
        self.writer.set(path.clone(), rating);
        self.analyzer.taste_changed();
        self.finish_mark(ctx, &path, next, advance, self.rating_affects_view());
    }

    pub(super) fn set_label(&mut self, ctx: &egui::Context, label: Option<Label>, advance: bool) {
        if let Some(path) = self.view.get(self.current).cloned() {
            self.apply_label(ctx, path, label, advance);
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
            self.apply_label(ctx, path, next, advance);
        }
    }

    fn apply_label(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        label: Option<Label>,
        advance: bool,
    ) {
        if !self.allowed(Change::Mark, Some(&path)) {
            return;
        }
        let next = self.next_path();
        self.session_labels.insert(path.clone(), label);
        self.writer.set_label(path.clone(), label);
        self.finish_mark(ctx, &path, next, advance, self.options.filter.has_colour());
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
        if let Some(rating) = self.session_ratings.get(path) {
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
                let rating = match self.session_ratings.get(*p) {
                    Some(rating) => *rating,
                    None => self.board.get(p).map(|k| k.rating).unwrap_or_default(),
                };
                rating == Rating::Rejected
            })
            .cloned()
            .collect()
    }

    pub(super) fn label_of(&self, path: &Path, image: Option<&LoadedImage>) -> Option<Label> {
        if let Some(label) = self.session_labels.get(path) {
            return *label;
        }
        if let Some(known) = self.board.get(path) {
            return known.label;
        }
        image.and_then(|image| image.label.known())
    }
}
