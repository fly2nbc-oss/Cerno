//! Menu bar, filter bar, details panel and filmstrip on and off; the language switch.

use std::time::{Duration, Instant};

use eframe::egui::{self, Id, LayerId, Order, Rect};

use crate::i18n::{self, Lang};
use crate::ui::details::DetailsMode;
use crate::ui::icons::Panel;
use crate::ui::overlays;

use super::CernoApp;

/// How long the flag stays after switching the language, and how long it fades out.
const LANGUAGE_FLASH: Duration = Duration::from_millis(1400);

const LANGUAGE_FADE: Duration = Duration::from_millis(450);

/// Which optional parts of the window are shown (the info bar always is).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Panels {
    side_bar: bool,
    toolbar: bool,
    details: bool,
    filmstrip: bool,
}

impl CernoApp {
    fn panels(&self) -> Panels {
        Panels {
            side_bar: self.show_side_bar,
            toolbar: self.show_toolbar,
            details: self.details != DetailsMode::Off,
            filmstrip: self.show_filmstrip,
        }
    }

    pub(super) fn save_panels(&self) {
        self.db.put_flag("side_bar", self.show_side_bar);
        self.db.put_flag("top_bar", self.show_toolbar);
        self.db.put_flag("filmstrip", self.show_filmstrip);
        self.db.put_setting("details_mode", self.details.id());
    }

    /// The menu bar shows: switched on, or for the keyboard (`Ctrl+K`, `Ctrl+M`, `E`).
    pub(super) fn side_bar_shown(&self) -> bool {
        self.show_side_bar || self.side_bar_temporary
    }

    /// `Ctrl+K`: the keyboard goes to the menu bar – shown for it while the bar is off.
    pub(super) fn open_side_bar(&mut self) {
        self.help_open = false;
        self.side_bar_temporary = !self.show_side_bar;
        self.side.take_keyboard();
    }

    /// `Ctrl+M` (*Visible photos*), `E` (*Edit elsewhere*): the same, on a group or a row.
    pub(super) fn open_side_bar_at(&mut self, section: &'static str, item: Option<usize>) {
        self.open_side_bar();
        self.side.take_keyboard_at(section, item);
    }

    /// The keyboard goes back to the photo; a bar shown only for it goes too, with its list.
    pub(super) fn leave_side_bar(&mut self) {
        self.side.release();
        self.row_list = None;
        self.list_after_draw = None;
        self.side_bar_temporary = false;
    }

    pub(super) fn set_details(&mut self, mode: DetailsMode) {
        self.details = mode;
        if mode != DetailsMode::Off {
            self.details_last = mode;
        }
    }

    /// Buttons and `T` / `Tab` / `F6`. The details panel comes back at the stage it had. The
    /// menu button switches the menu bar: a bar shown only for the keyboard stays, switched on.
    pub(super) fn toggle_panel(&mut self, panel: Panel) {
        match panel {
            Panel::Left if self.side_bar_temporary => {
                self.side_bar_temporary = false;
                self.show_side_bar = true;
            }
            Panel::Left => {
                self.show_side_bar = !self.show_side_bar;
                if !self.show_side_bar {
                    self.leave_side_bar();
                }
            }
            Panel::Top => self.show_toolbar = !self.show_toolbar,
            Panel::Bottom => self.show_filmstrip = !self.show_filmstrip,
            Panel::Right if self.details == DetailsMode::Off => self.details = self.details_last,
            Panel::Right => self.details = DetailsMode::Off,
        }
        self.save_panels();
    }

    /// `Shift+Tab`: hides menu bar, filter bar, details and filmstrip together – or shows all
    /// four if none is ("all" means all). The info bar stays either way.
    pub(super) fn toggle_all_panels(&mut self) {
        let Panels {
            side_bar,
            toolbar,
            details,
            filmstrip,
        } = self.panels();
        let show = !(side_bar || toolbar || details || filmstrip);
        if !show {
            self.leave_side_bar();
        }
        self.show_side_bar = show;
        self.show_toolbar = show;
        self.show_filmstrip = show;
        self.details = if show {
            self.details_last
        } else {
            DetailsMode::Off
        };
        self.save_panels();
    }

    pub(super) fn switch_language(&mut self, ctx: &egui::Context) {
        self.set_language(ctx, i18n::current().next());
    }

    pub(super) fn set_language(&mut self, ctx: &egui::Context, lang: Lang) {
        i18n::set(lang);
        self.db.put_setting("language", lang.code());
        self.language_flash = Some(Instant::now());
        ctx.request_repaint();
    }

    /// The flag in the middle of the photo area for a moment after switching.
    pub(super) fn draw_language_flash(&mut self, ctx: &egui::Context, area: Rect) {
        let Some(since) = self.language_flash else {
            return;
        };
        let elapsed = since.elapsed();
        if elapsed >= LANGUAGE_FLASH {
            self.language_flash = None;
            return;
        }
        let fade_start = LANGUAGE_FLASH - LANGUAGE_FADE;
        let opacity = if elapsed <= fade_start {
            1.0
        } else {
            1.0 - (elapsed - fade_start).as_secs_f32() / LANGUAGE_FADE.as_secs_f32()
        };
        let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("language-flash")));
        overlays::language_flash(&painter, area, i18n::current(), opacity);
        ctx.request_repaint();
    }
}
