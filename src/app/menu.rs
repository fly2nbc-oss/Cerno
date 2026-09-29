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
use crate::view::{FilterKind, SortKey};

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
    Fullscreen,
    Rate(Rating),
    Reject,
    Describe,
    DeleteCurrent,
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
            entries.push(Entry::Group(Group::new(
                t.menu_view,
                None,
                vec![
                    Row::new(Action::TopBar, t.button_toolbar, key("T")).toggle(self.show_toolbar),
                    Row::new(Action::Details, t.button_details, key("Tab"))
                        .toggle(self.details != DetailsMode::Off),
                    Row::new(Action::Filmstrip, t.button_filmstrip, key("F6"))
                        .toggle(self.show_filmstrip),
                    Row::new(
                        Action::AllPanels,
                        t.cmd_all_panels,
                        Some(i18n::with_shift("Tab")),
                    ),
                    Row::new(Action::Explanations, t.cmd_explanations, key("I")).toggle(
                        self.details != DetailsMode::Off && all_expanded(&self.details_expanded),
                    ),
                    Row::new(Action::Zoom, t.cmd_zoom, key("Z")).toggle(self.zoom.is_zoomed()),
                    Row::new(Action::Fullscreen, t.cmd_fullscreen, key("F")),
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
        ];
        entries.extend(rows.into_iter().map(Entry::Row));
        Group::nested(t.menu_this_photo, None, entries)
    }

    /// The filter boxes, with "Show all" on top while any is ticked.
    fn filter_rows(&self) -> Vec<palette::Row<Action>> {
        use palette::Row;
        let mut filters: Vec<Row<Action>> = FilterKind::ALL
            .into_iter()
            .map(|kind| {
                let row = Row::new(Action::Filter(kind), kind.label(), None)
                    .toggle(self.options.filter.contains(kind));
                match kind {
                    FilterKind::Colour(label) => row.swatch(crate::theme::label_color(label)),
                    _ => row,
                }
            })
            .collect();
        if !self.options.filter.is_all() {
            filters.insert(
                0,
                Row::new(Action::FilterClear, i18n::t().filter_clear, None),
            );
        }
        filters
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
        match action {
            Action::Open => self.pick_folder(ctx),
            Action::Sort(sort) => self.change_options(ctx, |o| o.sort = sort),
            Action::Filter(kind) => self.change_options(ctx, |o| o.filter.toggle(kind)),
            Action::FilterClear => self.change_options(ctx, |o| o.filter.clear()),
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
            Action::Fullscreen => {
                let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
            }
            Action::Rate(rating) => self.set_rating(ctx, rating, false),
            Action::Reject => self.toggle_reject(ctx, false),
            Action::Describe => self.open_description(ctx),
            Action::DeleteCurrent => self.delete_current(ctx),
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
