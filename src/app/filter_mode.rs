//! The filter mode (`Ctrl+F`, ticket CER-39): the filter bar from the keyboard. The bar shows
//! – also when it is switched off – and a frame Cerno draws itself stands on one of its boxes
//! (never egui's focus: `Space` would click that box again). `←`/`→` and `Home`/`End` move it,
//! `Space` or `Enter` tick the box or open its list (photos / videos / Top N, the camera, the
//! sort: a `palette` list below the box), the marking keys tick their own box – `0`–`5` the
//! stars, `6`–`9` the colours, `X` the rejected ones –, `Del` shows all, `Esc` or `Ctrl+F` end
//! the mode. Like a straighten or crop session it is a mode of the keys, not a `Layer`: the
//! photo's keys rest meanwhile, help and the lists still open over it. The box it ended on is
//! where it starts again, for the session.

use eframe::egui::{self, Rect};

use crate::i18n;
use crate::ui::filter_bar::{self, Cursor, Slot, Step};
use crate::ui::palette;
use crate::view::{FilterKind, Media, Scope, SortKey};

use super::CernoApp;
use super::command::Command;
use super::menu::ListKind;
use super::notice::Notice;

/// Which of the filter bar's boxes a list opened from the keyboard chooses for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Choices {
    Scope,
    Camera,
    Sort,
}

/// What one key asks for in the filter mode (`keys::read_filter_keys`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    /// `←`/`→`, `Home`/`End`.
    Move(Step),
    /// `Space` or `Enter`: tick the box, or open its list.
    Activate,
    /// A marking key: `0`–`5`, `6`–`9`, `X` tick their box, and the frame goes there.
    Tick(FilterKind),
    /// `Del`: *Show all* – every filter off, the sort stays.
    ShowAll,
    /// `Esc` or `Ctrl+F`.
    End,
}

/// The filter mode's state; `cursor` is `Some` while it is on.
#[derive(Debug, Default)]
pub(super) struct FilterMode {
    /// The frame's box while the mode is on.
    pub(super) cursor: Option<Slot>,
    /// The box the mode last ended on: it starts there again (this session, never saved).
    last: Option<Slot>,
    /// The frame moved by a key: the boxes scroll so it shows.
    scroll: bool,
    /// Where the frame's box was drawn last: a list opens below it.
    rect: Option<Rect>,
}

impl FilterMode {
    pub(super) fn is_on(&self) -> bool {
        self.cursor.is_some()
    }

    /// On, at the box it ended on last time – the first box the first time.
    fn start(&mut self) {
        self.cursor = Some(self.last.unwrap_or(Slot::Scope));
        self.scroll = true;
        self.rect = None;
    }

    pub(super) fn end(&mut self) {
        if let Some(slot) = self.cursor.take() {
            self.last = Some(slot);
        }
        self.rect = None;
    }
}

impl CernoApp {
    /// `Ctrl+F`: the filter mode on – only with a folder open, as the filter bar needs one –
    /// or off again.
    pub(super) fn toggle_filter_mode(&mut self) {
        if self.filter_mode.is_on() {
            self.filter_mode.end();
        } else if !self.folder.all.is_empty() {
            self.filter_mode.start();
        }
    }

    /// The filter bar's boxes as they are now: the file list's chip only while a list
    /// filters, *Refresh order* only while it shows.
    fn filter_slots(&self) -> Vec<Slot> {
        filter_bar::slots(
            self.options.name_list && self.name_list.counts().is_some(),
            self.order_stale(),
        )
    }

    /// New scores arrived since the view was sorted or filtered by them: *Refresh order*.
    pub(super) fn order_stale(&self) -> bool {
        self.view_options().depends_on_scores() && self.board.version() != self.browse.view_version
    }

    /// The frame for the filter bar this frame (its scroll request is used up).
    pub(super) fn filter_cursor(&mut self) -> Option<Cursor> {
        let slots = self.filter_slots();
        let slot = filter_bar::settle(&slots, self.filter_mode.cursor?);
        self.filter_mode.cursor = Some(slot);
        let scroll = std::mem::take(&mut self.filter_mode.scroll);
        Some(Cursor { slot, scroll })
    }

    /// What the filter bar reported: where the frame's box is, and a click – the frame goes
    /// to the box clicked.
    pub(super) fn filter_bar_drawn(&mut self, clicked: Option<Slot>, rect: Option<Rect>) {
        if !self.filter_mode.is_on() {
            return;
        }
        self.filter_mode.rect = rect;
        if let Some(slot) = clicked {
            self.filter_mode.cursor = Some(slot);
        }
    }

    /// The keys of one frame in the filter mode, in their order.
    pub(super) fn handle_filter_keys(&mut self, ctx: &egui::Context, actions: &[Action]) {
        for &action in actions {
            let Some(cursor) = self.filter_mode.cursor else {
                return;
            };
            match action {
                Action::Move(step) => {
                    let slots = self.filter_slots();
                    self.filter_mode.cursor = Some(filter_bar::step(&slots, cursor, step));
                    self.filter_mode.scroll = true;
                }
                Action::Activate => {
                    self.activate_slot(ctx, filter_bar::settle(&self.filter_slots(), cursor))
                }
                Action::Tick(kind) => {
                    self.filter_mode.cursor = Some(Slot::Filter(kind));
                    self.filter_mode.scroll = true;
                    self.activate_slot(ctx, Slot::Filter(kind));
                }
                Action::ShowAll => {
                    if self.options.is_filtered() {
                        self.change_options(ctx, |options| options.clear_filters());
                    }
                }
                Action::End => {
                    self.filter_mode.end();
                    return;
                }
            }
        }
    }

    /// `Space` / `Enter` on a box: what a click on it does – a list below the box for the
    /// three choices, else the box ticked or unticked. A box greyed out says why.
    fn activate_slot(&mut self, ctx: &egui::Context, slot: Slot) {
        let t = i18n::t();
        let open = |app: &mut Self, choices| {
            let at = app.filter_mode.rect;
            app.open_row_list(ListKind::Filter(choices), at);
        };
        match slot {
            Slot::Scope => open(self, Choices::Scope),
            Slot::Sort => open(self, Choices::Sort),
            Slot::Camera => {
                let cameras = self.cameras();
                if cameras.len() > 1 || self.options.camera.is_some() {
                    open(self, Choices::Camera);
                } else {
                    let reason = if cameras.is_empty() {
                        t.camera_none
                    } else {
                        t.camera_one
                    };
                    self.notice = Some(Notice::hint(reason));
                }
            }
            Slot::WithoutRejected => self.change_options(ctx, |o| o.toggle_hide_rejected()),
            Slot::Filter(FilterKind::Deleted)
                if !self.has_deleted() && !self.options.filter.contains(FilterKind::Deleted) =>
            {
                self.notice = Some(Notice::hint(t.filter_deleted_none));
            }
            Slot::Filter(kind) => self.change_options(ctx, |o| o.toggle_filter(kind)),
            Slot::Similar => self.toggle_similar(ctx),
            Slot::NameList => self.change_options(ctx, |o| o.name_list = false),
            Slot::Refresh => self.refresh_order(ctx),
        }
    }

    /// The rows of a filter box's list: the box's choices, the current one ticked. "Videos
    /// only" is greyed out in a folder without videos, as in the box's own list.
    pub(super) fn filter_choice_rows(&mut self, choices: Choices) -> Vec<palette::Row<Command>> {
        use palette::Row;
        let t = i18n::t();
        match choices {
            Choices::Scope => {
                let current = Scope::of(&self.options);
                let has_videos = self.has_videos();
                Scope::all()
                    .map(|scope| {
                        let label = match scope.purpose() {
                            Some(purpose) => format!("{}  ·  {purpose}", scope.label()),
                            None => scope.label(),
                        };
                        let choosable =
                            has_videos || scope != Scope::Media(Media::Videos) || scope == current;
                        Row::new(Command::SetScope(scope), label, None)
                            .choice(scope == current)
                            .disabled((!choosable).then_some(t.media_no_videos))
                    })
                    .collect()
            }
            Choices::Camera => {
                let cameras = self.cameras();
                let mut rows = vec![
                    Row::new(Command::FilterCamera(None), t.camera_all, None)
                        .choice(self.options.camera.is_none()),
                ];
                rows.extend(cameras.iter().map(|entry| {
                    // A body below its model, indented as in the box's own list.
                    let indent = if entry.pick.serial.is_some() {
                        "    "
                    } else {
                        ""
                    };
                    let label = format!(
                        "{indent}{}  ·  {}",
                        entry.label(),
                        (t.camera_time_photos)(entry.count)
                    );
                    Row::new(Command::FilterCamera(Some(entry.pick)), label, None)
                        .choice(self.options.camera == Some(entry.pick))
                }));
                rows
            }
            Choices::Sort => SortKey::ALL
                .into_iter()
                .map(|key| {
                    Row::new(Command::SetSort(key), key.label(), None)
                        .choice(self.options.sort == key)
                })
                .collect(),
        }
    }

    /// The pill under the photo while the mode is on: the box the frame stands on, and the
    /// mode's keys.
    pub(super) fn filter_mode_caption(&mut self) -> Option<String> {
        let slot = self.filter_mode.cursor?;
        let t = i18n::t();
        Some(match slot {
            Slot::Scope => Scope::of(&self.options).label(),
            Slot::Camera => match self.options.camera {
                Some(pick) => {
                    let cameras = self.cameras();
                    self.camera_label(pick, &cameras)
                }
                None => t.camera_all.to_owned(),
            },
            Slot::WithoutRejected => t.filter_hide_rejected.to_owned(),
            Slot::Filter(kind) => kind.label(),
            Slot::Similar => t.filter_similar.to_owned(),
            Slot::NameList => self
                .name_list
                .counts()
                .map(|(found, total)| {
                    // The chip's own × is for the mouse.
                    let chip = (t.name_list_chip)(found, total);
                    chip.trim_end_matches('×').trim_end().to_owned()
                })
                .unwrap_or_default(),
            Slot::Refresh => t.refresh_order.to_owned(),
            Slot::Sort => (t.sort)(self.options.sort.label()),
        })
    }
}
