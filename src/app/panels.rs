//! Filter bar, details panel and filmstrip on and off; the language switch.

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
    toolbar: bool,
    details: bool,
    filmstrip: bool,
}

impl CernoApp {
    fn panels(&self) -> Panels {
        Panels {
            toolbar: self.show_toolbar,
            details: self.details != DetailsMode::Off,
            filmstrip: self.show_filmstrip,
        }
    }

    pub(super) fn save_panels(&self) {
        self.db.put_flag("top_bar", self.show_toolbar);
        self.db.put_flag("filmstrip", self.show_filmstrip);
        self.db.put_setting("details_mode", self.details.id());
    }

    pub(super) fn set_details(&mut self, mode: DetailsMode) {
        self.details = mode;
        if mode != DetailsMode::Off {
            self.details_last = mode;
        }
    }

    /// Buttons and `T` / `Tab` / `F6`. The details panel comes back at the stage it had.
    pub(super) fn toggle_panel(&mut self, panel: Panel) {
        match panel {
            Panel::Top => self.show_toolbar = !self.show_toolbar,
            Panel::Bottom => self.show_filmstrip = !self.show_filmstrip,
            Panel::Right if self.details == DetailsMode::Off => self.details = self.details_last,
            Panel::Right => self.details = DetailsMode::Off,
        }
        self.save_panels();
    }

    /// `Shift+Tab`: hides top bar, details and filmstrip together – or shows all three if none
    /// is. The info bar stays either way.
    pub(super) fn toggle_all_panels(&mut self) {
        let Panels {
            toolbar,
            details,
            filmstrip,
        } = self.panels();
        let show = !(toolbar || details || filmstrip);
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
