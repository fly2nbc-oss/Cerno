use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::Iso,
    compass: ["N", "S", "E", "W"],
    key_ctrl: "Ctrl",
    key_shift: "Shift",

    open: "Open…",
    open_tooltip: "Open folder (Ctrl+O)",
    sort: |key| format!("Sort: {key}"),
    filter_summary: |list| format!("Filter: {list}"),
    filter_stars: |n| {
        if n == 1 {
            "1 star".to_owned()
        } else {
            format!("{n} stars")
        }
    },
    filter_blurry: "Blurry",
    filter_blurry_tooltip: "The blurriest 20 % of this folder",
    filter_duplicate: "Duplicates",
    filter_duplicate_tooltip: "Every photo except the first identical path",
    filter_clear: "Show all",
    refresh_order: "Refresh order",
    refresh_order_tooltip: "New scores arrived since sorting and filtering",
    analyzing_progress: |done, total| format!("Analyzing {done} / {total}"),
    analyzed: |total| format!("{total} analyzed"),
    enable_aesthetics: "Enable aesthetics…",
    enable_aesthetics_tooltip: "Downloads the CLIP image model (1.2 GB) once",
    downloading_model: |percent| format!("Downloading model {percent:.0} %"),
    aesthetics_ready: "Aesthetics: ready",
    aesthetics_loading: "Aesthetics: loading model…",
    aesthetics_backend: |backend| format!("Aesthetics: {backend}"),
    aesthetics_backend_tooltip: "Where the aesthetics model runs (DirectML = graphics card)",
    aesthetics_failed: "Aesthetics: failed",

    sort_name: "Name",
    sort_rating: "Rating",
    sort_laion: "Aesthetics (LAION)",
    sort_v25: "Aesthetics (V2.5)",
    sort_personal: "For you",
    sort_sharpness: "Sharpness",
    sort_taken: "Capture time",
    filter_all: "All",
    filter_unrated: "Unrated",

    filter_rejected: "Rejected",
    filter_label_all: "Any colour",
    label_filter: |name| format!("Colour: {name}"),
    label_red: "Red",
    label_yellow: "Yellow",
    label_green: "Green",
    label_blue: "Blue",
    label_purple: "Purple",
    meter_aesthetics: "Aesthetics",
    meter_aesthetics_tooltip: "L and V: aesthetics (LAION / V2.5). ★: For you – the stars Cerno thinks you would give. All on the scale 0–5\n– = not available yet",
    meter_sharpness: "Sharpness",
    meter_eyes: "Eyes",
    probably_blurry: "probably blurry",
    analyzing: "Analyzing…",
    saving: "Saving…",
    auto_advance_on: "Auto advance",
    series_position: |index, len| format!("Series {index} / {len}"),
    series_more: |n| format!("+{n}"),
    duplicate_of: |name| format!("Duplicate of {name}"),
    rejected: "Rejected",
    star_tooltip: |n| format!("{n} ★ – key {n}"),
    zoom: |percent| format!("Zoom {percent:.0} %"),
    digital_zoom: |ratio| format!("{ratio:.1}× digital zoom"),
    map_tooltip: |place| format!("{place}\nClick to open in Google Maps"),
    button_toolbar: "Sort and filter",
    button_details: "Details panel",
    button_filmstrip: "Filmstrip",
    button_help: "Help",
    button_menu: "Menu",
    button_language: |name| format!("Language: {name}"),

    cmd_explanations: "Expand or collapse all explanations in the details panel",
    cmd_all_panels: "Sort and filter, details and filmstrip",
    cmd_fullscreen: "Full screen",
    cmd_compare: "Compare",
    cmd_zoom: "Whole photo ↔ 100 %",
    cmd_first: "First photo",
    cmd_last: "Last photo",
    cmd_language: |name| format!("Language: {name}"),
    cmd_reject: "Reject",
    cmd_delete_rejected: |n| format!("Delete rejected photos ({n})"),
    cmd_auto_advance: "Auto advance",
    cmd_subfolders: "Include subfolders",
    cmd_best_of_series: "Best of each series",
    cmd_label: |name| format!("Colour: {name}"),
    menu_sort: "Sort",
    menu_filter: "Filter",
    menu_view: "View",
    menu_labels: "Colour labels",
    menu_language: "Language",
    loading: "Loading…",
    cannot_show: "Cannot show this image",
    no_match: "No photos match the filter",
    drop_to_open: "Drop to open",
    compare_left: "Left",
    compare_right: "Right",
    compare_left_badge: "L",
    keeps_this: |key| format!("{key} keeps this"),
    compare_needs_two: "Comparing needs at least two photos",
    deleting: |n| {
        if n == 1 {
            "Deleting 1 photo   ·   Esc to undo".to_owned()
        } else {
            format!("Deleting {n} photos   ·   Esc to undo")
        }
    },
    delete_failed: |n, name, err| format!("Could not delete {n} photo(s) – {name}: {err}"),
    blurry_tooltip: |eyes, percent| {
        let what = if eyes { "Eyes" } else { "Photo" };
        format!("Probably blurry: {what} sharper than only {percent:.0} % of this folder")
    },

    db_unavailable: |err| format!("Scores are not saved this session: {err}"),
    cannot_open: |path, err| format!("Cannot open {path}: {err}"),
    no_photos_in: |dir| format!("No JPEG or HEIC files in {dir}"),
    rating_not_saved: |err| format!("Rating not saved – {err}"),
    open_folder: "Open folder",
    transfer_menu: "Copy or move everything the current filter shows to …",
    transfer_copy: "Copy",
    transfer_move: "Move",
    transfer_copy_cmd: "Copy everything the current filter shows to …",
    transfer_move_cmd: "Move everything the current filter shows to …",
    transfer_same_folder: "That is already the open folder",
    transfer_busy: "A copy or move is already running",
    transfer_done: |moved, done, skipped, name, err| {
        let verb = if moved { "moved" } else { "copied" };
        let mut text = format!("{done} {verb}, {skipped} skipped");
        if !name.is_empty() {
            text.push_str(&format!(" – {name}: {err}"));
        }
        text
    },
    download_title: "Enable aesthetics scoring",
    download_text: |gb| {
        format!(
            "Cerno needs the CLIP ViT-L/14 image model to score aesthetics.\n\n\
             Download it now from Hugging Face (Xenova/clip-vit-large-patch14, {gb:.1} GB)? \
             It is stored in Cerno's data folder and only downloaded once."
        )
    },

    section_aesthetics: "Aesthetics",
    section_sharpness: "Sharpness (within folder)",
    section_exposure: "Exposure",
    section_attributes: "CLIP attributes",
    section_models: "Models",
    section_histogram: "Histogram",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "For you",
    row_frame: "Whole frame",
    row_eyes: "Eyes",
    row_highlights: "Blown highlights",
    row_shadows: "Crushed shadows",
    attributes: [
        "Overall quality",
        "Sharp",
        "Good lighting",
        "Well composed",
        "Low noise",
        "Colorful",
    ],
    explain_laion: "How beautiful the photo looks to an AI trained on many human ratings, converted to stars 0–5. Most photos get 2–3, from 4 on it is very good.",
    explain_v25: "A newer AI for the same question, better with everyday photos. Same star scale.",
    explain_personal: "The stars Cerno thinks you would give. It learns from your own stars and the photos you delete.",
    explain_frame: "How sharp the sharpest parts are, compared with the other photos in this folder. 80 % means sharper than 80 % of them.",
    explain_eyes: "Sharpness right at the eyes, if there is a face. For portraits this counts, not the background.",
    explain_highlights: "Parts that are pure white, without any detail left. More than 1 % is worth a look.",
    explain_shadows: "Parts that are pure black, without any detail left. Often intended; more than 5 % is marked.",
    explain_attributes: "The AI compares the photo with two opposite descriptions. 50 % means undecided, near 100 % clearly the first one.",
    explain_attribute: [
        "good photo – bad photo",
        "sharp – blurry",
        "good light – bad light",
        "well composed – badly composed",
        "clean – noisy",
        "colourful – dull",
    ],
    explain_models: "Where each AI runs: DirectML = graphics card, CPU = processor. ± is how far off For you typically is.",
    note_no_embedding: "no CLIP embedding yet",
    note_learning: |n, of| format!("learning – {n} of {of} photos"),
    note_analysing: "analysing…",
    note_no_face: "no face",
    note_faces_too_small: |n| {
        if n == 1 {
            "1 face, too small".to_owned()
        } else {
            format!("{n} faces, too small")
        }
    },
    note_needs_clip: "needs the CLIP model",
    model_missing: "not installed",
    model_downloading: |percent| format!("downloading {percent:.0} %"),
    model_ready: "ready",
    model_loading: "loading…",
    model_failed: "failed",
    model_faces: "Faces",
    model_personal: "For you",
    taste_trained: |n, error| format!("{n} photos, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} photos"),
    taste_untrained: "not trained yet",
    btn_reset_taste: "Reset For you",
    btn_delete_models: "Delete models",
    copy_models_path: "Copy path",
    models_path_copied: "Path copied",
    confirm_reset_taste_title: "Reset For you?",
    confirm_reset_taste_text: "Cerno will forget what it learned from your stars and deletions. Star ratings in the photo files stay unchanged.",
    confirm_delete_models_title: "Delete downloaded models?",
    confirm_delete_models_text: "Removes the CLIP and SigLIP model files from disk (about 3 GB). Saved scores stay in the database; aesthetics can be downloaded again later.",

    cmd_straighten: "Straighten",
    cmd_rotate_ccw: "Rotate 90° counter-clockwise",
    cmd_rotate_cw: "Rotate 90° clockwise",
    cmd_crop: "Crop",
    edit_not_jpeg: "Straighten and crop work on JPEG only.",
    edit_writing: "Writing the photo…",
    edit_reencoded: "JPEG re-encoded. Quality was set once more.",
    edit_failed: |detail| format!("Not written: {detail}"),
    edit_hint: "Enter applies, Esc cancels",
    ratio_original: "Original",
    crop_landscape: "Landscape",
    crop_portrait: "Portrait",

    help_title: "Help",
    help_intro: "Cerno shows your photos instantly and helps you sort them out. Stars go straight into the photo file, so other programs see them too – the file date stays untouched. Everything else stays in Cerno's own database. Sharpness and beauty are rated automatically in the background; Tab opens the details panel, I expands or collapses all explanations, Ctrl+K opens the menu.",
    help_drop: "Drop a folder or photo onto the window, or press Ctrl+O.",
    help_close: "Esc, H or F1 closes this page",
    help_sections: ["Browse", "Rate and sort out", "View", "More"],
    help_browse: [
        ("→, Space, PgDn", "Next photo (hold to run through)"),
        ("←, Backspace, PgUp", "Previous photo"),
        ("Home, End", "First / last photo"),
        (
            "Mouse wheel",
            "Over the filmstrip: scroll through the photos",
        ),
        ("Ctrl+O", "Open a folder (or drop it onto the window)"),
    ],
    help_rate: [
        ("1 – 5", "Give stars – written into the photo file"),
        ("Shift+1 – 5", "Give stars and go to the next photo"),
        ("0", "Remove stars or rejection"),
        (
            "X, Shift+X",
            "Reject – marked in the file, nothing is deleted; with Shift go on to the next photo",
        ),
        ("Del", "Delete: goes to the trash after 5 seconds"),
        ("Esc", "Bring back photos that are waiting to be deleted"),
        (
            "C",
            "Compare: pin this photo on the left, browse on the right",
        ),
        (
            "A, D",
            "Compare: keep left / keep right – the other one is rejected",
        ),
        (
            "6 – 9",
            "Colour label: red, yellow, green, blue – press again to remove",
        ),
        ("Shift+6 – 9", "Set that colour and go to the next photo"),
    ],
    help_view: [
        ("Z, Double-click", "Whole photo ↔ 100 %"),
        ("+, −, Mouse wheel", "Zoom in / out"),
        ("Drag", "Move the zoomed photo"),
        ("F11", "Full screen"),
        ("F", "Sort and filter"),
        ("Tab", "Details panel"),
        ("I", "Details: expand or collapse all explanations"),
        ("F6", "Filmstrip"),
        (
            "Shift+Tab",
            "Sort and filter, details and filmstrip together",
        ),
        (
            "S",
            "Straighten: grid, wheel and arrows rotate, Shift is finer, Enter applies",
        ),
        (
            "Ctrl+←, Ctrl+→",
            "Rotate 90° – lossless, via the JPEG orientation",
        ),
        (
            "R",
            "Crop: draw a frame, X flips landscape/portrait, A changes the ratio",
        ),
        ("Enter, Esc", "Apply or cancel straighten and crop"),
    ],
    help_more: [
        ("Ctrl+K", "Menu: every function"),
        ("Ctrl+L", "Switch language"),
        ("H, F1, ?", "This help"),
        ("Esc", "Step back: zoom, compare mode, full screen"),
    ],
};
