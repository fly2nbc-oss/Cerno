//! What a key, a row of the menu bar or a click asks for, and carrying it out. One frame's
//! keys become commands in a fixed order (`KeyInput::commands`); the menu bar's rows and the
//! list beside them carry one; the bars, the filmstrip, the grid and what lies over the photo
//! give one for a click. [`CernoApp::run`] carries one out guarded: a panic becomes the
//! *internal error* notice instead of the end of Cerno.

use std::time::Instant;

use eframe::egui::{self, ViewportCommand};

use crate::i18n::{self, Lang};
use crate::metadata::{Label, Rating};
use crate::overlay;
use crate::transfer::Mode as TransferMode;
use crate::ui::details::{DetailsMode, DetailsTab};
use crate::ui::icons::Panel;
use crate::ui::info_bar::View;
use crate::ui::viewer;

use super::CernoApp;
use super::keys::{Escapable, Escape, escape_target};
use super::layer::Layer;
use super::menu::ConfirmAction;
use super::notice::Notice;
use super::video::VideoKeys;

/// Zoom step for `+`/`-`.
const ZOOM_STEP: f32 = 1.25;

/// Whether a mark moves on to the next photo afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Advance {
    /// As the auto-advance switch says (the plain keys).
    Setting,
    /// Always (the keys with Shift).
    Yes,
    /// Never (the menu bar, the info bar's stars).
    No,
}

/// Where a command comes from. Keys and the menu bar let the grid step aside for what needs the
/// photo; the zoom keys act on the photo under the pointer, the menu's zoom on the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    Keys,
    Menu,
    Mouse,
}

/// The zoom keys of one frame (in the grid `+`/`−` size the cells instead).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct ZoomKeys {
    /// `Z`: 100 % at the pointer, or the whole photo again.
    pub(super) toggle: bool,
    /// `+`/`−`, with or without Ctrl.
    pub(super) zoom_in: bool,
    pub(super) zoom_out: bool,
    /// `Ctrl+0`: the whole photo.
    pub(super) fit: bool,
    /// `Ctrl+1`: 100 %.
    pub(super) actual: bool,
}

impl ZoomKeys {
    fn any(self) -> bool {
        self.toggle || self.zoom_in || self.zoom_out || self.fit || self.actual
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    // Going through the photos.
    Next,
    Prev,
    /// Page Down / Page Up: one photo on, in the grid one screen.
    PageDown,
    PageUp,
    /// `↓` (`down`) or `↑`: a row in the grid and the four-up view.
    Row {
        down: bool,
    },
    First,
    Last,
    /// A click on a cell of the filmstrip, its wheel, a browse arrow.
    GoTo {
        index: usize,
        direction: isize,
    },
    /// A click on a cell of the grid: the cursor goes there without scrolling.
    PickInGrid(usize),
    /// A double click on a cell of the grid: that photo, alone.
    OpenFromGrid(usize),
    /// `Space` and the video keys: on a video shown alone they act on it, elsewhere `Space`
    /// moves on.
    Video {
        keys: VideoKeys,
        space: bool,
    },
    /// `Enter`: in the grid the photo; on the description tab the keyword field.
    Enter,

    // Marks.
    SetRating(Rating, Advance),
    ToggleReject(Advance),
    ToggleLabel(Label, Advance),
    SetLabel(Option<Label>, Advance),
    AutoAdvance,

    // This photo.
    DeleteCurrent,
    Straighten,
    Crop,
    /// A quarter turn; `true` clockwise.
    Rotate(bool),
    Undo,
    /// The deleted photo shown goes back into its folder.
    Restore,
    /// Compare mode: the right photo's camera takes the left one's time.
    AlignCamera,
    /// `E`: the remembered program, else the list of programs.
    EditElsewhere,
    /// *Edit elsewhere*: the list of programs beside the row.
    EditList,
    /// One of the programs the system offers, by its place in the list.
    EditWith(usize),
    EditRemembered,
    EditWithOther,
    EditWithChooser,

    // The view.
    /// `F7`.
    ToggleGrid,
    /// The view switcher: the photo, the grid or the faces.
    ShowView(View),
    /// Ctrl + wheel over the grid.
    ResizeGrid(i32),
    Compare,
    /// `Shift+C`: four photos at once.
    Quad,
    /// `A` / `D` in compare mode: the right / the left photo is rejected.
    KeepLeft,
    KeepRight,
    Zoom(ZoomKeys),
    /// `O`: the next check overlay.
    NextOverlay,
    SetOverlay(overlay::Mode),
    Fullscreen,
    /// `Esc`: one thing at a time (`escape_target`).
    Escape,
    /// `G`: every face over the photo.
    FaceGrid,
    /// A face of the faces grid, large.
    ZoomToFace(usize),
    /// `M`: only photos like this one, or all again.
    Similar,
    /// The filter bar's *Refresh order*.
    RefreshOrder,

    // Visible photos.
    Copy,
    Move,
    DeleteSelection,
    DeleteRejected,
    /// Every deleted photo the filter shows goes back.
    RestoreShown,
    /// *By file list …*: the card for a pasted list.
    NameList,
    /// *Camera time …*: the card with every camera's offset.
    CameraTime,

    // Bars, cards, settings.
    TogglePanel(Panel),
    /// `Shift+Tab`: every bar.
    AllPanels,
    DetailsTab(DetailsTab),
    Help,
    Models,
    /// The filter bar's model status: the download card.
    DownloadModels,
    Open,
    Subfolders,
    /// RAW + JPG as one photo.
    Pairs,
    /// *Language*: the list of languages beside the row.
    LanguageList,
    Language(Lang),
    /// `Ctrl+L`, the start screen's and the help page's language.
    NextLanguage,
    /// *Settings ▸ Check for updates*.
    UpdateCheck,
    /// About Cerno's *Check now*.
    CheckUpdatesNow,
    /// A newer release's page in the browser.
    OpenRelease,
    /// About Cerno's *Open data folder*.
    OpenDataFolder,
}

impl Command {
    /// Needs the single photo: the grid steps aside for it (keys and menu bar).
    fn needs_photo(&self) -> bool {
        match self {
            Self::Straighten
            | Self::Crop
            | Self::Compare
            | Self::NextOverlay
            | Self::SetOverlay(_) => true,
            Self::Zoom(keys) => keys.toggle || keys.actual,
            _ => false,
        }
    }
}

/// What earlier commands of the same frame found out.
#[derive(Default)]
struct Run {
    /// The current photo was a video shown alone when the video keys were read.
    on_video: bool,
}

impl CernoApp {
    /// One command from the menu bar or a click, guarded like the keys.
    pub(super) fn run(
        &mut self,
        ctx: &egui::Context,
        command: Command,
        source: Source,
        frames: &[viewer::Frame],
    ) {
        let what = match source {
            Source::Keys => "key",
            Source::Menu => "menu command",
            Source::Mouse => "click",
        };
        self.guarded(ctx, what, |app| {
            app.execute_all(ctx, &[command], source, frames);
        });
    }

    /// A click's command: no zoom frames (a click zooms through its own handling).
    pub(super) fn click(&mut self, ctx: &egui::Context, command: Command) {
        self.run(ctx, command, Source::Mouse, &[]);
    }

    /// Carries out `commands` in their order. Editing, comparing, zooming and the overlay need
    /// the single photo: from a key or the menu bar the grid steps aside first.
    pub(super) fn execute_all(
        &mut self,
        ctx: &egui::Context,
        commands: &[Command],
        source: Source,
        frames: &[viewer::Frame],
    ) {
        if source != Source::Mouse && self.viewer.grid && commands.iter().any(Command::needs_photo)
        {
            self.set_grid(false);
        }
        let mut run = Run::default();
        for &command in commands {
            self.execute(ctx, command, source, frames, &mut run);
        }
    }

    fn advance(&self, advance: Advance) -> bool {
        match advance {
            Advance::Setting => self.marks.auto_advance,
            Advance::Yes => true,
            Advance::No => false,
        }
    }

    fn execute(
        &mut self,
        ctx: &egui::Context,
        command: Command,
        source: Source,
        frames: &[viewer::Frame],
        run: &mut Run,
    ) {
        match command {
            Command::Next => self.go_to(ctx, self.current.saturating_add(1), 1),
            Command::Prev => self.go_to(ctx, self.current.saturating_sub(1), -1),
            Command::PageDown => self.go_to(ctx, self.current.saturating_add(self.page()), 1),
            Command::PageUp => self.go_to(ctx, self.current.saturating_sub(self.page()), -1),
            Command::Row { down } => self.step_row(ctx, down),
            Command::First => self.go_to(ctx, 0, 1),
            Command::Last => self.go_to(ctx, usize::MAX, -1),
            Command::GoTo { index, direction } => self.go_to(ctx, index, direction),
            Command::PickInGrid(index) => {
                self.go_to(ctx, index, 1);
                // A click is no reason to scroll.
                self.viewer.grid_shown = Some(self.current);
            }
            Command::OpenFromGrid(index) => {
                self.go_to(ctx, index, 1);
                self.set_grid(false);
            }
            Command::Video { keys, space } => {
                // On a video shown alone the video keys act on it; Space plays and pauses there
                // and moves on everywhere else.
                run.on_video = self.current_video().is_some();
                if run.on_video {
                    self.video_keys(ctx, keys);
                } else if space {
                    self.go_to(ctx, self.current.saturating_add(1), 1);
                }
            }
            Command::Enter => {
                // In the grid Enter opens the photo (a video plays with Space only).
                if self.viewer.grid {
                    self.set_grid(false);
                }
                // On the description tab `Enter` puts the cursor into the keyword field.
                if !self.viewer.grid
                    && self.bars.details != DetailsMode::Off
                    && self.bars.details_tab == DetailsTab::Description
                {
                    self.drafts.focus_keyword = true;
                }
            }
            Command::SetRating(rating, advance) => {
                self.set_rating(ctx, rating, self.advance(advance));
            }
            Command::ToggleReject(advance) => self.toggle_reject(ctx, self.advance(advance)),
            Command::ToggleLabel(label, advance) => {
                self.toggle_label(ctx, label, self.advance(advance));
            }
            Command::SetLabel(label, advance) => {
                self.set_label(ctx, label, self.advance(advance));
            }
            Command::AutoAdvance => {
                self.marks.auto_advance = !self.marks.auto_advance;
                self.db.put_flag("auto_advance", self.marks.auto_advance);
            }
            Command::DeleteCurrent => self.delete_current(ctx),
            // Straighten and crop need the photo alone: the four-up view ends first (from the
            // menu bar too – up to 1.11 its rows opened the session unseen behind the four).
            Command::Straighten => {
                if self.viewer.quad.is_some() {
                    self.toggle_quad();
                }
                self.begin_straighten();
            }
            Command::Crop => {
                if self.viewer.quad.is_some() {
                    self.toggle_quad();
                }
                self.begin_crop();
            }
            Command::Rotate(clockwise) => self.rotate_quarter(clockwise),
            Command::Undo => self.undo(ctx),
            Command::Restore => self.restore_current(),
            Command::AlignCamera => self.align_right_camera(ctx),
            Command::EditElsewhere => self.edit_elsewhere(),
            // The lists beside the bar's rows open where the bar draws them.
            Command::EditList => self.open_editors_list(),
            Command::LanguageList => {}
            // Opening another program closes the list.
            Command::EditWith(index) => {
                self.layer.close_if(Layer::is_list);
                self.open_in_listed(index);
            }
            Command::EditRemembered => {
                self.layer.close_if(Layer::is_list);
                self.edit_elsewhere();
            }
            Command::EditWithOther => self.pick_editor(),
            Command::EditWithChooser => self.edit_with_chooser(),
            Command::ToggleGrid => self.set_grid(!self.viewer.grid),
            Command::ShowView(view) => self.show_view(view),
            Command::ResizeGrid(steps) => self.resize_grid(steps),
            Command::Compare => self.toggle_compare(ctx),
            Command::Quad => self.toggle_quad(),
            Command::KeepLeft => self.keep_left(ctx),
            Command::KeepRight => self.keep_right(ctx),
            Command::Zoom(keys) => self.zoom_keys(ctx, keys, source, frames, run),
            Command::NextOverlay => self.set_overlay(self.viewer.overlay.next()),
            Command::SetOverlay(mode) => self.set_overlay(mode),
            Command::Fullscreen => {
                let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
            }
            Command::Escape => self.escape(ctx),
            Command::FaceGrid => self.toggle_face_grid(),
            Command::ZoomToFace(index) => self.zoom_to_face(index),
            Command::Similar => self.toggle_similar(ctx),
            Command::RefreshOrder => self.refresh_order(ctx),
            Command::Copy => self.begin_transfer(ctx, TransferMode::Copy),
            Command::Move => self.begin_transfer(ctx, TransferMode::Move),
            Command::DeleteSelection => self.delete_selection(ctx),
            Command::DeleteRejected => self.delete_rejected(ctx),
            Command::RestoreShown => self.restore_shown(),
            Command::NameList => self.open_name_list(),
            Command::CameraTime => self.open_camera_time(),
            Command::TogglePanel(panel) => self.toggle_panel(panel),
            Command::AllPanels => self.toggle_all_panels(),
            Command::DetailsTab(tab) => self.set_details_tab(tab),
            Command::Help => self.open_help(),
            Command::Models => {
                self.layer = Layer::Models;
                self.exiftool_card_opens();
            }
            Command::DownloadModels => self.ask(ConfirmAction::DownloadModel, false),
            Command::Open => self.pick_folder(ctx),
            Command::Subfolders => self.toggle_subfolders(ctx),
            Command::Pairs => self.toggle_pairs(ctx),
            Command::Language(lang) => self.set_language(ctx, lang),
            Command::NextLanguage => self.switch_language(ctx),
            Command::UpdateCheck => self.toggle_update_check(),
            Command::CheckUpdatesNow => self.start_update_check(ctx),
            Command::OpenRelease => self.open_release_page(ctx),
            Command::OpenDataFolder => {
                let opened =
                    crate::paths::data_dir().and_then(|dir| crate::external::open_folder(&dir));
                if let Err(err) = opened {
                    self.notice = Some(Notice::error(format!("{err:#}")));
                }
            }
        }
    }

    /// A screen of cells in the grid, one photo otherwise.
    fn page(&self) -> usize {
        if self.viewer.grid {
            self.viewer.grid_page.max(1)
        } else {
            1
        }
    }

    /// `↓`/`↑`: a row in the grid; the four-up view's rows hold two photos.
    fn step_row(&mut self, ctx: &egui::Context, down: bool) {
        if self.viewer.grid {
            let target = crate::ui::grid::row_step(
                self.viewer.grid_columns,
                self.current,
                self.view.len(),
                down,
            );
            self.go_to(ctx, target, if down { 1 } else { -1 });
        }
        if self.viewer.quad.is_some() && !self.viewer.grid {
            if down {
                let below = self.current + 2;
                if below < self.view.len() {
                    self.go_to(ctx, below, 1);
                }
            } else if let Some(above) = self.current.checked_sub(2) {
                self.go_to(ctx, above, -1);
            }
        }
    }

    /// The zoom keys: in the grid `+` and `−` size the cells. A key acts on the photo under
    /// the mouse, otherwise on the current (right) one; the menu bar's on the current one.
    fn zoom_keys(
        &mut self,
        ctx: &egui::Context,
        keys: ZoomKeys,
        source: Source,
        frames: &[viewer::Frame],
        run: &Run,
    ) {
        if self.viewer.grid && (keys.zoom_in || keys.zoom_out) {
            self.resize_grid(if keys.zoom_in { 1 } else { -1 });
        }
        let pointer = ctx.pointer_hover_pos().filter(|_| source == Source::Keys);
        let hovered = frames
            .iter()
            .find(|f| pointer.is_some_and(|p| f.area.contains(p)))
            .or(frames.last());
        if let Some(frame) = hovered {
            let pointer = pointer.filter(|p| frame.area.contains(*p));
            let anchor = pointer.unwrap_or(frame.area.center());
            if keys.toggle {
                self.viewer.zoom.toggle(frame, pointer);
            }
            if keys.zoom_in {
                self.viewer.zoom.zoom_by(frame, ZOOM_STEP, anchor);
            }
            if keys.zoom_out {
                self.viewer.zoom.zoom_by(frame, 1.0 / ZOOM_STEP, anchor);
            }
            if keys.fit {
                self.viewer.zoom.fit();
            }
            if keys.actual {
                self.viewer.zoom.actual_size(frame, anchor);
            }
        }
        // A video has no zoom frame (see `ui`): the keys say why nothing happens.
        if run.on_video && keys.any() {
            self.notice = Some(Notice::hint(i18n::t().video_no_zoom));
        }
    }

    /// `Esc` ends one thing at a time (`escape_target`).
    fn escape(&mut self, ctx: &egui::Context) {
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let target = escape_target(Escapable {
            countdown: self.deletions.countdown(Instant::now()).is_some(),
            zoomed: self.viewer.zoom.is_zoomed(),
            grid: self.viewer.grid,
            quad: self.viewer.quad.is_some(),
            compare: self.viewer.pinned.is_some(),
            fullscreen,
        });
        match target {
            Escape::Deletions => self.undo_deletions(ctx),
            Escape::Zoom => self.viewer.zoom.scale = None,
            Escape::Quad => self.toggle_quad(),
            Escape::Compare => self.toggle_compare(ctx),
            Escape::Grid => self.set_grid(false),
            Escape::Fullscreen => ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false)),
            Escape::Notice => self.notice = None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_steps_aside_for_what_needs_the_photo() {
        for command in [
            Command::Straighten,
            Command::Crop,
            Command::Compare,
            Command::NextOverlay,
            Command::SetOverlay(overlay::Mode::Off),
        ] {
            assert!(command.needs_photo(), "{command:?}");
        }
        let toggle = ZoomKeys {
            toggle: true,
            ..ZoomKeys::default()
        };
        let actual = ZoomKeys {
            actual: true,
            ..ZoomKeys::default()
        };
        assert!(Command::Zoom(toggle).needs_photo());
        assert!(Command::Zoom(actual).needs_photo());
        // `+`/`−` size the cells, `Ctrl+0` fits a photo that isn't there.
        for keys in [
            ZoomKeys {
                zoom_in: true,
                ..ZoomKeys::default()
            },
            ZoomKeys {
                fit: true,
                ..ZoomKeys::default()
            },
        ] {
            assert!(!Command::Zoom(keys).needs_photo());
        }
        assert!(!Command::Next.needs_photo());
        assert!(!Command::Quad.needs_photo());
    }
}
