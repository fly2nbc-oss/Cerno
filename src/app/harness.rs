//! The whole app without a window, for tests: an index in memory, a writer without its thread,
//! a deletion that only records, ExifTool as found. Nothing touches the user's files, index,
//! models or programs: the photos are copies of the test fixture in a temp folder, and keys
//! that open dialogs, switch the language (`i18n::set` is not for tests) or reopen the folder
//! (that would start the analysis) are refused.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use eframe::App as _;
use eframe::egui::{self, Event, Key, Modifiers, RawInput, Rect, pos2, vec2};

use super::layer::Layer;
use super::{CernoApp, Parts, exiftool, keys};
use crate::db::Db;
use crate::filelock::FileLocks;
use crate::metadata::Rating;
use crate::rating::RatingWriter;
use crate::ui::help::Page;

/// A window of this size: wide enough for every bar.
const SCREEN: [f32; 2] = [1280.0, 860.0];

pub(super) struct Harness {
    pub(super) app: CernoApp,
    pub(super) ctx: egui::Context,
    dir: PathBuf,
    time: f64,
}

fn no_writer(_: egui::Context, _: Arc<Db>, _: Arc<FileLocks>) -> RatingWriter {
    RatingWriter::detached()
}

/// Deleting only says where the photo went; nothing moves.
fn recorder(path: &Path) -> Result<PathBuf, String> {
    Ok(path.with_extension("aside"))
}

impl Harness {
    /// The app on `photos` copies of `tests/fixtures/tiny.jpg` (`IMG_1.jpg` …), the first one
    /// current. The first-start question about updates counts as answered.
    pub(super) fn new(photos: usize) -> Self {
        Self::with(photos, true)
    }

    /// The same, with the update question still to come.
    pub(super) fn asking_about_updates(photos: usize) -> Self {
        Self::with(photos, false)
    }

    fn with(photos: usize, update_asked: bool) -> Self {
        static RUN: AtomicUsize = AtomicUsize::new(0);
        let ctx = egui::Context::default();
        keys::keep_zoom_keys(&ctx);
        let db = Db::open_in_memory().expect("an index in memory");
        if update_asked {
            db.put_setting("update_asked", "1");
        }
        let parts = Parts {
            db: Arc::new(db),
            notice: None,
            remover: recorder,
            writer: no_writer,
            exiftool: exiftool::ExifToolSetup::found,
        };
        let mut app = CernoApp::assemble(&ctx, parts, Instant::now());
        let dir = std::env::temp_dir().join(format!(
            "cerno-harness-{}-{}",
            std::process::id(),
            RUN.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("temp folder");
        let paths: Vec<PathBuf> = (1..=photos)
            .map(|n| {
                let path = dir.join(format!("IMG_{n}.jpg"));
                std::fs::copy("tests/fixtures/tiny.jpg", &path).expect("fixture copy");
                path
            })
            .collect();
        // What `open` does for the view, without `Library::open` and the analysis (it would
        // load the models).
        let paths = Arc::new(paths);
        app.all_index = paths
            .iter()
            .enumerate()
            .map(|(index, path)| (path.clone(), index))
            .collect();
        app.all = Arc::clone(&paths);
        app.library = paths;
        app.dir = Some(dir.clone());
        app.rebuild_view(&ctx, None);
        Self {
            app,
            ctx,
            dir,
            time: 0.0,
        }
    }

    /// One frame with these events, through the input hook like eframe's (`Tab`).
    pub(super) fn frame(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                vec2(SCREEN[0], SCREEN[1]),
            )),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        self.app.raw_input_hook(&self.ctx, &mut raw);
        let app = &mut self.app;
        let mut output = self.ctx.run_ui(raw, |ui| app.run_frame(ui));
        output.textures_delta.clear();
    }

    /// Two frames: an `Area` shows from its second one (egui measures it first).
    pub(super) fn settle(&mut self) {
        self.frame(Vec::new());
        self.frame(Vec::new());
    }

    /// One key with these modifiers, pressed and released in one frame.
    pub(super) fn press(&mut self, key: Key, modifiers: Modifiers) {
        self.frame(presses(&[(key, modifiers)]));
    }

    /// The current photo's mark as the app shows it (the session's, before the writer).
    pub(super) fn rating_of(&self, index: usize) -> Option<Rating> {
        let path = self.app.view.get(index)?;
        self.app.session_ratings.get(path).copied()
    }

    /// What lies over the window and takes the keyboard.
    pub(super) fn layer(&self) -> &'static str {
        match &self.app.layer {
            Layer::None => "none",
            Layer::Help(_) => "help",
            Layer::List(_) => "list",
            Layer::Models => "models",
            Layer::Confirm { .. } => "confirm",
            Layer::NameList(_) => "name list",
            Layer::CameraTime(_) => "camera time",
        }
    }

    /// The help page's tab, while it is open.
    pub(super) fn help_page(&self) -> Option<Page> {
        match self.app.layer {
            Layer::Help(page) => Some(page),
            _ => None,
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Key events for one frame. Refuses the keys that reach the user's system: `Ctrl+O` (folder
/// dialog), `Ctrl+L` (`i18n::set`), `Ctrl+U` (reopens the folder and starts the analysis).
pub(super) fn presses(keys: &[(Key, Modifiers)]) -> Vec<Event> {
    let mut events = Vec::new();
    for &(key, modifiers) in keys {
        assert!(
            !(modifiers.command && matches!(key, Key::O | Key::L | Key::U)),
            "{key:?} with Ctrl is not for tests"
        );
        events.push(Event::ModifiersChanged(modifiers));
        for pressed in [true, false] {
            events.push(Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            });
        }
    }
    // egui reads the modifiers as they are at the end of the frame; the next frame starts
    // without any (`RawInput::modifiers`).
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::menu::ConfirmAction;

    const NONE: Modifiers = Modifiers::NONE;

    #[test]
    fn the_harness_shows_the_first_photo() {
        let mut h = Harness::new(3);
        h.settle();
        assert_eq!(h.app.view.len(), 3);
        assert_eq!(h.app.current, 0);
        assert_eq!(h.layer(), "none");
        h.press(Key::ArrowRight, NONE);
        assert_eq!(h.app.current, 1);
    }

    /// A card takes the keyboard: no mark, no Tab.
    #[test]
    fn a_card_swallows_marks_and_tab() {
        let mut h = Harness::new(3);
        h.settle();
        h.app.layer = Layer::Models;
        h.settle();
        let details = h.app.bars.details;
        h.frame(presses(&[
            (Key::X, NONE),
            (Key::Num3, NONE),
            (Key::Tab, NONE),
        ]));
        assert_eq!(h.rating_of(0), None);
        assert_eq!(h.app.bars.details, details);
        assert_eq!(h.layer(), "models");
    }

    /// `G` closes the faces grid before anything else reads the frame's keys.
    #[test]
    fn g_and_x_in_one_frame_only_close_the_faces_grid() {
        let mut h = Harness::new(3);
        h.settle();
        h.app.faces.grid_open = true;
        h.frame(presses(&[(Key::G, NONE), (Key::X, NONE)]));
        assert!(!h.app.faces.grid_open);
        assert_eq!(h.rating_of(0), None);
    }

    #[test]
    fn help_takes_its_keys_and_ignores_marks() {
        let mut h = Harness::new(3);
        h.settle();
        h.press(Key::H, NONE);
        assert_eq!(h.layer(), "help");
        h.press(Key::ArrowRight, NONE);
        assert_eq!(h.help_page(), Some(Page::Tips));
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(0), None);
        assert_eq!(h.app.current, 0);
        h.press(Key::Escape, NONE);
        assert_eq!(h.layer(), "none");
        h.press(Key::H, NONE);
        h.press(Key::H, NONE);
        assert_eq!(h.layer(), "none");
    }

    /// Navigation runs before marks: the digit rates the photo the arrow went to.
    #[test]
    fn an_arrow_and_a_digit_in_one_frame_rate_the_next_photo() {
        let mut h = Harness::new(3);
        h.settle();
        h.frame(presses(&[(Key::ArrowRight, NONE), (Key::Num3, NONE)]));
        assert_eq!(h.app.current, 1);
        assert_eq!(h.rating_of(1), Some(Rating::Stars(3)));
        assert_eq!(h.rating_of(0), None);
    }

    /// Compare mode needs the photo: the grid steps aside first.
    #[test]
    fn c_in_the_grid_leaves_the_grid_first() {
        let mut h = Harness::new(3);
        h.settle();
        h.press(Key::F7, NONE);
        assert!(h.app.grid);
        h.press(Key::C, NONE);
        assert!(!h.app.grid);
        assert!(h.app.pinned.is_some());
        h.press(Key::Escape, NONE);
        assert!(h.app.pinned.is_none());
    }

    /// A deletion counting down comes first: `Esc` brings the photo back.
    #[test]
    fn escape_brings_a_deletion_back_first() {
        let mut h = Harness::new(3);
        h.settle();
        h.press(Key::C, NONE);
        h.press(Key::Delete, NONE);
        assert_eq!(h.app.view.len(), 2);
        h.press(Key::Escape, NONE);
        assert_eq!(h.app.view.len(), 3);
        assert!(h.app.pinned.is_some(), "compare mode is the next Esc");
    }

    /// A question asked from the models card goes back to it.
    #[test]
    fn a_question_from_the_models_card_returns_to_it() {
        let mut h = Harness::new(1);
        h.settle();
        h.app.layer = Layer::Models;
        h.app.ask(ConfirmAction::ResetTaste, true);
        assert_eq!(h.layer(), "confirm");
        h.settle();
        h.press(Key::Escape, NONE);
        assert_eq!(h.layer(), "models");
    }

    /// The first-start question waits while the help page is open.
    #[test]
    fn the_update_question_waits_for_the_help_page() {
        let mut h = Harness::asking_about_updates(1);
        h.app.layer = Layer::Help(Page::Keys);
        h.settle();
        assert_eq!(h.layer(), "help");
        h.app.layer = Layer::None;
        h.settle();
        assert!(matches!(
            h.app.layer,
            Layer::Confirm {
                action: ConfirmAction::UpdateCheck,
                ..
            }
        ));
    }

    /// The faces grid covers the photo only: help opens over it and leaves it open.
    #[test]
    fn help_over_the_faces_grid_leaves_it_open() {
        let mut h = Harness::new(1);
        h.settle();
        h.app.faces.grid_open = true;
        h.app.open_help();
        h.settle();
        assert!(h.app.faces.grid_open);
        assert_eq!(h.layer(), "help");
    }

    /// An open list takes the keyboard: `X` marks nothing, `Esc` closes only the list.
    #[test]
    fn an_open_list_ignores_marks() {
        let mut h = Harness::new(3);
        h.settle();
        h.app.open_language_list();
        h.settle();
        assert_eq!(h.layer(), "list");
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(0), None);
        h.press(Key::Escape, NONE);
        assert_eq!(h.layer(), "none");
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(0), Some(Rating::Rejected));
    }

    /// `Ctrl+K` gives the menu bar the keyboard and takes it back (removed with 1.12's keys).
    #[test]
    fn ctrl_k_gives_the_menu_bar_the_keyboard_and_takes_it_back() {
        let mut h = Harness::new(3);
        h.settle();
        h.press(Key::K, Modifiers::COMMAND);
        assert!(h.app.menu_bar.state.focus);
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(0), None, "the bar has the keyboard");
        h.press(Key::K, Modifiers::COMMAND);
        assert!(!h.app.menu_bar.state.focus);
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(0), Some(Rating::Rejected));
    }

    /// `Ctrl+M` opens the menu bar for the moment, with the keyboard; `Esc` takes both back
    /// (removed with 1.12's keys).
    #[test]
    fn ctrl_m_shows_the_menu_bar_for_the_moment() {
        let mut h = Harness::new(3);
        h.settle();
        assert!(!h.app.bars.side_bar);
        h.press(Key::M, Modifiers::COMMAND);
        h.settle();
        assert!(h.app.menu_bar.state.focus);
        assert!(h.app.menu_bar.temporary);
        h.press(Key::Escape, NONE);
        h.settle();
        assert!(!h.app.menu_bar.state.focus);
        assert!(!h.app.menu_bar.temporary);
        assert_eq!(h.app.view.len(), 3, "Esc on the bar touches nothing else");
    }
}
