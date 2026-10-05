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

/// `850 MB` / `4.1 GB` with this language's unit (French `Go`); 1 GB = 10⁹ bytes.
pub fn size(bytes: u64) -> String {
    format_sizes(t(), None, bytes)
}

/// `1.2 / 8.4 GB`: both in the unit of the total.
pub fn sizes(done: u64, total: u64) -> String {
    format_sizes(t(), Some(done), total)
}

fn format_sizes(t: &Texts, done: Option<u64>, total: u64) -> String {
    let [kb, mb, gb] = t.size_units;
    let (scale, unit, decimals) = match total {
        0..1_000_000 => (1e3, kb, 0),
        1_000_000..10_000_000 => (1e6, mb, 1),
        10_000_000..1_000_000_000 => (1e6, mb, 0),
        _ => (1e9, gb, 1),
    };
    let show = |bytes: u64| format!("{:.*}", decimals, bytes as f64 / scale);
    match done {
        Some(done) => format!("{} / {} {unit}", show(done), show(total)),
        None => format!("{} {unit}", show(total)),
    }
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

/// One section of the help page's tips: a title and its tips, a paragraph each.
pub type TipSection = (&'static str, &'static [&'static str]);

pub struct Texts {
    pub date_style: DateStyle,
    /// North, south, east, west.
    pub compass: [&'static str; 4],
    /// Kilobyte, megabyte, gigabyte (French `Ko`, `Mo`, `Go`).
    pub size_units: [&'static str; 3],
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
    /// People filter (a face found / none): the menu row and the icon chip's tooltip.
    pub filter_people: &'static str,
    pub filter_no_people: &'static str,
    pub filter_people_tooltip: &'static str,
    /// The 🗑 box: photos deleted in Cerno, lying in `.originals`.
    pub filter_deleted: &'static str,
    pub filter_deleted_tooltip: &'static str,
    pub filter_deleted_none: &'static str,
    /// "without ✕": the rejected photos are hidden (menu row, tooltip title).
    pub filter_hide_rejected: &'static str,
    /// The word before the cross in the filter bar's "without ✕" box.
    pub filter_without: &'static str,
    pub filter_hide_rejected_tooltip: &'static str,
    pub menu_name_list: &'static str,
    pub name_list_title: &'static str,
    pub name_list_intro: &'static str,
    pub name_list_hint: &'static str,
    pub name_list_missing: &'static str,
    pub name_list_ambiguous: &'static str,
    pub name_list_apply: &'static str,
    pub name_list_chip_tooltip: &'static str,
    pub name_list_found: fn(usize, usize) -> String,
    pub name_list_chip: fn(usize, usize) -> String,
    pub cmd_align_camera: &'static str,
    pub align_camera_hint: &'static str,
    pub align_needs_compare: &'static str,
    pub align_no_time: &'static str,
    pub align_same_camera: &'static str,
    pub menu_camera_time: &'static str,
    pub camera_time_title: &'static str,
    pub camera_time_intro: &'static str,
    pub camera_time_reset: &'static str,
    pub camera_time_invalid: &'static str,
    pub camera_time_apply: &'static str,
    pub camera_time_none: &'static str,
    pub camera_time_applied: &'static str,
    pub camera_aligned: fn(&str, &str) -> String,
    pub camera_time_photos: fn(usize) -> String,
    pub camera_time_tooltip: fn(&str, &str) -> String,
    pub filter_clear: &'static str,
    /// The filter bar's photos / videos box and the same rows in Filter ▸.
    pub media_all: &'static str,
    pub media_photos: &'static str,
    pub media_videos: &'static str,
    pub media_no_videos: &'static str,
    /// The filter bar's first box and Filter ▸ Best photos ▸: the best N photos.
    pub top_photos: fn(u16) -> String,
    /// What each Top level (10, 25, 50, 100, 250) is for, beside it in the list.
    pub top_purposes: [&'static str; 5],
    /// Tooltip of the first box: what it chooses, and how Top N picks.
    pub top_tooltip: &'static str,
    /// Visible photos ▸ Filter ▸, the group of the Top levels.
    pub menu_top: &'static str,
    /// Why the action menu's delete row is greyed out while Top N is on.
    pub bulk_delete_top: &'static str,
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
    pub enable_aesthetics_tooltip: fn(&str) -> String,
    /// Instead of `enable_aesthetics` when only V2.5 is missing.
    pub add_v25: &'static str,
    pub add_v25_tooltip: fn(&str) -> String,
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
    /// The action menu's rows, with the number of photos the filter shows.
    pub bulk_copy: fn(usize) -> String,
    pub bulk_move: fn(usize) -> String,
    pub bulk_delete: fn(usize) -> String,
    /// Tooltip of the delete row: what goes, and where.
    pub bulk_delete_hint: &'static str,
    pub delete_rejected_hint: &'static str,
    /// The action menu's row for the deleted photos the filter shows.
    pub bulk_restore: fn(usize) -> String,
    pub bulk_restore_hint: &'static str,
    /// The filter bar's count while a filter is on: shown of all.
    pub photos_shown: fn(usize, usize) -> String,
    /// The filter bar's count without a filter.
    pub photos_count: fn(usize) -> String,
    pub photos_badge_tooltip: &'static str,
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
    /// Tooltip over the light stars: the prediction, 0..=5.
    pub personal_hint: fn(f32) -> String,
    pub zoom: fn(f32) -> String,
    /// The zoom of a RAW file: 100 % is the size of its preview, not of the sensor.
    pub zoom_preview: fn(f32) -> String,
    pub digital_zoom: fn(f64) -> String,
    pub button_toolbar: &'static str,
    pub button_details: &'static str,
    pub button_filmstrip: &'static str,
    pub button_help: &'static str,
    pub button_menu: &'static str,
    pub button_language: fn(&str) -> String,

    // Command menu (`Ctrl+K`).
    pub cmd_all_panels: &'static str,
    pub cmd_fullscreen: &'static str,
    pub cmd_compare: &'static str,
    pub cmd_quad: &'static str,
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
    /// Hint after `Ctrl+U` (or the menu) turned subfolders on.
    pub subfolders_on: &'static str,
    pub subfolders_off: &'static str,
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
    /// Tooltip of the play button over a video's frame.
    pub video_play_hint: &'static str,
    /// Tooltips of the video bar.
    pub video_play_pause: &'static str,
    pub video_mute: &'static str,
    pub video_volume: &'static str,
    /// Zoom keys or compare on a video.
    pub video_no_zoom: &'static str,
    pub video_no_compare: &'static str,
    /// Once per run: no sound device, the video plays without sound.
    pub video_no_sound: &'static str,
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
    /// Why a deleted photo takes no mark or edit.
    pub busy_deleted: &'static str,
    /// Straighten, crop, quarter turns and Ctrl+Z are off while the index is the in-memory fallback.
    pub edit_needs_index: &'static str,
    pub edit_reencoded: &'static str,
    pub undo_done: &'static str,
    pub undo_nothing: &'static str,
    /// Puts the deleted photo shown back into its folder (`Ctrl+Z`).
    pub cmd_restore: &'static str,
    /// Photos put back, how many of them under a new name, and the (new) name of the first.
    pub restored: fn(usize, usize, &str) -> String,
    pub restore_failed: fn(usize, &str, &str) -> String,
    /// Beside the name in the info bar while a deleted photo shows.
    pub deleted_mark: &'static str,
    pub faces_loading: &'static str,
    pub faces_unknown: &'static str,
    pub faces_none: &'static str,
    pub faces_only_small: &'static str,
    pub face_eyes_blurry: &'static str,
    pub faces_zoom_hint: &'static str,
    pub faces_grid_hint: &'static str,
    pub cmd_faces: &'static str,
    pub cmd_face_grid: &'static str,
    pub faces_small: fn(usize) -> String,
    pub face_number: fn(usize) -> String,
    /// In the info bar while a RAW file shows: its embedded JPEG preview, not a development.
    pub raw_preview_fact: &'static str,
    /// After a size or a section title that is about a RAW file's preview.
    pub preview_word: &'static str,
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
    pub transfer_copy_cmd: &'static str,
    pub transfer_move_cmd: &'static str,
    pub transfer_same_folder: &'static str,
    pub transfer_busy: &'static str,
    /// `moved`: the verb; then how many succeeded, how many were skipped, and the first
    /// failure (`name` and `error` empty when every file worked).
    pub transfer_done: fn(bool, usize, usize, &str, &str) -> String,
    /// While a copy or move runs: `moved`: the verb; the photo in progress (from 1) of how many;
    /// the sizes so far (`i18n::sizes`); a large file in progress as `name (4.1 GB)`, else empty.
    pub transfer_progress: fn(bool, usize, usize, &str, &str) -> String,
    pub download_title: &'static str,
    /// Which models are missing (CLIP, V2.5), and their size together.
    pub download_text: fn(bool, bool, &str) -> String,
    pub btn_download: &'static str,
    pub btn_cancel: &'static str,
    pub btn_close: &'static str,
    /// One-time hint after the first folder opens while the CLIP model is missing; the size
    /// of the missing models.
    pub aesthetics_offer: fn(&str) -> String,
    /// One-time hint when CLIP is there but V2.5 is missing; its size.
    pub v25_offer: fn(&str) -> String,

    // Details panel.
    pub section_aesthetics: &'static str,
    pub section_sharpness: &'static str,
    pub section_exposure: &'static str,
    pub section_attributes: &'static str,
    /// The combined score's row in the details panel (`aesthetic::combined`).
    pub row_aesthetics: &'static str,
    pub section_histogram: &'static str,
    pub section_file: &'static str,
    pub row_size: &'static str,
    pub row_load_time: &'static str,
    pub row_file_size: &'static str,
    pub row_jpeg_quality: &'static str,
    pub explain_jpeg_quality: &'static str,
    pub explain_raw_preview: &'static str,
    /// The file section of a video (`playback::probe`).
    pub row_container: &'static str,
    pub row_duration: &'static str,
    pub row_video: &'static str,
    pub row_frame_rate: &'static str,
    pub row_video_bitrate: &'static str,
    pub row_audio: &'static str,
    pub row_audio_bitrate: &'static str,
    pub row_bitrate: &'static str,
    pub no_audio: &'static str,
    /// Details › File: the video states no fixed frame rate (phones record so).
    pub variable_frame_rate: &'static str,
    /// Sound channels: Mono, Stereo, 5.1 …
    pub channels: fn(u32) -> String,
    pub row_location: &'static str,
    /// Details panel tabs: the analysis values …
    pub tab_values: &'static str,
    /// … and the photo's comment and keywords.
    pub tab_description: &'static str,
    pub tab_faces: &'static str,
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
    pub explain_aesthetics: &'static str,
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
    /// Models card: where the prediction's examples come from (stars, rejected, deleted).
    pub taste_sources: fn(usize, usize, usize) -> String,
    pub btn_reset_taste: &'static str,
    pub btn_delete_models: &'static str,
    /// Hint once the model files are gone.
    pub models_deleted: &'static str,
    pub models_downloaded: &'static str,
    pub download_failed: fn(&str) -> String,
    /// Tooltip on the button that copies the models folder path.
    pub copy_models_path: &'static str,
    /// Shown briefly after that button copies the path.
    pub models_path_copied: &'static str,
    pub confirm_reset_taste_title: &'static str,
    pub confirm_reset_taste_text: &'static str,
    pub confirm_delete_models_title: &'static str,
    pub confirm_delete_models_text: fn(&str) -> String,

    // Help page and start screen.
    pub help_title: &'static str,
    pub help_intro: &'static str,
    pub help_drop: &'static str,
    pub help_close: &'static str,
    /// The help page's two tabs.
    pub help_tab_keys: &'static str,
    pub help_tab_tips: &'static str,
    pub help_pages_hint: &'static str,
    /// The tips page: how to work with Cerno, six short sections.
    pub help_tips: [TipSection; 6],
    /// Start screen: one sentence, the five keys to begin with, and where the rest is.
    pub welcome_intro: &'static str,
    pub welcome_keys: [HelpRow; 5],
    pub welcome_more: &'static str,
    pub help_sections: [&'static str; 8],
    pub help_browse: [HelpRow; 6],
    pub help_rate: [HelpRow; 7],
    pub help_cull: [HelpRow; 6],
    pub help_video: [HelpRow; 4],
    pub help_view: [HelpRow; 7],
    pub help_panels: [HelpRow; 5],
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
            let moving = (t.transfer_progress)(true, 12, 340, "1.2 / 8.4 GB", "V.MP4 (4.1 GB)");
            assert!(
                moving.contains("12")
                    && moving.contains("340")
                    && moving.contains("1.2 / 8.4 GB")
                    && moving.contains("V.MP4 (4.1 GB)"),
                "{name}"
            );
            assert!(
                (t.transfer_progress)(false, 1, 2, "3 / 5 KB", "").contains("3 / 5 KB"),
                "{name}"
            );
            assert!((t.star_tooltip)(4).contains('4'), "{name}");
            assert!((t.personal_hint)(2.4).contains("2.4"), "{name}");
            assert!((t.zoom)(250.0).contains("250"), "{name}");
            assert!((t.zoom_preview)(100.0).contains("100"), "{name}");
            assert!((t.bulk_restore)(12).contains("12"), "{name}");
            for list in [t.name_list_found, t.name_list_chip] {
                let text = list(23, 25);
                assert!(text.contains("23") && text.contains("25"), "{name}");
            }
            let aligned = (t.camera_aligned)("Pixel 7a", "+2:09:37");
            assert!(
                aligned.contains("Pixel 7a") && aligned.contains("+2:09:37"),
                "{name}"
            );
            let adjusted = (t.camera_time_tooltip)("24.08.2026 11:55", "+2:09:37");
            assert!(
                adjusted.contains("24.08.2026 11:55") && adjusted.contains("+2:09:37"),
                "{name}"
            );
            assert!((t.camera_time_photos)(312).contains("312"), "{name}");
            let back = (t.restored)(6, 2, "IMG_1 (2).jpg");
            assert!(back.contains('6') && back.contains('2'), "{name}");
            assert!(
                (t.restored)(1, 0, "IMG_1.jpg").contains("IMG_1.jpg"),
                "{name}"
            );
            assert!(
                (t.restored)(1, 1, "IMG_1 (2).jpg").contains("IMG_1 (2).jpg"),
                "{name}"
            );
            let failed = (t.restore_failed)(3, "b.jpg", "busy");
            assert!(
                failed.contains('3') && failed.contains("b.jpg") && failed.contains("busy"),
                "{name}"
            );
            assert!((t.digital_zoom)(2.0).contains('2'), "{name}");
            assert!((t.button_language)(name).contains(name), "{name}");
            assert!((t.cmd_delete_rejected)(7).contains('7'), "{name}");
            for count in [t.bulk_copy, t.bulk_move, t.bulk_delete, t.photos_count] {
                assert!(count(12).contains("12"), "{name}");
            }
            assert!((t.photos_shown)(12, 340).contains("12"), "{name}");
            assert!((t.top_photos)(50).contains("50"), "{name}");
            assert!((t.photos_shown)(12, 340).contains("340"), "{name}");
            assert!((t.deleting)(3).contains(".originals"), "{name}");
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
            let both = (t.download_text)(true, true, "2.9 GB");
            assert!(
                both.contains("2.9 GB") && both.contains("CLIP") && both.contains("V2.5"),
                "{name}"
            );
            let v25 = (t.download_text)(false, true, "1.7 GB");
            assert!(v25.contains("1.7 GB") && !v25.contains("CLIP"), "{name}");
            for with_size in [
                t.enable_aesthetics_tooltip,
                t.add_v25_tooltip,
                t.aesthetics_offer,
                t.v25_offer,
                t.confirm_delete_models_text,
            ] {
                assert!(with_size("1.7 GB").contains("1.7 GB"), "{name}");
            }
            assert!((t.download_failed)("timeout").contains("timeout"), "{name}");
            let learning = (t.note_learning)(3, 15);
            assert!(learning.contains('3') && learning.contains("15"), "{name}");
            assert!((t.note_faces_too_small)(2).contains('2'), "{name}");
            assert!((t.model_downloading)(42.0).contains("42"), "{name}");
            let trained = (t.taste_trained)(30, 0.7);
            assert!(trained.contains("30") && trained.contains("0.7"), "{name}");
            assert!((t.taste_photos)(30).contains("30"), "{name}");
            let sources = (t.taste_sources)(167, 2, 542);
            assert!(
                ["167", " 2 ", "542"].iter().all(|n| sources.contains(n)),
                "{name}"
            );
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
                .chain(&t.help_cull)
                .chain(&t.help_video)
                .chain(&t.help_view)
                .chain(&t.help_panels)
                .chain(&t.help_edit)
                .chain(&t.help_more)
                .chain(&t.welcome_keys);
            for (keys, action) in rows {
                assert!(!keys.trim().is_empty() && !action.trim().is_empty());
            }
            assert!(t.help_sections.iter().all(|title| !title.is_empty()));
            for (title, tips) in &t.help_tips {
                assert!(!title.is_empty() && !tips.is_empty());
                assert!(tips.iter().all(|tip| !tip.trim().is_empty()));
            }
            assert!(!t.help_tab_keys.is_empty() && !t.help_tab_tips.is_empty());
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
    fn sizes_take_the_unit_of_the_total() {
        let (en, fr) = (Lang::En.texts(), Lang::Fr.texts());
        assert_eq!(format_sizes(en, None, 4_100_000_000), "4.1 GB");
        assert_eq!(format_sizes(fr, None, 4_100_000_000), "4.1 Go");
        assert_eq!(format_sizes(en, None, 850_000_000), "850 MB");
        assert_eq!(format_sizes(en, None, 3_400_000), "3.4 MB");
        assert_eq!(format_sizes(en, None, 12_000), "12 KB");
        assert_eq!(
            format_sizes(en, Some(1_200_000_000), 8_400_000_000),
            "1.2 / 8.4 GB"
        );
        assert_eq!(format_sizes(en, Some(0), 0), "0 / 0 KB");
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
