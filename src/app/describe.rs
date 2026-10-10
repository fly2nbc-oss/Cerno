//! Comment and keywords of the current photo: the details panel's description tab.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Rect};

use crate::loader::LoadedImage;
use crate::metadata::Description;
use crate::ui::description;
use crate::ui::details::{DetailsMode, DetailsTab};

use super::CernoApp;
use super::gate::Change;

impl CernoApp {
    /// What the tab shows for `path`: this session's change, else what the file had. `None`
    /// while the photo is still loading – nothing may be written before it is known.
    pub(super) fn description_of(
        &self,
        path: &Path,
        image: Option<&LoadedImage>,
    ) -> Option<Description> {
        self.marks
            .descriptions
            .get(path)
            .cloned()
            .or_else(|| image.map(|image| image.description.clone()))
    }

    /// Writes the photo's new comment and keywords (debounced, like a star).
    fn set_description(&mut self, path: PathBuf, description: Description) {
        if !self.allowed(Change::Mark, Some(&path)) {
            return;
        }
        if self.drafts.path.as_ref() == Some(&path) {
            self.drafts.base = Some(description.clone());
        }
        self.marks
            .descriptions
            .insert(path.clone(), description.clone());
        self.describe_companion(&path, &description);
        self.writer.set_description(path, description);
    }

    /// `Ctrl+Tab` (`Ctrl+Shift+Tab` backwards): the details panel's next tab. A closed panel
    /// opens on the tab it had. A comment being typed is taken first, and its field lets go.
    pub(super) fn cycle_details_tab(&mut self, backwards: bool) {
        if self.bars.details == DetailsMode::Off {
            self.set_details(DetailsMode::On);
            self.save_panels();
            return;
        }
        let tab = if backwards {
            self.bars.details_tab.prev()
        } else {
            self.bars.details_tab.next()
        };
        self.set_details_tab(tab);
    }

    pub(super) fn set_details_tab(&mut self, tab: DetailsTab) {
        if self.bars.details_tab == tab {
            return;
        }
        self.commit_comment();
        self.bars.details_tab = tab;
        self.db.put_setting("details_tab", tab.id());
    }

    /// A comment typed but not taken yet (its field still had the cursor) is written now: before
    /// another photo shows, when the tab or the panel closes, on exit.
    pub(super) fn commit_comment(&mut self) {
        if !self.drafts.comment_changed() {
            return;
        }
        let (Some(path), Some(base)) = (self.drafts.path.clone(), self.drafts.base.clone()) else {
            return;
        };
        let keywords = self
            .marks
            .descriptions
            .get(&path)
            .map_or(base.keywords, |d| d.keywords.clone());
        let comment = self.drafts.comment.trim().to_owned();
        self.set_description(path, Description { comment, keywords });
    }

    /// The description tab for the current photo.
    pub(super) fn draw_description(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        path: &Path,
        image: Option<&LoadedImage>,
    ) {
        let current = self.description_of(path, image);
        if self.drafts.path.as_deref() != Some(path) {
            // Another photo: what was typed for the last one is written first.
            self.commit_comment();
            self.drafts.reset(path.to_path_buf(), current.as_ref());
        } else if self.drafts.base.is_none() && current.is_some() {
            // The photo finished loading.
            self.drafts.reset(path.to_path_buf(), current.as_ref());
        }
        let blocked = self.menu_block(Change::Mark, Some(path));
        let out = description::draw(ui, rect, current.as_ref(), &mut self.drafts, blocked);
        if let Some(changed) = out.changed {
            self.set_description(path.to_path_buf(), changed);
        }
    }
}
