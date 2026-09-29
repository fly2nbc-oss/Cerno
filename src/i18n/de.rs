use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('.'),
    compass: ["N", "S", "O", "W"],
    key_ctrl: "Strg",
    key_shift: "Umschalt",
    key_delete: "Entf",

    sort: |key| format!("Sortierung: {key}"),
    filter_stars: |n| {
        if n == 1 {
            "1 Stern".to_owned()
        } else {
            format!("{n} Sterne")
        }
    },
    filter_blurry: "Unscharfe",
    filter_blurry_tooltip: "Unter den unschärfsten 20 % dieses Ordners und deutlich weich",
    filter_duplicate: "Dubletten",
    filter_duplicate_tooltip: "Jedes Foto außer dem ersten gleichen Pfad",
    filter_clear: "Alle anzeigen",
    refresh_order: "Reihenfolge aktualisieren",
    refresh_order_tooltip: "Seit dem Sortieren und Filtern sind neue Bewertungen dazugekommen",
    analyzing_progress: |done, total| format!("Analyse {done} / {total}"),
    enable_aesthetics: "Ästhetik aktivieren…",
    enable_aesthetics_tooltip: "Lädt einmalig das CLIP-Bildmodell herunter (1.2 GB)",
    downloading_model: |percent| format!("Modell wird geladen: {percent:.0} %"),
    aesthetics_loading: "Ästhetik: Modell wird geladen…",
    aesthetics_failed: "Ästhetik: Fehler",

    sort_name: "Name",
    sort_rating: "Sterne",
    sort_laion: "Ästhetik (LAION)",
    sort_v25: "Ästhetik (V2.5)",
    sort_personal: "Für dich",
    sort_sharpness: "Schärfe",
    sort_taken: "Aufnahmezeit",
    filter_unrated: "Ohne Sterne",

    filter_rejected: "Abgelehnte",
    actions: "Aktion",
    actions_tooltip: "Kopieren, Verschieben oder Löschen der angezeigten Fotos",
    selection_delete: "Löschen",
    label_red: "Rot",
    label_yellow: "Gelb",
    label_green: "Grün",
    label_blue: "Blau",
    label_purple: "Lila",
    meter_aesthetics_tooltip: "L und V: Ästhetik (LAION / V2.5). Umriss-Stern: Für dich – die Sterne, die du laut Cerno geben würdest. Alles auf der Skala 0–5\n– = noch nicht verfügbar",
    meter_sharpness: "Schärfe",
    meter_eyes: "Augen",
    probably_blurry: "wohl unscharf",
    analyzing: "Wird analysiert…",
    saving: "Speichert…",
    auto_advance_on: "Automatisch weiter",
    series_position: |index, len| format!("Serie {index} / {len}"),
    duplicate_of: |name| format!("Dublette von {name}"),
    rejected: "Abgelehnt",
    star_tooltip: |n| format!("{n} ★ – Taste {n}"),
    zoom: |percent| format!("Zoom {percent:.0} %"),
    digital_zoom: |ratio| format!("{ratio:.1}× Digitalzoom"),
    button_toolbar: "Filterleiste",
    button_details: "Detailansicht",
    button_filmstrip: "Filmstreifen",
    button_help: "Hilfe",
    button_menu: "Menü",
    button_language: |name| format!("Sprache: {name}"),

    cmd_explanations: "Alle Erklärungen",
    cmd_all_panels: "Filterleiste, Details und Filmstreifen",
    cmd_fullscreen: "Vollbild",
    cmd_compare: "Vergleichen",
    cmd_zoom: "Ganzes Foto ↔ 100 %",
    cmd_reject: "Ablehnen",
    cmd_delete_rejected: |n| format!("Abgelehnte Fotos löschen ({n})"),
    cmd_auto_advance: "Automatisch weiter",
    cmd_subfolders: "Unterordner einlesen",
    menu_sort: "Sortieren",
    menu_filter: "Filter",
    menu_view: "Ansicht",
    menu_labels: "Farbmarken",
    menu_language: "Sprache",
    menu_models: "Modelle & Daten",
    menu_this_photo: "Dieses Foto",
    menu_visible: "Sichtbare Fotos",
    menu_settings: "Einstellungen",
    menu_stars: "Sterne",
    label_none: "Keine Farbe",
    loading: "Wird geladen…",
    cannot_show: "Dieses Bild kann nicht angezeigt werden",
    no_match: "Kein Foto passt zum Filter",
    drop_to_open: "Loslassen zum Öffnen",
    compare_left: "Links",
    compare_right: "Rechts",
    compare_left_badge: "L",
    keeps_this: |key| format!("{key} behält dieses"),
    compare_needs_two: "Zum Vergleichen braucht es mindestens zwei Fotos",
    deleting: |n| {
        if n == 1 {
            "1 Foto wird gelöscht   ·   Esc macht es rückgängig".to_owned()
        } else {
            format!("{n} Fotos werden gelöscht   ·   Esc macht es rückgängig")
        }
    },
    delete_failed: |n, name, err| {
        format!("{n} Foto(s) konnten nicht gelöscht werden – {name}: {err}")
    },
    blurry_tooltip: |eyes, percent| {
        let what = if eyes { "Augen" } else { "Foto" };
        format!("Wohl unscharf: {what} schärfer als nur {percent:.0} % dieses Ordners")
    },

    db_unavailable: |err| format!("Bewertungen werden in dieser Sitzung nicht gespeichert: {err}"),
    cannot_open: |path, err| format!("{path} lässt sich nicht öffnen: {err}"),
    no_photos_in: |dir| format!("Keine JPEG- oder HEIC-Dateien in {dir}"),
    rating_not_saved: |err| format!("Sterne nicht gespeichert – {err}"),
    open_folder: "Ordner öffnen",
    transfer_copy: "Kopieren",
    transfer_move: "Verschieben",
    transfer_copy_cmd: "Kopiere alles, was der aktuelle Filter zeigt, nach …",
    transfer_move_cmd: "Verschiebe alles, was der aktuelle Filter zeigt, nach …",
    transfer_same_folder: "Das ist schon der geöffnete Ordner",
    transfer_busy: "Kopieren oder Verschieben läuft schon",
    transfer_done: |moved, done, skipped, name, err| {
        let verb = if moved { "verschoben" } else { "kopiert" };
        let mut text = format!("{done} {verb}, {skipped} übersprungen");
        if !name.is_empty() {
            text.push_str(&format!(" – {name}: {err}"));
        }
        text
    },
    download_title: "Ästhetik-Bewertung aktivieren",
    download_text: |gb| {
        format!(
            "Für die Ästhetik-Bewertung braucht Cerno das Bildmodell CLIP ViT-L/14.\n\n\
             Jetzt von Hugging Face herunterladen (Xenova/clip-vit-large-patch14, {gb:.1} GB)? \
             Es wird in Cernos Datenordner gespeichert und nur einmal geladen."
        )
    },
    btn_download: "Herunterladen",
    btn_cancel: "Abbrechen",
    btn_close: "Schließen (Esc)",
    aesthetics_offer: "Die Ästhetik-Bewertung braucht ein Modell (1.2 GB): Menü → Modelle & Daten.",

    section_aesthetics: "Ästhetik",
    section_sharpness: "Schärfe (im Ordner)",
    section_exposure: "Belichtung",
    section_attributes: "CLIP-Merkmale",
    section_histogram: "Histogramm",
    section_file: "Datei",
    row_size: "Größe",
    row_load_time: "Ladezeit",
    row_location: "Ort",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "Für dich",
    row_frame: "Ganzes Bild",
    row_eyes: "Augen",
    row_highlights: "Ausgebrannte Lichter",
    row_shadows: "Abgesoffene Schatten",
    attributes: [
        "Gesamtqualität",
        "Scharf",
        "Gutes Licht",
        "Guter Bildaufbau",
        "Rauscharm",
        "Farbenfroh",
    ],
    explain_laion: "Wie schön eine KI das Foto findet – sie hat dafür viele Bewertungen von Menschen gelernt –, umgerechnet auf Sterne 0–5. Die meisten Fotos bekommen 2–3, ab 4 ist sehr gut.",
    explain_v25: "Eine neuere KI für dieselbe Frage, besser bei Alltagsfotos. Gleiche Sterne-Skala.",
    explain_personal: "So viele Sterne würdest du laut Cerno vergeben. Es lernt aus deinen Sternen und den Fotos, die du löschst.",
    explain_frame: "Wie scharf die schärfsten Stellen sind – im Vergleich zu den anderen Fotos im Ordner. 80 % heißt: schärfer als 80 % davon.",
    explain_eyes: "Schärfe direkt an den Augen, wenn ein Gesicht da ist. Bei Porträts zählt das, nicht der Hintergrund.",
    explain_highlights: "Stellen, die rein weiß sind und keine Zeichnung mehr haben. Ab 1 % lohnt ein Blick.",
    explain_shadows: "Stellen, die rein schwarz sind und keine Zeichnung mehr haben. Oft gewollt; ab 5 % wird es markiert.",
    explain_attributes: "Die KI vergleicht das Foto mit zwei gegensätzlichen Beschreibungen. 50 % heißt unentschieden, nahe 100 % klar die erste.",
    explain_attribute: [
        "gutes Foto – schlechtes Foto",
        "scharf – verschwommen",
        "gutes Licht – schlechtes Licht",
        "guter – schlechter Bildaufbau",
        "sauber – verrauscht",
        "farbenfroh – blass",
    ],
    explain_models: "Wo die KIs laufen: DirectML = Grafikkarte, CPU = Prozessor. ± zeigt, wie weit „Für dich“ typischerweise danebenliegt.",
    note_no_embedding: "noch keine CLIP-Daten",
    note_learning: |n, of| format!("lernt – {n} von {of} Fotos"),
    note_analysing: "wird analysiert…",
    note_no_face: "kein Gesicht",
    note_faces_too_small: |n| {
        if n == 1 {
            "1 Gesicht, zu klein".to_owned()
        } else {
            format!("{n} Gesichter, zu klein")
        }
    },
    note_needs_clip: "braucht das CLIP-Modell",
    model_missing: "nicht installiert",
    model_downloading: |percent| format!("lädt {percent:.0} %"),
    model_ready: "bereit",
    model_loading: "lädt…",
    model_removing: "wird entfernt…",
    model_failed: "Fehler",
    model_faces: "Gesichter",
    model_personal: "Für dich",
    taste_trained: |n, error| format!("{n} Fotos, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} Fotos"),
    taste_untrained: "noch nicht trainiert",
    btn_reset_taste: "Für dich zurücksetzen",
    btn_delete_models: "Modelle löschen",
    models_deleted: "Modelle gelöscht",
    copy_models_path: "Pfad kopieren",
    models_path_copied: "Pfad kopiert",
    confirm_reset_taste_title: "Für dich zurücksetzen?",
    confirm_reset_taste_text: "Cerno vergisst, was es aus deinen Sternen und Löschungen gelernt hat. Sterne in den Fotodateien bleiben unverändert.",
    confirm_delete_models_title: "Heruntergeladene Modelle löschen?",
    confirm_delete_models_text: "Entfernt die CLIP- und SigLIP-Modelldateien von der Festplatte (etwa 3 GB). Gespeicherte Werte bleiben in der Datenbank; Ästhetik kann später wieder geladen werden.",

    cmd_straighten: "Ausrichten",
    cmd_rotate_ccw: "90° gegen den Uhrzeigersinn",
    cmd_rotate_cw: "90° im Uhrzeigersinn",
    cmd_crop: "Ausschnitt",
    cmd_undo: "Rückgängig",
    edit_not_jpeg: "Ausrichten und Ausschnitt gibt es nur für JPEG.",
    edit_writing: "Foto wird geschrieben…",
    edit_cancelled: "Anderes Foto – Bearbeitung verworfen",
    busy_editing: "Die Bearbeitung ist noch offen – Enter übernimmt, Esc verwirft",
    busy_copying: "Das Foto wird gerade kopiert – gleich wieder möglich",
    busy_moving: "Das Foto wird gerade verschoben",
    edit_needs_index: "Bearbeiten braucht den Index, und der ließ sich nicht öffnen",
    edit_reencoded: "JPEG neu kodiert – Strg+Z holt das Original zurück.",
    undo_done: "Original zurückgeholt",
    undo_nothing: "Für dieses Foto ist kein Original aufbewahrt",
    edit_failed: |detail| format!("Nicht geschrieben: {detail}"),
    edit_hint_straighten: "Mausrad oder ←/→ dreht, Umschalt feiner · Enter übernimmt, Esc verwirft",
    edit_hint_crop: "A: Seitenverhältnis · X: Quer/Hoch · Enter übernimmt, Esc verwirft",
    ratio_original: "Original",
    crop_landscape: "Querformat",
    crop_portrait: "Hochformat",

    help_title: "Hilfe",
    help_intro: "Cerno zeigt deine Fotos ohne Wartezeit und hilft beim Aussortieren. Sterne und Farbmarken landen in der Fotodatei, das Dateidatum bleibt; alles andere speichert Cerno in seiner eigenen Datenbank.",
    help_drop: "Ordner oder Foto aufs Fenster ziehen oder Strg+O drücken.",
    help_close: "Esc, H oder F1 schließt diese Seite",
    welcome_intro: "Fotos ohne Wartezeit ansehen, bewerten und aussortieren – Sterne landen in der Datei, das Dateidatum bleibt.",
    welcome_keys: [
        ("←, →", "Vorheriges / nächstes Foto"),
        ("1 – 5", "Sterne vergeben"),
        ("X", "Ablehnen"),
        ("Entf", "Löschen – Esc holt es zurück"),
        ("Strg+K", "Menü mit allen Funktionen"),
    ],
    welcome_more: "Alle Tastenkürzel: H",
    help_sections: [
        "Blättern",
        "Bewerten und aussortieren",
        "Ansicht",
        "Bearbeiten",
        "Weiteres",
    ],
    help_browse: [
        (
            "→, Leertaste, Bild↓",
            "Nächstes Foto (gedrückt halten zum Durchlaufen)",
        ),
        ("←, Rücktaste, Bild↑", "Vorheriges Foto"),
        ("Pos1, Ende", "Erstes / letztes Foto"),
        ("Mausrad", "Über dem Filmstreifen: durch die Fotos blättern"),
        ("Strg+O", "Ordner öffnen (oder aufs Fenster ziehen)"),
    ],
    help_rate: [
        (
            "1 – 5",
            "Sterne vergeben – werden in die Fotodatei geschrieben",
        ),
        (
            "Umschalt+1 – 5",
            "Sterne vergeben und weiter zum nächsten Foto",
        ),
        ("0", "Sterne oder Ablehnung entfernen"),
        (
            "X, Umschalt+X",
            "Ablehnen – steht in der Datei, nichts wird gelöscht; mit Umschalt weiter zum nächsten Foto",
        ),
        ("Entf", "Löschen: nach 5 Sekunden in den Papierkorb"),
        ("Esc", "Fotos zurückholen, die aufs Löschen warten"),
        (
            "C",
            "Vergleichen: dieses Foto links festhalten, rechts blättern",
        ),
        (
            "A, D",
            "Vergleichen: links / rechts behalten – das andere wird abgelehnt, der Vergleich endet",
        ),
        (
            "6 – 9",
            "Farbmarke: Rot, Gelb, Grün, Blau – noch einmal entfernt sie",
        ),
        ("Umschalt+6 – 9", "Diese Farbe setzen und zum nächsten Foto"),
    ],
    help_view: [
        ("Z, Doppelklick", "Ganzes Foto ↔ 100 %"),
        ("+, −, Mausrad", "Hinein- / herauszoomen"),
        ("Ziehen", "Gezoomtes Foto verschieben"),
        ("F, F11", "Vollbild"),
        ("T", "Filterleiste: sortieren und filtern"),
        ("Tab", "Detailansicht"),
        ("I", "Details: alle Erklärungen auf- oder zuklappen"),
        ("F6", "Filmstreifen"),
        (
            "Umschalt+Tab",
            "Filterleiste, Details und Filmstreifen zusammen",
        ),
    ],
    help_edit: [
        (
            "S",
            "Ausrichten: Raster, Mausrad und Pfeile drehen, Umschalt feiner",
        ),
        (
            "R",
            "Ausschnitt: Rahmen aufziehen, A wechselt das Format, X dreht Quer/Hoch",
        ),
        (
            "Enter, Esc",
            "Ausrichten oder Ausschnitt übernehmen oder verwerfen",
        ),
        (
            "Strg+←, Strg+→",
            "90° drehen – verlustfrei über die JPEG-Orientierung",
        ),
        (
            "Strg+Z",
            "Letzte Änderung zurücknehmen – Originale bleiben 30 Tage",
        ),
    ],
    help_more: [
        ("Strg+K", "Menü: alle Funktionen"),
        (
            "Strg+M",
            "Aktion: Kopieren, Verschieben oder Löschen der angezeigten Fotos",
        ),
        ("Strg+L", "Sprache wechseln"),
        ("H, F1, ?", "Diese Hilfe"),
        ("Esc", "Schritt zurück: Zoom, Vergleich, Vollbild"),
    ],
};
