//! The menu bar's content and what its rows do (`ui/side_bar.rs` draws it), the list beside a
//! row, and everything drawn over the photos: the help page, the models card and the
//! confirmation cards.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Rect, ViewportCommand};

use crate::analysis::Status;
use crate::analysis::manifest::Pack;
use crate::i18n::{self, Lang};
use crate::loader::Lookup;
use crate::metadata::{Label, Rating};
use crate::transfer::Mode as TransferMode;
use crate::ui::side_bar::{self, Item, Look, Section, Segment, Segments};
use crate::ui::{confirm, help, models, palette, viewer};

use super::gate::Change;
use super::{CLIP_OFFER_SHOWN, CernoApp};

/// The setting with the menu bar's open groups.
pub(super) const SIDE_BAR_OPEN: &str = "side_bar_open";

/// The groups of the menu bar, by the id their open state is saved under.
const PHOTO: &str = side_bar::FIRST_OPEN;
const VISIBLE: &str = "visible";
const VIEW: &str = "view";
const SETTINGS: &str = "settings";

/// What the list beside a row of the bar lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ListKind {
    /// *Edit elsewhere*: the system's programs, one picked by hand, the system's chooser.
    Editors,
    Languages,
}

/// The list open beside a row of the bar.
pub(super) struct RowList {
    kind: ListKind,
    /// The row it opened from.
    at: Rect,
    state: palette::State,
    /// `E` opened it from the photo: closing it gives the keyboard back as well (a bar shown
    /// only for it goes), while one opened in the bar returns to its row.
    by_key: bool,
}

/// *Visible photos*' counts – rejected photos of the folder, deleted ones shown – kept, so
/// the bar doesn't count every photo every frame (CER-34).
#[derive(Debug, Default)]
pub(super) struct BarCounts {
    rejected: usize,
    deleted_shown: usize,
    /// The view it was counted for (when it was built), and when.
    counted: Option<(Instant, Instant)>,
}

/// How long a count may be old while the view stays the same (a mark changes the rejected
/// count without a new view).
const COUNTS_FRESH: Duration = Duration::from_millis(300);

/// What a confirmation card asks about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfirmAction {
    DownloadModel,
    /// ExifTool into the data folder (Windows).
    DownloadExifTool,
    ResetTaste,
    DeleteModels,
    /// The first start's question: may Cerno check for updates once a day?
    UpdateCheck,
}

/// What a row of the menu bar (or of the list beside it) does. Only what has no other home:
/// stars are in the info bar, sort and filters in the filter bar, the panels' buttons in the
/// info bar, help behind its button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Open,
    Compare,
    Zoom,
    Overlay(crate::overlay::Mode),
    Grid,
    Fullscreen,
    /// *Visible photos ▸ By file list …*: the card for a pasted list.
    NameList,
    /// `Shift+C`: four photos at once.
    Quad,
    /// Settings: RAW + JPG as one photo.
    Pairs,
    /// Compare mode: the right photo's camera takes the left one's time.
    AlignCamera,
    /// *Visible photos ▸ Camera time …*: the card with every camera's offset.
    CameraTime,
    /// `G`: every face over the photo.
    FaceGrid,
    /// *Settings ▸ Check for updates*.
    UpdateCheck,
    /// A newer release's page in the browser.
    OpenRelease,
    Reject,
    DeleteCurrent,
    /// *Edit elsewhere*: the list of programs beside the row.
    EditList,
    /// One of the programs the system offers, by its place in the list.
    EditWith(usize),
    EditRemembered,
    EditWithOther,
    EditWithChooser,
    DeleteRejected,
    Label(Option<Label>),
    AutoAdvance,
    Subfolders,
    Straighten,
    RotateCcw,
    RotateCw,
    Crop,
    Undo,
    /// *Language*: the list of languages beside the row.
    LanguageList,
    Language(Lang),
    Models,
    Copy,
    Move,
    DeleteSelection,
    /// The deleted photo shown goes back into its folder.
    Restore,
    /// Every deleted photo the filter shows goes back.
    RestoreShown,
}

impl CernoApp {
    /// Opens a confirmation card. `from_models`: the models card comes back afterwards.
    pub(super) fn ask(&mut self, action: ConfirmAction, from_models: bool) {
        self.leave_side_bar();
        self.help_open = false;
        self.models_open = false;
        self.confirm = Some((action, from_models));
    }

    fn confirm_card(action: ConfirmAction, status: &Status) -> confirm::Confirm<'static> {
        let t = i18n::t();
        match action {
            ConfirmAction::DownloadModel => {
                let missing = status.missing();
                let size = i18n::size(missing.iter().map(|pack| pack.bytes()).sum());
                confirm::Confirm {
                    title: t.download_title,
                    text: (t.download_text)(
                        missing.contains(&Pack::Clip),
                        missing.contains(&Pack::V25),
                        &size,
                    ),
                    confirm: t.btn_download,
                    cancel: t.btn_cancel,
                    danger: false,
                }
            }
            ConfirmAction::DownloadExifTool => confirm::Confirm {
                title: t.exiftool_title,
                text: (t.exiftool_text)(&i18n::size(crate::tools::EXIFTOOL.bytes)),
                confirm: t.btn_download,
                cancel: t.btn_cancel,
                danger: false,
            },
            ConfirmAction::ResetTaste => confirm::Confirm {
                title: t.confirm_reset_taste_title,
                text: t.confirm_reset_taste_text.to_owned(),
                confirm: t.btn_reset_taste,
                cancel: t.btn_cancel,
                danger: true,
            },
            ConfirmAction::DeleteModels => confirm::Confirm {
                title: t.confirm_delete_models_title,
                text: (t.confirm_delete_models_text)(&i18n::size(status.installed_bytes())),
                confirm: t.btn_delete_models,
                cancel: t.btn_cancel,
                danger: true,
            },
            ConfirmAction::UpdateCheck => confirm::Confirm {
                title: t.update_ask_title,
                text: t.update_ask_text.to_owned(),
                confirm: t.btn_update_yes,
                cancel: t.btn_update_no,
                danger: false,
            },
        }
    }

    fn carry_out(&mut self, ctx: &egui::Context, action: ConfirmAction) {
        match action {
            ConfirmAction::DownloadExifTool => self.download_exiftool(ctx),
            ConfirmAction::DownloadModel => {
                // Asked for: no hint about the models any more.
                self.db.put_setting(CLIP_OFFER_SHOWN, "1");
                self.analyzer.download_missing();
            }
            ConfirmAction::ResetTaste => self.analyzer.reset_taste_learning(),
            ConfirmAction::DeleteModels => self.analyzer.delete_installed_models(),
            ConfirmAction::UpdateCheck => self.answer_update_question(true),
        }
    }

    /// The help page, always on its shortcuts (`H`, `F1`, `?`, the button, the menu).
    pub(super) fn open_help(&mut self) {
        self.help_page = help::Page::Keys;
        self.help_open = true;
    }

    /// A card (models, confirmation) or the faces grid is open: keys and the photo's mouse
    /// handling pause.
    pub(super) fn modal_open(&self) -> bool {
        self.models_open
            || self.confirm.is_some()
            || self.faces.grid_open
            || self.name_list.card_open()
            || self.camera_time.card_open()
    }

    /// Help page, menus, models card and confirmation – in this order, the last on top.
    pub(super) fn draw_overlays(
        &mut self,
        ctx: &egui::Context,
        window: Rect,
        frames: &[viewer::Frame],
    ) {
        if self.faces.grid_open {
            let area = self.layout(window).area;
            let state = self.faces_of_current(ctx);
            let out = crate::ui::faces::grid(ctx, area, &state.shown());
            if let Some(face) = out.clicked {
                self.zoom_to_face(face);
            } else if out.close {
                self.faces.grid_open = false;
            }
        }
        if self.help_open {
            let update = self.update_line();
            let out = help::overlay(ctx, window, self.help_page, i18n::t(), &update);
            if out.check_updates {
                self.start_update_check(ctx);
            }
            if out.language {
                self.switch_language(ctx);
            }
            if let Some(page) = out.page {
                self.help_page = page;
            }
            if out.open_data_folder {
                let opened =
                    crate::paths::data_dir().and_then(|dir| crate::external::open_folder(&dir));
                if let Err(err) = opened {
                    self.notice = Some(super::notice::Notice::error(format!("{err:#}")));
                }
            }
            if out.close {
                self.help_open = false;
            }
        }
        // The list beside a row of the menu bar: a pick runs; Esc or a click beside it closes
        // it, and the bar keeps the keyboard if it had it – unless `E` opened the list.
        if let Some(mut list) = self.row_list.take() {
            let by_key = list.by_key;
            let entries = match list.kind {
                ListKind::Editors => self.editor_rows(),
                ListKind::Languages => language_rows(),
            };
            let out = palette::show(
                ctx,
                window,
                &mut list.state,
                &entries,
                palette::Placement(list.at),
            );
            let picked = out.run.is_some() && out.close;
            if !out.close {
                self.row_list = Some(list);
            } else if by_key && !picked {
                self.leave_side_bar();
            }
            if let Some(action) = out.run {
                // A program opens the photo elsewhere: the bar is done too.
                if picked {
                    self.leave_side_bar();
                }
                self.run(ctx, action, frames);
            }
        }
        self.draw_name_list_card(ctx, window);
        self.draw_camera_time_card(ctx, window);
        if self.models_open {
            let out = models::overlay(ctx, window, &self.analyzer.status(), &self.exiftool_row());
            if out.close {
                self.models_open = false;
            }
            for (asked, action) in [
                (out.download_exiftool, ConfirmAction::DownloadExifTool),
                (out.download, ConfirmAction::DownloadModel),
                (out.reset_taste, ConfirmAction::ResetTaste),
                (out.delete_models, ConfirmAction::DeleteModels),
            ] {
                if asked {
                    self.ask(action, true);
                }
            }
        }
        if let Some((action, back_to_models)) = self.confirm
            && let Some(yes) = confirm::show(
                ctx,
                window,
                &Self::confirm_card(action, &self.analyzer.status()),
            )
        {
            self.confirm = None;
            self.models_open = back_to_models;
            if yes {
                self.carry_out(ctx, action);
            } else if action == ConfirmAction::UpdateCheck {
                // *No* is an answer too: the question doesn't come again.
                self.answer_update_question(false);
            }
        }
    }

    /// The menu bar: *Open folder*, then the groups – what acts on this photo, on the photos
    /// the filter shows, the view, the settings. Switches show a box, choices a tick; rows
    /// that can't run now are greyed out with the reason as their tooltip. Without a folder
    /// only *Open folder* and the settings.
    fn side_bar_content(&self) -> side_bar::Bar<Action> {
        let t = i18n::t();
        let top = vec![Item::Row(palette::Row::new(
            Action::Open,
            t.open_folder,
            Some(i18n::with_ctrl("O")),
        ))];
        let mut sections = Vec::new();
        if !self.all.is_empty() {
            sections.push(Section {
                id: PHOTO,
                title: t.menu_this_photo.to_owned(),
                items: self.photo_items(),
            });
            sections.push(Section {
                id: VISIBLE,
                title: t.menu_visible.to_owned(),
                items: self.visible_items(),
            });
            sections.push(Section {
                id: VIEW,
                title: t.menu_view.to_owned(),
                items: self.view_items(),
            });
        }
        sections.push(Section {
            id: SETTINGS,
            title: t.menu_settings.to_owned(),
            items: self.settings_items(),
        });
        side_bar::Bar { top, sections }
    }

    /// "This photo": colour, rejection, compare, the edits, undo, edit elsewhere, deleting.
    /// The stars are in the info bar, the comment and keywords in Details › Description.
    fn photo_items(&self) -> Vec<Item<Action>> {
        use palette::Row;
        let t = i18n::t();
        let key = |k: &str| Some(k.to_owned());
        let current = self.view.get(self.current).map(PathBuf::as_path);
        let edit = self.menu_block(Change::Edit, current);
        let rewrite = self.menu_block(Change::Rewrite, current);
        let mark = self.menu_block(Change::Mark, current);
        let image = match self.loader.get(self.current) {
            Lookup::Ready(image) => Some(image),
            _ => None,
        };
        let rating = current.map_or(Rating::Unrated, |path| {
            self.rating_of(path, image.as_deref())
        });
        let colour = current.and_then(|path| self.label_of(path, image.as_deref()));
        // A pair's edits change the JPEG alone; the rows say so.
        let paired = current.is_some_and(|path| self.pairs.companion(path).is_some());
        let jpeg_only = |row: Row<Action>| {
            if paired {
                row.hint(t.pair_edit_jpeg_only)
            } else {
                row
            }
        };

        // "No colour" first, like the filter bar's colours; each with its key (purple has none).
        let mut colours = vec![Segment {
            action: Action::Label(None),
            look: Look::NoColour,
            tooltip: t.label_none.to_owned(),
            on: colour.is_none(),
        }];
        colours.extend(
            [
                (Label::Red, Some("6")),
                (Label::Yellow, Some("7")),
                (Label::Green, Some("8")),
                (Label::Blue, Some("9")),
                (Label::Purple, None),
            ]
            .into_iter()
            .map(|(label, digit)| {
                let name = i18n::label_name(label);
                Segment {
                    action: Action::Label(Some(label)),
                    look: Look::Swatch(crate::theme::label_color(label)),
                    tooltip: match digit {
                        Some(digit) => format!("{name} ({digit})"),
                        None => name.to_owned(),
                    },
                    on: colour == Some(label),
                }
            }),
        );
        let turns = Segments {
            label: t.bar_rotate.to_owned(),
            segments: vec![
                Segment {
                    action: Action::RotateCcw,
                    look: Look::Turn(false),
                    tooltip: format!("{} ({}+←)", t.cmd_rotate_ccw, t.key_ctrl),
                    on: false,
                },
                Segment {
                    action: Action::RotateCw,
                    look: Look::Turn(true),
                    tooltip: format!("{} ({}+→)", t.cmd_rotate_cw, t.key_ctrl),
                    on: false,
                },
            ],
            disabled: rewrite,
        };
        // What `Ctrl+Z` does, in its order (`editing::undo_target`): bring back a deletion
        // that counts down, put back the deleted photo shown, take back the session's newest
        // mark or edit – the row names it – or put back an original kept before.
        let undo = if self.deletions.countdown(Instant::now()).is_some() {
            Row::new(Action::Undo, t.cmd_undo, Some(i18n::with_ctrl("Z")))
        } else if current.is_some_and(|path| self.is_deleted(path)) {
            Row::new(Action::Restore, t.cmd_restore, Some(i18n::with_ctrl("Z")))
        } else if let Some((label, block)) = self.undo_row() {
            Row::new(Action::Undo, label, Some(i18n::with_ctrl("Z"))).disabled(block)
        } else {
            Row::new(Action::Undo, t.cmd_undo, Some(i18n::with_ctrl("Z"))).disabled(rewrite)
        };
        vec![
            Item::Segments(Segments {
                label: t.bar_colour.to_owned(),
                segments: colours,
                disabled: mark,
            }),
            Item::Row(
                Row::new(Action::Reject, t.cmd_reject, key("X"))
                    .toggle(rating == Rating::Rejected)
                    .disabled(mark),
            ),
            Item::Row(
                Row::new(Action::Compare, t.cmd_compare, key("C")).toggle(self.pinned.is_some()),
            ),
            Item::Row(
                Row::new(Action::Quad, t.cmd_quad, Some(i18n::with_shift("C")))
                    .toggle(self.quad.is_some()),
            ),
            Item::Row(jpeg_only(
                Row::new(Action::Straighten, t.cmd_straighten, key("S")).disabled(edit),
            )),
            Item::Row(jpeg_only(
                Row::new(Action::Crop, t.cmd_crop, key("R")).disabled(edit),
            )),
            Item::Segments(turns),
            Item::Row(undo),
            Item::List(
                Row::new(Action::EditList, t.menu_external, key("E"))
                    .disabled(self.menu_block(Change::External, current)),
            ),
            Item::Row(
                Row::new(Action::AlignCamera, t.bar_align_camera, None)
                    .hint(t.align_camera_hint)
                    .disabled(self.align_block()),
            ),
            Item::Row(
                Row::new(
                    Action::DeleteCurrent,
                    t.selection_delete,
                    Some(t.key_delete.to_owned()),
                )
                .disabled(self.menu_block(Change::Delete, current)),
            ),
        ]
    }

    /// "Visible photos": the file list and the camera clocks, then what acts on every photo
    /// the filter shows. Sort, filters and Top N are in the filter bar.
    fn visible_items(&self) -> Vec<Item<Action>> {
        use palette::Row;
        let t = i18n::t();
        let mut items = vec![
            Item::Row(Row::new(Action::NameList, t.menu_name_list, None).hint(t.name_list_intro)),
            Item::Row(
                Row::new(Action::CameraTime, t.menu_camera_time, None).hint(t.camera_time_intro),
            ),
        ];
        items.extend(self.bulk_rows().into_iter().map(Item::Row));
        items
    }

    /// "View": zoom, the check overlay's three modes, the grid, the faces, full screen. The
    /// bars' switches are buttons in the info bar.
    fn view_items(&self) -> Vec<Item<Action>> {
        use crate::overlay::Mode;
        use palette::Row;
        let t = i18n::t();
        let key = |k: &str| Some(k.to_owned());
        let overlay = Segments {
            label: t.menu_overlay.to_owned(),
            segments: [
                (Mode::Off, t.overlay_off, t.overlay_off),
                (
                    Mode::Sharpness,
                    t.bar_overlay_sharpness,
                    t.overlay_sharpness,
                ),
                (Mode::Exposure, t.bar_overlay_exposure, t.overlay_exposure),
            ]
            .into_iter()
            .map(|(mode, short, name)| Segment {
                action: Action::Overlay(mode),
                look: Look::Text(short.to_owned()),
                tooltip: format!("{name} (O)"),
                on: self.overlay == mode,
            })
            .collect(),
            disabled: None,
        };
        vec![
            Item::Row(Row::new(Action::Zoom, t.cmd_zoom, key("Z")).toggle(self.zoom.is_zoomed())),
            Item::Segments(overlay),
            Item::Row(Row::new(Action::Grid, t.cmd_grid, key("F7")).toggle(self.grid)),
            Item::Row(Row::new(Action::FaceGrid, t.cmd_face_grid, key("G"))),
            Item::Row(Row::new(Action::Fullscreen, t.cmd_fullscreen, key("F"))),
        ]
    }

    /// "Settings"; a newer release's page last, while there is one.
    fn settings_items(&self) -> Vec<Item<Action>> {
        use palette::Row;
        let t = i18n::t();
        let mut items = vec![
            Item::Row(
                Row::new(Action::AutoAdvance, t.cmd_auto_advance, None).toggle(self.auto_advance),
            ),
            Item::Row(
                Row::new(
                    Action::Subfolders,
                    t.cmd_subfolders,
                    Some(i18n::with_ctrl("U")),
                )
                .toggle(self.subfolders),
            ),
            Item::Row(
                Row::new(Action::Pairs, t.cmd_pairs, None)
                    .toggle(self.pair_mode)
                    .hint(t.pairs_hint),
            ),
            Item::Row(
                Row::new(Action::UpdateCheck, t.cmd_update_check, None)
                    .toggle(self.updates.enabled)
                    .hint(t.update_check_hint),
            ),
            Item::List(Row::new(
                Action::LanguageList,
                t.menu_language,
                Some(i18n::with_ctrl("L")),
            )),
            Item::Row(Row::new(Action::Models, t.menu_models, None)),
        ];
        // Nothing is downloaded by Cerno: the row opens the release's page in the browser.
        if let Some(version) = self.updates.newer() {
            let label = (t.cmd_update_download)(&version.to_string());
            items.push(Item::Row(Row::new(Action::OpenRelease, label, None)));
        }
        items
    }

    /// "Edit elsewhere": the programs the system offers for the photo's type (the remembered
    /// one ticked, with `E`), one picked by hand, the system's chooser. Picking one opens the
    /// photo there.
    fn editor_rows(&self) -> Vec<palette::Row<Action>> {
        use palette::Row;
        let t = i18n::t();
        let current = self.view.get(self.current).map(PathBuf::as_path);
        let block = self.menu_block(Change::External, current);
        let remembered = self.external_editor.as_ref();
        let listed = self.editors_for_current();
        let mut rows = Vec::new();
        if let Some(editor) = remembered
            && !listed.iter().any(|e| e.id == editor.id)
        {
            rows.push(
                Row::new(
                    Action::EditRemembered,
                    editor.name.clone(),
                    Some("E".into()),
                )
                .choice(true)
                .disabled(block),
            );
        }
        for (index, editor) in listed.iter().enumerate() {
            let chosen = remembered.is_some_and(|r| r.id == editor.id);
            rows.push(
                Row::new(
                    Action::EditWith(index),
                    editor.name.clone(),
                    chosen.then(|| "E".to_owned()),
                )
                .choice(chosen)
                .disabled(block),
            );
        }
        rows.push(Row::new(Action::EditWithOther, t.external_other, None).disabled(block));
        // Linux has no chooser to call: `xdg-open` starts the default program.
        let chooser = if cfg!(windows) {
            t.external_chooser
        } else {
            t.external_default
        };
        rows.push(Row::new(Action::EditWithChooser, chooser, None).disabled(block));
        rows
    }

    /// `E` before a program is remembered: the bar on *Edit elsewhere*, its list open.
    pub(super) fn open_editors_list(&mut self) {
        let index = self
            .photo_items()
            .iter()
            .position(|item| matches!(item, Item::List(row) if row.action == Action::EditList));
        self.open_side_bar_at(PHOTO, index);
        self.list_after_draw = Some(ListKind::Editors);
    }

    /// `Ctrl+M`: the bar on *Visible photos* – copy, move, delete what the filter shows.
    pub(super) fn open_visible_photos(&mut self) {
        self.open_side_bar_at(VISIBLE, None);
    }

    /// The list beside a row of the bar. "Edit elsewhere" asks the system for its programs
    /// now, once per file type.
    fn open_row_list(&mut self, kind: ListKind, at: Rect, by_key: bool) {
        if kind == ListKind::Editors {
            self.prepare_editors();
        }
        self.row_list = Some(RowList {
            kind,
            at,
            state: palette::State::default(),
            by_key,
        });
    }

    /// The menu bar, and what its rows ran.
    pub(super) fn draw_side_bar(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frames: &[viewer::Frame],
    ) {
        let ctx = ui.ctx().clone();
        self.refresh_bar_counts();
        let bar = self.side_bar_content();
        // Its keys only while it has the keyboard and nothing lies over it.
        let listening =
            self.side.focus && self.row_list.is_none() && !self.modal_open() && !self.help_open;
        let out = side_bar::show(ui, rect, &mut self.side, &bar, listening);
        if out.folded {
            self.db.put_setting(SIDE_BAR_OPEN, &self.side.saved());
        }
        if let Some(kind) = self.list_after_draw.take()
            && let Some(at) = out.cursor
        {
            self.open_row_list(kind, at, true);
        }
        if out.leave {
            self.leave_side_bar();
        }
        let Some(action) = out.run else {
            return;
        };
        match (action, out.row) {
            (Action::EditList, Some(at)) => self.open_row_list(ListKind::Editors, at, false),
            (Action::LanguageList, Some(at)) => {
                self.open_row_list(ListKind::Languages, at, false);
            }
            _ => {
                // A plain command gives the keyboard back – and a bar shown only for it goes.
                if !out.keep {
                    self.leave_side_bar();
                }
                self.run(&ctx, action, frames);
            }
        }
    }

    /// *Visible photos*' counts, again when the view changed and at most every 300 ms.
    fn refresh_bar_counts(&mut self) {
        let now = Instant::now();
        let fresh = self
            .bar_counts
            .counted
            .is_some_and(|(built, at)| built == self.view_built && now - at < COUNTS_FRESH);
        if fresh {
            return;
        }
        self.bar_counts.rejected = self.rejected().len();
        self.bar_counts.deleted_shown = self.deleted_shown();
        self.bar_counts.counted = Some((self.view_built, now));
    }

    /// What acts on many photos at once: copy, move and delete what the filter shows, delete
    /// the rejected ones, put back the deleted ones shown. Every row is always there – greyed
    /// out with the reason when it has nothing to do – so nothing moves under the pointer.
    fn bulk_rows(&self) -> Vec<palette::Row<Action>> {
        use palette::Row;
        let t = i18n::t();
        let transfer = if self.transfers.is_busy() {
            Some(t.transfer_busy)
        } else {
            self.menu_block(Change::Transfer, None)
        };
        let delete = self.menu_block(Change::Delete, None);
        // Top N shows the best photos: deleting what it shows would delete exactly those.
        let delete_shown = delete.or(self.options.top.map(|_| t.bulk_delete_top));
        // Harmless first: copy, then move, then delete. Each row says how many photos it
        // takes – the ones the filter shows; "rejected" counts the whole folder, and says so.
        // Deleted photos (the 🗑 box) stay where they are; they can only go back.
        let BarCounts {
            rejected,
            deleted_shown: deleted,
            ..
        } = self.bar_counts;
        let shown = self.view.len().saturating_sub(deleted);
        vec![
            Row::new(Action::Copy, (t.bulk_copy)(shown), None)
                .hint(t.transfer_copy_cmd)
                .disabled(transfer),
            Row::new(Action::Move, (t.bulk_move)(shown), None)
                .hint(t.transfer_move_cmd)
                .disabled(transfer),
            Row::new(Action::DeleteSelection, (t.bulk_delete)(shown), None)
                .hint(t.bulk_delete_hint)
                .disabled(delete_shown),
            Row::new(
                Action::DeleteRejected,
                (t.cmd_delete_rejected)(rejected),
                None,
            )
            .hint(t.delete_rejected_hint)
            .disabled(delete.or((rejected == 0).then_some(t.bar_none_rejected))),
            Row::new(Action::RestoreShown, (t.bulk_restore)(deleted), None)
                .hint(t.bulk_restore_hint)
                .disabled((deleted == 0).then_some(t.bar_none_deleted)),
        ]
    }

    fn run(&mut self, ctx: &egui::Context, action: Action, frames: &[viewer::Frame]) {
        self.guarded(ctx, "menu command", |app| {
            app.run_unguarded(ctx, action, frames)
        });
    }

    fn run_unguarded(&mut self, ctx: &egui::Context, action: Action, frames: &[viewer::Frame]) {
        // Like their keys: these need the single photo, so the grid steps aside first.
        if self.grid
            && matches!(
                action,
                Action::Compare
                    | Action::Straighten
                    | Action::Crop
                    | Action::Zoom
                    | Action::Overlay(_)
            )
        {
            self.set_grid(false);
        }
        match action {
            Action::Open => self.pick_folder(ctx),
            Action::Compare => self.toggle_compare(ctx),
            Action::Straighten => self.begin_straighten(),
            Action::RotateCcw => self.rotate_quarter(false),
            Action::RotateCw => self.rotate_quarter(true),
            Action::Crop => self.begin_crop(),
            Action::Undo => self.undo(ctx),
            Action::Zoom => {
                if let Some(frame) = frames.last() {
                    self.zoom.toggle(frame, None);
                }
            }
            Action::Overlay(mode) => self.set_overlay(mode),
            Action::Grid => self.set_grid(!self.grid),
            Action::NameList => self.open_name_list(),
            Action::Quad => self.toggle_quad(),
            Action::Pairs => self.toggle_pairs(ctx),
            Action::AlignCamera => self.align_right_camera(ctx),
            Action::CameraTime => self.open_camera_time(),
            Action::FaceGrid => self.toggle_face_grid(),
            Action::Fullscreen => {
                let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
            }
            Action::Reject => self.toggle_reject(ctx, false),
            Action::DeleteCurrent => self.delete_current(ctx),
            // The lists beside the bar's rows open where the bar draws them.
            Action::EditList => self.open_editors_list(),
            Action::LanguageList => {}
            // Opening another program closes the list.
            Action::EditWith(index) => {
                self.row_list = None;
                self.open_in_listed(index);
            }
            Action::EditRemembered => {
                self.row_list = None;
                self.edit_elsewhere();
            }
            Action::EditWithOther => self.pick_editor(),
            Action::EditWithChooser => self.edit_with_chooser(),
            Action::DeleteRejected => self.delete_rejected(ctx),
            Action::Label(label) => match label {
                Some(label) => self.toggle_label(ctx, label, false),
                None => self.set_label(ctx, None, false),
            },
            Action::AutoAdvance => {
                self.auto_advance = !self.auto_advance;
                self.db.put_flag("auto_advance", self.auto_advance);
            }
            Action::Subfolders => self.toggle_subfolders(ctx),
            Action::Language(lang) => self.set_language(ctx, lang),
            Action::Models => {
                self.models_open = true;
                self.exiftool_card_opens();
            }
            Action::Copy => self.begin_transfer(ctx, TransferMode::Copy),
            Action::Move => self.begin_transfer(ctx, TransferMode::Move),
            Action::DeleteSelection => self.delete_selection(ctx),
            Action::Restore => self.restore_current(),
            Action::RestoreShown => self.restore_shown(),
            Action::UpdateCheck => self.toggle_update_check(),
            Action::OpenRelease => self.open_release_page(ctx),
        }
    }
}

/// The languages, the current one ticked (*Language*'s list).
fn language_rows() -> Vec<palette::Row<Action>> {
    Lang::ALL
        .into_iter()
        .map(|lang| {
            palette::Row::new(Action::Language(lang), lang.name(), None)
                .choice(i18n::current() == lang)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every language is in the list once, and only the current one is ticked.
    #[test]
    fn the_language_list_ticks_the_current_one() {
        let rows = language_rows();
        assert_eq!(rows.len(), Lang::ALL.len());
        let ticked = rows
            .iter()
            .filter(|row| row.mark == palette::Mark::Choice(true))
            .count();
        assert_eq!(ticked, 1);
    }
}
