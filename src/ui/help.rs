//! Help page: three tabs – every shortcut with a short explanation, tips on working with
//! Cerno, and About Cerno (version, licence, where to report a problem or suggest an idea).
//! `H`/`F1` shows it over the photos (and over the start screen), always on the shortcuts; a
//! click on a tab or `←`/`→` switches. The start screen is a small card: one sentence, "Open
//! folder" and the five keys to begin with.

use eframe::egui::{
    Align, Align2, Area, Color32, Context, CursorIcon, FontId, Grid, Hyperlink, Id, Label, Layout,
    Order, Painter, Pos2, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, UiBuilder,
    pos2, vec2,
};

use crate::i18n::{self, HelpRow, Texts, TipSection};
use crate::theme::{self, text, tokens};
use crate::ui::icons;
use crate::ui::tabs::{self, Widths};

const MAX_WIDTH: f32 = 1340.0;
/// The start screen is a small card: one sentence, the button and five keys.
const WELCOME_WIDTH: f32 = 560.0;
const PAD: f32 = 28.0;
/// Two columns of shortcuts from this content width on, three from the next.
const TWO_COLUMNS: f32 = 700.0;
const THREE_COLUMNS: f32 = 1100.0;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const COPYRIGHT: &str = concat!("© 2026 ", env!("CARGO_PKG_AUTHORS"));

/// The help page's tabs.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    /// Every shortcut.
    #[default]
    Keys,
    /// How to work with Cerno.
    Tips,
    /// Version, licence, links to report a problem or suggest an idea.
    About,
}

impl Page {
    pub const ALL: [Page; 3] = [Self::Keys, Self::Tips, Self::About];

    /// The page to the right (`→`), from the last one round to the first.
    pub fn next(self) -> Self {
        let at = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(at + 1) % Self::ALL.len()]
    }

    /// The page to the left (`←`).
    pub fn prev(self) -> Self {
        let at = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(at + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HelpOutput {
    pub close: bool,
    pub open_folder: bool,
    pub language: bool,
    /// Another tab was clicked.
    pub page: Option<Page>,
    /// About Cerno: "Open data folder" (where `crash.log` is).
    pub open_data_folder: bool,
    /// About Cerno: "Check now".
    pub check_updates: bool,
}

/// Modal page over the whole window; a click beside the card closes it. `update`: what About
/// Cerno says about updates.
pub fn overlay(
    ctx: &Context,
    window: Rect,
    page: Page,
    t: &Texts,
    update: &UpdateLine,
) -> HelpOutput {
    Area::new(Id::new("help"))
        .order(Order::Foreground)
        .fixed_pos(window.min)
        .show(ctx, |ui| {
            let backdrop = ui.allocate_rect(window, Sense::click());
            ui.painter()
                .rect_filled(window, 0.0, Color32::from_black_alpha(170));
            let extra = Extra {
                setup: Setup::READY,
                update: Some(update),
            };
            let mut out = card_with_content(ui, window, false, page, t, extra);
            out.close |= backdrop.clicked();
            out
        })
        .inner
}

/// What is set up for writing marks and for the aesthetics, for the start screen's line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setup {
    pub exiftool: bool,
    pub aesthetics: bool,
}

impl Setup {
    pub const READY: Self = Self {
        exiftool: true,
        aesthetics: true,
    };
}

/// About Cerno's update section: the state of the check, and a newer version's link (its
/// label, its page).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateLine {
    pub status: String,
    pub release: Option<(String, String)>,
    /// A check runs: *Check now* waits.
    pub checking: bool,
}

/// What a page says besides the fixed texts.
#[derive(Clone, Copy)]
struct Extra<'a> {
    setup: Setup,
    update: Option<&'a UpdateLine>,
}

/// The start screen: the help content with the "Open folder" button, centred in `rect`; a
/// line names what is still missing and where to get it.
pub fn welcome(ui: &mut Ui, rect: Rect, t: &Texts, setup: Setup) -> HelpOutput {
    let extra = Extra {
        setup,
        update: None,
    };
    card_with_content(ui, rect, true, Page::Keys, t, extra)
}

/// The card, centred in `space`: as high as its content was last frame, at most the space.
/// Each page remembers its own height, so switching does not flicker.
fn card_with_content(
    ui: &mut Ui,
    space: Rect,
    welcome: bool,
    page: Page,
    t: &Texts,
    extra: Extra<'_>,
) -> HelpOutput {
    const MARGIN_Y: f32 = 20.0;
    let height_id = Id::new(("help-height", welcome, page));
    let max_height = (space.height() - 48.0).max(200.0);
    let height = ui
        .data(|d| d.get_temp::<f32>(height_id))
        .map_or(max_height, |content| {
            (content + 2.0 * MARGIN_Y).min(max_height)
        });
    let card = Rect::from_center_size(
        space.center(),
        vec2(
            (space.width() - 48.0).clamp(200.0, if welcome { WELCOME_WIDTH } else { MAX_WIDTH }),
            height,
        ),
    );
    // Swallows clicks on the card, so only the backdrop closes the page.
    ui.interact(card, Id::new(("help-card", welcome)), Sense::click());
    let painter = ui.painter();
    painter.rect_filled(card, 10.0, tokens::SURFACE);
    painter.rect_stroke(
        card,
        10.0,
        Stroke::new(1.0, tokens::LINE),
        StrokeKind::Inside,
    );
    let mut inner = ui.new_child(
        UiBuilder::new()
            .max_rect(card.shrink2(vec2(PAD, MARGIN_Y)))
            .id_salt(("help", welcome)),
    );
    // Not `content_size`: without auto-shrinking it is at least the visible area.
    let (out, content_height) = ScrollArea::vertical()
        .auto_shrink(false)
        .show(&mut inner, |ui| content(ui, welcome, page, t, extra))
        .inner;
    if ui.data(|d| d.get_temp::<f32>(height_id)) != Some(content_height) {
        ui.data_mut(|d| d.insert_temp(height_id, content_height));
        ui.ctx().request_repaint();
    }
    out
}

/// Draws the page; returns what was clicked and the height it needs.
fn content(
    ui: &mut Ui,
    welcome: bool,
    page: Page,
    t: &Texts,
    extra: Extra<'_>,
) -> (HelpOutput, f32) {
    let setup = extra.setup;
    let mut out = HelpOutput::default();
    let top = ui.cursor().min;
    let width = ui.available_width();
    let (left, right) = (top.x, top.x + width);
    let painter = ui.painter().clone();
    let mut y = top.y + 4.0;

    // Header: "Cerno 0.7.0 · Help", language flag and (over the photos) a close button.
    let title = painter.layout_no_wrap(
        "Cerno".into(),
        FontId::proportional(text::TITLE),
        tokens::TEXT,
    );
    let title_height = title.size().y;
    let title_width = title.size().x;
    painter.galley(pos2(left, y), title, tokens::TEXT);
    let subtitle = if welcome {
        env!("CARGO_PKG_VERSION").to_owned()
    } else {
        format!("{}   ·   {}", env!("CARGO_PKG_VERSION"), t.help_title)
    };
    painter.text(
        pos2(left + title_width + 10.0, y + title_height - 6.0),
        Align2::LEFT_BOTTOM,
        subtitle,
        FontId::proportional(text::BODY),
        tokens::MUTED,
    );
    let mut x = right;
    if !welcome {
        let area = Rect::from_min_size(pos2(x - 28.0, y), vec2(28.0, 28.0));
        x -= 34.0;
        if header_button(ui, area, t.help_close, |p, c, color| {
            let d = 5.5;
            for (a, b) in [(vec2(-d, -d), vec2(d, d)), (vec2(-d, d), vec2(d, -d))] {
                p.line_segment([c + a, c + b], Stroke::new(1.6, color));
            }
        }) {
            out.close = true;
        }
    }
    // The language as a quiet word (the flag only shows while switching).
    let name = painter.layout_no_wrap(
        i18n::current().name().to_owned(),
        FontId::proportional(text::BODY),
        tokens::MUTED,
    );
    let area = Rect::from_min_size(
        pos2(x - name.size().x - 16.0, y),
        vec2(name.size().x + 16.0, 28.0),
    );
    let tooltip = format!(
        "{} ({})",
        (t.button_language)(i18n::current().name()),
        i18n::with_ctrl("L")
    );
    if header_button(ui, area, &tooltip, |p, c, color| {
        p.text(
            c,
            Align2::CENTER_CENTER,
            i18n::current().name(),
            FontId::proportional(text::BODY),
            color,
        );
    }) {
        out.language = true;
    }
    y += title_height + 12.0;

    if !welcome {
        let strip = Rect::from_min_size(pos2(left, y), vec2(width, 34.0));
        painter.hline(
            strip.x_range(),
            strip.bottom() - 0.5,
            Stroke::new(1.0, tokens::LINE),
        );
        let labels = [t.help_tab_keys, t.help_tab_tips, t.help_tab_about];
        let current = Page::ALL.iter().position(|p| *p == page).unwrap_or(0);
        if let Some(i) = tabs::strip(
            ui,
            strip,
            &labels,
            current,
            Id::new("help-tab"),
            Widths::Natural,
        ) {
            out.page = Some(Page::ALL[i]);
        }
        y = strip.bottom() + 16.0;
    }
    if page == Page::Tips && !welcome {
        y = tips(&painter, left, y, width, &t.help_tips);
        y = footer(&painter, left, y, width, t);
        ui.allocate_space(vec2(width, y - top.y));
        return (out, y - top.y);
    }
    if page == Page::About && !welcome {
        let space = Rect::from_min_size(pos2(left, y), vec2(width.min(760.0), f32::INFINITY));
        let mut column = ui.new_child(
            UiBuilder::new()
                .max_rect(space)
                .layout(Layout::top_down(Align::Min)),
        );
        about(&mut column, &mut out, t, extra.update);
        y = footer(&painter, left, column.min_rect().bottom() + 18.0, width, t);
        ui.allocate_space(vec2(width, y - top.y));
        return (out, y - top.y);
    }

    // Intro.
    let intro = painter.layout(
        i18n::keep_together(if welcome {
            t.welcome_intro
        } else {
            t.help_intro
        }),
        FontId::proportional(text::BODY),
        tokens::TEXT,
        width.min(760.0),
    );
    let intro_height = intro.size().y;
    painter.galley(pos2(left, y), intro, tokens::TEXT);
    y += intro_height + 18.0;

    if welcome {
        let button = Rect::from_center_size(pos2(left + width / 2.0, y + 19.0), vec2(200.0, 38.0));
        out.open_folder = ui
            .put(button, theme::primary_button(t.open_folder))
            .clicked();
        y += 46.0;
        painter.text(
            pos2(left + width / 2.0, y),
            Align2::CENTER_TOP,
            t.help_drop,
            FontId::proportional(text::BODY),
            tokens::MUTED,
        );
        y += 34.0;
        // Only the keys to begin with; H shows the rest.
        y = section(&painter, left, y, width, "", &t.welcome_keys) + 10.0;
        painter.text(
            pos2(left + width / 2.0, y),
            Align2::CENTER_TOP,
            t.welcome_more,
            FontId::proportional(text::BODY),
            tokens::MUTED,
        );
        y += 24.0;
        if setup != Setup::READY {
            let line = painter.layout(
                i18n::keep_together(&(t.setup_line)(setup.exiftool, setup.aesthetics)),
                FontId::proportional(text::SMALL),
                tokens::MUTED,
                width,
            );
            let height = line.size().y;
            painter.galley(
                pos2(left + (width - line.size().x) / 2.0, y + 6.0),
                line,
                tokens::MUTED,
            );
            y += height + 10.0;
        }
        ui.allocate_space(vec2(width, y - top.y));
        return (out, y - top.y);
    }

    // Shortcuts in reading order, split into columns of about the same length: three on wide
    // windows (so the page needs no scrolling: browse, rate, sort out │ video, view │ panels,
    // edit, more), else two – left what culling needs (browse, rate, sort out, video), right
    // the view, the panels, editing and the rest.
    let sections: [(&str, &[HelpRow]); 8] = [
        (t.help_sections[0], &t.help_browse),
        (t.help_sections[1], &t.help_rate),
        (t.help_sections[2], &t.help_cull),
        (t.help_sections[3], &t.help_video),
        (t.help_sections[4], &t.help_view),
        (t.help_sections[5], &t.help_panels),
        (t.help_sections[6], &t.help_edit),
        (t.help_sections[7], &t.help_more),
    ];
    let columns: Vec<&[(&str, &[HelpRow])]> = if width >= THREE_COLUMNS {
        vec![&sections[..3], &sections[3..5], &sections[5..]]
    } else if width >= TWO_COLUMNS {
        vec![&sections[..4], &sections[4..]]
    } else {
        vec![&sections[..]]
    };
    let gap = 36.0;
    let column_width = (width - gap * (columns.len() - 1) as f32) / columns.len() as f32;
    let mut bottom = y;
    for (i, column) in columns.iter().enumerate() {
        let x = left + i as f32 * (column_width + gap);
        let mut cy = y;
        for (title, rows) in column.iter() {
            cy = section(&painter, x, cy, column_width, title, rows) + 14.0;
        }
        bottom = bottom.max(cy);
    }
    y = footer(&painter, left, bottom, width, t);
    ui.allocate_space(vec2(width, y - top.y));
    (out, y - top.y)
}

/// About Cerno: what it is, version, copyright, licence and source; where to report a problem
/// or suggest an idea – a GitHub issue with version and system filled in, never a path or a
/// photo's name – and the third parties. The links only open the browser; Cerno sends nothing.
fn about(ui: &mut Ui, out: &mut HelpOutput, t: &Texts, update: Option<&UpdateLine>) {
    let body = |text: &str, color| {
        RichText::new(i18n::keep_together(text))
            .font(FontId::proportional(text::BODY))
            .color(color)
    };
    let paragraph = |ui: &mut Ui, text: &str| ui.add(Label::new(body(text, tokens::TEXT)).wrap());
    let heading = |ui: &mut Ui, title: &str| {
        ui.add_space(14.0);
        ui.label(
            RichText::new(title.to_uppercase())
                .font(FontId::proportional(text::LABEL))
                .color(tokens::MUTED),
        );
    };
    ui.spacing_mut().item_spacing.y = 6.0;
    paragraph(ui, t.about_intro);
    ui.add_space(8.0);
    Grid::new("about-facts")
        .num_columns(2)
        .spacing(vec2(24.0, 6.0))
        .show(ui, |ui| {
            for (label, value) in [
                (t.about_version, VERSION),
                ("Copyright", COPYRIGHT),
                (t.about_license, env!("CARGO_PKG_LICENSE")),
            ] {
                ui.label(body(label, tokens::MUTED));
                ui.label(body(value, tokens::TEXT));
                ui.end_row();
            }
            ui.label(body(t.about_source, tokens::MUTED));
            ui.add(link(REPOSITORY.trim_start_matches("https://"), REPOSITORY));
            ui.end_row();
        });

    // Updates: what is asked, that it can be turned off, where the check stands.
    if let Some(update) = update {
        heading(ui, t.about_updates);
        paragraph(ui, t.about_updates_text);
        ui.horizontal(|ui| {
            ui.label(body(&update.status, tokens::TEXT));
            ui.add_space(18.0);
            let button = eframe::egui::Button::new(t.update_check_now);
            if ui.add_enabled(!update.checking, button).clicked() {
                out.check_updates = true;
            }
            if let Some((label, url)) = &update.release {
                ui.add_space(18.0);
                ui.add(link(label, url));
            }
        });
    }

    heading(ui, t.about_bugs);
    paragraph(ui, t.about_bugs_text);
    ui.horizontal(|ui| {
        let report = issue_url("bug_report.yml", VERSION, &crate::system::name());
        ui.add(link(t.about_bug_link, &report));
        ui.add_space(18.0);
        let folder = crate::paths::data_dir()
            .map(|dir| dir.display().to_string())
            .unwrap_or_default();
        if ui.button(t.about_open_data).on_hover_text(folder).clicked() {
            out.open_data_folder = true;
        }
    });

    heading(ui, t.about_wishes);
    paragraph(ui, t.about_wishes_text);
    let wish = issue_url("feature_request.yml", VERSION, &crate::system::name());
    ui.add(link(t.about_wish_link, &wish));

    heading(ui, t.about_third_party);
    paragraph(ui, t.about_third_party_text);
    ui.add(link(t.about_third_party_link, &third_party_url(VERSION)));
}

/// A link that opens in the browser, in the accent colour (interaction) so it reads as one.
fn link(label: &str, url: &str) -> Hyperlink {
    Hyperlink::from_label_and_url(
        RichText::new(label)
            .font(FontId::proportional(text::BODY))
            .color(tokens::ACCENT),
        url,
    )
    .open_in_new_tab(true)
}

/// A new GitHub issue from one of the repository's forms, with the version and the system
/// filled in (the forms' field ids).
fn issue_url(template: &str, version: &str, system: &str) -> String {
    format!(
        "{REPOSITORY}/issues/new?template={template}&version={}&os={}",
        encode(version),
        encode(system)
    )
}

/// The list of third parties as it was for this version.
fn third_party_url(version: &str) -> String {
    format!("{REPOSITORY}/blob/v{version}/THIRD_PARTY.md")
}

/// Percent-encodes everything but the unreserved characters of a URL.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// "←/→ switches the page · Esc, H or F1 closes this page"; returns the bottom.
fn footer(painter: &Painter, left: f32, y: f32, width: f32, t: &Texts) -> f32 {
    painter.text(
        pos2(left + width / 2.0, y + 4.0),
        Align2::CENTER_TOP,
        format!("{}   ·   {}", t.help_pages_hint, t.help_close),
        FontId::proportional(text::SMALL),
        tokens::MUTED,
    );
    y + 24.0
}

/// The tips: their sections in columns like the shortcuts, each tip a paragraph with a dot in
/// front. Returns the bottom.
fn tips(painter: &Painter, left: f32, y: f32, width: f32, sections: &[TipSection]) -> f32 {
    let columns: Vec<&[TipSection]> = if width >= THREE_COLUMNS {
        balanced(sections, 3)
    } else if width >= TWO_COLUMNS {
        balanced(sections, 2)
    } else {
        vec![sections]
    };
    let gap = 36.0;
    let column_width = (width - gap * (columns.len() - 1) as f32) / columns.len() as f32;
    let mut bottom = y;
    for (i, column) in columns.iter().enumerate() {
        let x = left + i as f32 * (column_width + gap);
        let mut cy = y;
        for (title, paragraphs) in column.iter() {
            painter.text(
                pos2(x, cy),
                Align2::LEFT_TOP,
                title.to_uppercase(),
                FontId::proportional(text::LABEL),
                tokens::MUTED,
            );
            cy += 22.0;
            for paragraph in paragraphs.iter() {
                let galley = painter.layout(
                    i18n::keep_together(paragraph),
                    FontId::proportional(text::BODY),
                    tokens::TEXT,
                    column_width - 14.0,
                );
                painter.circle_filled(pos2(x + 3.0, cy + 9.0), 2.0, tokens::MUTED);
                let height = galley.size().y;
                painter.galley(pos2(x + 14.0, cy), galley, tokens::TEXT);
                cy += height + 6.0;
            }
            cy += 14.0;
        }
        bottom = bottom.max(cy);
    }
    bottom
}

/// The sections in reading order, cut into `columns` columns of about the same height – by
/// their text, not their number, so one long section doesn't make a column run on.
fn balanced(sections: &[TipSection], columns: usize) -> Vec<&[TipSection]> {
    let weight = |(_, paragraphs): &TipSection| {
        1.0 + paragraphs
            .iter()
            .map(|paragraph| 1.0 + paragraph.len() as f32 / 80.0)
            .sum::<f32>()
    };
    let target = sections.iter().map(weight).sum::<f32>() / columns.max(1) as f32;
    let mut out = Vec::new();
    let (mut start, mut filled) = (0, 0.0);
    for (i, section) in sections.iter().enumerate() {
        let w = weight(section);
        // A new column when more than half of this section would land past the target.
        if i > start && out.len() + 1 < columns && filled + w / 2.0 > target {
            out.push(&sections[start..i]);
            (start, filled) = (i, 0.0);
        }
        filled += w;
    }
    out.push(&sections[start..]);
    out
}

/// Section title and its rows; returns the bottom.
fn section(painter: &Painter, x: f32, y: f32, width: f32, title: &str, rows: &[HelpRow]) -> f32 {
    let mut y = y;
    if !title.is_empty() {
        painter.text(
            pos2(x, y),
            Align2::LEFT_TOP,
            title.to_uppercase(),
            FontId::proportional(text::LABEL),
            tokens::MUTED,
        );
        y += 22.0;
    }
    let keys_width = (width * 0.38).min(190.0);
    for (keys, action) in rows {
        let caps_height = keycaps(painter, pos2(x, y), keys_width, keys);
        let description = painter.layout(
            i18n::keep_together(action),
            FontId::proportional(text::BODY),
            tokens::TEXT,
            width - keys_width - 12.0,
        );
        let description_height = description.size().y;
        painter.galley(
            pos2(x + keys_width + 12.0, y + 3.0),
            description,
            tokens::TEXT,
        );
        y += caps_height.max(description_height + 3.0) + 7.0;
    }
    y
}

/// Keys of one row as key caps, wrapping within `max_width`; returns the height used.
fn keycaps(painter: &Painter, origin: Pos2, max_width: f32, keys: &str) -> f32 {
    const GAP: f32 = 5.0;
    let (mut x, mut y) = (origin.x, origin.y);
    let mut line_height: f32 = 0.0;
    for key in keys.split(", ") {
        let galley = painter.layout_no_wrap(
            key.to_owned(),
            FontId::proportional(text::SMALL),
            tokens::TEXT,
        );
        let size = galley.size() + vec2(14.0, 6.0);
        if x > origin.x && x + size.x > origin.x + max_width {
            x = origin.x;
            y += line_height + GAP;
        }
        let cap = Rect::from_min_size(pos2(x, y), size);
        painter.rect_filled(cap, 4.0, tokens::SURFACE_MUTED);
        painter.rect_stroke(cap, 4.0, Stroke::new(1.0, tokens::LINE), StrokeKind::Inside);
        painter.galley(cap.min + vec2(7.0, 3.0), galley, tokens::TEXT);
        x += size.x + GAP;
        line_height = line_height.max(size.y);
    }
    y + line_height - origin.y
}

fn header_button(
    ui: &Ui,
    area: Rect,
    tooltip: &str,
    paint: impl FnOnce(&Painter, Pos2, Color32),
) -> bool {
    let response = ui
        .interact(area, ui.id().with(("help-button", tooltip)), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    icons::button_background(painter, area, response.hovered(), false);
    let color = if response.hovered() {
        tokens::ACCENT_STRONG
    } else {
        tokens::MUTED
    };
    paint(painter, area.center(), color);
    response.on_hover_text(tooltip).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::RawInput;

    /// The start screen is a small card that fits a 1280 × 720 window without scrolling.
    #[test]
    fn start_screen_fits_a_small_window() {
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 720.0));
        for lang in i18n::Lang::ALL {
            let ctx = Context::default();
            // The card takes the content height of the previous frame.
            for _ in 0..2 {
                let mut output = ctx.run_ui(
                    RawInput {
                        screen_rect: Some(window),
                        ..Default::default()
                    },
                    |ui| {
                        let setup = Setup {
                            exiftool: false,
                            aesthetics: false,
                        };
                        welcome(ui, window, lang.texts(), setup);
                    },
                );
                output.textures_delta.clear();
            }
            let content = ctx
                .data(|d| d.get_temp::<f32>(Id::new(("help-height", true, Page::Keys))))
                .expect("content height");
            assert!(
                content + 40.0 <= window.height() - 48.0,
                "{lang:?}: start screen needs {content} px"
            );
        }
    }

    #[test]
    fn the_arrows_go_round_the_pages() {
        assert_eq!(Page::Keys.next(), Page::Tips);
        assert_eq!(Page::Tips.next(), Page::About);
        assert_eq!(Page::About.next(), Page::Keys);
        assert_eq!(Page::Keys.prev(), Page::About);
        for page in Page::ALL {
            assert_eq!(page.next().prev(), page);
        }
    }

    /// The forms get the version and the system – nothing else, and encoded.
    #[test]
    fn a_report_carries_version_and_system() {
        assert_eq!(
            issue_url("bug_report.yml", "1.8.1", "Ubuntu 24.04.1 LTS (x86_64)"),
            "https://github.com/fly2nbc-oss/Cerno/issues/new?template=bug_report.yml\
             &version=1.8.1&os=Ubuntu%2024.04.1%20LTS%20%28x86_64%29"
        );
        assert_eq!(encode("Ä&b=c"), "%C3%84%26b%3Dc");
        assert_eq!(
            third_party_url("1.8.1"),
            "https://github.com/fly2nbc-oss/Cerno/blob/v1.8.1/THIRD_PARTY.md"
        );
        assert!(COPYRIGHT.ends_with("fly2nbc-oss"), "{COPYRIGHT}");
    }

    /// Every page – the shortcuts, the tips and About – fits a full HD window without
    /// scrolling, in every language (handed in: tests never switch the language).
    #[test]
    fn every_page_fits_a_full_hd_window_in_every_language() {
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1080.0));
        for lang in i18n::Lang::ALL {
            for page in Page::ALL {
                let ctx = Context::default();
                for _ in 0..2 {
                    let mut output = ctx.run_ui(
                        RawInput {
                            screen_rect: Some(window),
                            ..Default::default()
                        },
                        |ui| {
                            // The longest the update section gets: a newer version's link.
                            let update = UpdateLine {
                                status: (lang.texts().update_newer)("10.10.10"),
                                release: Some((
                                    (lang.texts().cmd_update_download)("10.10.10"),
                                    "https://example.org".to_owned(),
                                )),
                                checking: false,
                            };
                            overlay(ui.ctx(), window, page, lang.texts(), &update);
                        },
                    );
                    output.textures_delta.clear();
                }
                let content = ctx
                    .data(|d| d.get_temp::<f32>(Id::new(("help-height", false, page))))
                    .expect("content height");
                assert!(
                    content + 40.0 <= window.height() - 48.0,
                    "{lang:?} {page:?} needs {content} px"
                );
            }
        }
    }

    /// A shortcut's description is a few words; what it means in detail is in the tips.
    #[test]
    fn shortcut_descriptions_stay_short() {
        for lang in i18n::Lang::ALL {
            let t = lang.texts();
            let rows = [
                &t.help_browse[..],
                &t.help_rate,
                &t.help_cull,
                &t.help_video,
                &t.help_view,
                &t.help_panels,
                &t.help_edit,
                &t.help_more,
                &t.welcome_keys,
            ];
            for (keys, description) in rows.into_iter().flatten() {
                assert!(
                    description.chars().count() <= 40,
                    "{lang:?} {keys}: {description}"
                );
            }
        }
    }

    #[test]
    fn tips_columns_are_balanced_and_keep_the_order() {
        let t = i18n::Lang::En.texts();
        for columns in [2, 3] {
            let split = balanced(&t.help_tips, columns);
            assert_eq!(split.len(), columns);
            assert!(split.iter().all(|column| !column.is_empty()));
            let joined: Vec<_> = split.concat();
            assert_eq!(joined, t.help_tips.to_vec());
        }
    }
}
