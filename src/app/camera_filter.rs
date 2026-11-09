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
use super::command::Command;

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

    /// The camera row's button was clicked: only its camera's photos – or, when that is what
    /// the view shows already, every camera again.
    pub(super) fn camera_button(&mut self, ctx: &egui::Context, row: &CameraRow) {
        let pick = (!row.on).then_some(row.pick);
        self.click(ctx, Command::FilterCamera(pick));
    }

    /// Only this camera's photos, or every camera again (`None`).
    pub(super) fn filter_camera(&mut self, ctx: &egui::Context, pick: Option<CameraPick>) {
        if self.options.camera != pick {
            self.change_options(ctx, |options| options.camera = pick);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::harness::Harness;

    /// The button picks the body when the model has two, else the model; a second click shows
    /// every camera again. A photo without a camera has no row; one camera greys it out.
    #[test]
    fn the_camera_button_picks_the_body_and_takes_it_back() {
        let mut h = Harness::new(4);
        h.settle();
        let paths = Arc::clone(&h.app.folder.all);
        let board = Arc::clone(&h.app.board);
        board.put_camera(&paths[0], "ILCE-7M4", Some("0474821"));
        board.put_camera(&paths[1], "ILCE-7M4", Some("0473021"));
        board.put_camera(&paths[2], "Pixel 7a", None);
        let ctx = h.ctx.clone();
        h.app.rebuild_view(&ctx, None);

        let row = h.app.camera_row(&paths[0]).expect("a camera");
        assert_eq!(row.label, "ILCE-7M4 ·4821");
        assert_eq!((row.on, row.blocked), (false, None));
        assert!(h.app.camera_row(&paths[3]).is_none(), "no camera, no row");
        let pixel = h.app.camera_row(&paths[2]).expect("a camera");
        assert_eq!(
            (pixel.label.as_str(), pixel.pick.serial),
            ("Pixel 7a", None)
        );

        h.app.camera_button(&ctx, &row);
        assert_eq!(*h.app.view.paths, [paths[0].clone()]);
        assert!(h.app.options.is_filtered());
        let row = h.app.camera_row(&paths[0]).expect("a camera");
        assert!(row.on);
        h.app.camera_button(&ctx, &row);
        assert_eq!(h.app.options.camera, None);
        assert_eq!(h.app.view.len(), 4);

        // One camera left in the folder: nothing to choose between.
        board.put_camera(&paths[0], "Pixel 7a", None);
        board.put_camera(&paths[1], "Pixel 7a", None);
        let row = h.app.camera_row(&paths[0]).expect("a camera");
        assert!(row.blocked.is_some());
    }
}
