use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::Iso,
    compass: ["N", "S", "E", "W"],
    key_ctrl: "Ctrl",
    key_shift: "Shift",

    sort: |key| format!("Sort: {key}"),
    filter_stars: |n| {
        if n == 1 {
            "1 star".to_owned()
        } else {
            format!("{n} stars")
        }
    },
    filter_blurry: "Blurry",
    filter_blurry_tooltip: "Among the blurriest 20 % of this folder and clearly soft",
    filter_duplicate: "Duplicates",
    filter_duplicate_tooltip: "Every photo except the first identical path",
    filter_clear: "Show all",
    refresh_order: "Refresh order",
    refresh_order_tooltip: "New scores arrived since sorting and filtering",
    analyzing_progress: |done, total| format!("Analyzing {done} / {total}"),
    enable_aesthetics: "Enable aesthetics…",
    enable_aesthetics_tooltip: "Downloads the CLIP image model (1.2 GB) once",
    downloading_model: |percent| format!("Downloading model {percent:.0} %"),
    aesthetics_loading: "Aesthetics: loading model…",
    aesthetics_failed: "Aesthetics: failed",

    sort_name: "Name",
    sort_rating: "Rating",
    sort_laion: "Aesthetics (LAION)",
    sort_v25: "Aesthetics (V2.5)",
    sort_personal: "For you",
    sort_sharpness: "Sharpness",
    sort_taken: "Capture time",
    filter_unrated: "Unrated",

    filter_rejected: "Rejected",
    actions: "Action",
    actions_tooltip: "Copy, move or delete the photos on screen",
    selection_delete: "Delete",
    label_red: "Red",
    label_yellow: "Yellow",
    label_green: "Green",
    label_blue: "Blue",
    label_purple: "Purple",
    meter_aesthetics_tooltip: "L and V: aesthetics (LAION / V2.5). Outline star: For you – the stars Cerno thinks you would give. All on the scale 0–5\n– = not available yet",
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
    button_toolbar: "Filter bar",
    button_details: "Details panel",
    button_filmstrip: "Filmstrip",
    button_help: "Help",
    button_menu: "Menu",
    button_language: |name| format!("Language: {name}"),

    cmd_explanations: "All explanations",
    cmd_all_panels: "Filter bar, details and filmstrip",
    cmd_fullscreen: "Full screen",
    cmd_compare: "Compare",
    cmd_zoom: "Whole photo ↔ 100 %",
    cmd_reject: "Reject",
    cmd_delete_rejected: |n| format!("Delete rejected photos ({n})"),
    cmd_auto_advance: "Auto advance",
    cmd_subfolders: "Include subfolders",
    cmd_best_of_series: "Best of each series",
    menu_sort: "Sort",
    menu_filter: "Filter",
    menu_view: "View",
    menu_edit: "Edit",
    menu_photo: "Photo",
    menu_labels: "Colour labels",
    menu_language: "Language",
    menu_models: "Models & data",
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
    btn_download: "Download",
    btn_cancel: "Cancel",
    btn_close: "Close (Esc)",
    aesthetics_offer: "Aesthetics scoring needs a model (1.2 GB): Menu → Models & data.",

    section_aesthetics: "Aesthetics",
    section_sharpness: "Sharpness (within folder)",
    section_exposure: "Exposure",
    section_attributes: "CLIP attributes",
    section_histogram: "Histogram",
    section_file: "File",
    row_size: "Size",
    row_load_time: "Load time",
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
    cmd_undo: "Undo",
    edit_not_jpeg: "Straighten and crop work on JPEG only.",
    edit_writing: "Writing the photo…",
    edit_cancelled: "Another photo is shown – the edit was cancelled",
    busy_editing: "The edit is still open – Enter applies, Esc cancels",
    busy_copying: "This photo is being copied – possible again in a moment",
    busy_moving: "This photo is being moved",
    edit_reencoded: "JPEG re-encoded – Ctrl+Z brings the original back.",
    undo_done: "Original restored",
    undo_nothing: "No original kept for this photo",
    edit_failed: |detail| format!("Not written: {detail}"),
    edit_hint_straighten: "Wheel or ←/→ rotates, Shift is finer · Enter applies, Esc cancels",
    edit_hint_crop: "A: ratio · X: landscape/portrait · Enter applies, Esc cancels",
    ratio_original: "Original",
    crop_landscape: "Landscape",
    crop_portrait: "Portrait",

    help_title: "Help",
    help_intro: "Cerno shows your photos instantly and helps you sort them out. Stars and colour labels go into the photo file with its date untouched; everything else stays in Cerno's own database.",
    help_drop: "Drop a folder or photo onto the window, or press Ctrl+O.",
    help_close: "Esc, H or F1 closes this page",
    welcome_intro: "View, rate and cull photos without waiting – stars go into the file, its date stays.",
    welcome_keys: [
        ("←, →", "Previous / next photo"),
        ("1 – 5", "Give stars"),
        ("X", "Reject"),
        ("Del", "Delete – Esc brings it back"),
        ("Ctrl+K", "Menu with every function"),
    ],
    welcome_more: "All shortcuts: H",
    help_sections: ["Browse", "Rate and sort out", "View", "Edit", "More"],
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
        ("F, F11", "Full screen"),
        ("T", "Filter bar: sort and filter"),
        ("Tab", "Details panel"),
        ("I", "Details: expand or collapse all explanations"),
        ("F6", "Filmstrip"),
        ("Shift+Tab", "Filter bar, details and filmstrip together"),
    ],
    help_edit: [
        (
            "S",
            "Straighten: grid, wheel and arrows rotate, Shift is finer",
        ),
        (
            "R",
            "Crop: draw a frame, A changes the ratio, X flips landscape/portrait",
        ),
        ("Enter, Esc", "Apply or cancel straighten and crop"),
        (
            "Ctrl+←, Ctrl+→",
            "Rotate 90° – lossless, via the JPEG orientation",
        ),
        (
            "Ctrl+Z",
            "Undo the last change – originals are kept for 30 days",
        ),
    ],
    help_more: [
        ("Ctrl+K", "Menu: every function"),
        (
            "Ctrl+M",
            "Action: copy, move or delete the photos on screen",
        ),
        ("Ctrl+L", "Switch language"),
        ("H, F1, ?", "This help"),
        ("Esc", "Step back: zoom, compare mode, full screen"),
    ],
};
