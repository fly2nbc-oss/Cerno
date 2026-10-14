//! Cerno's values of the design system v2.0 (`ui_design_system_v2.0.md` in the parent
//! folder), mapped onto egui. Dark only: a light surround shifts how bright a photo looks.
//! Surfaces are neutral grey – tinted chrome around a photo biases colour judgement – so only
//! the accent and status colours carry hue.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, RichText, Stroke, StrokeKind, Theme, Visuals,
};

pub mod tokens {
    use eframe::egui::{Color32, Shadow};

    pub const BG: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);
    /// Behind photos, the grid and videos: the same neutral grey as the window.
    pub const CANVAS: Color32 = BG;
    /// Bars and panels.
    pub const SURFACE: Color32 = Color32::from_rgb(0x20, 0x20, 0x20);
    /// Cards, lists, notices and pills – always with `SHADOW` (`paint_card`).
    pub const SURFACE_RAISED: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);
    /// Resting fill of buttons and chips.
    pub const SURFACE_MUTED: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x2A);
    /// Under the pointer – neutral; blue means "on", never "hovered".
    pub const HOVER: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x2A);
    pub const SURFACE_PRESSED: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
    pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE6, 0xE6);
    /// ≈ 5.8:1 on `SURFACE`.
    pub const MUTED: Color32 = Color32::from_rgb(0x9A, 0x9A, 0x9A);
    /// Greyed-out content; the reason is in the tooltip.
    pub const DISABLED: Color32 = Color32::from_rgb(0x5A, 0x5A, 0x5A);
    /// Dividers and the faint border of buttons, chips and segment groups.
    pub const LINE: Color32 = Color32::from_rgb(0x35, 0x35, 0x35);
    /// Only where the border is the element: boxes and text fields (≥ 3:1 on cards).
    pub const LINE_STRONG: Color32 = Color32::from_rgb(0x70, 0x70, 0x70);
    pub const ACCENT: Color32 = Color32::from_rgb(0x5B, 0x8E, 0xC4);
    pub const ACCENT_STRONG: Color32 = Color32::from_rgb(0x7A, 0xAC, 0xD8);
    /// "On" and the highlighted row of a list.
    pub const ACCENT_SUBTLE: Color32 = Color32::from_rgb(0x1E, 0x3A, 0x52);
    /// Fill of the primary button: white text 4.5:1.
    pub const ACCENT_FILL: Color32 = Color32::from_rgb(0x45, 0x7A, 0xAE);
    pub const ACCENT_FILL_HOVER: Color32 = Color32::from_rgb(0x3F, 0x6E, 0x9E);
    /// Fill of a button that does something that can't be undone.
    pub const DANGER_FILL: Color32 = Color32::from_rgb(0xC9, 0x4A, 0x3F);
    pub const DANGER_FILL_HOVER: Color32 = Color32::from_rgb(0xB4, 0x43, 0x3A);
    pub const STATUS_ERROR: Color32 = Color32::from_rgb(0xE6, 0x6A, 0x5E);
    pub const STATUS_ERROR_BG: Color32 = Color32::from_rgb(0x3D, 0x12, 0x10);
    pub const STATUS_WARN: Color32 = Color32::from_rgb(0xF0, 0xA3, 0x30);
    /// Dims the window under a card (65 %).
    pub const SCRIM: Color32 = Color32::from_black_alpha(166);
    /// Dims the window under a list (30 %).
    pub const SCRIM_LIGHT: Color32 = Color32::from_black_alpha(77);
    /// Under every raised surface: cards, lists, notices, pills, tooltips.
    pub const SHADOW: Shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(102),
    };
    /// Round controls and banners over the photo (browse arrows, the play button, the edit
    /// banner): dark enough on any photo, still letting it show through.
    pub const PHOTO_VEIL: Color32 = Color32::from_black_alpha(170);
    /// The same, at rest – the browse arrows and the play button while not hovered.
    pub const PHOTO_VEIL_LIGHT: Color32 = Color32::from_black_alpha(110);
    /// Colour labels. The only hues besides the accent and status colours: they are the
    /// mark itself, not chrome around the photo.
    pub const LABEL_RED: Color32 = Color32::from_rgb(0xE2, 0x4B, 0x4B);
    pub const LABEL_YELLOW: Color32 = Color32::from_rgb(0xE6, 0xC2, 0x29);
    pub const LABEL_GREEN: Color32 = Color32::from_rgb(0x3D, 0xAB, 0x6E);
    /// A purer blue than the accent (CIEDE2000 13.5 from it, 2.9 before), ~3.2:1 on the
    /// surfaces.
    pub const LABEL_BLUE: Color32 = Color32::from_rgb(0x33, 0x66, 0xE6);
    pub const LABEL_PURPLE: Color32 = Color32::from_rgb(0xA5, 0x6B, 0xC7);
}

/// The only font sizes of the UI – a few roles, as in the design system, instead of one size
/// per place. `OVERLINE`, `VALUE` and `TITLE` are Semibold (`strong`, `overline`).
pub mod text {
    /// Uppercase section and group titles, meter names, badges – Semibold (`overline`).
    pub const OVERLINE: f32 = 11.0;
    /// Secondary lines: capture data, shortcuts, hints, key caps.
    pub const SMALL: f32 = 12.0;
    /// Standard text: names, menu rows, notices, explanations.
    pub const BODY: f32 = 13.0;
    /// Values in the details panel, banners, placeholders in the photo area.
    pub const LARGE: f32 = 15.0;
    /// Scores in the info bar, card titles – Semibold.
    pub const VALUE: f32 = 17.0;
    /// Help title and the language flash – Semibold.
    pub const TITLE: f32 = 22.0;
}

/// Corner radii: nothing else is used.
pub mod radius {
    /// Boxes, key caps, small badges.
    pub const SM: u8 = 4;
    /// Buttons, fields, chips, segments, row highlights, notices, thumbnails.
    pub const MD: u8 = 6;
    /// Lists, pills, segment groups, the view switcher.
    pub const LG: u8 = 8;
    /// Cards.
    pub const XL: u8 = 12;
    /// Counter badges.
    pub const PILL: u8 = u8::MAX;
}

/// Height of buttons, fields and icon buttons.
pub const CONTROL: f32 = 32.0;
/// Height of chips, segments and combo boxes inside a bar.
pub const COMPACT: f32 = 24.0;

/// Lucide icons (ISC, https://lucide.dev) of the embedded subset (`assets/lucide`, made by
/// `tools/make_lucide_subset.py`). They are characters of the proportional font: they take the
/// text's colour and size.
#[allow(
    dead_code,
    reason = "taken into use as the painted icons give way to Lucide (CER-56)"
)]
pub mod icon {
    pub const CHECK: char = '\u{E070}';
    pub const CHEVRON_DOWN: char = '\u{E071}';
    pub const CHEVRON_LEFT: char = '\u{E072}';
    pub const CHEVRON_RIGHT: char = '\u{E073}';
    pub const CIRCLE_HELP: char = '\u{E082}';
    pub const COPY: char = '\u{E0A2}';
    pub const COPY_CHECK: char = '\u{E400}';
    pub const EYE: char = '\u{E0BE}';
    pub const FILE_WARNING: char = '\u{E330}';
    pub const FILTER: char = '\u{E0D5}';
    pub const FOLDER_OPEN: char = '\u{E247}';
    pub const IMAGE: char = '\u{E0F9}';
    pub const LAYOUT_GRID: char = '\u{E105}';
    pub const PANEL_BOTTOM: char = '\u{E431}';
    pub const PANEL_LEFT: char = '\u{E12E}';
    pub const PANEL_RIGHT: char = '\u{E436}';
    pub const PANEL_TOP: char = '\u{E43A}';
    pub const PAUSE: char = '\u{E132}';
    pub const PLAY: char = '\u{E140}';
    pub const ROTATE_CCW: char = '\u{E14C}';
    pub const ROTATE_CW: char = '\u{E14D}';
    pub const SCAN_FACE: char = '\u{E375}';
    pub const TRASH: char = '\u{E18E}';
    pub const TRIANGLE_ALERT: char = '\u{E193}';
    pub const USER: char = '\u{E19F}';
    pub const USER_X: char = '\u{E1A3}';
    pub const VOLUME_1: char = '\u{E1AA}';
    pub const VOLUME_2: char = '\u{E1AB}';
    pub const VOLUME_X: char = '\u{E1AC}';
    pub const X: char = '\u{E1B2}';

    /// Every icon above, for the test that checks the subset holds them.
    #[cfg(test)]
    pub const ALL: [char; 30] = [
        CHECK,
        CHEVRON_DOWN,
        CHEVRON_LEFT,
        CHEVRON_RIGHT,
        CIRCLE_HELP,
        COPY,
        COPY_CHECK,
        EYE,
        FILE_WARNING,
        FILTER,
        FOLDER_OPEN,
        IMAGE,
        LAYOUT_GRID,
        PANEL_BOTTOM,
        PANEL_LEFT,
        PANEL_RIGHT,
        PANEL_TOP,
        PAUSE,
        PLAY,
        ROTATE_CCW,
        ROTATE_CW,
        SCAN_FACE,
        TRASH,
        TRIANGLE_ALERT,
        USER,
        USER_X,
        VOLUME_1,
        VOLUME_2,
        VOLUME_X,
        X,
    ];
}

pub fn label_color(label: crate::metadata::Label) -> Color32 {
    use crate::metadata::Label;
    match label {
        Label::Red => tokens::LABEL_RED,
        Label::Yellow => tokens::LABEL_YELLOW,
        Label::Green => tokens::LABEL_GREEN,
        Label::Blue => tokens::LABEL_BLUE,
        Label::Purple => tokens::LABEL_PURPLE,
    }
}

pub fn apply(ctx: &egui::Context) {
    use tokens::*;

    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    // Tooltips and the combo boxes' lists are raised surfaces like Cerno's own cards.
    visuals.window_fill = SURFACE_RAISED;
    visuals.window_stroke = Stroke::new(1.0, LINE);
    visuals.window_corner_radius = CornerRadius::same(radius::XL);
    visuals.menu_corner_radius = CornerRadius::same(radius::LG);
    visuals.window_shadow = SHADOW;
    visuals.popup_shadow = SHADOW;
    visuals.extreme_bg_color = BG;
    visuals.faint_bg_color = SURFACE_MUTED;
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = ACCENT_SUBTLE;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    for (state, fill, border) in [
        (&mut widgets.inactive, SURFACE_MUTED, LINE),
        (&mut widgets.hovered, HOVER, ACCENT),
        (&mut widgets.active, SURFACE_PRESSED, ACCENT_STRONG),
        (&mut widgets.open, SURFACE_MUTED, ACCENT),
    ] {
        state.weak_bg_fill = fill;
        state.bg_fill = fill;
        // Buttons, combo boxes and text fields share these. Cerno has no egui checkbox (it
        // would turn round above 4 px), so they all get the buttons' radius.
        state.corner_radius = CornerRadius::same(radius::MD);
        // The faint line for buttons and combo boxes (the border rule); text fields get the
        // strong one in their own scope (`field_scope`).
        state.bg_stroke = Stroke::new(1.0, border);
    }

    ctx.set_visuals_of(Theme::Dark, visuals);
    ctx.set_theme(Theme::Dark);
    install_fonts(ctx);
}

/// Text fields: the border is the element, so it takes `LINE_STRONG` (≥ 3:1) – everything
/// else keeps the faint `LINE` (the border rule).
pub fn field_scope<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope(|ui| {
        ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.0, tokens::LINE_STRONG);
        add(ui)
    })
    .inner
}

/// The family of the Semibold roles (`overline`, `strong`).
pub fn semibold() -> FontFamily {
    FontFamily::Name(SEMIBOLD.into())
}

const SEMIBOLD: &str = "semibold";

/// A Semibold font: card titles, values, the active tab, counter badges.
pub fn strong(size: f32) -> FontId {
    FontId::new(size, semibold())
}

/// A section or group title: 11 px Semibold, uppercase, spaced, muted.
pub fn overline(text: &str) -> RichText {
    RichText::new(text.to_uppercase())
        .font(strong(text::OVERLINE))
        .extra_letter_spacing(OVERLINE_SPACING)
        .color(tokens::MUTED)
}

/// `overline` for a painter, in any colour (a meter name turns the warning colour).
pub fn overline_job(text: &str, color: Color32) -> egui::text::LayoutJob {
    egui::text::LayoutJob::single_section(
        text.to_uppercase(),
        egui::TextFormat {
            font_id: strong(text::OVERLINE),
            extra_letter_spacing: OVERLINE_SPACING,
            color,
            ..Default::default()
        },
    )
}

const OVERLINE_SPACING: f32 = 0.5;

/// A raised surface: its shadow, then `SURFACE_RAISED` with a faint border.
pub fn paint_card(painter: &egui::Painter, rect: egui::Rect, corner: u8) {
    paint_raised(painter, rect, corner, tokens::SURFACE_RAISED, tokens::LINE);
}

/// A raised surface in other colours: an error notice, a pill whose border says what runs.
pub fn paint_raised(
    painter: &egui::Painter,
    rect: egui::Rect,
    corner: u8,
    fill: Color32,
    border: Color32,
) {
    painter.add(tokens::SHADOW.as_shape(rect, CornerRadius::same(corner)));
    painter.rect(
        rect,
        CornerRadius::same(corner),
        fill,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Primary,
    Secondary,
    Danger,
}

/// A button of the design system: 32 high (`compact`: 24), radius 6, the fill changing under
/// the pointer.
pub struct ActionButton<'a> {
    kind: Kind,
    text: &'a str,
    min_size: egui::Vec2,
}

/// The one primary action of a view: white on `ACCENT_FILL`, at least 120 wide.
pub fn primary_button(text: &str) -> ActionButton<'_> {
    ActionButton {
        kind: Kind::Primary,
        text,
        min_size: egui::vec2(120.0, CONTROL),
    }
}

/// Every other action: `SURFACE_MUTED` with a faint border, the accent border under the
/// pointer.
pub fn secondary_button(text: &str) -> ActionButton<'_> {
    ActionButton {
        kind: Kind::Secondary,
        text,
        min_size: egui::vec2(0.0, CONTROL),
    }
}

/// An action that can't be undone: white on `DANGER_FILL`.
pub fn danger_button(text: &str) -> ActionButton<'_> {
    ActionButton {
        kind: Kind::Danger,
        text,
        min_size: egui::vec2(120.0, CONTROL),
    }
}

impl ActionButton<'_> {
    /// 24 high, for a bar.
    pub fn compact(mut self) -> Self {
        self.min_size.y = COMPACT;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_size.x = width;
        self
    }
}

impl egui::Widget for ActionButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        use tokens::*;
        // The fill depends on the hover the button reports, so it goes in behind it afterwards.
        let behind = ui.painter().add(egui::Shape::Noop);
        let (rest, hover, color, border) = match self.kind {
            Kind::Primary => (ACCENT_FILL, ACCENT_FILL_HOVER, Color32::WHITE, None),
            Kind::Danger => (DANGER_FILL, DANGER_FILL_HOVER, Color32::WHITE, None),
            Kind::Secondary => (SURFACE_MUTED, SURFACE_MUTED, TEXT, Some(LINE)),
        };
        let response = ui.add(
            egui::Button::new(RichText::new(self.text).size(text::BODY).color(color))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .corner_radius(radius::MD)
                .min_size(self.min_size),
        );
        let hovered = response.hovered() && ui.is_enabled();
        let fill = if hovered { hover } else { rest };
        let stroke = match border {
            Some(_) if hovered => Stroke::new(1.0, ACCENT),
            Some(line) => Stroke::new(1.0, line),
            None => Stroke::NONE,
        };
        ui.painter().set(
            behind,
            egui::epaint::RectShape::new(
                response.rect,
                CornerRadius::same(radius::MD),
                fill,
                stroke,
                StrokeKind::Inside,
            ),
        );
        response
    }
}

/// The design system's UI fonts (Segoe UI and Segoe UI Semibold) or a common Linux sans, read
/// from the system at start-up, with the Lucide icons. egui's bundled fonts stay as fallback –
/// they lack arrows (← →) and other symbols, but carry ★.
fn install_fonts(ctx: &egui::Context) {
    let (regular, semibold) = system_fonts();
    ctx.set_fonts(definitions(regular, semibold));
}

/// Regular and Semibold of the same family, as files the system has.
fn system_fonts() -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let pairs: Vec<(std::path::PathBuf, std::path::PathBuf)> = if cfg!(windows) {
        let windir = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
        let fonts = std::path::Path::new(&windir).join("Fonts");
        vec![(fonts.join("segoeui.ttf"), fonts.join("seguisb.ttf"))]
    } else {
        // DejaVu has no Semibold: its Bold is the nearest.
        [
            (
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            ),
            (
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
            ),
            (
                "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
                "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf",
            ),
            (
                "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
            ),
            (
                "/usr/share/fonts/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/noto/NotoSans-SemiBold.ttf",
            ),
            (
                "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/google-noto/NotoSans-SemiBold.ttf",
            ),
        ]
        .map(|(regular, semibold)| (regular.into(), semibold.into()))
        .to_vec()
    };
    for (regular, semibold) in pairs {
        let Ok(bytes) = std::fs::read(&regular) else {
            continue;
        };
        log::info!("UI font: {}", regular.display());
        let semibold = match std::fs::read(&semibold) {
            Ok(bytes) => {
                log::info!("UI font: {}", semibold.display());
                Some(bytes)
            }
            Err(_) => {
                log::info!("no {} – titles in the regular font", semibold.display());
                None
            }
        };
        return (Some(bytes), semibold);
    }
    log::info!("no system UI font found – using egui's bundled fonts");
    (None, None)
}

const LUCIDE: &[u8] = include_bytes!("../assets/lucide/lucide-subset.ttf");

/// The font families: the system font first (it sets the rows' height and baseline), Lucide
/// second – never first, its metrics would set every row, never behind egui's fonts, which
/// use some of the same private code points – then egui's fonts. The Semibold family is the
/// same list behind the Semibold face, or the regular list without one.
fn definitions(regular: Option<Vec<u8>>, bold: Option<Vec<u8>>) -> egui::FontDefinitions {
    let mut defs = egui::FontDefinitions::default();
    let lucide = egui::FontData::from_static(LUCIDE).tweak(egui::epaint::text::FontTweak {
        scale: 1.15,
        y_offset_factor: 0.04,
        ..Default::default()
    });
    defs.font_data.insert("lucide".into(), Arc::new(lucide));
    let mut family = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if let Some(bytes) = regular {
        defs.font_data.insert(
            "system-ui".into(),
            Arc::new(egui::FontData::from_owned(bytes)),
        );
        family.insert(0, "system-ui".into());
    }
    family.insert(family.len().min(1), "lucide".into());
    let mut strong = family.clone();
    if let Some(bytes) = bold {
        defs.font_data.insert(
            "system-ui-semibold".into(),
            Arc::new(egui::FontData::from_owned(bytes)),
        );
        strong.insert(0, "system-ui-semibold".into());
    }
    defs.families.insert(FontFamily::Proportional, family);
    defs.families.insert(semibold(), strong);
    defs
}

/// A context with Cerno's font families but without the system's fonts, so tests measure the
/// same on every machine: egui's bundled fonts, Lucide, and the Semibold family bound to them.
#[cfg(test)]
pub fn test_context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_fonts(definitions(None, None));
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_in_the_subset() {
        let ctx = egui::Context::default();
        let mut defs = egui::FontDefinitions::empty();
        defs.font_data.insert(
            "lucide".into(),
            Arc::new(egui::FontData::from_static(LUCIDE)),
        );
        defs.families
            .insert(FontFamily::Proportional, vec!["lucide".into()]);
        ctx.set_fonts(defs);
        ctx.run_ui(Default::default(), |_| {})
            .textures_delta
            .clear();
        let font = FontId::proportional(text::BODY);
        for glyph in icon::ALL {
            assert!(
                ctx.fonts_mut(|f| f.has_glyph(&font, glyph)),
                "U+{:04X} is not in assets/lucide/lucide-subset.ttf",
                glyph as u32
            );
        }
    }

    #[test]
    fn lucide_comes_second_and_the_semibold_family_exists() {
        let defs = definitions(Some(Vec::new()), None);
        let family = &defs.families[&FontFamily::Proportional];
        assert_eq!(family[0], "system-ui");
        assert_eq!(family[1], "lucide");
        assert_eq!(&defs.families[&semibold()], family);
        let without = definitions(None, None);
        assert_eq!(without.families[&FontFamily::Proportional][1], "lucide");
    }

    #[test]
    fn a_test_context_lays_out_semibold_and_icons() {
        let ctx = test_context();
        ctx.run_ui(Default::default(), |ui| {
            ui.label(overline("Sharpness"));
            ui.label(RichText::new(icon::X.to_string()).size(text::BODY));
        })
        .textures_delta
        .clear();
    }
}
