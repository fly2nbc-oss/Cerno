//! UI languages: German, English, French, Spanish and Italian.
//!
//! Every language is one `Texts` literal, so a missing text is a compile error. Texts with
//! values are plain functions, which keeps the word order free per language and the
//! arguments type-checked. The current language is global (read on every frame, changed by
//! `L`); it starts as English, so tests are independent of the machine's locale.

use std::sync::atomic::{AtomicU8, Ordering};

mod de;
mod en;
mod es;
mod fr;
mod it;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
    Fr,
    Es,
    It,
}

impl Lang {
    /// Order of `L`.
    pub const ALL: [Lang; 5] = [Self::De, Self::En, Self::Fr, Self::Es, Self::It];

    pub fn code(self) -> &'static str {
        match self {
            Self::De => "de",
            Self::En => "en",
            Self::Fr => "fr",
            Self::Es => "es",
            Self::It => "it",
        }
    }

    /// `de`, `de-DE`, `de_CH.UTF-8` → German.
    pub fn from_code(code: &str) -> Option<Self> {
        let primary = code.split(['-', '_', '.']).next()?.to_ascii_lowercase();
        Self::ALL.into_iter().find(|l| l.code() == primary)
    }

    /// The language's own name.
    pub fn name(self) -> &'static str {
        match self {
            Self::De => "Deutsch",
            Self::En => "English",
            Self::Fr => "Français",
            Self::Es => "Español",
            Self::It => "Italiano",
        }
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|l| *l == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn texts(self) -> &'static Texts {
        match self {
            Self::De => &de::TEXTS,
            Self::En => &en::TEXTS,
            Self::Fr => &fr::TEXTS,
            Self::Es => &es::TEXTS,
            Self::It => &it::TEXTS,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(1);

pub fn current() -> Lang {
    Lang::ALL
        .get(CURRENT.load(Ordering::Relaxed) as usize)
        .copied()
        .unwrap_or(Lang::En)
}

pub fn set(lang: Lang) {
    let index = Lang::ALL.iter().position(|l| *l == lang).unwrap_or(1);
    CURRENT.store(index as u8, Ordering::Relaxed);
}

/// Texts of the current language.
pub fn t() -> &'static Texts {
    current().texts()
}

/// The colour's name in the current language. The file still stores the English XMP name.
pub fn label_name(label: crate::metadata::Label) -> &'static str {
    use crate::metadata::Label;
    let t = t();
    match label {
        Label::Red => t.label_red,
        Label::Yellow => t.label_yellow,
        Label::Green => t.label_green,
        Label::Blue => t.label_blue,
        Label::Purple => t.label_purple,
    }
}

/// The system language if Cerno speaks it, otherwise English.
pub fn system_default() -> Lang {
    sys_locale::get_locale()
        .as_deref()
        .and_then(Lang::from_code)
        .unwrap_or(Lang::En)
}

/// How capture dates are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateStyle {
    /// 2026-09-12
    Iso,
    /// 12.09.2026 or 12/09/2026
    DayMonthYear(char),
}

/// `2026-09-12 14:03` (as `metadata` stores it) in the current language's style.
pub fn date(taken: &str) -> String {
    format_date(t(), taken)
}

fn format_date(t: &Texts, taken: &str) -> String {
    let style = t.date_style;
    let (Some(y), Some(m), Some(d)) = (taken.get(0..4), taken.get(5..7), taken.get(8..10)) else {
        return taken.to_owned();
    };
    let time = taken.get(10..).unwrap_or_default();
    match style {
        DateStyle::Iso => taken.to_owned(),
        DateStyle::DayMonthYear(sep) => format!("{d}{sep}{m}{sep}{y}{time}"),
    }
}

/// `48.52160° N, 9.05760° E` with the language's compass letters.
pub fn coordinates(lat: f64, lon: f64) -> String {
    format_coordinates(t(), lat, lon)
}

fn format_coordinates(t: &Texts, lat: f64, lon: f64) -> String {
    let [north, south, east, west] = t.compass;
    format!(
        "{:.5}° {}, {:.5}° {}",
        lat.abs(),
        if lat < 0.0 { south } else { north },
        lon.abs(),
        if lon < 0.0 { west } else { east },
    )
}

/// `Strg+K` / `Ctrl+K` with this language's key name.
pub fn with_ctrl(key: &str) -> String {
    format!("{}+{key}", t().key_ctrl)
}

/// `Umschalt+Tab` / `Shift+Tab` with this language's key name.
pub fn with_shift(key: &str) -> String {
    format!("{}+{key}", t().key_shift)
}

/// Glues "50 %" and French "valeur :" together, so wrapping never leaves a lone sign at the
/// start of a line (egui does not break at U+00A0).
pub fn keep_together(text: &str) -> String {
    let mut out = text.to_owned();
    for sign in ['%', ':', ';', '?', '!'] {
        out = out.replace(&format!(" {sign}"), &format!("\u{a0}{sign}"));
    }
    out
}

/// One line of the help page: keys (comma-separated, each drawn as a key cap) and what they do.
pub type HelpRow = (&'static str, &'static str);

pub struct Texts {
    pub date_style: DateStyle,
    /// North, south, east, west.
    pub compass: [&'static str; 4],
    /// Modifier key names as printed on this language's keyboards (`Strg`, `Umschalt`).
    pub key_ctrl: &'static str,
    pub key_shift: &'static str,
    /// The Delete key as printed on keyboards (menu shortcuts).
    pub key_delete: &'static str,

    // Toolbar.
    pub sort: fn(&str) -> String,
    pub filter_stars: fn(u8) -> String,
    pub filter_blurry: &'static str,
    pub filter_blurry_tooltip: &'static str,
    /// The later copy, not the first identical photo.
    pub filter_duplicate: &'static str,
    pub filter_duplicate_tooltip: &'static str,
    pub filter_clear: &'static str,
    /// Why "Show all" is greyed out: nothing is filtered.
    pub filter_none_active: &'static str,
    /// The filter bar's last box: only photos like the chosen one.
    pub filter_similar: &'static str,
    /// That box while it is on, with the (shortened) name of the photo it is about.
    pub filter_similar_to: fn(&str) -> String,
    /// Visible photos ▸ Filter ▸, the last row.
    pub menu_similar: &'static str,
    pub menu_similar_to: fn(&str) -> String,
    /// Tooltip of the box; the threshold in percent.
    pub similar_tooltip: fn(f32) -> String,
    /// In the info bar while the filter is on: how alike the current photo is, in percent.
    pub similar_fact: fn(f32) -> String,
    /// Hints when `M` can't filter.
    pub similar_needs_model: &'static str,
    pub similar_not_analysed: &'static str,
    pub similar_none: fn(f32) -> String,
    pub refresh_order: &'static str,
    pub refresh_order_tooltip: &'static str,
    pub analyzing_progress: fn(usize, usize) -> String,
    pub enable_aesthetics: &'static str,
    pub enable_aesthetics_tooltip: &'static str,
    pub downloading_model: fn(f64) -> String,
    pub aesthetics_loading: &'static str,
    pub aesthetics_failed: &'static str,

    // Sorting and filtering.
    pub sort_name: &'static str,
    pub sort_rating: &'static str,
    pub sort_aesthetics: &'static str,
    pub sort_personal: &'static str,
    pub sort_sharpness: &'static str,
    pub sort_taken: &'static str,
    pub filter_unrated: &'static str,
    pub filter_rejected: &'static str,
    pub actions: &'static str,
    pub actions_tooltip: &'static str,
    pub selection_delete: &'static str,
    pub label_red: &'static str,
    pub label_yellow: &'static str,
    pub label_green: &'static str,
    pub label_blue: &'static str,
    pub label_purple: &'static str,

    // Info bar.
    pub meter_aesthetics_tooltip: &'static str,
    pub meter_sharpness: &'static str,
    pub meter_eyes: &'static str,
    pub probably_blurry: &'static str,
    pub analyzing: &'static str,
    pub saving: &'static str,
    /// Shown while auto-advance is on.
    pub auto_advance_on: &'static str,
    /// Position within a series, 1-based, and how many photos the series has.
    pub series_position: fn(u32, u32) -> String,
    pub duplicate_of: fn(&str) -> String,
    /// Marks a rejected photo (info bar, compare label).
    pub rejected: &'static str,
    /// Tooltip of a video's cell in the filmstrip (the play sign).
    pub filmstrip_video: &'static str,
    pub star_tooltip: fn(u8) -> String,
    pub zoom: fn(f32) -> String,
    pub digital_zoom: fn(f64) -> String,
    pub button_toolbar: &'static str,
    pub button_details: &'static str,
    pub button_filmstrip: &'static str,
    pub button_help: &'static str,
    pub button_menu: &'static str,
    pub button_language: fn(&str) -> String,

    // Command menu (`Ctrl+K`).
    pub cmd_explanations: &'static str,
    pub cmd_all_panels: &'static str,
    pub cmd_fullscreen: &'static str,
    pub cmd_compare: &'static str,
    /// This photo ▸ (`M`): only photos like this one.
    pub cmd_similar: &'static str,
    pub cmd_zoom: &'static str,
    /// View ▸ Overlay ▸ (`O`): marks on the photo.
    pub menu_overlay: &'static str,
    pub overlay_off: &'static str,
    pub overlay_sharpness: &'static str,
    pub overlay_exposure: &'static str,
    /// In the info bar while the overlay is on.
    pub overlay_fact_sharpness: &'static str,
    pub overlay_fact_exposure: &'static str,
    /// Hints when `O` switches, with the legend.
    pub overlay_hint_sharpness: &'static str,
    pub overlay_hint_exposure: &'static str,
    pub overlay_hint_off: &'static str,
    /// Tooltip of the eye next to Sharpness and Exposure in the details panel.
    pub overlay_show_on_photo: &'static str,
    /// View ▸ (`F7`): every photo as a thumbnail.
    pub cmd_grid: &'static str,
    pub cmd_reject: &'static str,
    /// Menu row (This photo): the Description tab of the details panel (`B`).
    pub cmd_description: &'static str,
    pub cmd_delete_rejected: fn(usize) -> String,
    pub cmd_auto_advance: &'static str,
    pub cmd_subfolders: &'static str,
    pub menu_sort: &'static str,
    pub menu_filter: &'static str,
    pub menu_view: &'static str,
    pub menu_labels: &'static str,
    /// Submenu of This photo: open it in another program (`E`).
    pub menu_external: &'static str,
    /// Pick a program file by hand (remembered).
    pub external_other: &'static str,
    /// Windows: the system's own "Open with" chooser – what is picked there is not remembered.
    pub external_chooser: &'static str,
    /// Linux, instead of `external_chooser`: `xdg-open` starts the default program, it shows no choice.
    pub external_default: &'static str,
    /// Title of the file dialog for another program.
    pub external_pick_title: &'static str,
    /// The photo is open in another program.
    pub external_opened: fn(&str) -> String,
    /// Another program saved the photo; Cerno shows the new version.
    pub external_reloaded: fn(&str) -> String,
    /// The program could not be started (or the original not be kept).
    pub external_failed: fn(&str) -> String,
    pub menu_language: &'static str,
    pub menu_models: &'static str,
    /// Menu group: everything that acts on the current photo.
    pub menu_this_photo: &'static str,
    /// Menu group: sorting and filtering, and what acts on every photo the filter shows.
    pub menu_visible: &'static str,
    /// Menu group: auto advance, subfolders, language, models.
    pub menu_settings: &'static str,
    /// Submenu of This photo: 0–5 stars.
    pub menu_stars: &'static str,
    /// Colour label submenu: remove the colour.
    pub label_none: &'static str,

    // Straighten and crop.
    pub cmd_straighten: &'static str,
    pub cmd_rotate_ccw: &'static str,
    pub cmd_rotate_cw: &'static str,
    pub cmd_crop: &'static str,
    pub cmd_undo: &'static str,
    pub edit_not_jpeg: &'static str,
    /// Over a video's frame: how to play it.
    pub video_play_hint: &'static str,
    /// Over a video's placeholder when ffmpeg is not installed.
    pub video_no_ffmpeg: &'static str,
    /// The system's player could not be started.
    pub video_play_failed: fn(&str) -> String,
    pub edit_writing: &'static str,
    /// The edited photo is no longer the current one (a copy finished, a filter changed).
    pub edit_cancelled: &'static str,
    /// Why an action waits: one action at a time on a photo.
    pub busy_editing: &'static str,
    pub busy_copying: &'static str,
    pub busy_moving: &'static str,
    /// Straighten, crop, quarter turns and Ctrl+Z are off while the index is the in-memory fallback.
    pub edit_needs_index: &'static str,
    pub edit_reencoded: &'static str,
    pub undo_done: &'static str,
    pub undo_nothing: &'static str,
    pub edit_failed: fn(&str) -> String,
    /// Banner hints while straightening and cropping.
    pub edit_hint_straighten: &'static str,
    pub edit_hint_crop: &'static str,
    pub ratio_original: &'static str,
    pub crop_landscape: &'static str,
    pub crop_portrait: &'static str,

    // Photo area, compare mode and deletion.
    pub loading: &'static str,
    pub cannot_show: &'static str,
    pub no_match: &'static str,
    pub drop_to_open: &'static str,
    pub compare_left: &'static str,
    pub compare_right: &'static str,
    /// Filmstrip badge of the left photo in compare mode (one letter).
    pub compare_left_badge: &'static str,
    pub keeps_this: fn(&str) -> String,
    pub compare_needs_two: &'static str,
    pub deleting: fn(usize) -> String,
    pub delete_failed: fn(usize, &str, &str) -> String,
    /// `true`: the eyes were measured, `false`: the whole photo; percent 0..100.
    pub blurry_tooltip: fn(bool, f32) -> String,

    // Notices and dialogs.
    pub db_unavailable: fn(&str) -> String,
    pub cannot_open: fn(&str, &str) -> String,
    pub no_photos_in: fn(&str) -> String,
    pub rating_not_saved: fn(&str) -> String,
    pub open_folder: &'static str,
    pub transfer_copy: &'static str,
    pub transfer_move: &'static str,
    pub transfer_copy_cmd: &'static str,
    pub transfer_move_cmd: &'static str,
    pub transfer_same_folder: &'static str,
    pub transfer_busy: &'static str,
    /// `moved`: the verb; then how many succeeded, how many were skipped, and the first
    /// failure (`name` and `error` empty when every file worked).
    pub transfer_done: fn(bool, usize, usize, &str, &str) -> String,
    pub download_title: &'static str,
    /// Download size in GB.
    pub download_text: fn(f64) -> String,
    pub btn_download: &'static str,
    pub btn_cancel: &'static str,
    pub btn_close: &'static str,
    /// One-time hint after the first folder opens while the CLIP model is missing.
    pub aesthetics_offer: &'static str,

    // Details panel.
    pub section_aesthetics: &'static str,
    pub section_sharpness: &'static str,
    pub section_exposure: &'static str,
    pub section_attributes: &'static str,
    pub section_histogram: &'static str,
    pub section_file: &'static str,
    pub row_size: &'static str,
    pub row_load_time: &'static str,
    pub row_location: &'static str,
    /// Details panel tabs: the analysis values …
    pub tab_values: &'static str,
    /// … and the photo's comment and keywords.
    pub tab_description: &'static str,
    pub section_comment: &'static str,
    pub section_keywords: &'static str,
    /// Placeholder of the empty comment field.
    pub comment_hint: &'static str,
    /// Placeholder of the field that adds a keyword.
    pub keyword_hint: &'static str,
    /// Tooltip of a keyword's ×.
    pub keyword_remove: &'static str,
    /// Under the fields: how they are used and where the values go.
    pub description_note: &'static str,
    /// While the photo is still loading, its comment and keywords are not known yet.
    pub description_waiting: &'static str,
    pub row_laion: &'static str,
    pub row_v25: &'static str,
    pub row_personal: &'static str,
    pub row_frame: &'static str,
    pub row_eyes: &'static str,
    pub row_highlights: &'static str,
    pub row_shadows: &'static str,
    /// Same order as `analysis::attributes`.
    pub attributes: [&'static str; 6],
    pub explain_laion: &'static str,
    pub explain_v25: &'static str,
    pub explain_personal: &'static str,
    pub explain_frame: &'static str,
    pub explain_eyes: &'static str,
    pub explain_highlights: &'static str,
    pub explain_shadows: &'static str,
    pub explain_attributes: &'static str,
    /// Per attribute: the two descriptions that are compared.
    pub explain_attribute: [&'static str; 6],
    pub explain_models: &'static str,
    pub note_no_embedding: &'static str,
    pub note_learning: fn(usize, usize) -> String,
    pub note_analysing: &'static str,
    pub note_no_face: &'static str,
    pub note_faces_too_small: fn(u8) -> String,
    pub note_needs_clip: &'static str,
    pub model_missing: &'static str,
    pub model_downloading: fn(f64) -> String,
    pub model_ready: &'static str,
    pub model_loading: &'static str,
    /// While "Delete models" waits for a running analysis and removes the files.
    pub model_removing: &'static str,
    pub model_failed: &'static str,
    pub model_faces: &'static str,
    pub model_personal: &'static str,
    pub taste_trained: fn(usize, f32) -> String,
    pub taste_photos: fn(usize) -> String,
    pub taste_untrained: &'static str,
    pub btn_reset_taste: &'static str,
    pub btn_delete_models: &'static str,
    /// Hint once the model files are gone.
    pub models_deleted: &'static str,
    /// Tooltip on the button that copies the models folder path.
    pub copy_models_path: &'static str,
    /// Shown briefly after that button copies the path.
    pub models_path_copied: &'static str,
    pub confirm_reset_taste_title: &'static str,
    pub confirm_reset_taste_text: &'static str,
    pub confirm_delete_models_title: &'static str,
    pub confirm_delete_models_text: &'static str,

    // Help page and start screen.
    pub help_title: &'static str,
    pub help_intro: &'static str,
    pub help_drop: &'static str,
    pub help_close: &'static str,
    /// Start screen: one sentence, the five keys to begin with, and where the rest is.
    pub welcome_intro: &'static str,
    pub welcome_keys: [HelpRow; 5],
    pub welcome_more: &'static str,
    pub help_sections: [&'static str; 5],
    pub help_browse: [HelpRow; 5],
    pub help_rate: [HelpRow; 10],
    pub help_view: [HelpRow; 14],
    pub help_edit: [HelpRow; 6],
    pub help_more: [HelpRow; 5],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_cycle() {
        assert_eq!(Lang::from_code("de-DE"), Some(Lang::De));
        assert_eq!(Lang::from_code("es_ES.UTF-8"), Some(Lang::Es));
        assert_eq!(Lang::from_code("FR"), Some(Lang::Fr));
        assert_eq!(Lang::from_code("pt-BR"), None);
        let mut lang = Lang::De;
        for expected in [Lang::En, Lang::Fr, Lang::Es, Lang::It, Lang::De] {
            lang = lang.next();
            assert_eq!(lang, expected);
        }
    }

    /// Every language keeps the values it is given (a template that drops one is a bug).
    #[test]
    fn texts_show_their_values() {
        for lang in Lang::ALL {
            let t = lang.texts();
            let name = lang.name();
            assert!((t.sort)("X").contains('X'), "{name}");
            assert!((t.filter_stars)(1).contains('1'), "{name}");
            assert!((t.filter_stars)(3).contains('3'), "{name}");
            let progress = (t.analyzing_progress)(4, 9);
            assert!(progress.contains('4') && progress.contains('9'), "{name}");
            assert!((t.downloading_model)(42.0).contains("42"), "{name}");
            let moved = (t.transfer_done)(true, 4, 2, "a.jpg", "busy");
            assert!(
                moved.contains('4')
                    && moved.contains('2')
                    && moved.contains("a.jpg")
                    && moved.contains("busy"),
                "{name}"
            );
            let copied = (t.transfer_done)(false, 1, 0, "", "");
            assert!(copied.contains('1') && copied.contains('0'), "{name}");
            assert!((t.star_tooltip)(4).contains('4'), "{name}");
            assert!((t.zoom)(250.0).contains("250"), "{name}");
            assert!((t.digital_zoom)(2.0).contains('2'), "{name}");
            assert!((t.button_language)(name).contains(name), "{name}");
            assert!((t.cmd_delete_rejected)(7).contains('7'), "{name}");
            let series = (t.series_position)(3, 7);
            assert!(series.contains('3') && series.contains('7'), "{name}");
            assert!((t.duplicate_of)("a.jpg").contains("a.jpg"), "{name}");
            assert!((t.filter_similar_to)("a.jpg").contains("a.jpg"), "{name}");
            assert!((t.menu_similar_to)("a.jpg").contains("a.jpg"), "{name}");
            assert!((t.similar_tooltip)(85.0).contains("85"), "{name}");
            assert!((t.similar_fact)(93.4).contains("93"), "{name}");
            assert!((t.similar_none)(85.0).contains("85"), "{name}");
            assert!((t.keeps_this)("A").contains('A'), "{name}");
            assert!((t.deleting)(1).contains('1'), "{name}");
            assert!((t.deleting)(12).contains("12"), "{name}");
            let failed = (t.delete_failed)(2, "a.jpg", "busy");
            assert!(failed.contains('2') && failed.contains("a.jpg") && failed.contains("busy"));
            assert!((t.blurry_tooltip)(true, 12.0).contains("12"), "{name}");
            assert!((t.blurry_tooltip)(false, 7.0).contains('7'), "{name}");
            assert!((t.db_unavailable)("disk").contains("disk"), "{name}");
            let open = (t.cannot_open)("D:/x", "gone");
            assert!(open.contains("D:/x") && open.contains("gone"), "{name}");
            assert!((t.no_photos_in)("D:/x").contains("D:/x"), "{name}");
            assert!((t.rating_not_saved)("locked").contains("locked"), "{name}");
            assert!((t.download_text)(1.2).contains("1.2"), "{name}");
            let learning = (t.note_learning)(3, 15);
            assert!(learning.contains('3') && learning.contains("15"), "{name}");
            assert!((t.note_faces_too_small)(2).contains('2'), "{name}");
            assert!((t.model_downloading)(42.0).contains("42"), "{name}");
            let trained = (t.taste_trained)(30, 0.7);
            assert!(trained.contains("30") && trained.contains("0.7"), "{name}");
            assert!((t.taste_photos)(30).contains("30"), "{name}");
            assert!((t.edit_failed)("locked").contains("locked"), "{name}");
            assert!((t.video_play_failed)("no app").contains("no app"), "{name}");
            assert!((t.external_opened)("GIMP").contains("GIMP"), "{name}");
            assert!((t.external_reloaded)("a.jpg").contains("a.jpg"), "{name}");
            assert!((t.external_failed)("gone").contains("gone"), "{name}");
            assert_eq!(t.compare_left_badge.chars().count(), 1, "{name}");
        }
    }

    #[test]
    fn help_rows_are_filled() {
        for lang in Lang::ALL {
            let t = lang.texts();
            let rows = t
                .help_browse
                .iter()
                .chain(&t.help_rate)
                .chain(&t.help_view)
                .chain(&t.help_edit)
                .chain(&t.help_more)
                .chain(&t.welcome_keys);
            for (keys, action) in rows {
                assert!(!keys.trim().is_empty() && !action.trim().is_empty());
            }
        }
    }

    #[test]
    fn signs_stay_with_their_word() {
        assert_eq!(
            keep_together("50 % – valeur : oui ?"),
            "50\u{a0}% – valeur\u{a0}: oui\u{a0}?"
        );
        assert_eq!(keep_together("1–10; x"), "1–10; x");
    }

    #[test]
    fn dates_and_coordinates() {
        // Tests never switch the global language: they run in parallel.
        let (de, en) = (Lang::De.texts(), Lang::En.texts());
        assert_eq!(format_date(de, "2026-09-12 14:03"), "12.09.2026 14:03");
        assert_eq!(format_date(en, "2026-09-12 14:03"), "2026-09-12 14:03");
        assert_eq!(format_date(de, "odd"), "odd");
        assert_eq!(
            format_coordinates(de, 48.5216, -9.0576),
            "48.52160° N, 9.05760° W"
        );
        assert_eq!(
            format_coordinates(de, -33.9, 151.2),
            "33.90000° S, 151.20000° O"
        );
    }
}
