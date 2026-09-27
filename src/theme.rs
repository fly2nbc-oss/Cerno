//! Dark tokens of `ui_design_system_v1.2.md`, mapped onto egui – with neutral greys instead of
//! the system's blue-grey surfaces: tinted chrome around a photo biases colour judgement, so
//! (like Lightroom or Capture One) only the accent and status colours carry hue.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Theme, Visuals};

pub mod tokens {
    use eframe::egui::Color32;

    pub const BG: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);
    pub const SURFACE: Color32 = Color32::from_rgb(0x20, 0x20, 0x20);
    pub const SURFACE_MUTED: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x2A);
    pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE6, 0xE6);
    /// ≈ 5.8:1 on `SURFACE`.
    pub const MUTED: Color32 = Color32::from_rgb(0x9A, 0x9A, 0x9A);
    pub const LINE: Color32 = Color32::from_rgb(0x35, 0x35, 0x35);
    pub const ACCENT: Color32 = Color32::from_rgb(0x5B, 0x8E, 0xC4);
    pub const ACCENT_STRONG: Color32 = Color32::from_rgb(0x7A, 0xAC, 0xD8);
    pub const ACCENT_SUBTLE: Color32 = Color32::from_rgb(0x1E, 0x3A, 0x52);
    pub const STATUS_ERROR: Color32 = Color32::from_rgb(0xE0, 0x5A, 0x4E);
    pub const STATUS_ERROR_BG: Color32 = Color32::from_rgb(0x3D, 0x12, 0x10);
    pub const STATUS_WARN: Color32 = Color32::from_rgb(0xF0, 0xA3, 0x30);
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
        // egui shares one radius between buttons and checkboxes; 6 px turns a checkbox into a
        // circle, so use the design system's small radius.
        state.corner_radius = CornerRadius::same(4);
    }
    // Without a border, checkboxes vanish on the dark surfaces.
    widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_STRONG);

    ctx.set_visuals_of(Theme::Dark, visuals);
    ctx.set_theme(Theme::Dark);
    install_system_font(ctx);
}

/// The design system's UI font (Segoe UI) or a common Linux sans, read from the system at
/// start-up. egui's bundled fonts stay as fallback – they lack arrows (← →) and other symbols.
fn install_system_font(ctx: &egui::Context) {
    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};

    let candidates: Vec<std::path::PathBuf> = if cfg!(windows) {
        let windir = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
        vec![std::path::Path::new(&windir).join(r"Fonts\segoeui.ttf")]
    } else {
        [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
        ]
        .map(Into::into)
        .to_vec()
    };
    let Some((path, bytes)) = candidates
        .into_iter()
        .find_map(|p| std::fs::read(&p).ok().map(|b| (p, b)))
    else {
        log::info!("no system UI font found – using egui's bundled fonts");
        return;
    };
    log::info!("UI font: {}", path.display());
    ctx.add_font(FontInsert::new(
        "system-ui",
        egui::FontData::from_owned(bytes),
        vec![InsertFontFamily {
            family: egui::FontFamily::Proportional,
            priority: FontPriority::Highest,
        }],
    ));
}

/// Primary button fill per the design system (one primary action per view).
pub fn primary_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(text).color(Color32::WHITE).size(14.0))
        .fill(tokens::ACCENT)
        .min_size(egui::vec2(140.0, 38.0))
}
