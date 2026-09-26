//! Dark tokens of `ui_design_system_v1.2.md`, mapped onto egui.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Theme, Visuals};

pub mod tokens {
    use eframe::egui::Color32;

    pub const BG: Color32 = Color32::from_rgb(0x0F, 0x19, 0x23);
    pub const SURFACE: Color32 = Color32::from_rgb(0x1A, 0x25, 0x35);
    pub const SURFACE_MUTED: Color32 = Color32::from_rgb(0x1F, 0x2E, 0x40);
    pub const TEXT: Color32 = Color32::from_rgb(0xE2, 0xEA, 0xF4);
    pub const MUTED: Color32 = Color32::from_rgb(0x8A, 0x9B, 0xB0);
    pub const LINE: Color32 = Color32::from_rgb(0x2A, 0x3A, 0x4E);
    pub const ACCENT: Color32 = Color32::from_rgb(0x5B, 0x8E, 0xC4);
    pub const ACCENT_STRONG: Color32 = Color32::from_rgb(0x7A, 0xAC, 0xD8);
    pub const ACCENT_SUBTLE: Color32 = Color32::from_rgb(0x1E, 0x3A, 0x52);
    pub const STATUS_ERROR: Color32 = Color32::from_rgb(0xE0, 0x5A, 0x4E);
    pub const STATUS_ERROR_BG: Color32 = Color32::from_rgb(0x3D, 0x12, 0x10);
    /// Neutral grey behind photos – deliberately not `BG`: its blue tint would bias colour
    /// judgement.
    pub const CANVAS: Color32 = Color32::from_rgb(0x16, 0x16, 0x16);
}

pub fn apply(ctx: &egui::Context) {
    use tokens::*;

    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = SURFACE;
    visuals.window_stroke = Stroke::new(1.0, LINE);
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.extreme_bg_color = BG;
    visuals.faint_bg_color = SURFACE_MUTED;
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = ACCENT_SUBTLE;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    for (state, fill) in [
        (&mut widgets.inactive, SURFACE_MUTED),
        (&mut widgets.hovered, ACCENT_SUBTLE),
        (&mut widgets.active, ACCENT_SUBTLE),
        (&mut widgets.open, ACCENT_SUBTLE),
    ] {
        state.weak_bg_fill = fill;
        state.bg_fill = fill;
        state.corner_radius = CornerRadius::same(6);
    }
    widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_STRONG);

    ctx.set_visuals_of(Theme::Dark, visuals);
    ctx.set_theme(Theme::Dark);
}

/// Primary button fill per the design system (one primary action per view).
pub fn primary_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(text).color(Color32::WHITE).size(14.0))
        .fill(tokens::ACCENT)
        .min_size(egui::vec2(140.0, 38.0))
}
