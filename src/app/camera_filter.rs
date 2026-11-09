//! The camera filter from the app's side: the open folder's cameras (`view::camera_entries`,
//! counted again when the score board changed), the label of the camera chosen, and the
//! button beside Details › File's camera row that shows only the current photo's camera.

use std::path::Path;
use std::sync::Arc;

use eframe::egui;

use crate::i18n;
use crate::ui::details::CameraRow;
use crate::view::{self, CameraEntry, CameraPick};

use super::CernoApp;

impl CernoApp {
    /// The folder's cameras, the most photos first – what the camera box lists. Counted over
    /// the folder's photos (not the deleted ones), like the camera time card.
    pub(super) fn cameras(&mut self) -> Arc<Vec<CameraEntry>> {
        let version = self.board.version();
        let (built, list, _) = &self.browse.cameras;
        if *built != version || !std::sync::Weak::ptr_eq(list, &Arc::downgrade(&self.folder.all)) {
            let board = &self.board;
            let entries = view::camera_entries(
                self.folder.all.iter().map(|path| {
                    board
                        .get(path)
                        .map_or((None, None), |known| (known.camera, known.serial))
                }),
                |model| board.camera_name(model),
                |serial| board.serial_number(serial),
            );
            self.browse.cameras = (version, Arc::downgrade(&self.folder.all), Arc::new(entries));
        }
        Arc::clone(&self.browse.cameras.2)
    }

    /// What the camera box shows for a chosen camera: its entry's label, else (a deleted
    /// photo's camera, none of the folder's) the model's name.
    pub(super) fn camera_label(&self, pick: CameraPick, entries: &[CameraEntry]) -> String {
        entries
            .iter()
            .find(|entry| entry.pick == pick)
            .map(CameraEntry::label)
            .or_else(|| self.board.camera_name(pick.model))
            .unwrap_or_default()
    }

    /// The camera a photo belongs to in the camera box: its body's entry when the model is
    /// split by serial number, else the model's.
    fn camera_of(&self, path: &Path, entries: &[CameraEntry]) -> Option<CameraPick> {
        let known = self.board.get(path)?;
        let model = known.camera?;
        let body = entries.iter().find(|entry| {
            entry.pick.serial.is_some() && entry.pick.accepts(Some(model), known.serial)
        });
        Some(body.map_or(
            CameraPick {
                model,
                serial: None,
            },
            |entry| entry.pick,
        ))
    }

    /// Details › File's camera row for the photo at `path`: the camera as the camera box names
    /// it, and whether its button shows only its photos or takes that back. Greyed out in a
    /// folder with one camera – unless it filters and has to be switched off.
    pub(super) fn camera_row(&mut self, path: &Path) -> Option<CameraRow> {
        let entries = self.cameras();
        let pick = self.camera_of(path, &entries)?;
        let on = self.options.camera == Some(pick);
        let t = i18n::t();
        Some(CameraRow {
            label: self.camera_label(pick, &entries),
            pick,
            on,
            blocked: (entries.len() < 2 && !on).then_some(t.camera_one),
        })
    }

    /// The camera box or Details' button: only this camera's photos, or every camera again.
    pub(super) fn filter_camera(&mut self, ctx: &egui::Context, pick: Option<CameraPick>) {
        if self.options.camera != pick {
            self.change_options(ctx, |options| options.camera = pick);
        }
    }
}
