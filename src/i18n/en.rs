use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::Iso,
    compass: ["N", "S", "E", "W"],
    size_units: ["KB", "MB", "GB"],
    key_ctrl: "Ctrl",
    key_shift: "Shift",
    key_delete: "Del",

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
    filter_people: "With people",
    filter_no_people: "Without people",
    filter_people_tooltip: "Found by their faces: people seen from behind or very small in the picture don't count.",
    filter_deleted: "Deleted",
    filter_deleted_tooltip: "Photos deleted in this folder. They lie in the hidden .originals folder – Ctrl+Z puts one back.",
    filter_deleted_none: "No deleted photos in this folder",
    filter_hide_rejected: "Hide rejected",
    filter_without: "without",
    filter_hide_rejected_tooltip: "The rejected photos go, every other one stays. × shows only the rejected ones instead.",
    filter_clear: "Show all",
    media_all: "Photos and videos",
    media_photos: "Photos only",
    media_videos: "Videos only",
    media_no_videos: "This folder has no videos",
    top_photos: |n| format!("Top {n} photos"),
    top_purposes: [
        "Highlights",
        "Preview",
        "Slideshow",
        "Photo book",
        "Gallery",
    ],
    top_tooltip: "Photos, videos or both – or only the best photos, by your stars (else the prediction), aesthetics and sharpness, the best of each series first. Rejected and blurry photos and duplicates don't count. The choice stays until a filter changes or you choose Refresh order. Nothing in the photos changes.",
    menu_top: "Best photos",
    bulk_delete_top: "Not with Top – these are the best photos",
    filter_none_active: "No filter is on",
    filter_similar: "≈ Similar",
    filter_similar_to: |name| format!("≈ like {name}"),
    menu_similar: "Similar photos",
    menu_similar_to: |name| format!("Similar to {name}"),
    similar_tooltip: |percent| {
        format!("Only photos that look like the current one (from {percent:.0} %) – key M")
    },
    similar_fact: |percent| format!("Similar {percent:.0} %"),
    similar_needs_model: "Similar photos need the aesthetics model (CLIP)",
    similar_not_analysed: "This photo has not been analysed yet",
    similar_none: |percent| format!("No similar photos (from {percent:.0} %)"),
    refresh_order: "Refresh order",
    refresh_order_tooltip: "New scores arrived since sorting and filtering",
    analyzing_progress: |done, total| format!("Analyzing {done} / {total}"),
    enable_aesthetics: "Enable aesthetics…",
    enable_aesthetics_tooltip: |size| {
        format!("Downloads the image models for aesthetics once ({size})")
    },
    add_v25: "Load V2.5…",
    add_v25_tooltip: |size| {
        format!(
            "Downloads the second aesthetics model once (SigLIP + V2.5, {size}) – aesthetics is then the mean of both models"
        )
    },
    downloading_model: |percent| format!("Downloading model {percent:.0} %"),
    aesthetics_loading: "Aesthetics: loading model…",
    aesthetics_failed: "Aesthetics: failed",

    sort_name: "Name",
    sort_rating: "Rating",
    sort_aesthetics: "Aesthetics",
    sort_personal: "Prediction",
    sort_sharpness: "Sharpness",
    sort_taken: "Capture time",
    filter_unrated: "Unrated",

    filter_rejected: "Rejected",
    actions: "Action",
    actions_tooltip: "Copy, move or delete the photos on screen",
    selection_delete: "Delete",
    bulk_copy: |n| {
        format!(
            "Copy to … ({n} {})",
            if n == 1 { "photo" } else { "photos" }
        )
    },
    bulk_move: |n| {
        format!(
            "Move to … ({n} {})",
            if n == 1 { "photo" } else { "photos" }
        )
    },
    bulk_delete: |n| format!("Delete ({n} {})", if n == 1 { "photo" } else { "photos" }),
    bulk_delete_hint: "Every photo the filter shows now. After 5 seconds they move into the hidden .originals folder beside them – nothing is deleted for good, Esc brings them back.",
    delete_rejected_hint: "Every rejected photo in the folder, also those the filter hides now. After 5 seconds they move into the hidden .originals folder – nothing is deleted for good.",
    bulk_restore: |n| format!("Put back ({n} {})", if n == 1 { "photo" } else { "photos" }),
    bulk_restore_hint: "Every deleted photo the filter shows goes back into its folder. When its name is taken there it gets a number – nothing is overwritten.",
    photos_shown: |shown, total| format!("{shown} of {total} photos"),
    photos_count: |n| format!("{n} {}", if n == 1 { "photo" } else { "photos" }),
    photos_badge_tooltip: "How many photos the filter shows now – Action works on exactly these.",
    label_red: "Red",
    label_yellow: "Yellow",
    label_green: "Green",
    label_blue: "Blue",
    label_purple: "Purple",
    meter_aesthetics_tooltip: "Aesthetics: the mean of LAION and V2.5 on a fixed scale – a photo reads the same in every folder. Sharpness instead compares with the other photos in the folder.\nSingle scores: details panel (Tab)",
    meter_sharpness: "Sharpness",
    meter_eyes: "Eyes",
    probably_blurry: "probably blurry",
    analyzing: "Analyzing…",
    saving: "Saving…",
    auto_advance_on: "Auto advance",
    series_position: |index, len| format!("Series {index} / {len}"),
    duplicate_of: |name| format!("Duplicate of {name}"),
    rejected: "Rejected",
    filmstrip_video: "Video",
    star_tooltip: |n| format!("{n} ★ – key {n}"),
    personal_hint: |stars| {
        format!(
            "Prediction: {stars:.1} ★ – the stars Cerno thinks you would give. Not your rating yet"
        )
    },
    zoom: |percent| format!("Zoom {percent:.0} %"),
    zoom_preview: |percent| format!("Zoom {percent:.0} % of the preview"),
    digital_zoom: |ratio| format!("{ratio:.1}× digital zoom"),
    button_toolbar: "Filter bar",
    button_details: "Details panel",
    button_filmstrip: "Filmstrip",
    button_help: "Help",
    button_menu: "Menu",
    button_language: |name| format!("Language: {name}"),

    cmd_all_panels: "Filter bar, details and filmstrip",
    cmd_fullscreen: "Full screen",
    cmd_compare: "Compare",
    cmd_similar: "Show only similar photos",
    cmd_zoom: "Whole photo ↔ 100 %",
    menu_overlay: "Overlay",
    overlay_off: "Off",
    overlay_sharpness: "Sharp edges",
    overlay_exposure: "Clipped highlights and shadows",
    overlay_fact_sharpness: "Overlay: sharpness",
    overlay_fact_exposure: "Overlay: exposure",
    overlay_hint_sharpness: "Sharpness: purple marks the photo's sharpest edges",
    overlay_hint_exposure: "Exposure: red = blown out, blue = crushed black",
    overlay_hint_off: "Overlay off",
    overlay_show_on_photo: "Show on the photo (O)",
    cmd_grid: "Grid",
    cmd_reject: "Reject",
    cmd_description: "Comment and keywords",
    cmd_delete_rejected: |n| {
        format!(
            "Delete rejected ({n} {})",
            if n == 1 { "photo" } else { "photos" }
        )
    },
    cmd_auto_advance: "Auto advance",
    cmd_subfolders: "Include subfolders",
    subfolders_on: "Subfolders included",
    subfolders_off: "This folder only, without subfolders",
    menu_sort: "Sort",
    menu_filter: "Filter",
    menu_view: "View",
    menu_labels: "Colour labels",
    menu_external: "Edit elsewhere",
    external_other: "Other program …",
    external_chooser: "System “Open with” …",
    external_default: "Open with the default program",
    external_pick_title: "Choose a program to edit with",
    external_opened: |name| {
        format!(
            "Opened in {name} – once saved there, Cerno shows the new version; the original stays in .originals"
        )
    },
    external_reloaded: |name| format!("{name} was saved elsewhere – reloaded"),
    external_failed: |err| format!("Not opened: {err}"),
    menu_language: "Language",
    menu_models: "Models & data",
    menu_this_photo: "This photo",
    menu_visible: "Photos on screen",
    menu_settings: "Settings",
    menu_stars: "Stars",
    label_none: "No colour",
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
            "Moving 1 photo into the hidden .originals folder   ·   Esc to undo".to_owned()
        } else {
            format!("Moving {n} photos into the hidden .originals folder   ·   Esc to undo")
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
    transfer_progress: |moved, at, total, sizes, file| {
        let verb = if moved { "Moving" } else { "Copying" };
        let mut text = format!("{verb} {at} / {total} photos – {sizes}");
        if !file.is_empty() {
            text.push_str(&format!(" – {file}"));
        }
        text
    },
    download_title: "Enable aesthetics scoring",
    download_text: |clip, v25, size| {
        let mut text = String::from("Aesthetics scoring is missing:\n");
        if clip {
            text.push_str("\n• CLIP ViT-L/14 – from Hugging Face (Xenova/clip-vit-large-patch14)");
        }
        if v25 {
            text.push_str("\n• SigLIP + Aesthetic Predictor V2.5 – from Cerno's GitHub release models-1 (the V2.5 part is AGPL-3.0)");
        }
        text.push_str(&format!(
            "\n\nDownload now ({size})? The files go into Cerno's data folder and are downloaded only once; an interrupted download continues next time."
        ));
        text
    },
    btn_download: "Download",
    btn_cancel: "Cancel",
    btn_close: "Close (Esc)",
    aesthetics_offer: |size| {
        format!("Aesthetics scoring needs image models ({size}): Menu → Models & data.")
    },
    v25_offer: |size| {
        format!(
            "More reliable aesthetics with the second model V2.5 ({size}): Menu → Models & data."
        )
    },

    section_aesthetics: "Aesthetics",
    section_sharpness: "Sharpness (within folder)",
    section_exposure: "Exposure",
    section_attributes: "CLIP attributes",
    row_aesthetics: "Aesthetics (mean)",
    section_histogram: "Histogram",
    section_file: "File",
    row_size: "Size",
    row_load_time: "Load time",
    row_file_size: "File size",
    row_jpeg_quality: "JPEG quality",
    explain_jpeg_quality: "Estimated from the file's quantisation tables – the quality itself is not stored. 4:2:0 means colour at half resolution (usual for cameras), 4:4:4 at full.",
    explain_raw_preview: "For RAW files Cerno shows the JPEG the camera embedded – no RAW development. Colour and exposure are the camera's, and 100 % is this preview's size, not the sensor's. The histogram and the exposure measure the preview.",
    row_container: "Container",
    row_duration: "Duration",
    row_video: "Video",
    row_frame_rate: "Frame rate",
    row_video_bitrate: "Video bitrate",
    row_audio: "Audio",
    row_audio_bitrate: "Audio bitrate",
    row_bitrate: "Total bitrate",
    no_audio: "no audio track",
    variable_frame_rate: "variable",
    channels: |n| match n {
        1 => "Mono".to_owned(),
        2 => "Stereo".to_owned(),
        6 => "5.1".to_owned(),
        8 => "7.1".to_owned(),
        n => format!("{n} channels"),
    },
    row_location: "Location",
    tab_values: "Values",
    tab_description: "Description",
    tab_faces: "Faces",
    section_comment: "Comment",
    section_keywords: "Keywords",
    comment_hint: "Write a comment …",
    keyword_hint: "Add a keyword …",
    keyword_remove: "Remove",
    description_note: "Enter adds a keyword, Esc leaves the field. Comment and keywords are written into the file (IPTC and XMP), where Windows, Lightroom and digiKam read them.",
    description_waiting: "Reading …",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "Prediction",
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
    explain_laion: "How beautiful the photo looks to an AI trained on many human ratings. It likes people, portraits and food most.",
    explain_v25: "A newer AI for the same question, better with everyday photos. It likes landscapes, water and aerial shots most.",
    explain_personal: "The stars Cerno thinks you would give. It learns from your own stars and the photos you reject or delete.",
    explain_aesthetics: "The value under the photo: the mean of the two AIs below, from 0 % (unappealing) to 100 % (very appealing). Most photos land at 40–60 %, from 80 % on it is very good. The scale is fixed – a photo reads the same in every folder.",
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
    explain_models: "Where each AI runs: DirectML = graphics card, CPU = processor. ± is how far off the prediction typically is.",
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
    model_removing: "removing…",
    model_failed: "failed",
    model_faces: "Faces",
    model_personal: "Prediction",
    taste_trained: |n, error| format!("{n} photos, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} photos"),
    taste_untrained: "not trained yet",
    taste_sources: |stars, rejected, deleted| {
        format!(
            "Learned from {stars} photos with stars, {rejected} rejected and {deleted} deleted ones – rejected and deleted photos count as 0 ★."
        )
    },
    btn_reset_taste: "Reset prediction",
    btn_delete_models: "Delete models",
    models_deleted: "Models deleted",
    models_downloaded: "Models downloaded – aesthetics are being added now",
    download_failed: |err| {
        format!("Download failed: {err} – trying again continues where it stopped")
    },
    copy_models_path: "Copy path",
    models_path_copied: "Path copied",
    confirm_reset_taste_title: "Reset the prediction?",
    confirm_reset_taste_text: "Cerno will forget what it learned from your stars and deletions. Star ratings in the photo files stay unchanged.",
    confirm_delete_models_title: "Delete downloaded models?",
    confirm_delete_models_text: |size| {
        format!(
            "Removes the downloaded model files from disk ({size}). Saved scores stay in the database; the models can be downloaded again later."
        )
    },

    cmd_straighten: "Straighten",
    cmd_rotate_ccw: "Rotate 90° counter-clockwise",
    cmd_rotate_cw: "Rotate 90° clockwise",
    cmd_crop: "Crop",
    cmd_undo: "Undo",
    edit_not_jpeg: "Straighten, crop, turns and Ctrl+Z work on JPEG only.",
    video_play_hint: "Play (Space)",
    video_play_pause: "Play / pause (Space)",
    video_mute: "Sound on / off",
    video_volume: "Volume (↑ ↓)",
    video_no_zoom: "Videos are not zoomed",
    video_no_compare: "Videos can't be compared",
    video_no_sound: "No sound: Cerno found no audio output – the video plays silently",
    video_no_ffmpeg: "No preview frame: Cerno needs ffmpeg for it (e.g. winget install Gyan.FFmpeg)",
    video_play_failed: |err| format!("Cannot play the video: {err}"),
    edit_writing: "Writing the photo…",
    edit_cancelled: "Another photo is shown – the edit was cancelled",
    busy_editing: "The edit is still open – Enter applies, Esc cancels",
    busy_copying: "This photo is being copied – possible again in a moment",
    busy_moving: "This photo is being moved",
    busy_deleted: "Deleted photo – put it back first (Ctrl+Z)",
    edit_needs_index: "Editing needs the index, which could not be opened",
    edit_reencoded: "JPEG re-encoded – Ctrl+Z brings the original back.",
    undo_done: "Original restored",
    undo_nothing: "No original kept for this photo",
    cmd_restore: "Put back",
    restored: |n, renamed, name| match (n, renamed) {
        (1, 0) => format!("Put back: {name}"),
        (1, _) => format!("Put back as {name} – the name was taken"),
        (n, 0) => format!("{n} photos put back"),
        (n, r) => format!("{n} photos put back, {r} of them under a new name"),
    },
    restore_failed: |n, name, err| format!("Could not put back {n} photo(s) – {name}: {err}"),
    deleted_mark: "Deleted",
    faces_loading: "Looking for faces…",
    faces_unknown: "Not analysed yet – the faces follow.",
    faces_none: "No faces found",
    faces_only_small: "Only small faces – too small to judge",
    face_eyes_blurry: "Eyes probably blurry",
    faces_zoom_hint: "Click to zoom to this face",
    faces_grid_hint: "A click or its number (1–9) zooms to a face · Esc closes",
    cmd_faces: "Faces",
    cmd_face_grid: "All faces large",
    faces_small: |n| {
        format!(
            "+ {n} small {} – too small to judge",
            if n == 1 { "face" } else { "faces" }
        )
    },
    face_number: |n| format!("Face {n}"),
    raw_preview_fact: "RAW preview",
    preview_word: "preview",
    edit_failed: |detail| format!("Not written: {detail}"),
    edit_hint_straighten: "Wheel or ←/→ rotates, Shift is finer · Enter applies, Esc cancels",
    edit_hint_crop: "Arrows move · +/− size, Shift finer · A: ratio · X: landscape/portrait · Enter applies, Esc cancels",
    ratio_original: "Original",
    crop_landscape: "Landscape",
    crop_portrait: "Portrait",

    help_title: "Help",
    help_intro: "Cerno shows your photos instantly and helps you sort them out. Stars and colour labels go into the photo file with its date untouched; everything else stays in Cerno's own database.",
    help_drop: "Drop a folder or photo onto the window, or press Ctrl+O.",
    help_close: "Esc, H or F1 closes this page",
    help_tab_keys: "Shortcuts",
    help_tab_tips: "Tips",
    help_pages_hint: "←/→ switches the page",
    help_tips: [
        (
            "Sort out in two passes",
            &[
                "First browse quickly (Space) and reject what failed with X – don't think twice.",
                "Then tick “without ×” in the filter bar: the rejected ones are gone, now rate with 1–5.",
                "Finally Action › “Delete rejected” (Ctrl+M).",
            ],
        ),
        (
            "Series and comparing",
            &[
                "Sorted by capture time, series stay together, the sharpest photo first.",
                "C shows two photos side by side; A keeps the left one, D the right one, the other is rejected.",
                "M shows only photos like the current one.",
            ],
        ),
        (
            "The best photos",
            &[
                "Choose “Top 50 photos” in the filter bar's first box: Cerno suggests the best, one from each series first.",
                "Aesthetics and sharpness under the photo help you decide – the stars are yours to give.",
            ],
        ),
        (
            "Colour labels",
            &[
                "Colours have no fixed meaning in Cerno. Two common readings:",
                "Progress: red to check · yellow to edit · green done · blue exported",
                "Use: red client · yellow social media · green portfolio · blue print",
                "6–9 set red to blue; the filter bar shows one colour.",
            ],
        ),
        (
            "Deleted is not gone",
            &[
                "Deleted photos move into the hidden .originals folder beside the photos.",
                "The bin box in the filter bar shows them; Ctrl+Z puts one back.",
                "Before the first straighten, crop or turn Cerno keeps the original – Ctrl+Z brings it back.",
            ],
        ),
        (
            "The prediction",
            &[
                "Cerno learns from your stars and your rejected and deleted photos what you like.",
                "On photos without stars it shows its guess as lightly filled stars – the stars are still yours to give.",
                "Sorted by prediction, the photos you will probably like come first.",
            ],
        ),
    ],
    welcome_intro: "View, rate and cull photos without waiting – stars go into the file, its date stays.",
    welcome_keys: [
        ("←, →", "Previous / next photo"),
        ("1 – 5", "Give stars"),
        ("X", "Reject"),
        ("Del", "Delete – Esc brings it back"),
        ("Ctrl+K", "Menu with every function"),
    ],
    welcome_more: "All shortcuts: H",
    help_sections: [
        "Browse", "Rate", "Sort out", "Video", "View", "Panels", "Edit", "More",
    ],
    help_browse: [
        ("→, Space, PgDn", "Next photo (hold to run through)"),
        ("←, Backspace, PgUp", "Previous photo"),
        ("Home, End", "First / last photo"),
        (
            "Mouse wheel",
            "Over the filmstrip: scroll through the photos",
        ),
        ("Ctrl+O", "Open a folder (or drop it onto the window)"),
        ("Ctrl+U", "Include subfolders – on / off"),
    ],
    help_rate: [
        ("1 – 5", "Give stars – written into the photo file"),
        ("Shift+1 – 5", "Give stars and go to the next photo"),
        ("0", "Remove stars or rejection"),
        (
            "X, Shift+X",
            "Reject – marked in the file, nothing is deleted; with Shift go on to the next photo",
        ),
        (
            "6 – 9",
            "Colour label: red, yellow, green, blue – press again to remove",
        ),
        ("Shift+6 – 9", "Set that colour and go to the next photo"),
        ("B", "Description: edit the comment and keywords"),
    ],
    help_cull: [
        (
            "C",
            "Compare: pin this photo on the left, browse on the right",
        ),
        (
            "A, D",
            "Compare: keep left / keep right – the other one is rejected, compare mode ends",
        ),
        ("M", "Show only similar photos – again: all of them"),
        (
            "Del",
            "Delete: moves into the hidden .originals folder after 5 seconds – nothing is lost",
        ),
        ("Esc", "Bring back photos that are waiting to be deleted"),
    ],
    help_video: [
        ("Space", "Play / pause (Shift+Space: next photo)"),
        ("Alt+←, Alt+→", "5 s back / on"),
        (",, .", "One frame back / on (paused)"),
        ("↑, ↓", "Volume"),
    ],
    help_view: [
        ("Z, Double-click", "Whole photo ↔ 100 %"),
        ("Ctrl+0, Ctrl+1", "Whole photo / 100 %"),
        ("+, −, Mouse wheel", "Zoom in / out – with Ctrl too"),
        ("Drag", "Move the zoomed photo"),
        (
            "O",
            "Overlay: sharp edges → clipped highlights and shadows → off",
        ),
        (
            "F7",
            "Grid of all photos: ↑ ↓ a row, + − size, Enter opens the photo",
        ),
        ("F, F11", "Full screen"),
    ],
    help_panels: [
        (
            "T",
            "Filter bar: sort and filter – by stars, colours, sharpness, people",
        ),
        ("Tab", "Details panel"),
        ("F6", "Filmstrip"),
        ("Shift+Tab", "Filter bar, details and filmstrip together"),
        (
            "G, Shift+G",
            "Faces: in the details panel (G) or all of them large (Shift+G) – a click zooms there",
        ),
    ],
    help_edit: [
        (
            "S",
            "Straighten: grid, wheel and arrows rotate, Shift is finer",
        ),
        (
            "R",
            "Crop: draw a frame or set it with the arrows and +/−, A changes the ratio, X flips landscape/portrait",
        ),
        ("Enter, Esc", "Apply or cancel straighten and crop"),
        (
            "Ctrl+←, Ctrl+→",
            "Rotate 90° – lossless, via the JPEG orientation",
        ),
        (
            "Ctrl+Z",
            "Bring back the original – it stays in .originals beside the photo; put a deleted photo back",
        ),
        (
            "E",
            "Edit in another program – the remembered one, or choose one",
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
        ("Esc", "Step back: zoom, compare mode, grid, full screen"),
    ],
};
