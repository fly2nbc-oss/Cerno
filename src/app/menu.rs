//! Everything drawn over the photos: the burger menu (`Ctrl+K`), the action menu (`Ctrl+M`),
//! the help page, the models card and the confirmation cards.

use std::path::PathBuf;

use eframe::egui::{self, Rect, ViewportCommand, pos2, vec2};

use crate::i18n::{self, Lang};
use crate::loader::Lookup;
use crate::metadata::{Label, Rating};
use crate::transfer::Mode as TransferMode;
use crate::ui::details::{DetailsMode, all_expanded};
use crate::ui::icons::Panel;
use crate::ui::{confirm, filter_bar, help, models, palette, viewer};
use crate::view::{FilterKind, SortKey, ViewOptions};

use super::gate::Change;
use super::{CLIP_OFFER_SHOWN, CernoApp};

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
    FilterClear,
    Refresh,
    EnableAesthetics,
    TopBar,
    Details,
    Explanations,
    Filmstrip,
    AllPanels,
    Compare,
    Zoom,
    Overlay(crate::overlay::Mode),
    Grid,
    Fullscreen,
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

    fn confirm_card(action: ConfirmAction) -> confirm::Confirm<'static> {
        let t = i18n::t();
        match action {
            ConfirmAction::DownloadModel => confirm::Confirm {
                title: t.download_title,
                text: (t.download_text)(crate::analysis::aesthetic::MODEL_BYTES as f64 / 1e9),
                confirm: t.btn_download,
                danger: false,
            },
            ConfirmAction::ResetTaste => confirm::Confirm {
                title: t.confirm_reset_taste_title,
                text: t.confirm_reset_taste_text.to_owned(),
                confirm: t.btn_reset_taste,
                danger: true,
            },
            ConfirmAction::DeleteModels => confirm::Confirm {
                title: t.confirm_delete_models_title,
                text: t.confirm_delete_models_text.to_owned(),
                confirm: t.btn_delete_models,
                danger: true,
            },
        }
    }

    fn carry_out(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::DownloadModel => {
                self.db.put_setting(CLIP_OFFER_SHOWN, "1");
                self.analyzer.download_model();
            }
            ConfirmAction::ResetTaste => self.analyzer.reset_taste_learning(),
            ConfirmAction::DeleteModels => self.analyzer.delete_installed_models(),
        }
    }

    /// A card (models, confirmation) is open: keys and the photo's mouse handling pause.
    pub(super) fn modal_open(&self) -> bool {
        self.models_open || self.confirm.is_some()
    }

    /// Help page, menus, models card and confirmation – in this order, the last on top.
    pub(super) fn draw_overlays(
        &mut self,
        ctx: &egui::Context,
        window: Rect,
        frames: &[viewer::Frame],
    ) {
        if self.help_open {
            let out = help::overlay(ctx, window);
            if out.language {
                self.switch_language(ctx);
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
            && let Some(yes) = confirm::show(ctx, window, &Self::confirm_card(action))
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
                Entry::Group(Group::new(t.menu_filter, None, self.filter_rows())),
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
                    row(
                        Row::new(Action::Explanations, t.cmd_explanations, key("I")).toggle(
                            self.details != DetailsMode::Off
                                && all_expanded(&self.details_expanded),
                        ),
                    ),
                    row(Row::new(Action::Zoom, t.cmd_zoom, key("Z")).toggle(self.zoom.is_zoomed())),
                    Entry::Group(Group::new(t.menu_overlay, key("O"), overlays)),
                    row(Row::new(Action::Grid, t.cmd_grid, key("F7")).toggle(self.grid)),
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
                Row::new(Action::Subfolders, t.cmd_subfolders, None).toggle(self.subfolders),
            ),
            Entry::Group(Group::new(
                t.menu_language,
                Some(i18n::with_ctrl("L")),
                languages,
            )),
            Entry::Row(Row::new(Action::Models, t.menu_models, None)),
        ];
        if self.analyzer.status().aesthetics == crate::analysis::ModelState::Missing {
            settings.push(Entry::Row(Row::new(
                Action::EnableAesthetics,
                t.enable_aesthetics,
                None,
            )));
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

        let stars = [Rating::Unrated]
            .into_iter()
            .chain((1..=5).map(Rating::Stars))
            .map(|value| {
                let (label, digit) = match value {
                    Rating::Stars(n) => ((t.filter_stars)(n), n),
                    _ => (t.filter_unrated.to_owned(), 0),
                };
                Row::new(Action::Rate(value), label, Some(digit.to_string()))
                    .choice(rating == value)
                    .disabled(mark)
            })
            .collect();
        let mut labels: Vec<Row<Action>> = [
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
        })
        .collect();
        labels.push(
            Row::new(Action::Label(None), t.label_none, None)
                .choice(colour.is_none())
                .disabled(mark),
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
            Row::new(Action::Undo, t.cmd_undo, Some(i18n::with_ctrl("Z"))).disabled(rewrite),
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
    fn filter_rows(&self) -> Vec<palette::Row<Action>> {
        let similar_to = self
            .similar_to
            .as_ref()
            .map(|(path, _)| self.photo_name(path));
        filter_rows(&self.options, similar_to.as_deref())
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
        let mut rows = vec![
            Row::new(Action::Copy, t.transfer_copy, None).disabled(transfer),
            Row::new(Action::Move, t.transfer_move, None).disabled(transfer),
            Row::new(Action::DeleteSelection, t.selection_delete, None).disabled(delete),
        ];
        let rejected = self.rejected().len();
        if rejected > 0 {
            rows.push(
                Row::new(
                    Action::DeleteRejected,
                    (t.cmd_delete_rejected)(rejected),
                    None,
                )
                .disabled(delete),
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
            Action::Filter(kind) => self.change_options(ctx, |o| o.filter.toggle(kind)),
            Action::FilterClear => self.change_options(ctx, |o| {
                o.filter.clear();
                o.similar = false;
            }),
            Action::Similar => self.toggle_similar(ctx),
            Action::Refresh => self.rebuild_view(ctx, None),
            Action::EnableAesthetics => self.ask(ConfirmAction::DownloadModel, false),
            Action::TopBar => self.toggle_panel(Panel::Top),
            Action::Details => self.toggle_panel(Panel::Right),
            Action::Explanations => self.toggle_explanations(),
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
            Action::Subfolders => {
                self.subfolders = !self.subfolders;
                self.db
                    .put_setting("subfolders", if self.subfolders { "1" } else { "0" });
                if let Some(dir) = self.dir.clone() {
                    self.open(ctx, &dir);
                }
            }
            Action::Language(lang) => self.set_language(ctx, lang),
            Action::Models => self.models_open = true,
            Action::Copy => self.begin_transfer(ctx, TransferMode::Copy),
            Action::Move => self.begin_transfer(ctx, TransferMode::Move),
            Action::DeleteSelection => self.delete_selection(ctx),
            Action::Help => self.help_open = true,
        }
    }
}

/// Visible photos ▸ Filter ▸: "Show all" first – always, greyed out while nothing is filtered,
/// so ticking the first filter doesn't push every row down under the cursor – then a switch per
/// filter, and "similar photos" last (named after its photo while on).
fn filter_rows(options: &ViewOptions, similar_to: Option<&str>) -> Vec<palette::Row<Action>> {
    use palette::Row;
    let t = i18n::t();
    let nothing = options.filter.is_all() && !options.similar;
    let clear = Row::new(Action::FilterClear, t.filter_clear, None)
        .disabled(nothing.then_some(t.filter_none_active));
    let similar_label = match similar_to.filter(|_| options.similar) {
        Some(name) => (t.menu_similar_to)(name),
        None => t.menu_similar.to_owned(),
    };
    let similar =
        Row::new(Action::Similar, similar_label, Some("M".to_owned())).toggle(options.similar);
    std::iter::once(clear)
        .chain(FilterKind::ALL.into_iter().map(|kind| {
            let row = Row::new(Action::Filter(kind), kind.label(), None)
                .toggle(options.filter.contains(kind));
            match kind {
                FilterKind::Colour(label) => row.swatch(crate::theme::label_color(label)),
                _ => row,
            }
        }))
        .chain(std::iter::once(similar))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ticking the first filter keeps every row where it was; only "Show all" wakes up.
    #[test]
    fn filter_rows_stay_in_place() {
        let none = ViewOptions::default();
        let mut some = none;
        some.filter.set(FilterKind::Stars(3), true);
        let (before, after) = (filter_rows(&none, None), filter_rows(&some, None));
        let actions =
            |rows: &[palette::Row<Action>]| rows.iter().map(|r| r.action).collect::<Vec<_>>();
        assert_eq!(actions(&before), actions(&after));
        assert_eq!(before[0].action, Action::FilterClear);
        assert!(before[0].disabled.is_some() && after[0].disabled.is_none());
    }

    /// "Similar photos" is always the last row; while on it names its photo and "Show all" can
    /// switch it off.
    #[test]
    fn similar_photos_is_the_last_filter_row() {
        let off = filter_rows(&ViewOptions::default(), Some("IMG_1.JPG"));
        let on_options = ViewOptions {
            similar: true,
            ..ViewOptions::default()
        };
        let on = filter_rows(&on_options, Some("IMG_1.JPG"));
        assert_eq!(off.len(), on.len());
        let (last_off, last_on) = (off.last().unwrap(), on.last().unwrap());
        assert_eq!(
            (last_off.action, last_on.action),
            (Action::Similar, Action::Similar)
        );
        assert_eq!(last_off.label, i18n::t().menu_similar);
        assert!(last_on.label.contains("IMG_1.JPG"));
        assert!(on[0].disabled.is_none(), "Show all switches it off");
    }
}
