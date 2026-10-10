//! Menu bar, filter bar, details panel and filmstrip on and off; the language switch.

use std::time::{Duration, Instant};

use eframe::egui::{self, Id, LayerId, Order, Rect};

use crate::db::Db;
use crate::i18n::{self, Lang};
use crate::ui::details::{DetailsMode, DetailsTab};
use crate::ui::icons::Panel;
use crate::ui::overlays;

use super::CernoApp;
use super::layer::Layer;

/// How long the flag stays after switching the language, and how long it fades out.
const LANGUAGE_FLASH: Duration = Duration::from_millis(1400);

const LANGUAGE_FADE: Duration = Duration::from_millis(450);

/// Which bars show (the info bar always does), and the details panel's stage and tab. Saved
/// but for the CLIP attributes and `toolbar_held`.
pub(super) struct Bars {
    /// The menu bar on the left is switched on (the menu button, saved as `side_bar`).
    pub(super) side_bar: bool,
    /// Filter bar (`T`), filmstrip (`F6`) and details panel (`Tab`).
    pub(super) toolbar: bool,
    pub(super) filmstrip: bool,
    pub(super) details: DetailsMode,
    /// The stage `Tab` brings back.
    pub(super) details_last: DetailsMode,
    /// Which tab the details panel shows (`Ctrl+Tab` steps through them).
    pub(super) details_tab: DetailsTab,
    /// The CLIP attributes are folded out in the details panel (session-wide).
    pub(super) attributes_open: bool,
    /// The filter bar showed on its own (a filter hid everything) and the pointer is on it: it
    /// stays until the pointer leaves.
    pub(super) toolbar_held: bool,
}

impl Bars {
    /// The saved bars: only the photo, the filmstrip and the info bar by default.
    pub(super) fn restore(db: &Db) -> Self {
        let details = db
            .setting("details_mode")
            .and_then(|m| DetailsMode::from_id(&m))
            .unwrap_or(DetailsMode::Off);
        Self {
            // Off until the menu button switches it on (the user's decision F3 of 2026-10-10).
            side_bar: db.setting("side_bar").as_deref() == Some("1"),
            toolbar: db.setting("top_bar").as_deref() == Some("1"),
            filmstrip: db.setting("filmstrip").as_deref() != Some("0"),
            details,
            details_last: if details == DetailsMode::Off {
                DetailsMode::On
            } else {
                details
            },
            details_tab: db
                .setting("details_tab")
                .and_then(|id| DetailsTab::from_id(&id))
                .unwrap_or(DetailsTab::Values),
            attributes_open: false,
            toolbar_held: false,
        }
    }
}

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
            side_bar: self.bars.side_bar,
            toolbar: self.bars.toolbar,
            details: self.bars.details != DetailsMode::Off,
            filmstrip: self.bars.filmstrip,
        }
    }

    pub(super) fn save_panels(&self) {
        self.db.put_flag("side_bar", self.bars.side_bar);
        self.db.put_flag("top_bar", self.bars.toolbar);
        self.db.put_flag("filmstrip", self.bars.filmstrip);
        self.db.put_setting("details_mode", self.bars.details.id());
    }

    /// The menu bar shows: switched on, or for the keyboard (`Ctrl+K`, `Ctrl+M`, `E`).
    pub(super) fn side_bar_shown(&self) -> bool {
        self.bars.side_bar || self.menu_bar.temporary
    }

    /// `Ctrl+K`: the keyboard goes to the menu bar – shown for it while the bar is off.
    pub(super) fn open_side_bar(&mut self) {
        self.layer.close_if(Layer::is_help);
        self.menu_bar.temporary = !self.bars.side_bar;
        self.menu_bar.state.take_keyboard();
    }

    /// `Ctrl+M` (*Visible photos*), `E` (*Edit elsewhere*): the same, on a group or a row.
    pub(super) fn open_side_bar_at(&mut self, section: &'static str, item: Option<usize>) {
        self.open_side_bar();
        self.menu_bar.state.take_keyboard_at(section, item);
    }

    /// The keyboard goes back to the photo; a bar shown only for it goes too, with its list.
    pub(super) fn leave_side_bar(&mut self) {
        self.menu_bar.state.release();
        self.layer.close_if(Layer::is_list);
        self.menu_bar.list_after_draw = None;
        self.menu_bar.temporary = false;
    }

    pub(super) fn set_details(&mut self, mode: DetailsMode) {
        self.bars.details = mode;
        if mode != DetailsMode::Off {
            self.bars.details_last = mode;
        }
    }

    /// Buttons and `T` / `Tab` / `F6`. The details panel comes back at the stage it had. The
    /// menu button switches the menu bar: a bar shown only for the keyboard stays, switched on.
    pub(super) fn toggle_panel(&mut self, panel: Panel) {
        match panel {
            Panel::Left if self.menu_bar.temporary => {
                self.menu_bar.temporary = false;
                self.bars.side_bar = true;
            }
            Panel::Left => {
                self.bars.side_bar = !self.bars.side_bar;
                if !self.bars.side_bar {
                    self.leave_side_bar();
                }
            }
            Panel::Top => self.bars.toolbar = !self.bars.toolbar,
            Panel::Bottom => self.bars.filmstrip = !self.bars.filmstrip,
            Panel::Right if self.bars.details == DetailsMode::Off => {
                self.bars.details = self.bars.details_last
            }
            Panel::Right => self.bars.details = DetailsMode::Off,
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
        self.bars.side_bar = show;
        self.bars.toolbar = show;
        self.bars.filmstrip = show;
        self.bars.details = if show {
            self.bars.details_last
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
