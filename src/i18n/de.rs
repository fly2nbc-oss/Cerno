use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('.'),
    compass: ["N", "S", "O", "W"],
    key_ctrl: "Strg",
    key_shift: "Umschalt",

    open: "Öffnen…",
    open_tooltip: "Ordner öffnen (Strg+O)",
    photos: |n| {
        if n == 1 {
            "1 Foto".to_owned()
        } else {
            format!("{n} Fotos")
        }
    },
    photos_shown: |shown, total| format!("{shown} von {total} Fotos"),
    sort: |key| format!("Sortierung: {key}"),
    show: |filter| format!("Anzeigen: {filter}"),
    hide_blurry: "Unscharfe ausblenden",
    hide_blurry_tooltip: "Blendet die unschärfsten 20 % dieses Ordners aus",
    refresh_order: "Reihenfolge aktualisieren",
    refresh_order_tooltip: "Seit dem Sortieren und Filtern sind neue Bewertungen dazugekommen",
    analyzing_progress: |done, total| format!("Analyse {done} / {total}"),
    analyzed: |total| format!("{total} analysiert"),
    enable_aesthetics: "Ästhetik aktivieren…",
    enable_aesthetics_tooltip: "Lädt einmalig das CLIP-Bildmodell herunter (1.2 GB)",
    downloading_model: |percent| format!("Modell wird geladen: {percent:.0} %"),
    aesthetics_ready: "Ästhetik: bereit",
    aesthetics_loading: "Ästhetik: Modell wird geladen…",
    aesthetics_backend: |backend| format!("Ästhetik: {backend}"),
    aesthetics_backend_tooltip: "Wo das Ästhetik-Modell läuft (DirectML = Grafikkarte)",
    aesthetics_failed: "Ästhetik: Fehler",

    sort_name: "Name",
    sort_rating: "Sterne",
    sort_laion: "Ästhetik (LAION)",
    sort_v25: "Ästhetik (V2.5)",
    sort_personal: "Persönlicher Geschmack",
    sort_sharpness: "Schärfe",
    filter_all: "Alle",
    filter_five: "5 Sterne",
    filter_at_least: |n| format!("ab {n} Sternen"),
    filter_unrated: "Ohne Sterne",

    filter_rejected: "Abgelehnte",
    meter_aesthetics: "Ästhetik",
    meter_aesthetics_tooltip: "LAION / V2.5 / dein persönlicher Geschmack – alle auf der Sterne-Skala 0–5\n– = noch nicht verfügbar",
    meter_sharpness: "Schärfe",
    meter_eyes: "Augen",
    probably_blurry: "wohl unscharf",
    analyzing: "Wird analysiert…",
    saving: "Speichert…",
    rejected: "Abgelehnt",
    star_tooltip: |n| format!("{n} ★ – Taste {n}"),
    zoom: |percent| format!("Zoom {percent:.0} %"),
    digital_zoom: |ratio| format!("{ratio:.1}× Digitalzoom"),
    map_tooltip: |place| format!("{place}\nKlick öffnet Google Maps"),
    button_toolbar: "Obere Leiste",
    button_details: "Detailansicht",
    button_filmstrip: "Filmstreifen",
    button_help: "Hilfe",
    button_language: |name| format!("Sprache: {name}"),

    palette_placeholder: "Befehl eingeben…",
    palette_empty: "Kein passender Befehl",
    cmd_explanations: "Erklärungen in der Detailansicht",
    cmd_all_panels: "Obere Leiste, Details und Filmstreifen",
    cmd_fullscreen: "Vollbild",
    cmd_compare: "Vergleichen",
    cmd_zoom: "Ganzes Foto ↔ 100 %",
    cmd_first: "Erstes Foto",
    cmd_last: "Letztes Foto",
    cmd_language: |name| format!("Sprache: {name}"),
    cmd_reject: "Ablehnen",
    cmd_delete_rejected: |n| format!("Abgelehnte Fotos löschen ({n})"),
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
    download_title: "Ästhetik-Bewertung aktivieren",
    download_text: |gb| {
        format!(
            "Für die Ästhetik-Bewertung braucht Cerno das Bildmodell CLIP ViT-L/14.\n\n\
             Jetzt von Hugging Face herunterladen (Xenova/clip-vit-large-patch14, {gb:.1} GB)? \
             Es wird in Cernos Datenordner gespeichert und nur einmal geladen."
        )
    },

    section_aesthetics: "Ästhetik",
    section_sharpness: "Schärfe (im Ordner)",
    section_exposure: "Belichtung",
    section_attributes: "CLIP-Merkmale",
    section_models: "Modelle",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "Persönlicher Geschmack",
    row_frame: "Ganzes Bild",
    row_eyes: "Augen",
    row_highlights: "Ausgebrannte Lichter",
    row_shadows: "Abgesoffene Schatten",
    attributes: [
        "Gesamtqualität",
        "Scharf",
        "Gutes Licht",
        "Gut aufgebaut",
        "Rauscharm",
        "Farbig",
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
        "gut aufgebaut – schlecht aufgebaut",
        "sauber – verrauscht",
        "farbenfroh – blass",
    ],
    explain_models: "Wo die KIs laufen: DirectML = Grafikkarte, CPU = Prozessor. ± zeigt, wie weit dein Geschmacksmodell typischerweise danebenliegt.",
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
    model_failed: "Fehler",
    model_faces: "Gesichter",
    model_personal: "Geschmack",
    taste_trained: |n, error| format!("{n} Fotos, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} Fotos"),
    taste_untrained: "noch nicht trainiert",

    help_title: "Hilfe",
    help_intro: "Cerno zeigt deine Fotos ohne Wartezeit und hilft beim Aussortieren. Sterne landen direkt in der Fotodatei, damit andere Programme sie auch sehen – das Dateidatum bleibt unverändert. Alles andere speichert Cerno in seiner eigenen Datenbank. Schärfe und Schönheit werden im Hintergrund automatisch bewertet; Tab zeigt alle Werte, I schaltet die Erklärungen dazu, Strg+K findet jeden Befehl.",
    help_drop: "Ordner oder Foto aufs Fenster ziehen oder Strg+O drücken.",
    help_close: "Esc, H oder F1 schließt diese Seite",
    help_sections: [
        "Blättern",
        "Bewerten und aussortieren",
        "Ansicht",
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
            "Vergleichen: links / rechts behalten – das andere wird abgelehnt",
        ),
    ],
    help_view: [
        ("Z, Doppelklick", "Ganzes Foto ↔ 100 %"),
        ("+, −, Mausrad", "Hinein- / herauszoomen"),
        ("Ziehen", "Gezoomtes Foto verschieben"),
        ("F11, F", "Vollbild"),
        ("T", "Obere Leiste"),
        ("Tab", "Detailansicht"),
        ("I", "Details: Werte → mit Erklärungen → aus"),
        ("F6", "Filmstreifen"),
        (
            "Umschalt+Tab",
            "Obere Leiste, Details und Filmstreifen zusammen",
        ),
    ],
    help_more: [
        (
            "Strg+K",
            "Befehlspalette: jeden Befehl suchen und ausführen",
        ),
        ("Strg+L", "Sprache wechseln"),
        ("H, F1, ?", "Diese Hilfe"),
        ("Esc", "Schritt zurück: Zoom, Vergleich, Vollbild"),
    ],
};
