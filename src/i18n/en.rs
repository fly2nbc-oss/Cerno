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
    filter_incomplete: "Incomplete",
    filter_incomplete_tooltip: "JPEGs whose file stops before the image ends – cut off while copying or downloading; the missing part shows grey",
    filter_people: "With people",
    filter_no_people: "Without people",
    filter_people_tooltip: "Found by their faces: people seen from behind or very small in the picture don't count.",
    filter_deleted: "Deleted",
    filter_deleted_tooltip: "Photos deleted in this folder. They lie in the hidden .originals folder – Ctrl+Z puts one back.",
    filter_deleted_none: "No deleted photos in this folder",
    filter_hide_rejected: "Hide rejected",
    filter_without: "without",
    filter_hide_rejected_tooltip: "The rejected photos go, every other one stays. × shows only the rejected ones instead.",
    menu_name_list: "By file list …",
    name_list_title: "Filter by file list",
    name_list_intro: "Paste the file names or numbers a client chose – one per line, or separated by commas or semicolons. Case and extension don't matter; a number finds the digits a name ends with.",
    name_list_hint: "IMG_0345, IMG_0351 …",
    name_list_missing: "Not found:",
    name_list_ambiguous: "In several folders (all taken):",
    name_list_apply: "Apply",
    name_list_chip_tooltip: "Only the photos of the pasted list – a click shows all again",
    name_list_found: |found, total| format!("{found} of {total} found"),
    name_list_chip: |found, total| format!("List {found}/{total}  ×"),
    cmd_align_camera: "Match right camera to left",
    align_camera_hint: "Moves the capture time of every photo from the right camera in this folder, so the right photo was taken at the same moment as the left one. Only in the index – the files keep their time.",
    align_needs_compare: "Compare two photos from different cameras first (C)",
    align_no_time: "One of the two photos has no camera or capture time",
    align_same_camera: "Both photos come from the same camera",
    menu_camera_time: "Camera time …",
    camera_time_title: "Camera time",
    camera_time_intro: "When a camera's clock was off, an offset moves the capture time of all its photos in this folder – for sorting by capture time, the series and the display. Only in the index; the files keep their time.",
    camera_time_reset: "Reset",
    camera_time_invalid: "Offset as +h:mm:ss or −h:mm:ss, e.g. +1:30:00",
    camera_time_apply: "Apply",
    camera_time_none: "No photos with a camera and capture time in this folder yet.",
    camera_time_applied: "Camera time applied",
    camera_aligned: |camera, offset| format!("{camera}: capture time moved by {offset}"),
    camera_time_photos: |n| {
        if n == 1 {
            "1 photo".into()
        } else {
            format!("{n} photos")
        }
    },
    camera_time_tooltip: |taken, offset| format!("Camera time {taken} – adjusted by {offset}"),
    cmd_pairs: "RAW+JPG as one photo",
    pairs_hint: "A RAW and a JPG of the same name appear as one photo – the JPG is shown. Stars, colour and description go into both files, copy, move and delete take both; edits change the JPG only.",
    cmd_update_check: "Check for updates",
    update_check_hint: "Once a day one request to GitHub – nothing about you or your photos",
    update_first_hint: "Cerno checks for updates once a day – turn it off under Settings › Check for updates.",
    update_available: |v| format!("Cerno {v} is available – Help › About Cerno"),
    cmd_update_download: |v| format!("Download Cerno {v} …"),
    pairs_on: "RAW+JPG appear as one photo",
    pairs_off: "RAW and JPG appear separately",
    pair_badge: "RAW+JPG",
    pair_edit_jpeg_only: "Changes the JPG only – the RAW stays untouched.",
    pair_raw_marks: |marks| format!("RAW: {marks}"),
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
    incomplete_fact: "File incomplete",
    incomplete_tooltip: "The file ends before the image does – the missing part shows grey. Check the original, for example on the memory card.",
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
    cmd_quad: "Four-up view",
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
    writer_stopped: "Marks are no longer written into the files – an internal error (crash.log in the data folder). Please restart Cerno.",
    internal_error: "Internal error – the command was stopped, Cerno goes on (details in crash.log in the data folder).",
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
    exiftool_offer: |size| {
        format!("Cerno saves stars and colours with ExifTool ({size}): Menu → Models & data.")
    },
    exiftool_title: "Download ExifTool",
    exiftool_text: |size| {
        format!(
            "Cerno writes stars, colour labels, comments and edits into the photos with ExifTool – Phil Harvey's free tool.\n\nDownload it now from the official source (SourceForge, {size})? It goes into Cerno's data folder; an interrupted download continues next time."
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
    row_complete: "Complete",
    incomplete_value: "no – the end is missing",
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
    explain_personal: "The stars Cerno thinks you would give. It learns from your own stars and the photos you reject.",
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
    exiftool_found: |version, downloaded| {
        let from = if downloaded {
            "downloaded by Cerno"
        } else {
            "installed"
        };
        if version.is_empty() {
            from.to_owned()
        } else {
            format!("{version} · {from}")
        }
    },
    exiftool_absent: "missing – no stars or colours without ExifTool",
    exiftool_old_state: |version| format!("{version} – too old (12.24 or newer needed)"),
    exiftool_downloading: |percent| format!("downloading … {percent:.0} %"),
    btn_exiftool: |size| format!("Download ExifTool ({size})"),
    taste_sources: |stars, rejected| {
        format!(
            "Learned from {stars} photos with stars and {rejected} rejected ones – rejected photos count as 0 ★, deleted ones not at all."
        )
    },
    btn_reset_taste: "Reset prediction",
    btn_delete_models: "Delete models",
    models_deleted: "Models deleted",
    models_downloaded: "Models downloaded – aesthetics are being added now",
    download_failed: |err| {
        format!("Download failed: {err} – trying again continues where it stopped")
    },
    exiftool_ready: "ExifTool is ready – stars and colours are saved now",
    exiftool_failed: |err| {
        format!("ExifTool could not be downloaded: {err} – trying again continues there")
    },
    copy_path: "Copy path",
    path_copied: "Path copied",
    confirm_reset_taste_title: "Reset the prediction?",
    confirm_reset_taste_text: "Cerno will forget what it learned from your stars and rejected photos. Star ratings in the photo files stay unchanged.",
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
    video_play_failed: |err| format!("Cannot play the video: {err}"),
    edit_writing: "Writing the photo…",
    edit_cancelled: "Another photo is shown – the edit was cancelled",
    busy_editing: "The edit is still open – Enter applies, Esc cancels",
    busy_copying: "This photo is being copied – possible again in a moment",
    busy_moving: "This photo is being moved",
    busy_deleted: "Deleted photo – put it back first (Ctrl+Z)",
    edit_needs_index: "Editing needs the index, which could not be opened",
    exiftool_missing: "Stars, colours and editing need ExifTool – Menu → Models & data",
    exiftool_too_old: "The installed ExifTool is too old (12.24 or newer needed) – Menu → Models & data",
    exiftool_loading: "ExifTool is downloading – stars and colours work in a moment",
    exiftool_install: |command| {
        match command {
        Some(command) => format!("Stars, colours and editing need ExifTool. Install it with: {command}"),
        None => "Stars, colours and editing need ExifTool. Install it with your package manager (package “exiftool” or “perl-image-exiftool”).".to_owned(),
    }
    },
    edit_reencoded: "JPEG re-encoded – Ctrl+Z brings the original back.",
    undo_done: "Original restored",
    undo_nothing: "No original kept for this photo",
    undo_mark_row: |what, name| format!("Undo {what} – {name}"),
    undo_what_stars: "stars",
    undo_what_reject: "rejection",
    undo_what_colour: "colour",
    undo_what_edit: "edit",
    undo_mark_done: |name, value| format!("Undone – {name}: {value}"),
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
    faces_zoom_hint: "Click to zoom to this face",
    faces_grid_hint: "A click or its number (1–9) zooms to a face · Esc closes",
    cmd_face_grid: "All faces large",
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
    help_tab_about: "About Cerno",
    about_intro: "Cerno shows photos without waiting and helps you sort them out – locally, without an account or a cloud. Free software: the source code is open.",
    about_version: "Version",
    about_license: "Licence",
    about_source: "Source code",
    about_bugs: "Report a problem",
    about_bugs_text: "Something doesn't work? Describe it in a GitHub issue – that needs a free GitHub account. Version and system are filled in already; photos and file names are not needed. If Cerno crashed, attach the file crash.log from the data folder.",
    about_bug_link: "Report it on GitHub",
    about_open_data: "Open data folder",
    about_wishes: "Ideas",
    about_wishes_text: "An idea that makes sorting photos with Cerno simpler or faster? Write it as a feature request – also with a GitHub account.",
    about_wish_link: "Suggest it on GitHub",
    about_third_party: "Third parties",
    about_third_party_text: "Cerno uses free libraries by others, among them libheif and GStreamer with FFmpeg (LGPL), and downloads ExifTool and the aesthetics models when asked (V2.5 under AGPL). Their licence texts come with the program.",
    about_third_party_link: "All third parties and licences",
    about_updates: "Updates",
    about_updates_text: "Once a day Cerno asks GitHub whether there is a newer version – nothing about you or your photos is sent, and nothing is downloaded or installed. Turn it off under Settings › Check for updates. Models and ExifTool are downloaded only when you ask.",
    update_off: "Checking for updates is off.",
    update_never: "Not checked yet.",
    update_checking: "Checking …",
    update_current: "Cerno is up to date.",
    update_failed: "GitHub could not be reached – try again later.",
    update_newer: |v| format!("Cerno {v} is available."),
    update_check_now: "Check now",
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
                "Two cameras whose clocks differ: compare two photos taken at the same moment, then This photo › “Match right camera to left”.",
                "Shift+C shows four photos at once: the frame is the current photo, ↑ ↓ move it a row.",
            ],
        ),
        (
            "The best photos",
            &[
                "Choose “Top 50 photos” in the filter bar's first box: Cerno suggests the best, one from each series first.",
                "Aesthetics and sharpness under the photo help you decide – the stars are yours to give.",
                "Filter › By file list …: paste the names a client chose – only those photos show.",
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
                "Cerno learns from your stars and your rejected photos what you like – deleted ones don't count: often one of many alike was just one too many.",
                "On photos without stars it shows its guess as lightly filled stars – the stars are still yours to give.",
                "Sorted by prediction, the photos you will probably like come first.",
            ],
        ),
        (
            "Good to know",
            &[
                "The mouse wheel over the filmstrip steps through the photos; over the photo it zooms.",
                "Drop a folder onto the window to open it. Hold → to run through the photos.",
                "In the grid (F7) ↑ ↓ move a row, + − change the size, Enter opens the photo.",
                "Straighten (S): the wheel and the arrows turn, Shift is finer. Crop (R): draw a frame or move it with the arrows, +/− size it, A changes the ratio, X turns it.",
                "On the description tab Enter puts the cursor into the keyword field.",
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
    setup_line: |exiftool, aesthetics| {
        format!(
            "Setup: ExifTool {} · aesthetics {} – Ctrl+K › Settings › Models & data",
            if exiftool { "ready" } else { "missing" },
            if aesthetics { "ready" } else { "missing" },
        )
    },
    help_sections: [
        "Browse", "Rate", "Sort out", "Video", "View", "Panels", "Edit", "More",
    ],
    help_browse: [
        ("→, Space, PgDn", "Next photo"),
        ("←, Backspace, PgUp", "Previous photo"),
        ("Home, End", "First / last photo"),
        ("Ctrl+O", "Open a folder"),
        ("Ctrl+U", "Subfolders on / off"),
    ],
    help_rate: [
        ("1 – 5", "Stars"),
        ("0", "No stars, not rejected"),
        ("X", "Reject"),
        ("6 – 9", "Red, yellow, green, blue"),
        ("Shift+…", "Same, then the next photo"),
        ("Ctrl+Z", "Undo"),
    ],
    help_cull: [
        ("C", "Compare two photos"),
        ("A, D", "Keep left / right"),
        ("Shift+C", "Four photos at once"),
        ("M", "Only similar photos"),
        ("Del", "Delete (into .originals)"),
        ("Esc", "Bring the deleted back"),
    ],
    help_video: [
        ("Space", "Play / pause"),
        ("Shift+Space", "Next photo"),
        ("Alt+←, Alt+→", "5 s back / on"),
        (",, .", "One frame back / on"),
        ("↑, ↓", "Volume"),
    ],
    help_view: [
        ("Z, Double-click", "Whole photo ↔ 100 %"),
        ("Ctrl+0, Ctrl+1", "Whole photo / 100 %"),
        ("+, −, Mouse wheel", "Zoom in / out"),
        ("Drag", "Move the zoomed photo"),
        ("O", "Check: sharp edges, clipping"),
        ("F7", "Grid of all photos"),
        ("G", "All faces large"),
        ("F, F11", "Full screen"),
    ],
    help_panels: [
        ("T", "Filter bar"),
        ("Tab", "Details panel"),
        ("F6", "Filmstrip"),
        ("Shift+Tab", "All three panels"),
        ("Ctrl+Tab", "Next tab of the details"),
    ],
    help_edit: [
        ("S", "Straighten"),
        ("R", "Crop"),
        ("Enter, Esc", "Apply / cancel"),
        ("Ctrl+←, Ctrl+→", "Rotate 90°"),
        ("E", "Edit in another program"),
    ],
    help_more: [
        ("Ctrl+K", "Menu: every function"),
        ("Ctrl+M", "Copy, move, delete the view"),
        ("Ctrl+L", "Language"),
        ("H, F1, ?", "This help"),
        ("Esc", "One step back"),
    ],
};
