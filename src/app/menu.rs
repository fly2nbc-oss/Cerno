//! Everything drawn over the photos: the burger menu (`Ctrl+K`), the action menu (`Ctrl+M`),
//! the help page, the models card and the confirmation cards.

use std::path::PathBuf;

use eframe::egui::{self, Rect, ViewportCommand, pos2, vec2};

use crate::analysis::Status;
use crate::analysis::manifest::Pack;
use crate::i18n::{self, Lang};
use crate::loader::Lookup;
use crate::metadata::{Label, Rating};
use crate::transfer::Mode as TransferMode;
use crate::ui::details::DetailsMode;
use crate::ui::icons::Panel;
use crate::ui::palette::RowIcon;
use crate::ui::{confirm, filter_bar, help, models, palette, viewer};
use crate::view::{FilterKind, Media, Scope, SortKey, TOP_LEVELS, ViewOptions};

use super::gate::Change;
use super::{CLIP_OFFER_SHOWN, CernoApp, V25_OFFER_SHOWN};

/// What a confirmation card asks about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfirmAction {
    DownloadModel,
    ResetTaste,
    DeleteModels,
}

/// What a palette entry does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Open,
    Sort(SortKey),
    Filter(FilterKind),
    /// "Hide rejected" (`ViewOptions::hide_rejected`).
    HideRejected,
    FilterClear,
    Media(Media),
    /// Only the best N photos (`ViewOptions::top`).
    Top(u16),
    Refresh,
    EnableAesthetics,
    TopBar,
    Details,
    Filmstrip,
    AllPanels,
    Compare,
    Zoom,
    Overlay(crate::overlay::Mode),
    Grid,
    Fullscreen,
    /// *Filter ▸ By file list …*: the card for a pasted list.
    NameList,
    /// `G`: the faces tab; `Shift+G`: every face over the photo.
    Faces,
    FaceGrid,
    Rate(Rating),
    Reject,
    /// Only photos like this one (`M`), or all again.
    Similar,
    Describe,
    DeleteCurrent,
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
    Language(Lang),
    Models,
    Help,
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
        self.palette = None;
        self.close_action_menu();
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
                    danger: false,
                }
            }
            ConfirmAction::ResetTaste => confirm::Confirm {
                title: t.confirm_reset_taste_title,
                text: t.confirm_reset_taste_text.to_owned(),
                confirm: t.btn_reset_taste,
                danger: true,
            },
            ConfirmAction::DeleteModels => confirm::Confirm {
                title: t.confirm_delete_models_title,
                text: (t.confirm_delete_models_text)(&i18n::size(status.installed_bytes())),
                confirm: t.btn_delete_models,
                danger: true,
            },
        }
    }

    fn carry_out(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::DownloadModel => {
                // Asked for: no hint about the models any more.
                self.db.put_setting(CLIP_OFFER_SHOWN, "1");
                self.db.put_setting(V25_OFFER_SHOWN, "1");
                self.analyzer.download_missing();
            }
            ConfirmAction::ResetTaste => self.analyzer.reset_taste_learning(),
            ConfirmAction::DeleteModels => self.analyzer.delete_installed_models(),
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
            let out = help::overlay(ctx, window, self.help_page);
            if out.language {
                self.switch_language(ctx);
            }
            if let Some(page) = out.page {
                self.help_page = page;
            }
            if out.close {
                self.help_open = false;
            }
        }
        if let Some(mut state) = self.palette.take() {
            // "Edit elsewhere" lists the system's programs: asked once per file type.
            self.prepare_editors();
            let entries = self.menu();
            let out = palette::show(
                ctx,
                window,
                &mut state,
                &entries,
                palette::Placement::BottomRight,
            );
            if !out.close {
                self.palette = Some(state);
            }
            if let Some(action) = out.run {
                self.run(ctx, action, frames);
            }
        }
        if let Some(mut state) = self.action_menu.take() {
            let entries = self.action_entries();
            // Until the filter bar has been drawn once, open under its right end.
            let anchor = self.action_anchor.unwrap_or_else(|| {
                Rect::from_min_size(
                    pos2(window.right() - 100.0, window.top()),
                    vec2(88.0, filter_bar::TOOLBAR_HEIGHT),
                )
            });
            let out = palette::show(
                ctx,
                window,
                &mut state,
                &entries,
                palette::Placement::Below(anchor),
            );
            self.action_menu = Some(state);
            if out.close {
                self.close_action_menu();
            }
            if let Some(action) = out.run {
                self.run(ctx, action, frames);
            }
        }
        self.draw_name_list_card(ctx, window);
        if self.models_open {
            let out = models::overlay(ctx, window, &self.analyzer.status());
            if out.close {
                self.models_open = false;
            }
            for (asked, action) in [
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
                self.carry_out(action);
            }
        }
    }

    /// Burger menu, grouped by what a command acts on: this photo, the photos on screen, the
    /// view, the settings. Switches show a box, choices a tick on the current value; rows that
    /// can't run now are greyed out with the reason as their tooltip.
    fn menu(&self) -> Vec<palette::Entry<Action>> {
        use palette::{Entry, Group, Row};
        let t = i18n::t();
        let key = |k: &str| Some(k.to_owned());
        let mut entries = vec![Entry::Row(Row::new(
            Action::Open,
            t.open_folder,
            Some(i18n::with_ctrl("O")),
        ))];
        if !self.all.is_empty() {
            entries.push(Entry::Group(self.photo_group()));
            let mut visible = vec![
                Entry::Group(Group::new(
                    t.menu_sort,
                    None,
                    SortKey::ALL
                        .into_iter()
                        .map(|sort| {
                            Row::new(Action::Sort(sort), sort.label(), None)
                                .choice(self.options.sort == sort)
                        })
                        .collect(),
                )),
                Entry::Group(Group::nested(t.menu_filter, None, self.filter_rows())),
            ];
            if self.options.depends_on_scores() && self.board.version() != self.view_version {
                visible.push(Entry::Row(Row::new(Action::Refresh, t.refresh_order, None)));
            }
            visible.extend(self.bulk_rows().into_iter().map(Entry::Row));
            entries.push(Entry::Group(Group::nested(t.menu_visible, None, visible)));
            let overlays = crate::overlay::Mode::ALL
                .into_iter()
                .map(|mode| {
                    let label = match mode {
                        crate::overlay::Mode::Off => t.overlay_off,
                        crate::overlay::Mode::Sharpness => t.overlay_sharpness,
                        crate::overlay::Mode::Exposure => t.overlay_exposure,
                    };
                    Row::new(Action::Overlay(mode), label, None).choice(self.overlay == mode)
                })
                .collect();
            let row = |row| Entry::Row(row);
            entries.push(Entry::Group(Group::nested(
                t.menu_view,
                None,
                vec![
                    row(Row::new(Action::TopBar, t.button_toolbar, key("T"))
                        .toggle(self.show_toolbar)),
                    row(Row::new(Action::Details, t.button_details, key("Tab"))
                        .toggle(self.details != DetailsMode::Off)),
                    row(Row::new(Action::Filmstrip, t.button_filmstrip, key("F6"))
                        .toggle(self.show_filmstrip)),
                    row(Row::new(
                        Action::AllPanels,
                        t.cmd_all_panels,
                        Some(i18n::with_shift("Tab")),
                    )),
                    row(Row::new(Action::Zoom, t.cmd_zoom, key("Z")).toggle(self.zoom.is_zoomed())),
                    Entry::Group(Group::new(t.menu_overlay, key("O"), overlays)),
                    row(Row::new(Action::Grid, t.cmd_grid, key("F7")).toggle(self.grid)),
                    row(Row::new(Action::Faces, t.cmd_faces, key("G"))),
                    row(Row::new(
                        Action::FaceGrid,
                        t.cmd_face_grid,
                        Some(i18n::with_shift("G")),
                    )),
                    row(Row::new(Action::Fullscreen, t.cmd_fullscreen, key("F"))),
                ],
            )));
        }
        let languages = Lang::ALL
            .into_iter()
            .map(|lang| {
                Row::new(Action::Language(lang), lang.name(), None).choice(i18n::current() == lang)
            })
            .collect();
        let mut settings = vec![
            Entry::Row(
                Row::new(Action::AutoAdvance, t.cmd_auto_advance, None).toggle(self.auto_advance),
            ),
            Entry::Row(
                Row::new(
                    Action::Subfolders,
                    t.cmd_subfolders,
                    Some(i18n::with_ctrl("U")),
                )
                .toggle(self.subfolders),
            ),
            Entry::Group(Group::new(
                t.menu_language,
                Some(i18n::with_ctrl("L")),
                languages,
            )),
            Entry::Row(Row::new(Action::Models, t.menu_models, None)),
        ];
        let missing = self.analyzer.status().missing();
        if !missing.is_empty() {
            let (label, _) = models::download_label(&missing);
            settings.push(Entry::Row(Row::new(Action::EnableAesthetics, label, None)));
        }
        entries.push(Entry::Group(Group::nested(t.menu_settings, None, settings)));
        entries.push(Entry::Row(Row::new(Action::Help, t.help_title, key("H"))));
        entries
    }

    /// "This photo": stars, rejection and colour, compare, the edits and deleting.
    fn photo_group(&self) -> palette::Group<Action> {
        use palette::{Entry, Group, Row};
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

        // Each row shows what it sets, as the filmstrip does: ☆, then 1–5 stars.
        let stars = [Rating::Unrated]
            .into_iter()
            .chain((1..=5).map(Rating::Stars))
            .map(|value| {
                let (label, digit, icon) = match value {
                    Rating::Stars(n) => ((t.filter_stars)(n), n, RowIcon::Stars(n)),
                    _ => (t.filter_unrated.to_owned(), 0, RowIcon::NoStars),
                };
                Row::new(Action::Rate(value), label, Some(digit.to_string()))
                    .icon(icon)
                    .choice(rating == value)
                    .disabled(mark)
            })
            .collect();
        // "No colour" on top, like "No stars": an empty ring, then the colours.
        let mut labels = vec![
            Row::new(Action::Label(None), t.label_none, None)
                .icon(RowIcon::NoColour)
                .choice(colour.is_none())
                .disabled(mark),
        ];
        labels.extend(
            [
                (Label::Red, Some("6")),
                (Label::Yellow, Some("7")),
                (Label::Green, Some("8")),
                (Label::Blue, Some("9")),
                (Label::Purple, None),
            ]
            .into_iter()
            .map(|(label, shortcut)| {
                Row::new(
                    Action::Label(Some(label)),
                    i18n::label_name(label),
                    shortcut.map(str::to_owned),
                )
                .choice(colour == Some(label))
                .swatch(crate::theme::label_color(label))
                .disabled(mark)
            }),
        );

        let rows = [
            Row::new(Action::Reject, t.cmd_reject, key("X"))
                .toggle(rating == Rating::Rejected)
                .disabled(mark),
            Row::new(Action::Describe, t.cmd_description, key("B")).disabled(mark),
            Row::new(Action::Compare, t.cmd_compare, key("C")).toggle(self.pinned.is_some()),
            Row::new(Action::Similar, t.cmd_similar, key("M")).toggle(self.options.similar),
            Row::new(Action::Straighten, t.cmd_straighten, key("S")).disabled(edit),
            Row::new(Action::Crop, t.cmd_crop, key("R")).disabled(edit),
            Row::new(
                Action::RotateCcw,
                t.cmd_rotate_ccw,
                Some(format!("{}+←", t.key_ctrl)),
            )
            .disabled(rewrite),
            Row::new(
                Action::RotateCw,
                t.cmd_rotate_cw,
                Some(format!("{}+→", t.key_ctrl)),
            )
            .disabled(rewrite),
            // On a deleted photo `Ctrl+Z` undoes the deletion; the row says so in its place.
            if current.is_some_and(|path| self.is_deleted(path)) {
                Row::new(Action::Restore, t.cmd_restore, Some(i18n::with_ctrl("Z")))
            } else {
                Row::new(Action::Undo, t.cmd_undo, Some(i18n::with_ctrl("Z"))).disabled(rewrite)
            },
            Row::new(
                Action::DeleteCurrent,
                t.selection_delete,
                Some(t.key_delete.to_owned()),
            )
            .disabled(self.menu_block(Change::Delete, current)),
        ];
        let mut entries = vec![
            Entry::Group(Group::new(t.menu_stars, None, stars)),
            Entry::Group(Group::new(t.menu_labels, None, labels)),
            Entry::Group(self.editor_group()),
        ];
        entries.extend(rows.into_iter().map(Entry::Row));
        Group::nested(t.menu_this_photo, None, entries)
    }

    /// "Edit elsewhere": the programs the system offers for the photo's type (the remembered
    /// one ticked, with `E`), one picked by hand, the system's chooser. Picking one opens the
    /// photo there and closes the menu.
    fn editor_group(&self) -> palette::Group<Action> {
        use palette::{Group, Row};
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
        Group::new(t.menu_external, Some("E".into()), rows)
    }

    /// The menu opened at "This photo › Edit elsewhere" (`E` before a program is remembered).
    pub(super) fn menu_at_editors(&self) -> palette::State {
        let t = i18n::t();
        let entries = self.menu();
        let path = entries
            .iter()
            .position(|e| e.is_group(t.menu_this_photo))
            .and_then(|top| {
                let palette::Entry::Group(photo) = &entries[top] else {
                    return None;
                };
                let inner = photo
                    .entries
                    .iter()
                    .position(|e| e.is_group(t.menu_external))?;
                Some(vec![top, inner])
            })
            .unwrap_or_default();
        palette::State::opened(path)
    }

    /// The filter boxes, with "Show all" on top while any is ticked.
    fn filter_rows(&self) -> Vec<palette::Entry<Action>> {
        let similar_to = self
            .similar_to
            .as_ref()
            .map(|(path, _)| self.photo_name(path));
        filter_rows(&self.options, similar_to.as_deref(), self.has_deleted())
    }

    /// What acts on many photos at once – the same in "Photos on screen" and in the action
    /// menu: copy, move and delete what the filter shows, delete the rejected ones.
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
        let deleted = self.deleted_shown();
        let shown = self.view.len() - deleted;
        let mut rows = vec![
            Row::new(Action::Copy, (t.bulk_copy)(shown), None)
                .hint(t.transfer_copy_cmd)
                .disabled(transfer),
            Row::new(Action::Move, (t.bulk_move)(shown), None)
                .hint(t.transfer_move_cmd)
                .disabled(transfer),
            Row::new(Action::DeleteSelection, (t.bulk_delete)(shown), None)
                .hint(t.bulk_delete_hint)
                .disabled(delete_shown),
        ];
        let rejected = self.rejected().len();
        if rejected > 0 {
            rows.push(
                Row::new(
                    Action::DeleteRejected,
                    (t.cmd_delete_rejected)(rejected),
                    None,
                )
                .hint(t.delete_rejected_hint)
                .disabled(delete),
            );
        }
        if deleted > 0 {
            rows.push(
                Row::new(Action::RestoreShown, (t.bulk_restore)(deleted), None)
                    .hint(t.bulk_restore_hint),
            );
        }
        rows
    }

    /// The action menu under the filter bar's "Action" button (`Ctrl+M`).
    fn action_entries(&self) -> Vec<palette::Entry<Action>> {
        self.bulk_rows()
            .into_iter()
            .map(palette::Entry::Row)
            .collect()
    }

    /// `Ctrl+M` or the button: the filter bar shows while the action menu is open.
    pub(super) fn open_action_menu(&mut self) {
        self.palette = None;
        self.help_open = false;
        if !self.show_toolbar {
            self.toolbar_before_actions = Some(false);
            self.show_toolbar = true;
        }
        self.action_menu = Some(palette::State::default());
    }

    /// Closing puts the filter bar back the way it was.
    pub(super) fn close_action_menu(&mut self) {
        self.action_menu = None;
        if let Some(shown) = self.toolbar_before_actions.take() {
            self.show_toolbar = shown;
        }
    }

    fn run(&mut self, ctx: &egui::Context, action: Action, frames: &[viewer::Frame]) {
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
            Action::Sort(sort) => self.change_options(ctx, |o| o.sort = sort),
            Action::Filter(kind) => self.change_options(ctx, |o| o.toggle_filter(kind)),
            Action::HideRejected => self.change_options(ctx, ViewOptions::toggle_hide_rejected),
            Action::FilterClear => self.change_options(ctx, ViewOptions::clear_filters),
            Action::Media(media) => self.change_options(ctx, |o| Scope::Media(media).apply(o)),
            Action::Top(n) => self.change_options(ctx, |o| Scope::Top(n).apply(o)),
            Action::Similar => self.toggle_similar(ctx),
            Action::Refresh => self.refresh_order(ctx),
            Action::EnableAesthetics => self.ask(ConfirmAction::DownloadModel, false),
            Action::TopBar => self.toggle_panel(Panel::Top),
            Action::Details => self.toggle_panel(Panel::Right),
            Action::Filmstrip => self.toggle_panel(Panel::Bottom),
            Action::AllPanels => self.toggle_all_panels(),
            Action::Compare => self.toggle_compare(ctx),
            Action::Straighten => self.begin_straighten(),
            Action::RotateCcw => self.rotate_quarter(false),
            Action::RotateCw => self.rotate_quarter(true),
            Action::Crop => self.begin_crop(),
            Action::Undo => self.undo_edit(),
            Action::Zoom => {
                if let Some(frame) = frames.last() {
                    self.zoom.toggle(frame, None);
                }
            }
            Action::Overlay(mode) => self.set_overlay(mode),
            Action::Grid => self.set_grid(!self.grid),
            Action::Faces => self.open_faces(),
            Action::NameList => self.open_name_list(),
            Action::FaceGrid => self.toggle_face_grid(),
            Action::Fullscreen => {
                let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
            }
            Action::Rate(rating) => self.set_rating(ctx, rating, false),
            Action::Reject => self.toggle_reject(ctx, false),
            Action::Describe => self.open_description(ctx),
            Action::DeleteCurrent => self.delete_current(ctx),
            // Choices keep a menu open; opening another program closes it.
            Action::EditWith(index) => {
                self.palette = None;
                self.open_in_listed(index);
            }
            Action::EditRemembered => {
                self.palette = None;
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
                self.db
                    .put_setting("auto_advance", if self.auto_advance { "1" } else { "0" });
            }
            Action::Subfolders => self.toggle_subfolders(ctx),
            Action::Language(lang) => self.set_language(ctx, lang),
            Action::Models => self.models_open = true,
            Action::Copy => self.begin_transfer(ctx, TransferMode::Copy),
            Action::Move => self.begin_transfer(ctx, TransferMode::Move),
            Action::DeleteSelection => self.delete_selection(ctx),
            Action::Restore => self.restore_current(),
            Action::RestoreShown => self.restore_shown(),
            Action::Help => self.open_help(),
        }
    }
}

/// Visible photos ▸ Filter ▸, in the filter bar's order: "Show all" first – always, greyed out
/// while nothing is filtered, so ticking the first filter doesn't push every row down under
/// the cursor – then photos, videos or both and the best N photos (one choice, like the bar's
/// first box), a switch per filter group by group, and "similar photos" last (named after its
/// photo while on). The 🗑 row is greyed out in a folder without deleted photos, unless it is
/// on and has to be switched off.
fn filter_rows(
    options: &ViewOptions,
    similar_to: Option<&str>,
    has_deleted: bool,
) -> Vec<palette::Entry<Action>> {
    use palette::{Entry, Group, Row};
    let t = i18n::t();
    let nothing = !options.is_filtered();
    let clear = Row::new(Action::FilterClear, t.filter_clear, None)
        .disabled(nothing.then_some(t.filter_none_active));
    let similar_label = match similar_to.filter(|_| options.similar) {
        Some(name) => (t.menu_similar_to)(name),
        None => t.menu_similar.to_owned(),
    };
    let similar =
        Row::new(Action::Similar, similar_label, Some("M".to_owned())).toggle(options.similar);
    let name_list = Row::new(Action::NameList, t.menu_name_list, None).hint(t.name_list_intro);
    let current = Scope::of(options);
    let media = Media::ALL.into_iter().map(|media| {
        Entry::Row(
            Row::new(Action::Media(media), media.label(), None)
                .choice(current == Scope::Media(media)),
        )
    });
    let top = Group::new(
        t.menu_top,
        None,
        TOP_LEVELS
            .into_iter()
            .map(|n| {
                let scope = Scope::Top(n);
                let label = match scope.purpose() {
                    Some(purpose) => format!("{} – {purpose}", scope.label()),
                    None => scope.label(),
                };
                Row::new(Action::Top(n), label, None).choice(current == scope)
            })
            .collect(),
    );
    // Before the boxes, like "without ✕" before the rating group in the bar.
    let hide_rejected =
        Row::new(Action::HideRejected, t.filter_hide_rejected, None).toggle(options.hide_rejected);
    let boxes = FilterKind::GROUPS.into_iter().flatten().map(|&kind| {
        let row = Row::new(Action::Filter(kind), kind.label(), None)
            .toggle(options.filter.contains(kind));
        // The bar's symbols in front: ✕, 🗑, ☆, ★ and the colours.
        Entry::Row(match kind {
            FilterKind::Colour(label) => row.swatch(crate::theme::label_color(label)),
            FilterKind::Rejected => row.icon(RowIcon::Reject),
            FilterKind::Unrated => row.icon(RowIcon::NoStars),
            FilterKind::Stars(_) => row.icon(RowIcon::Stars(1)),
            FilterKind::Deleted if !has_deleted && !options.filter.contains(kind) => row
                .icon(RowIcon::Trash)
                .disabled(Some(t.filter_deleted_none)),
            FilterKind::Deleted => row.icon(RowIcon::Trash),
            _ => row,
        })
    });
    std::iter::once(Entry::Row(clear))
        .chain(media)
        .chain(std::iter::once(Entry::Group(top)))
        .chain(std::iter::once(Entry::Row(hide_rejected)))
        .chain(boxes)
        .chain(std::iter::once(Entry::Row(name_list)))
        .chain(std::iter::once(Entry::Row(similar)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The action of each row, `None` for a submenu.
    fn actions(entries: &[palette::Entry<Action>]) -> Vec<Option<Action>> {
        entries
            .iter()
            .map(|entry| match entry {
                palette::Entry::Row(row) => Some(row.action),
                palette::Entry::Group(_) => None,
            })
            .collect()
    }

    fn row(entry: &palette::Entry<Action>) -> &palette::Row<Action> {
        match entry {
            palette::Entry::Row(row) => row,
            palette::Entry::Group(group) => panic!("{} is a submenu", group.label),
        }
    }

    /// Ticking the first filter keeps every row where it was; only "Show all" wakes up.
    #[test]
    fn filter_rows_stay_in_place() {
        let none = ViewOptions::default();
        let mut some = none;
        some.filter.set(FilterKind::Stars(3), true);
        let (before, after) = (
            filter_rows(&none, None, false),
            filter_rows(&some, None, false),
        );
        assert_eq!(actions(&before), actions(&after));
        assert_eq!(row(&before[0]).action, Action::FilterClear);
        assert!(row(&before[0]).disabled.is_some() && row(&after[0]).disabled.is_none());
    }

    /// "Similar photos" is always the last row; while on it names its photo and "Show all" can
    /// switch it off.
    #[test]
    fn similar_photos_is_the_last_filter_row() {
        let off = filter_rows(&ViewOptions::default(), Some("IMG_1.JPG"), false);
        let on_options = ViewOptions {
            similar: true,
            ..ViewOptions::default()
        };
        let on = filter_rows(&on_options, Some("IMG_1.JPG"), false);
        assert_eq!(off.len(), on.len());
        let (last_off, last_on) = (row(off.last().unwrap()), row(on.last().unwrap()));
        assert_eq!(
            (last_off.action, last_on.action),
            (Action::Similar, Action::Similar)
        );
        assert_eq!(last_off.label, i18n::t().menu_similar);
        assert!(last_on.label.contains("IMG_1.JPG"));
        assert!(row(&on[0]).disabled.is_none(), "Show all switches it off");
    }

    /// Photos, videos or both and the best N are one choice: Top N unticks the media rows,
    /// and its submenu follows them, before the filters in the bar's order.
    #[test]
    fn top_is_one_choice_with_the_media_rows() {
        let top = ViewOptions {
            top: Some(50),
            media: Media::Photos,
            ..ViewOptions::default()
        };
        let entries = filter_rows(&top, None, false);
        assert_eq!(
            actions(&entries[..5]),
            vec![
                Some(Action::FilterClear),
                Some(Action::Media(Media::All)),
                Some(Action::Media(Media::Photos)),
                Some(Action::Media(Media::Videos)),
                None,
            ]
        );
        assert!(
            entries[1..4]
                .iter()
                .all(|entry| row(entry).mark != palette::Mark::Choice(true))
        );
        let palette::Entry::Group(group) = &entries[4] else {
            panic!("Top is a submenu");
        };
        let ticked: Vec<Action> = group
            .entries
            .iter()
            .map(row)
            .filter(|row| row.mark == palette::Mark::Choice(true))
            .map(|row| row.action)
            .collect();
        assert_eq!(ticked, vec![Action::Top(50)]);
        assert_eq!(row(&entries[5]).action, Action::HideRejected);
        assert_eq!(
            row(&entries[6]).action,
            Action::Filter(FilterKind::Rejected)
        );
        assert!(row(&entries[0]).disabled.is_none(), "Show all ends Top N");
    }

    /// The 🗑 row is always there – rows must not move under the cursor – and greyed out
    /// while the folder has no deleted photos, unless it is on.
    #[test]
    fn the_deleted_row_waits_for_deleted_photos() {
        let deleted_row = |options: &ViewOptions, has: bool| {
            filter_rows(options, None, has)
                .iter()
                .find_map(|entry| match entry {
                    palette::Entry::Row(row)
                        if row.action == Action::Filter(FilterKind::Deleted) =>
                    {
                        Some(row.disabled.is_some())
                    }
                    _ => None,
                })
                .expect("the row is there")
        };
        let mut on = ViewOptions::default();
        on.filter.set(FilterKind::Deleted, true);
        assert!(deleted_row(&ViewOptions::default(), false), "greyed out");
        assert!(!deleted_row(&ViewOptions::default(), true));
        assert!(!deleted_row(&on, false), "can be switched off");
        assert_eq!(
            filter_rows(&ViewOptions::default(), None, false).len(),
            filter_rows(&ViewOptions::default(), None, true).len()
        );
    }
}
