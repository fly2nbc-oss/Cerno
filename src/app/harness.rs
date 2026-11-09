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
        let ctx = crate::theme::test_context();
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
        app.folder.all_index = paths
            .iter()
            .enumerate()
            .map(|(index, path)| (path.clone(), index))
            .collect();
        app.folder.all = Arc::clone(&paths);
        app.folder.library = paths;
        app.folder.dir = Some(dir.clone());
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
        self.window_frame(events, false);
    }

    /// One frame in which the window's × (or `Alt+F4`) asks to close it; what the app told the
    /// window (`CancelClose`, `Close`).
    pub(super) fn close_window(&mut self) -> Vec<egui::ViewportCommand> {
        self.window_frame(Vec::new(), true)
    }

    /// One frame; `close`: the window is asked to close. What the app told the window.
    pub(super) fn window_frame(
        &mut self,
        events: Vec<Event>,
        close: bool,
    ) -> Vec<egui::ViewportCommand> {
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
        if close {
            raw.viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .events
                .push(egui::ViewportEvent::Close);
        }
        self.app.raw_input_hook(&self.ctx, &mut raw);
        let app = &mut self.app;
        let mut output = self.ctx.run_ui(raw, |ui| app.run_frame(ui));
        output.textures_delta.clear();
        output
            .viewport_output
            .remove(&egui::ViewportId::ROOT)
            .map(|viewport| viewport.commands)
            .unwrap_or_default()
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
        self.app.marks.ratings.get(path).copied()
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
            Layer::Keywords(_) => "keywords",
            Layer::CameraTime(_) => "camera time",
            Layer::Unsaved { .. } => "unsaved",
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
    use crate::app::command::{Advance, Command, Source};
    use crate::app::editing::EditSession;
    use crate::app::menu::ConfirmAction;
    use crate::ui::filter_bar::Slot;
    use crate::view::FilterKind;

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
        h.app.viewer.faces.grid_open = true;
        h.frame(presses(&[(Key::G, NONE), (Key::X, NONE)]));
        assert!(!h.app.viewer.faces.grid_open);
        assert_eq!(h.rating_of(0), None);
    }

    /// Over the faces grid the arrows, `Space` and `End` move to another photo and the grid
    /// stays open for its faces; marks still pause.
    #[test]
    fn the_faces_grid_browses_and_stays_open() {
        let mut h = Harness::new(4);
        h.settle();
        h.app.viewer.faces.grid_open = true;
        h.press(Key::ArrowRight, NONE);
        assert_eq!(h.app.current, 1);
        h.press(Key::Space, NONE);
        assert_eq!(h.app.current, 2);
        h.press(Key::ArrowLeft, NONE);
        assert_eq!(h.app.current, 1);
        h.press(Key::End, NONE);
        assert_eq!(h.app.current, 3);
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(3), None);
        assert!(h.app.viewer.faces.grid_open);
    }

    /// Help over the faces grid keeps its arrows for its tabs.
    #[test]
    fn help_over_the_faces_grid_keeps_the_arrows() {
        let mut h = Harness::new(3);
        h.settle();
        h.app.viewer.faces.grid_open = true;
        h.app.open_help();
        h.settle();
        h.press(Key::ArrowRight, NONE);
        assert_eq!(h.app.current, 0);
        assert_eq!(h.layer(), "help");
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
        assert!(h.app.viewer.grid);
        h.press(Key::C, NONE);
        assert!(!h.app.viewer.grid);
        assert!(h.app.viewer.pinned.is_some());
        h.press(Key::Escape, NONE);
        assert!(h.app.viewer.pinned.is_none());
    }

    /// The session's mark of `path`.
    fn mark_of(h: &Harness, path: &Path) -> Option<Rating> {
        h.app.marks.ratings.get(path).copied()
    }

    /// `A` keeps the left photo and ends the comparison on it.
    #[test]
    fn a_keeps_the_left_photo_and_ends_the_comparison() {
        let mut h = Harness::new(3);
        h.settle();
        let paths = h.app.view.paths.clone();
        h.press(Key::C, NONE);
        h.press(Key::A, NONE);
        assert!(h.app.viewer.pinned.is_none());
        assert_eq!(h.app.view.get(h.app.current), Some(&paths[0]));
        assert_eq!(mark_of(&h, &paths[1]), Some(Rating::Rejected));
    }

    /// `Shift+A` rejects the right photo: the left one stays pinned and the next comes in on
    /// the right, at the same zoom. `Shift+D` rejects the left one: the right one moves to the
    /// left. At the end of the view the comparison ends on the winner, with a hint (CER-64).
    #[test]
    fn shift_a_and_shift_d_keep_the_winner_and_bring_the_next() {
        let shift = Modifiers::SHIFT;
        let mut h = Harness::new(4);
        h.settle();
        let paths = h.app.view.paths.clone();
        h.press(Key::C, NONE);
        assert_eq!(h.app.viewer.pinned.as_ref(), Some(&paths[0]));
        assert_eq!(h.app.current, 1);
        h.app.viewer.zoom.scale = Some(1.0);

        h.press(Key::A, shift);
        assert_eq!(mark_of(&h, &paths[1]), Some(Rating::Rejected));
        assert_eq!(
            h.app.viewer.pinned.as_ref(),
            Some(&paths[0]),
            "the winner stays"
        );
        assert_eq!(h.app.view.get(h.app.current), Some(&paths[2]));
        assert_eq!(h.app.viewer.zoom.scale, Some(1.0), "the shared zoom stays");

        h.press(Key::D, shift);
        assert_eq!(mark_of(&h, &paths[0]), Some(Rating::Rejected));
        assert_eq!(
            h.app.viewer.pinned.as_ref(),
            Some(&paths[2]),
            "the right one moves left"
        );
        assert_eq!(h.app.view.get(h.app.current), Some(&paths[3]));
        assert_eq!(h.app.viewer.zoom.scale, Some(1.0));

        // Nothing follows the last photo: the comparison ends on the winner.
        h.press(Key::A, shift);
        assert_eq!(mark_of(&h, &paths[3]), Some(Rating::Rejected));
        assert!(h.app.viewer.pinned.is_none());
        assert_eq!(h.app.view.get(h.app.current), Some(&paths[2]));
        assert!(
            h.app
                .notice
                .as_ref()
                .is_some_and(|notice| notice.text == "No more photos – the comparison ends")
        );
        // `Ctrl+Z` takes the last rejection back; compare mode stays ended.
        h.press(Key::Z, Modifiers::COMMAND);
        assert_eq!(mark_of(&h, &paths[3]), Some(Rating::Unrated));
        assert!(h.app.viewer.pinned.is_none());
    }

    /// A photo already rejected is out of the running: `Shift+A` passes over it.
    #[test]
    fn shift_a_passes_over_rejected_photos() {
        let mut h = Harness::new(4);
        h.settle();
        let paths = h.app.view.paths.clone();
        h.press(Key::End, NONE);
        h.press(Key::ArrowLeft, NONE);
        h.press(Key::X, NONE);
        assert_eq!(mark_of(&h, &paths[2]), Some(Rating::Rejected));
        h.press(Key::Home, NONE);
        h.press(Key::C, NONE);
        h.press(Key::A, Modifiers::SHIFT);
        assert_eq!(h.app.viewer.pinned.as_ref(), Some(&paths[0]));
        assert_eq!(h.app.view.get(h.app.current), Some(&paths[3]));
    }

    /// Frames until the loader has decoded the current photo (at most a few seconds).
    fn wait_for_photo(h: &mut Harness) {
        for _ in 0..500 {
            h.frame(Vec::new());
            if matches!(
                h.app.loader.get(h.app.current),
                crate::loader::Lookup::Ready(_)
            ) {
                h.frame(Vec::new());
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the photo was not decoded");
    }

    /// `Shift+Z` shows the photo whole and at 100 %: both sides follow the arrows, marks act on
    /// the one photo, the 100 % side is aimed at each new photo and keeps its scale. `Esc`
    /// ends it before the zoom, and the single photo is whole again (CER-49).
    #[test]
    fn shift_z_shows_the_photo_whole_and_at_100_percent() {
        let shift = Modifiers::SHIFT;
        let mut h = Harness::new(3);
        wait_for_photo(&mut h);
        let paths = h.app.view.paths.clone();
        h.press(Key::Z, shift);
        assert!(h.app.viewer.split);
        assert_eq!(h.app.viewer.zoom.scale, Some(1.0));
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let slots = h.app.slots(area);
        assert_eq!(slots.len(), 2, "the same photo twice");
        assert!(
            !slots[0].zooms() && slots[1].zooms(),
            "whole left, 100 % right"
        );
        wait_for_photo(&mut h);
        assert_eq!(h.app.viewer.split_aim.as_ref(), Some(&paths[0]), "aimed");

        h.press(Key::ArrowRight, NONE);
        assert!(h.app.viewer.split, "it follows the arrows");
        assert_eq!(h.app.current, 1);
        assert_eq!(h.app.viewer.zoom.scale, Some(1.0), "the scale stays");
        wait_for_photo(&mut h);
        assert_eq!(
            h.app.viewer.split_aim.as_ref(),
            Some(&paths[1]),
            "aimed again"
        );

        h.press(Key::X, NONE);
        assert_eq!(mark_of(&h, &paths[1]), Some(Rating::Rejected));
        assert!(h.app.viewer.split);

        h.press(Key::Escape, NONE);
        assert!(!h.app.viewer.split, "Esc ends it first");
        assert_eq!(h.app.viewer.zoom.scale, None, "whole again");
        assert_eq!(h.app.slots(area).len(), 1);
    }

    /// The split view, compare mode, the four-up view and the grid exclude each other; the
    /// menu bar's row switches it like `Shift+Z`.
    #[test]
    fn the_split_view_makes_way_and_takes_over() {
        let shift = Modifiers::SHIFT;
        let mut h = Harness::new(5);
        h.settle();
        h.press(Key::Z, shift);
        h.press(Key::C, NONE);
        assert!(!h.app.viewer.split && h.app.viewer.pinned.is_some());
        h.press(Key::Z, shift);
        assert!(h.app.viewer.split && h.app.viewer.pinned.is_none());
        h.press(Key::C, shift);
        assert!(!h.app.viewer.split && h.app.viewer.quad.is_some());
        h.press(Key::Z, shift);
        assert!(h.app.viewer.split && h.app.viewer.quad.is_none());
        h.press(Key::F7, NONE);
        assert!(!h.app.viewer.split && h.app.viewer.grid);
        h.press(Key::Z, shift);
        assert!(
            h.app.viewer.split && !h.app.viewer.grid,
            "the grid steps aside"
        );
        let ctx = h.ctx.clone();
        h.app.run(&ctx, Command::Straighten, Source::Menu, &[]);
        assert!(!h.app.viewer.split, "straightening needs the photo alone");
        h.press(Key::Escape, NONE);
        h.app.run(&ctx, Command::Split, Source::Menu, &[]);
        assert!(h.app.viewer.split);
        h.app.run(&ctx, Command::Split, Source::Menu, &[]);
        assert!(!h.app.viewer.split);
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
        assert!(
            h.app.viewer.pinned.is_some(),
            "compare mode is the next Esc"
        );
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

    /// The menu bar's Straighten and Crop end the four-up view first, like `S` and `R`.
    #[test]
    fn straighten_from_the_menu_ends_the_four_up_view() {
        let mut h = Harness::new(5);
        h.settle();
        for command in [Command::Straighten, Command::Crop] {
            h.press(Key::C, Modifiers::SHIFT);
            assert!(h.app.viewer.quad.is_some());
            let ctx = h.ctx.clone();
            h.app.run(&ctx, command, Source::Menu, &[]);
            assert!(h.app.viewer.quad.is_none(), "{command:?}");
            h.settle();
        }
    }

    /// The first-start question waits for a straighten or crop session to end.
    #[test]
    fn the_update_question_waits_for_an_edit_session() {
        let mut h = Harness::asking_about_updates(1);
        let path = h.app.view.paths.first().cloned().expect("a photo");
        h.app.edits.session = Some(EditSession::straightening(path));
        h.settle();
        assert_eq!(h.layer(), "none");
        h.app.edits.session = None;
        h.settle();
        assert_eq!(h.layer(), "confirm");
    }

    /// The faces grid covers the photo only: help opens over it and leaves it open.
    #[test]
    fn help_over_the_faces_grid_leaves_it_open() {
        let mut h = Harness::new(1);
        h.settle();
        h.app.viewer.faces.grid_open = true;
        h.app.open_help();
        h.settle();
        assert!(h.app.viewer.faces.grid_open);
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

    /// `F10` switches the menu bar like `T`, `Tab` and `F6` the other bars: it stays while
    /// the photos change, takes no key, and is saved.
    #[test]
    fn f10_switches_the_menu_bar_and_it_stays() {
        let mut h = Harness::new(3);
        h.settle();
        assert!(!h.app.bars.side_bar);
        h.press(Key::F10, NONE);
        assert!(h.app.bars.side_bar);
        assert_eq!(h.app.db.setting("side_bar").as_deref(), Some("1"));
        h.press(Key::ArrowRight, NONE);
        h.press(Key::X, NONE);
        assert_eq!(h.app.current, 1, "the bar takes no key");
        assert_eq!(h.rating_of(1), Some(Rating::Rejected));
        assert!(h.app.bars.side_bar, "it stays while browsing");
        h.press(Key::F10, NONE);
        assert!(!h.app.bars.side_bar);
    }

    /// `Ctrl+K` and `Ctrl+M` are gone (the menu bar is for the mouse).
    #[test]
    fn ctrl_k_and_ctrl_m_do_nothing() {
        let mut h = Harness::new(3);
        h.settle();
        h.press(Key::K, Modifiers::COMMAND);
        h.press(Key::M, Modifiers::COMMAND);
        h.settle();
        assert!(!h.app.bars.side_bar);
        assert_eq!(h.layer(), "none");
        assert!(!h.app.options.similar);
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(0), Some(Rating::Rejected));
    }

    /// `E` without a remembered program opens the programs' list over the photo – not the
    /// menu bar; `Esc` closes it and the keys reach the photo again.
    #[test]
    fn e_without_a_program_opens_the_list_over_the_photo() {
        let mut h = Harness::new(3);
        // The system is not asked for its programs here.
        h.app.external.editors.insert("jpg".into(), Vec::new());
        h.settle();
        h.press(Key::E, NONE);
        h.settle();
        assert_eq!(h.layer(), "list");
        assert!(!h.app.bars.side_bar, "the menu bar stays off");
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(0), None, "the list has the keyboard");
        h.press(Key::Escape, NONE);
        assert_eq!(h.layer(), "none");
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(0), Some(Rating::Rejected));
    }

    /// The stars row of the menu bar rates like the info bar's stars (no moving on).
    #[test]
    fn the_stars_row_rates_without_moving_on() {
        let mut h = Harness::new(3);
        h.app.marks.auto_advance = true;
        h.settle();
        let ctx = h.ctx.clone();
        h.app.run(
            &ctx,
            Command::SetRating(Rating::Stars(4), Advance::No),
            Source::Menu,
            &[],
        );
        assert_eq!(h.rating_of(0), Some(Rating::Stars(4)));
        assert_eq!(h.app.current, 0);
    }

    /// What the writer reports: `unsaved` files not written, `asked` tries asked for, of
    /// which `done` have run.
    fn report(h: &Harness, unsaved: &[usize], asked: usize, done: usize) {
        use crate::rating::{UnsavedMark, WriterStatus};
        let unsaved = unsaved
            .iter()
            .map(|&n| UnsavedMark {
                path: h.app.folder.all[n].clone(),
                error: "locked by another program".to_owned(),
            })
            .collect();
        h.app.writer.report(WriterStatus {
            unsaved,
            retry_asked: asked,
            retry_done: done,
            ..WriterStatus::default()
        });
    }

    /// Nothing waiting, nothing failed: the window closes at once (ticket CER-57).
    #[test]
    fn a_clean_close_is_not_held() {
        let mut h = Harness::new(2);
        h.settle();
        let told = h.close_window();
        assert!(!told.contains(&egui::ViewportCommand::CancelClose));
        assert_eq!(h.layer(), "none");
    }

    /// Marks not written hold the close: the writer tries again first; still not written, the
    /// card asks – `Enter` tries once more, and once everything is written the window closes
    /// on its own. `Esc` stays in Cerno.
    #[test]
    fn closing_with_marks_not_saved_tries_again_and_asks() {
        let mut h = Harness::new(2);
        h.settle();
        report(&h, &[0], 0, 0);
        h.frame(Vec::new());
        let told = h.close_window();
        assert!(told.contains(&egui::ViewportCommand::CancelClose));
        assert_eq!(h.app.writer.status().retry_asked, 1, "tried again");
        assert_eq!(h.layer(), "none", "the try runs first");

        // The try ran, the file is still locked: the card asks.
        report(&h, &[0], 1, 1);
        h.settle();
        assert_eq!(h.layer(), "unsaved");
        assert!(matches!(h.app.layer, Layer::Unsaved { exiting: true }));
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(0), None, "the card has the keyboard");
        h.press(Key::Enter, NONE);
        assert_eq!(h.app.writer.status().retry_asked, 2, "Enter tries again");

        // Written now: Cerno closes as asked.
        report(&h, &[], 2, 2);
        let told = h.window_frame(Vec::new(), false);
        assert!(told.contains(&egui::ViewportCommand::Close));
        assert_eq!(h.layer(), "none");
        let told = h.close_window();
        assert!(
            !told.contains(&egui::ViewportCommand::CancelClose),
            "that close goes through"
        );

        // Another time: `Esc` on the card stays in Cerno, and a new close asks again.
        let mut h = Harness::new(2);
        h.settle();
        report(&h, &[1], 0, 0);
        h.close_window();
        report(&h, &[1], 1, 1);
        h.settle();
        assert_eq!(h.layer(), "unsaved");
        h.press(Key::Escape, NONE);
        assert_eq!(h.layer(), "none");
        let told = h.close_window();
        assert!(told.contains(&egui::ViewportCommand::CancelClose));
    }

    /// The card of the marks not written closes on its own once its try wrote them all.
    #[test]
    fn the_card_closes_once_everything_is_written() {
        let mut h = Harness::new(2);
        h.settle();
        report(&h, &[0, 1], 0, 0);
        h.app.show_unsaved();
        h.settle();
        assert!(matches!(h.app.layer, Layer::Unsaved { exiting: false }));
        h.press(Key::Enter, NONE);
        assert_eq!(h.app.writer.status().retry_asked, 1);
        h.settle();
        assert_eq!(h.layer(), "unsaved", "still trying");
        report(&h, &[], 1, 1);
        h.settle();
        assert_eq!(h.layer(), "none");
        assert!(
            h.app
                .notice
                .as_ref()
                .is_some_and(|notice| notice.text == "All marks saved.")
        );
    }

    /// The filter bar shows in this frame's layout.
    fn bar_shown(h: &Harness) -> bool {
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(SCREEN[0], SCREEN[1]));
        h.app.layout(window).toolbar.is_some()
    }

    /// `Ctrl+F` shows the filter bar although it is off and takes the keys: the arrows move
    /// the frame, not the photo; `4` ticks 4★ and rates nothing; `Del` shows all and deletes
    /// nothing; `Esc` ends the mode, the bar goes again and the keys are the photo's.
    #[test]
    fn the_filter_mode_takes_the_keys_until_esc() {
        let ctrl = Modifiers::COMMAND;
        let mut h = Harness::new(3);
        h.settle();
        assert!(!h.app.bars.toolbar && !bar_shown(&h));
        h.press(Key::F, ctrl);
        assert!(h.app.filter_mode.is_on());
        assert!(bar_shown(&h), "the bar shows for the mode");
        assert!(!h.app.bars.toolbar, "its switch stays off");
        h.press(Key::ArrowRight, NONE);
        assert_eq!(h.app.current, 0, "the arrows move the frame");
        assert_eq!(h.app.filter_mode.cursor, Some(Slot::Camera));
        h.press(Key::Num4, NONE);
        assert!(h.app.options.filter.contains(FilterKind::Stars(4)));
        assert_eq!(
            h.app.filter_mode.cursor,
            Some(Slot::Filter(FilterKind::Stars(4)))
        );
        assert_eq!(h.rating_of(0), None, "the digit rated nothing");
        h.press(Key::X, NONE);
        assert!(h.app.options.filter.contains(FilterKind::Rejected));
        h.press(Key::Delete, NONE);
        assert!(!h.app.options.is_filtered(), "Del shows all");
        assert_eq!(h.app.view.len(), 3, "and deletes nothing");
        assert!(h.app.deletions.countdown(Instant::now()).is_none());
        h.press(Key::Space, NONE);
        assert!(
            h.app.options.filter.contains(FilterKind::Rejected),
            "Space ticks the box"
        );
        h.press(Key::Space, NONE);
        assert!(!h.app.options.is_filtered(), "and unticks it");
        h.press(Key::Escape, NONE);
        assert!(!h.app.filter_mode.is_on());
        assert!(!bar_shown(&h), "the bar goes with the mode");
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(0), Some(Rating::Rejected));
        // It starts where it ended; Ctrl+F ends it too.
        h.press(Key::F, ctrl);
        assert_eq!(
            h.app.filter_mode.cursor,
            Some(Slot::Filter(FilterKind::Rejected))
        );
        h.press(Key::F, ctrl);
        assert!(!h.app.filter_mode.is_on());
    }

    /// `Space` on a choice box opens its list below it: it takes the keys, a pick closes it and
    /// the mode goes on; `Esc` closes only the list.
    #[test]
    fn a_choice_box_opens_its_list() {
        let mut h = Harness::new(3);
        h.settle();
        h.press(Key::F, Modifiers::COMMAND);
        h.settle();
        h.press(Key::Space, NONE);
        h.settle();
        assert_eq!(h.layer(), "list");
        h.press(Key::X, NONE);
        assert_eq!(h.rating_of(0), None);
        assert!(
            !h.app.options.filter.contains(FilterKind::Rejected),
            "the list has the keys"
        );
        h.press(Key::Escape, NONE);
        assert_eq!(h.layer(), "none");
        assert!(h.app.filter_mode.is_on(), "Esc closed only the list");
        // Photos and videos is ticked; one down is photos only.
        h.press(Key::Enter, NONE);
        h.settle();
        assert_eq!(h.layer(), "list");
        h.press(Key::ArrowDown, NONE);
        h.press(Key::Enter, NONE);
        assert_eq!(h.app.options.media, crate::view::Media::Photos);
        assert_eq!(h.layer(), "none", "a pick closes the list");
        assert!(h.app.filter_mode.is_on());
        // The sort's list, from the last box.
        h.press(Key::End, NONE);
        h.settle();
        h.press(Key::Space, NONE);
        h.settle();
        h.press(Key::ArrowDown, NONE);
        h.press(Key::Enter, NONE);
        assert_ne!(h.app.options.sort, crate::view::SortKey::Name);
    }

    /// A straighten session takes the keys from the filter mode and ends it.
    #[test]
    fn an_edit_session_ends_the_filter_mode() {
        let mut h = Harness::new(1);
        h.settle();
        h.press(Key::F, Modifiers::COMMAND);
        assert!(h.app.filter_mode.is_on());
        let path = h.app.view.paths.first().cloned().expect("a photo");
        h.app.edits.session = Some(EditSession::straightening(path));
        h.press(Key::Num4, NONE);
        assert!(!h.app.filter_mode.is_on());
        assert!(!h.app.options.filter.contains(FilterKind::Stars(4)));
    }

    /// `Alt+1` is the menu bar, `Alt+2` the details, `Alt+0` every bar – beside `F10`, `Tab`
    /// and `Shift+Tab`.
    #[test]
    fn alt_digits_switch_the_bars() {
        let alt = Modifiers::ALT;
        let mut h = Harness::new(2);
        h.settle();
        h.press(Key::Num1, alt);
        assert!(h.app.bars.side_bar);
        h.press(Key::Num2, alt);
        assert_ne!(h.app.bars.details, crate::ui::details::DetailsMode::Off);
        h.press(Key::Num0, alt);
        assert!(!h.app.bars.side_bar && !h.app.bars.toolbar && !h.app.bars.filmstrip);
        assert_eq!(h.app.bars.details, crate::ui::details::DetailsMode::Off);
        h.press(Key::Num0, alt);
        assert!(h.app.bars.side_bar && h.app.bars.toolbar && h.app.bars.filmstrip);
        assert_eq!(h.rating_of(0), None, "no Alt digit rates");
        assert_eq!(h.app.current, 0);
    }

    /// In the filter mode `Esc` first brings back a deletion counting down, as everywhere
    /// (`escape_target`); the mode stays on, and the next `Esc` ends it.
    #[test]
    fn escape_in_the_filter_mode_brings_a_deletion_back_first() {
        let mut h = Harness::new(3);
        h.settle();
        h.press(Key::Delete, NONE);
        assert_eq!(h.app.view.len(), 2);
        h.press(Key::F, Modifiers::COMMAND);
        assert!(h.app.filter_mode.is_on());
        h.press(Key::Escape, NONE);
        assert!(h.app.deletions.countdown(Instant::now()).is_none());
        assert_eq!(h.app.view.len(), 3, "the photo is back");
        assert!(h.app.filter_mode.is_on(), "the mode is the next Esc");
        h.press(Key::Escape, NONE);
        assert!(!h.app.filter_mode.is_on());
        // `Ctrl+F` ends the mode at once, whatever counts down.
        h.press(Key::Delete, NONE);
        h.press(Key::F, Modifiers::COMMAND);
        h.press(Key::F, Modifiers::COMMAND);
        assert!(!h.app.filter_mode.is_on());
        assert!(h.app.deletions.countdown(Instant::now()).is_some());
    }

    /// `Ctrl+F` over the faces grid closes the grid: the arrows then move the mode's frame,
    /// not the photo.
    #[test]
    fn ctrl_f_closes_the_faces_grid() {
        let mut h = Harness::new(3);
        h.settle();
        h.app.viewer.faces.grid_open = true;
        h.press(Key::F, Modifiers::COMMAND);
        assert!(!h.app.viewer.faces.grid_open);
        assert!(h.app.filter_mode.is_on());
        h.press(Key::ArrowRight, NONE);
        assert_eq!(h.app.current, 0, "the photo stays");
        assert_eq!(h.app.filter_mode.cursor, Some(Slot::Camera));
    }
}
