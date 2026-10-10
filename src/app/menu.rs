//! The menu bar's content and what its rows do (`ui/side_bar.rs` draws it), the list beside a
//! row, and everything drawn over the photos: the help page, the models card and the
//! confirmation cards.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Rect};

use crate::analysis::Status;
use crate::analysis::manifest::Pack;
use crate::i18n::{self, Lang};
use crate::loader::Lookup;
use crate::metadata::{Label, Rating};
use crate::ui::side_bar::{self, Item, Look, Section, Segment, Segments};
use crate::ui::{confirm, help, models, palette, viewer};

use super::command::{Advance, Command, Source, ZoomKeys};
use super::gate::Change;
use super::layer::Layer;
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

/// The menu bar's state; whether it shows is `show_side_bar`.
pub(super) struct MenuBar {
    /// Its open groups (saved as `side_bar_open`) and whether it has the keyboard.
    pub(super) state: side_bar::State,
    /// `Ctrl+K`, `Ctrl+M` or `E` showed the bar while it is off: it goes again with the
    /// keyboard.
    pub(super) temporary: bool,
    /// `E` asked for the programs' list: it opens beside its row once the bar is drawn.
    pub(super) list_after_draw: Option<ListKind>,
    /// The counts of *Visible photos* (rejected in the folder, deleted on screen), refreshed
    /// with the view and at most every 300 ms – the bar shows them every frame.
    counts: BarCounts,
}

impl MenuBar {
    /// The saved open groups.
    pub(super) fn restore(db: &crate::db::Db) -> Self {
        Self {
            state: side_bar::State::restore(db.setting(SIDE_BAR_OPEN).as_deref()),
            temporary: false,
            list_after_draw: None,
            counts: BarCounts::default(),
        }
    }
}

/// *Visible photos*' counts – rejected photos of the folder, deleted ones shown – kept, so
/// the bar doesn't count every photo every frame (CER-34).
#[derive(Debug, Default)]
struct BarCounts {
    rejected: usize,
    deleted_shown: usize,
    /// The view it was counted for (when it was built), and when.
    counted: Option<(Instant, Instant)>,
}

impl BarCounts {
    /// Counted for the view built at `built`, less than `COUNTS_FRESH` before `now`.
    fn fresh(&self, built: Instant, now: Instant) -> bool {
        self.counted
            .is_some_and(|(counted_for, at)| counted_for == built && now - at < COUNTS_FRESH)
    }
}

/// The menu bar's *Zoom*: 100 % or the whole photo, like `Z`.
const ZOOM_TOGGLE: ZoomKeys = ZoomKeys {
    toggle: true,
    zoom_in: false,
    zoom_out: false,
    fit: false,
    actual: false,
};

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

impl CernoApp {
    /// Opens a confirmation card. `from_models`: the models card comes back afterwards.
    pub(super) fn ask(&mut self, action: ConfirmAction, from_models: bool) {
        self.leave_side_bar();
        self.layer = Layer::Confirm {
            action,
            back_to_models: from_models,
        };
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
        self.layer = Layer::Help(help::Page::Keys);
    }

    /// A card (models, confirmation) or the faces grid is open: keys and the photo's mouse
    /// handling pause.
    pub(super) fn modal_open(&self) -> bool {
        self.layer.is_card() || self.viewer.faces.grid_open
    }

    /// Help page, menus, models card and confirmation – in this order, the last on top.
    pub(super) fn draw_overlays(
        &mut self,
        ctx: &egui::Context,
        window: Rect,
        frames: &[viewer::Frame],
    ) {
        if self.viewer.faces.grid_open {
            let area = self.layout(window).area;
            let state = self.faces_of_current(ctx);
            let out = crate::ui::faces::grid(ctx, area, &state.shown());
            if let Some(face) = out.clicked {
                self.click(ctx, Command::ZoomToFace(face));
            } else if out.close {
                self.viewer.faces.grid_open = false;
            }
        }
        if let Layer::Help(page) = self.layer {
            let update = self.update_line();
            let out = help::overlay(ctx, window, page, i18n::t(), &update);
            if out.check_updates {
                self.click(ctx, Command::CheckUpdatesNow);
            }
            if out.language {
                self.click(ctx, Command::NextLanguage);
            }
            if let Some(page) = out.page
                && let Layer::Help(shown) = &mut self.layer
            {
                *shown = page;
            }
            if out.open_data_folder {
                self.click(ctx, Command::OpenDataFolder);
            }
            if out.close {
                self.layer.close_if(Layer::is_help);
            }
        }
        // The list beside a row of the menu bar: a pick runs; Esc or a click beside it closes
        // it, and the bar keeps the keyboard if it had it – unless `E` opened the list.
        if self.layer.is_list()
            && let Layer::List(mut list) = std::mem::take(&mut self.layer)
        {
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
                self.layer = Layer::List(list);
            } else if by_key && !picked {
                self.leave_side_bar();
            }
            if let Some(action) = out.run {
                // A program opens the photo elsewhere: the bar is done too.
                if picked {
                    self.leave_side_bar();
                }
                self.run(ctx, action, Source::Menu, frames);
            }
        }
        self.draw_name_list_card(ctx, window);
        self.draw_camera_time_card(ctx, window);
        if matches!(self.layer, Layer::Models) {
            let out = models::overlay(ctx, window, &self.analyzer.status(), &self.exiftool_row());
            if out.close {
                self.layer = Layer::None;
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
        if let Layer::Confirm {
            action,
            back_to_models,
        } = self.layer
            && let Some(yes) = confirm::show(
                ctx,
                window,
                &Self::confirm_card(action, &self.analyzer.status()),
            )
        {
            self.layer = if back_to_models {
                Layer::Models
            } else {
                Layer::None
            };
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
    fn side_bar_content(&self) -> side_bar::Bar<Command> {
        let t = i18n::t();
        let top = vec![Item::Row(palette::Row::new(
            Command::Open,
            t.open_folder,
            Some(i18n::with_ctrl("O")),
        ))];
        let mut sections = Vec::new();
        if !self.folder.all.is_empty() {
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
    fn photo_items(&self) -> Vec<Item<Command>> {
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
        let paired = current.is_some_and(|path| self.folder.pairs.companion(path).is_some());
        let jpeg_only = |row: Row<Command>| {
            if paired {
                row.hint(t.pair_edit_jpeg_only)
            } else {
                row
            }
        };

        // "No colour" first, like the filter bar's colours; each with its key (purple has none).
        let mut colours = vec![Segment {
            action: Command::SetLabel(None, Advance::No),
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
                    action: Command::ToggleLabel(label, Advance::No),
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
                    action: Command::Rotate(false),
                    look: Look::Turn(false),
                    tooltip: format!("{} ({}+←)", t.cmd_rotate_ccw, t.key_ctrl),
                    on: false,
                },
                Segment {
                    action: Command::Rotate(true),
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
            Row::new(Command::Undo, t.cmd_undo, Some(i18n::with_ctrl("Z")))
        } else if current.is_some_and(|path| self.is_deleted(path)) {
            Row::new(Command::Restore, t.cmd_restore, Some(i18n::with_ctrl("Z")))
        } else if let Some((label, block)) = self.undo_row() {
            Row::new(Command::Undo, label, Some(i18n::with_ctrl("Z"))).disabled(block)
        } else {
            Row::new(Command::Undo, t.cmd_undo, Some(i18n::with_ctrl("Z"))).disabled(rewrite)
        };
        vec![
            Item::Segments(Segments {
                label: t.bar_colour.to_owned(),
                segments: colours,
                disabled: mark,
            }),
            Item::Row(
                Row::new(Command::ToggleReject(Advance::No), t.cmd_reject, key("X"))
                    .toggle(rating == Rating::Rejected)
                    .disabled(mark),
            ),
            Item::Row(
                Row::new(Command::Compare, t.cmd_compare, key("C"))
                    .toggle(self.viewer.pinned.is_some()),
            ),
            Item::Row(
                Row::new(Command::Quad, t.cmd_quad, Some(i18n::with_shift("C")))
                    .toggle(self.viewer.quad.is_some()),
            ),
            Item::Row(jpeg_only(
                Row::new(Command::Straighten, t.cmd_straighten, key("S")).disabled(edit),
            )),
            Item::Row(jpeg_only(
                Row::new(Command::Crop, t.cmd_crop, key("R")).disabled(edit),
            )),
            Item::Segments(turns),
            Item::Row(undo),
            Item::List(
                Row::new(Command::EditList, t.menu_external, key("E"))
                    .disabled(self.menu_block(Change::External, current)),
            ),
            Item::Row(
                Row::new(Command::AlignCamera, t.bar_align_camera, None)
                    .hint(t.align_camera_hint)
                    .disabled(self.align_block()),
            ),
            Item::Row(
                Row::new(
                    Command::DeleteCurrent,
                    t.selection_delete,
                    Some(t.key_delete.to_owned()),
                )
                .disabled(self.menu_block(Change::Delete, current)),
            ),
        ]
    }

    /// "Visible photos": the file list and the camera clocks, then what acts on every photo
    /// the filter shows. Sort, filters and Top N are in the filter bar.
    fn visible_items(&self) -> Vec<Item<Command>> {
        use palette::Row;
        let t = i18n::t();
        let mut items = vec![
            Item::Row(Row::new(Command::NameList, t.menu_name_list, None).hint(t.name_list_intro)),
            Item::Row(
                Row::new(Command::CameraTime, t.menu_camera_time, None).hint(t.camera_time_intro),
            ),
        ];
        items.extend(self.bulk_rows().into_iter().map(Item::Row));
        items
    }

    /// "View": zoom, the check overlay's three modes, the grid, the faces, full screen. The
    /// bars' switches are buttons in the info bar.
    fn view_items(&self) -> Vec<Item<Command>> {
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
                action: Command::SetOverlay(mode),
                look: Look::Text(short.to_owned()),
                tooltip: format!("{name} (O)"),
                on: self.viewer.overlay == mode,
            })
            .collect(),
            disabled: None,
        };
        vec![
            Item::Row(
                Row::new(Command::Zoom(ZOOM_TOGGLE), t.cmd_zoom, key("Z"))
                    .toggle(self.viewer.zoom.is_zoomed()),
            ),
            Item::Segments(overlay),
            Item::Row(Row::new(Command::Fullscreen, t.cmd_fullscreen, key("F"))),
        ]
    }

    /// "Settings"; a newer release's page last, while there is one.
    fn settings_items(&self) -> Vec<Item<Command>> {
        use palette::Row;
        let t = i18n::t();
        let mut items = vec![
            Item::Row(
                Row::new(Command::AutoAdvance, t.cmd_auto_advance, None)
                    .toggle(self.marks.auto_advance),
            ),
            Item::Row(
                Row::new(
                    Command::Subfolders,
                    t.cmd_subfolders,
                    Some(i18n::with_ctrl("U")),
                )
                .toggle(self.folder.subfolders),
            ),
            Item::Row(
                Row::new(Command::Pairs, t.cmd_pairs, None)
                    .toggle(self.folder.pair_mode)
                    .hint(t.pairs_hint),
            ),
            Item::Row(
                Row::new(Command::UpdateCheck, t.cmd_update_check, None)
                    .toggle(self.updates.enabled)
                    .hint(t.update_check_hint),
            ),
            Item::List(Row::new(
                Command::LanguageList,
                t.menu_language,
                Some(i18n::with_ctrl("L")),
            )),
            Item::Row(Row::new(Command::Models, t.menu_models, None)),
        ];
        // Nothing is downloaded by Cerno: the row opens the release's page in the browser.
        if let Some(version) = self.updates.newer() {
            let label = (t.cmd_update_download)(&version.to_string());
            items.push(Item::Row(Row::new(Command::OpenRelease, label, None)));
        }
        items
    }

    /// "Edit elsewhere": the programs the system offers for the photo's type (the remembered
    /// one ticked, with `E`), one picked by hand, the system's chooser. Picking one opens the
    /// photo there.
    fn editor_rows(&self) -> Vec<palette::Row<Command>> {
        use palette::Row;
        let t = i18n::t();
        let current = self.view.get(self.current).map(PathBuf::as_path);
        let block = self.menu_block(Change::External, current);
        let remembered = self.external.remembered.as_ref();
        let listed = self.editors_for_current();
        let mut rows = Vec::new();
        if let Some(editor) = remembered
            && !listed.iter().any(|e| e.id == editor.id)
        {
            rows.push(
                Row::new(
                    Command::EditRemembered,
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
                    Command::EditWith(index),
                    editor.name.clone(),
                    chosen.then(|| "E".to_owned()),
                )
                .choice(chosen)
                .disabled(block),
            );
        }
        rows.push(Row::new(Command::EditWithOther, t.external_other, None).disabled(block));
        // Linux has no chooser to call: `xdg-open` starts the default program.
        let chooser = if cfg!(windows) {
            t.external_chooser
        } else {
            t.external_default
        };
        rows.push(Row::new(Command::EditWithChooser, chooser, None).disabled(block));
        rows
    }

    /// `E` before a program is remembered: the bar on *Edit elsewhere*, its list open.
    pub(super) fn open_editors_list(&mut self) {
        let index = self
            .photo_items()
            .iter()
            .position(|item| matches!(item, Item::List(row) if row.action == Command::EditList));
        self.open_side_bar_at(PHOTO, index);
        self.menu_bar.list_after_draw = Some(ListKind::Editors);
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
        self.layer = Layer::List(RowList {
            kind,
            at,
            state: palette::State::default(),
            by_key,
        });
    }

    /// The language list, as from its row (tests: it asks the system for nothing).
    #[cfg(test)]
    pub(super) fn open_language_list(&mut self) {
        self.open_row_list(ListKind::Languages, Rect::NOTHING, false);
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
        let listening = self.menu_bar.state.focus && !self.layer.is_open() && !self.modal_open();
        let out = side_bar::show(ui, rect, &mut self.menu_bar.state, &bar, listening);
        if out.folded {
            self.db
                .put_setting(SIDE_BAR_OPEN, &self.menu_bar.state.saved());
        }
        if let Some(kind) = self.menu_bar.list_after_draw.take()
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
            (Command::EditList, Some(at)) => self.open_row_list(ListKind::Editors, at, false),
            (Command::LanguageList, Some(at)) => {
                self.open_row_list(ListKind::Languages, at, false);
            }
            _ => {
                // A plain command gives the keyboard back – and a bar shown only for it goes.
                if !out.keep {
                    self.leave_side_bar();
                }
                self.run(&ctx, action, Source::Menu, frames);
            }
        }
    }

    /// *Visible photos*' counts, again when the view changed and at most every 300 ms.
    fn refresh_bar_counts(&mut self) {
        let now = Instant::now();
        if self.menu_bar.counts.fresh(self.browse.view_built, now) {
            return;
        }
        self.menu_bar.counts.rejected = self.rejected().len();
        self.menu_bar.counts.deleted_shown = self.deleted_shown();
        self.menu_bar.counts.counted = Some((self.browse.view_built, now));
    }

    /// What acts on many photos at once: copy, move and delete what the filter shows, delete
    /// the rejected ones, put back the deleted ones shown. Every row is always there – greyed
    /// out with the reason when it has nothing to do – so nothing moves under the pointer.
    fn bulk_rows(&self) -> Vec<palette::Row<Command>> {
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
        } = self.menu_bar.counts;
        let shown = self.view.len().saturating_sub(deleted);
        vec![
            Row::new(Command::Copy, (t.bulk_copy)(shown), None)
                .hint(t.transfer_copy_cmd)
                .disabled(transfer),
            Row::new(Command::Move, (t.bulk_move)(shown), None)
                .hint(t.transfer_move_cmd)
                .disabled(transfer),
            Row::new(Command::DeleteSelection, (t.bulk_delete)(shown), None)
                .hint(t.bulk_delete_hint)
                .disabled(delete_shown),
            Row::new(
                Command::DeleteRejected,
                (t.cmd_delete_rejected)(rejected),
                None,
            )
            .hint(t.delete_rejected_hint)
            .disabled(delete.or((rejected == 0).then_some(t.bar_none_rejected))),
            Row::new(Command::RestoreShown, (t.bulk_restore)(deleted), None)
                .hint(t.bulk_restore_hint)
                .disabled((deleted == 0).then_some(t.bar_none_deleted)),
        ]
    }
}

/// The languages, the current one ticked (*Language*'s list).
fn language_rows() -> Vec<palette::Row<Command>> {
    Lang::ALL
        .into_iter()
        .map(|lang| {
            palette::Row::new(Command::Language(lang), lang.name(), None)
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

    /// The counts hold for one view and a short while.
    #[test]
    fn bar_counts_go_stale_with_a_new_view_or_time() {
        let built = Instant::now();
        let mut counts = BarCounts::default();
        assert!(!counts.fresh(built, built), "never counted");
        counts.counted = Some((built, built));
        assert!(counts.fresh(built, built + COUNTS_FRESH / 2));
        assert!(!counts.fresh(built, built + COUNTS_FRESH));
        assert!(!counts.fresh(built + COUNTS_FRESH, built), "another view");
    }
}
