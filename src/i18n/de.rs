use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('.'),
    compass: ["N", "S", "O", "W"],
    size_units: ["KB", "MB", "GB"],
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
    filter_people: "Mit Personen",
    filter_no_people: "Ohne Personen",
    filter_people_tooltip: "Erkannt an Gesichtern: Personen von hinten oder sehr klein im Bild zählen nicht.",
    filter_deleted: "Gelöschte",
    filter_deleted_tooltip: "Gelöschte Fotos dieses Ordners. Sie liegen im versteckten Ordner .originals – Strg+Z legt eins zurück.",
    filter_deleted_none: "Keine gelöschten Fotos in diesem Ordner",
    filter_hide_rejected: "Abgelehnte ausblenden",
    filter_without: "ohne",
    filter_hide_rejected_tooltip: "Die abgelehnten Fotos verschwinden, alle anderen bleiben. × zeigt dagegen nur die Abgelehnten.",
    menu_name_list: "Nach Dateiliste …",
    name_list_title: "Nach Dateiliste filtern",
    name_list_intro: "Dateinamen oder Nummern einfügen, die ein Kunde ausgewählt hat – je Zeile oder mit Komma oder Semikolon getrennt. Groß-/Kleinschreibung und Endung sind egal, eine Nummer findet die letzte Ziffernfolge eines Namens.",
    name_list_hint: "IMG_0345, IMG_0351 …",
    name_list_missing: "Nicht gefunden:",
    name_list_ambiguous: "In mehreren Ordnern (alle genommen):",
    name_list_apply: "Anwenden",
    name_list_chip_tooltip: "Nur die Fotos der eingefügten Liste – Klick zeigt wieder alle",
    name_list_found: |found, total| format!("{found} von {total} gefunden"),
    name_list_chip: |found, total| format!("Liste {found}/{total}  ×"),
    cmd_align_camera: "Rechte Kamera an linke angleichen",
    align_camera_hint: "Verschiebt die Aufnahmezeit aller Fotos der rechten Kamera in diesem Ordner, sodass das rechte Foto zur selben Zeit entstand wie das linke. Nur im Index – die Dateien behalten ihre Zeit.",
    align_needs_compare: "Erst zwei Fotos verschiedener Kameras vergleichen (C)",
    align_no_time: "Einem der beiden Fotos fehlt die Kamera oder die Aufnahmezeit",
    align_same_camera: "Beide Fotos stammen von derselben Kamera",
    menu_camera_time: "Kamerazeit angleichen …",
    camera_time_title: "Kamerazeit angleichen",
    camera_time_intro: "Ging die Uhr einer Kamera falsch, verschiebt ein Versatz die Aufnahmezeit all ihrer Fotos in diesem Ordner – für die Sortierung nach Aufnahmezeit, die Serien und die Anzeige. Nur im Index, die Dateien behalten ihre Zeit.",
    camera_time_reset: "Zurücksetzen",
    camera_time_invalid: "Versatz als +h:mm:ss oder −h:mm:ss, z. B. +1:30:00",
    camera_time_apply: "Übernehmen",
    camera_time_none: "Noch keine Fotos mit Kamera und Aufnahmezeit in diesem Ordner.",
    camera_time_applied: "Kamerazeit übernommen",
    camera_aligned: |camera, offset| format!("{camera}: Aufnahmezeit um {offset} verschoben"),
    camera_time_photos: |n| {
        if n == 1 {
            "1 Foto".into()
        } else {
            format!("{n} Fotos")
        }
    },
    camera_time_tooltip: |taken, offset| format!("Kamerazeit {taken} – angeglichen um {offset}"),
    cmd_pairs: "RAW+JPG als ein Foto",
    pairs_hint: "Ein RAW und ein JPG gleichen Namens erscheinen als ein Foto – gezeigt wird das JPG. Sterne, Farbe und Beschreibung gehen in beide Dateien, Kopieren, Verschieben und Löschen nehmen beide mit; Bearbeitungen ändern nur das JPG.",
    pairs_on: "RAW+JPG erscheinen als ein Foto",
    pairs_off: "RAW und JPG erscheinen getrennt",
    pair_badge: "RAW+JPG",
    pair_edit_jpeg_only: "Ändert nur das JPG – das RAW bleibt unberührt.",
    pair_raw_marks: |marks| format!("RAW: {marks}"),
    filter_clear: "Alle anzeigen",
    media_all: "Fotos und Videos",
    media_photos: "Nur Fotos",
    media_videos: "Nur Videos",
    media_no_videos: "Dieser Ordner enthält keine Videos",
    top_photos: |n| format!("Top {n} Fotos"),
    top_purposes: ["Highlights", "Vorschau", "Diashow", "Fotobuch", "Galerie"],
    top_tooltip: "Fotos, Videos oder beides – oder nur die besten Fotos, nach deinen Sternen (sonst die Vorhersage), Ästhetik und Schärfe, aus jeder Serie zuerst das beste. Abgelehnte und unscharfe Fotos und Dubletten zählen nicht. Die Auswahl bleibt, bis sich ein Filter ändert oder du „Reihenfolge aktualisieren“ wählst. An den Fotos wird nichts geändert.",
    menu_top: "Beste Fotos",
    bulk_delete_top: "Nicht bei „Top“ – das wären die besten Fotos",
    filter_none_active: "Kein Filter aktiv",
    filter_similar: "≈ Ähnliche",
    filter_similar_to: |name| format!("≈ wie {name}"),
    menu_similar: "Ähnliche Fotos",
    menu_similar_to: |name| format!("Ähnlich zu {name}"),
    similar_tooltip: |percent| {
        format!("Nur Fotos, die dem aktuellen ähneln (ab {percent:.0} %) – Taste M")
    },
    similar_fact: |percent| format!("Ähnlich {percent:.0} %"),
    similar_needs_model: "Ähnliche Fotos brauchen das Ästhetik-Modell (CLIP)",
    similar_not_analysed: "Dieses Foto ist noch nicht analysiert",
    similar_none: |percent| format!("Keine ähnlichen Fotos (ab {percent:.0} %)"),
    refresh_order: "Reihenfolge aktualisieren",
    refresh_order_tooltip: "Seit dem Sortieren und Filtern sind neue Bewertungen dazugekommen",
    analyzing_progress: |done, total| format!("Analyse {done} / {total}"),
    enable_aesthetics: "Ästhetik aktivieren…",
    enable_aesthetics_tooltip: |size| {
        format!("Lädt einmalig die Bildmodelle für die Ästhetik herunter ({size})")
    },
    add_v25: "V2.5 laden…",
    add_v25_tooltip: |size| {
        format!(
            "Lädt einmalig das zweite Ästhetik-Modell herunter (SigLIP + V2.5, {size}) – die Ästhetik ist dann der Mittelwert beider Modelle"
        )
    },
    downloading_model: |percent| format!("Modell wird geladen: {percent:.0} %"),
    aesthetics_loading: "Ästhetik: Modell wird geladen…",
    aesthetics_failed: "Ästhetik: Fehler",

    sort_name: "Name",
    sort_rating: "Sterne",
    sort_aesthetics: "Ästhetik",
    sort_personal: "Vorhersage",
    sort_sharpness: "Schärfe",
    sort_taken: "Aufnahmezeit",
    filter_unrated: "Ohne Sterne",

    filter_rejected: "Abgelehnte",
    actions: "Aktion",
    actions_tooltip: "Kopieren, Verschieben oder Löschen der angezeigten Fotos",
    selection_delete: "Löschen",
    bulk_copy: |n| {
        format!(
            "Kopieren nach … ({n} {})",
            if n == 1 { "Foto" } else { "Fotos" }
        )
    },
    bulk_move: |n| {
        format!(
            "Verschieben nach … ({n} {})",
            if n == 1 { "Foto" } else { "Fotos" }
        )
    },
    bulk_delete: |n| format!("Löschen ({n} {})", if n == 1 { "Foto" } else { "Fotos" }),
    bulk_delete_hint: "Alle Fotos, die der Filter gerade zeigt. Sie kommen nach 5 Sekunden in den versteckten Ordner .originals neben den Fotos – nichts wird endgültig gelöscht, Esc holt sie zurück.",
    delete_rejected_hint: "Alle abgelehnten Fotos des Ordners, auch die, die der Filter gerade ausblendet. Sie kommen nach 5 Sekunden in den versteckten Ordner .originals – nichts wird endgültig gelöscht.",
    bulk_restore: |n| {
        format!(
            "Zurücklegen ({n} {})",
            if n == 1 { "Foto" } else { "Fotos" }
        )
    },
    bulk_restore_hint: "Alle gelöschten Fotos, die der Filter gerade zeigt, kommen zurück in ihren Ordner. Ist ein Name dort vergeben, bekommt das Foto eine Nummer – nichts wird überschrieben.",
    photos_shown: |shown, total| format!("{shown} von {total} Fotos"),
    photos_count: |n| format!("{n} {}", if n == 1 { "Foto" } else { "Fotos" }),
    photos_badge_tooltip: "So viele Fotos zeigt der Filter gerade – auf genau diese wirkt „Aktion“.",
    label_red: "Rot",
    label_yellow: "Gelb",
    label_green: "Grün",
    label_blue: "Blau",
    label_purple: "Lila",
    meter_aesthetics_tooltip: "Ästhetik: Mittel aus LAION und V2.5 auf einer festen Skala – ein Foto hat in jedem Ordner denselben Wert. Die Schärfe vergleicht dagegen mit den anderen Fotos im Ordner.\nEinzelwerte: Detailfenster (Tab)",
    meter_sharpness: "Schärfe",
    meter_eyes: "Augen",
    probably_blurry: "wohl unscharf",
    analyzing: "Wird analysiert…",
    saving: "Speichert…",
    auto_advance_on: "Automatisch weiter",
    series_position: |index, len| format!("Serie {index} / {len}"),
    duplicate_of: |name| format!("Dublette von {name}"),
    rejected: "Abgelehnt",
    filmstrip_video: "Video",
    star_tooltip: |n| format!("{n} ★ – Taste {n}"),
    personal_hint: |stars| {
        format!(
            "Vorhersage: {stars:.1} ★ – so viele Sterne würdest du laut Cerno geben. Noch nicht deine Bewertung"
        )
    },
    zoom: |percent| format!("Zoom {percent:.0} %"),
    zoom_preview: |percent| format!("Zoom {percent:.0} % der Vorschau"),
    digital_zoom: |ratio| format!("{ratio:.1}× Digitalzoom"),
    button_toolbar: "Filterleiste",
    button_details: "Detailansicht",
    button_filmstrip: "Filmstreifen",
    button_help: "Hilfe",
    button_menu: "Menü",
    button_language: |name| format!("Sprache: {name}"),

    cmd_all_panels: "Filterleiste, Details und Filmstreifen",
    cmd_fullscreen: "Vollbild",
    cmd_compare: "Vergleichen",
    cmd_quad: "Viereransicht",
    cmd_similar: "Nur ähnliche Fotos zeigen",
    cmd_zoom: "Ganzes Foto ↔ 100 %",
    menu_overlay: "Overlay",
    overlay_off: "Aus",
    overlay_sharpness: "Scharfe Kanten",
    overlay_exposure: "Über- und Unterbelichtung",
    overlay_fact_sharpness: "Overlay: Schärfe",
    overlay_fact_exposure: "Overlay: Belichtung",
    overlay_hint_sharpness: "Schärfe: Lila markiert die schärfsten Kanten des Fotos",
    overlay_hint_exposure: "Belichtung: Rot = ausgebrannte Lichter, Blau = verlorene Tiefen",
    overlay_hint_off: "Overlay aus",
    overlay_show_on_photo: "Auf dem Foto zeigen (O)",
    cmd_grid: "Raster",
    cmd_reject: "Ablehnen",
    cmd_description: "Kommentar und Stichwörter",
    cmd_delete_rejected: |n| {
        format!(
            "Abgelehnte löschen ({n} {})",
            if n == 1 { "Foto" } else { "Fotos" }
        )
    },
    cmd_auto_advance: "Automatisch weiter",
    cmd_subfolders: "Unterordner einlesen",
    subfolders_on: "Unterordner werden mit eingelesen",
    subfolders_off: "Nur dieser Ordner, ohne Unterordner",
    menu_sort: "Sortieren",
    menu_filter: "Filter",
    menu_view: "Ansicht",
    menu_labels: "Farbmarken",
    menu_external: "Extern bearbeiten",
    external_other: "Anderes Programm …",
    external_chooser: "Systemauswahl „Öffnen mit“ …",
    external_default: "Mit dem Standardprogramm öffnen",
    external_pick_title: "Programm zum Bearbeiten wählen",
    external_opened: |name| {
        format!(
            "In {name} geöffnet – nach dem Speichern zeigt Cerno die neue Fassung, das Original bleibt in .originals"
        )
    },
    external_reloaded: |name| format!("{name} wurde extern gespeichert – neu geladen"),
    external_failed: |err| format!("Nicht geöffnet: {err}"),
    menu_language: "Sprache",
    menu_models: "Modelle & Daten",
    menu_this_photo: "Dieses Foto",
    menu_visible: "Sichtbare Fotos",
    menu_settings: "Einstellungen",
    menu_stars: "Sterne",
    label_none: "Ohne Farbe",
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
            "1 Foto wird in den versteckten Ordner .originals verschoben   ·   Esc macht es rückgängig".to_owned()
        } else {
            format!(
                "{n} Fotos werden in den versteckten Ordner .originals verschoben   ·   Esc macht es rückgängig"
            )
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
    transfer_progress: |moved, at, total, sizes, file| {
        let verb = if moved { "Verschiebe" } else { "Kopiere" };
        let mut text = format!("{verb} {at} / {total} Fotos – {sizes}");
        if !file.is_empty() {
            text.push_str(&format!(" – {file}"));
        }
        text
    },
    download_title: "Ästhetik-Bewertung aktivieren",
    download_text: |clip, v25, size| {
        let mut text = String::from("Für die Ästhetik-Bewertung fehlt:\n");
        if clip {
            text.push_str("\n• CLIP ViT-L/14 – von Hugging Face (Xenova/clip-vit-large-patch14)");
        }
        if v25 {
            text.push_str("\n• SigLIP + Aesthetic Predictor V2.5 – aus Cernos GitHub-Release models-1 (der V2.5-Teil steht unter AGPL-3.0)");
        }
        text.push_str(&format!(
            "\n\nJetzt herunterladen ({size})? Die Dateien kommen in Cernos Datenordner und werden nur einmal geladen; ein unterbrochener Download macht beim nächsten Mal weiter."
        ));
        text
    },
    btn_download: "Herunterladen",
    btn_cancel: "Abbrechen",
    btn_close: "Schließen (Esc)",
    aesthetics_offer: |size| {
        format!("Die Ästhetik-Bewertung braucht Bildmodelle ({size}): Menü → Modelle & Daten.")
    },
    v25_offer: |size| {
        format!(
            "Verlässlichere Ästhetik mit dem zweiten Modell V2.5 ({size}): Menü → Modelle & Daten."
        )
    },
    exiftool_offer: |size| {
        format!("Sterne und Farben speichert Cerno mit ExifTool ({size}): Menü → Modelle & Daten.")
    },
    exiftool_title: "ExifTool laden",
    exiftool_text: |size| {
        format!(
            "Sterne, Farbmarken, Kommentare und Bearbeitungen schreibt Cerno mit ExifTool in die Fotos – dem freien Werkzeug von Phil Harvey.\n\nJetzt von der offiziellen Quelle (SourceForge) laden ({size})? Es kommt in Cernos Datenordner; ein unterbrochener Download macht beim nächsten Mal weiter."
        )
    },

    section_aesthetics: "Ästhetik",
    section_sharpness: "Schärfe (im Ordner)",
    section_exposure: "Belichtung",
    section_attributes: "CLIP-Merkmale",
    row_aesthetics: "Ästhetik (Mittel)",
    section_histogram: "Histogramm",
    section_file: "Datei",
    row_size: "Größe",
    row_load_time: "Ladezeit",
    row_file_size: "Dateigröße",
    row_jpeg_quality: "JPEG-Qualität",
    explain_jpeg_quality: "Geschätzt aus den Quantisierungstabellen der Datei – die Qualität selbst steht nicht darin. 4:2:0 heißt: Farbe in halber Auflösung (üblich bei Kameras), 4:4:4: Farbe in voller.",
    explain_raw_preview: "Bei RAW-Dateien zeigt Cerno das JPEG, das die Kamera eingebettet hat – keine RAW-Entwicklung. Farbe und Belichtung sind die der Kamera, 100 % ist die Größe dieser Vorschau, nicht die des Sensors. Histogramm und Belichtung messen die Vorschau.",
    row_container: "Container",
    row_duration: "Dauer",
    row_video: "Video",
    row_frame_rate: "Bildrate",
    row_video_bitrate: "Video-Bitrate",
    row_audio: "Ton",
    row_audio_bitrate: "Ton-Bitrate",
    row_bitrate: "Gesamt-Bitrate",
    no_audio: "keine Tonspur",
    variable_frame_rate: "variabel",
    channels: |n| match n {
        1 => "Mono".to_owned(),
        2 => "Stereo".to_owned(),
        6 => "5.1".to_owned(),
        8 => "7.1".to_owned(),
        n => format!("{n} Kanäle"),
    },
    row_location: "Ort",
    tab_values: "Werte",
    tab_description: "Beschreibung",
    tab_faces: "Gesichter",
    section_comment: "Kommentar",
    section_keywords: "Stichwörter",
    comment_hint: "Kommentar schreiben …",
    keyword_hint: "Stichwort hinzufügen …",
    keyword_remove: "Entfernen",
    description_note: "Enter fügt ein Stichwort hinzu, Esc verlässt das Feld. Kommentar und Stichwörter stehen in der Datei (IPTC und XMP), wie Windows, Lightroom und digiKam sie lesen.",
    description_waiting: "Wird gelesen …",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "Vorhersage",
    row_frame: "Ganzes Bild",
    row_eyes: "Augen",
    row_highlights: "Ausgebrannte Lichter",
    row_shadows: "Verlorene Tiefen",
    attributes: [
        "Gesamtqualität",
        "Scharf",
        "Gutes Licht",
        "Guter Bildaufbau",
        "Rauscharm",
        "Farbenfroh",
    ],
    explain_laion: "Wie schön eine KI das Foto findet – sie hat dafür viele Bewertungen von Menschen gelernt. Sie mag vor allem Menschen, Porträts und Essen.",
    explain_v25: "Eine neuere KI für dieselbe Frage, besser bei Alltagsfotos. Sie mag vor allem Landschaft, Wasser und Luftaufnahmen.",
    explain_personal: "So viele Sterne würdest du laut Cerno vergeben. Sie lernt aus deinen Sternen und den Fotos, die du ablehnst oder löschst.",
    explain_aesthetics: "Der Wert unter dem Foto: das Mittel aus den beiden KIs darunter, von 0 % (wenig ansprechend) bis 100 % (sehr ansprechend). Die meisten Fotos liegen bei 40–60 %, ab 80 % ist sehr gut. Die Skala ist fest – ein Foto hat in jedem Ordner denselben Wert.",
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
    explain_models: "Wo die KIs laufen: DirectML = Grafikkarte, CPU = Prozessor. ± zeigt, wie weit die Vorhersage typischerweise danebenliegt.",
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
    model_personal: "Vorhersage",
    taste_trained: |n, error| format!("{n} Fotos, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} Fotos"),
    taste_untrained: "noch nicht trainiert",
    exiftool_found: |version, downloaded| {
        let from = if downloaded {
            "von Cerno geladen"
        } else {
            "installiert"
        };
        if version.is_empty() {
            from.to_owned()
        } else {
            format!("{version} · {from}")
        }
    },
    exiftool_absent: "fehlt – ohne ExifTool keine Sterne und Farben",
    exiftool_old_state: |version| format!("{version} – zu alt (12.24 oder neuer nötig)"),
    exiftool_downloading: |percent| format!("wird geladen … {percent:.0} %"),
    btn_exiftool: |size| format!("ExifTool laden ({size})"),
    taste_sources: |stars, rejected, deleted| {
        format!(
            "Gelernt aus {stars} Fotos mit Sternen, {rejected} abgelehnten und {deleted} gelöschten – Abgelehnte und Gelöschte zählen als 0 ★."
        )
    },
    btn_reset_taste: "Vorhersage zurücksetzen",
    btn_delete_models: "Modelle löschen",
    models_deleted: "Modelle gelöscht",
    models_downloaded: "Modelle geladen – die Ästhetik wird jetzt nachgerechnet",
    download_failed: |err| {
        format!(
            "Download fehlgeschlagen: {err} – ein neuer Versuch macht dort weiter, wo er aufgehört hat"
        )
    },
    exiftool_ready: "ExifTool ist da – Sterne und Farben werden jetzt gespeichert",
    exiftool_failed: |err| {
        format!("ExifTool ließ sich nicht laden: {err} – ein neuer Versuch macht dort weiter")
    },
    copy_models_path: "Pfad kopieren",
    models_path_copied: "Pfad kopiert",
    confirm_reset_taste_title: "Vorhersage zurücksetzen?",
    confirm_reset_taste_text: "Cerno vergisst, was es aus deinen Sternen und Löschungen gelernt hat. Sterne in den Fotodateien bleiben unverändert.",
    confirm_delete_models_title: "Heruntergeladene Modelle löschen?",
    confirm_delete_models_text: |size| {
        format!(
            "Entfernt die heruntergeladenen Modelldateien von der Festplatte ({size}). Gespeicherte Werte bleiben in der Datenbank; die Modelle lassen sich später wieder laden."
        )
    },

    cmd_straighten: "Ausrichten",
    cmd_rotate_ccw: "90° gegen den Uhrzeigersinn",
    cmd_rotate_cw: "90° im Uhrzeigersinn",
    cmd_crop: "Ausschnitt",
    cmd_undo: "Rückgängig",
    edit_not_jpeg: "Ausrichten, Ausschnitt, Drehen und Strg+Z gibt es nur für JPEG.",
    video_play_hint: "Abspielen (Leertaste)",
    video_play_pause: "Abspielen / Pause (Leertaste)",
    video_mute: "Ton an / aus",
    video_volume: "Lautstärke (↑ ↓)",
    video_no_zoom: "Videos werden nicht gezoomt",
    video_no_compare: "Videos lassen sich nicht vergleichen",
    video_no_sound: "Kein Ton: Cerno hat kein Audioausgabegerät gefunden – das Video läuft stumm",
    video_play_failed: |err| format!("Kann das Video nicht abspielen: {err}"),
    edit_writing: "Foto wird geschrieben…",
    edit_cancelled: "Anderes Foto – Bearbeitung verworfen",
    busy_editing: "Die Bearbeitung ist noch offen – Enter übernimmt, Esc verwirft",
    busy_copying: "Das Foto wird gerade kopiert – gleich wieder möglich",
    busy_moving: "Das Foto wird gerade verschoben",
    busy_deleted: "Gelöschtes Foto – erst zurücklegen (Strg+Z)",
    edit_needs_index: "Bearbeiten braucht den Index, und der ließ sich nicht öffnen",
    exiftool_missing: "Sterne, Farben und Bearbeiten brauchen ExifTool – Menü → Modelle & Daten",
    exiftool_too_old: "Das installierte ExifTool ist zu alt (12.24 oder neuer nötig) – Menü → Modelle & Daten",
    exiftool_loading: "ExifTool wird geladen – gleich lassen sich Sterne und Farben setzen",
    exiftool_install: |command| {
        match command {
        Some(command) => format!("Sterne, Farben und Bearbeiten brauchen ExifTool. Installieren mit: {command}"),
        None => "Sterne, Farben und Bearbeiten brauchen ExifTool. Installieren über die Paketverwaltung (Paket „exiftool“ oder „perl-image-exiftool“).".to_owned(),
    }
    },
    edit_reencoded: "JPEG neu kodiert – Strg+Z holt das Original zurück.",
    undo_done: "Original zurückgeholt",
    undo_nothing: "Für dieses Foto ist kein Original aufbewahrt",
    cmd_restore: "Zurücklegen",
    restored: |n, renamed, name| match (n, renamed) {
        (1, 0) => format!("Zurückgelegt: {name}"),
        (1, _) => format!("Zurückgelegt als {name} – der Name war vergeben"),
        (n, 0) => format!("{n} Fotos zurückgelegt"),
        (n, r) => format!("{n} Fotos zurückgelegt, {r} davon unter neuem Namen"),
    },
    restore_failed: |n, name, err| {
        format!("{n} Foto(s) konnten nicht zurückgelegt werden – {name}: {err}")
    },
    deleted_mark: "Gelöscht",
    faces_loading: "Gesichter werden gesucht…",
    faces_unknown: "Noch nicht analysiert – die Gesichter folgen.",
    faces_none: "Keine Gesichter erkannt",
    faces_only_small: "Nur kleine Gesichter – zu klein zum Beurteilen",
    face_eyes_blurry: "Augen wohl unscharf",
    faces_zoom_hint: "Klick zoomt auf dieses Gesicht",
    faces_grid_hint: "Klick oder Nummer (1–9) zoomt auf ein Gesicht · Esc schließt",
    cmd_faces: "Gesichter",
    cmd_face_grid: "Alle Gesichter groß",
    faces_small: |n| {
        format!(
            "+ {n} {} – zu klein zum Beurteilen",
            if n == 1 {
                "kleines Gesicht"
            } else {
                "kleine Gesichter"
            }
        )
    },
    face_number: |n| format!("Gesicht {n}"),
    raw_preview_fact: "RAW-Vorschau",
    preview_word: "Vorschau",
    edit_failed: |detail| format!("Nicht geschrieben: {detail}"),
    edit_hint_straighten: "Mausrad oder ←/→ dreht, Umschalt feiner · Enter übernimmt, Esc verwirft",
    edit_hint_crop: "Pfeile verschieben · +/− Größe, Umschalt feiner · A: Seitenverhältnis · X: Quer/Hoch · Enter übernimmt, Esc verwirft",
    ratio_original: "Original",
    crop_landscape: "Querformat",
    crop_portrait: "Hochformat",

    help_title: "Hilfe",
    help_intro: "Cerno zeigt deine Fotos ohne Wartezeit und hilft beim Aussortieren. Sterne und Farbmarken landen in der Fotodatei, das Dateidatum bleibt; alles andere speichert Cerno in seiner eigenen Datenbank.",
    help_drop: "Ordner oder Foto aufs Fenster ziehen oder Strg+O drücken.",
    help_close: "Esc, H oder F1 schließt diese Seite",
    help_tab_keys: "Tastenkürzel",
    help_tab_tips: "Tipps",
    help_pages_hint: "←/→ wechselt die Seite",
    help_tab_about: "Über Cerno",
    about_intro: "Cerno zeigt Fotos ohne Wartezeit und hilft beim Aussortieren – lokal, ohne Konto und ohne Cloud. Freie Software: Der Quellcode ist offen.",
    about_version: "Version",
    about_license: "Lizenz",
    about_source: "Quellcode",
    about_bugs: "Fehler melden",
    about_bugs_text: "Etwas funktioniert nicht? Beschreib es in einem GitHub-Issue – dafür braucht es ein kostenloses GitHub-Konto. Version und System sind schon eingetragen; Fotos und Dateinamen braucht es nicht. Ist Cerno abgestürzt, häng die Datei crash.log aus dem Datenordner an.",
    about_bug_link: "Fehler auf GitHub melden",
    about_open_data: "Datenordner öffnen",
    about_wishes: "Wünsche",
    about_wishes_text: "Eine Idee, die das Aussortieren mit Cerno einfacher oder schneller macht? Schreib sie als Wunsch – ebenfalls mit GitHub-Konto.",
    about_wish_link: "Wunsch auf GitHub schreiben",
    about_third_party: "Drittanbieter",
    about_third_party_text: "Cerno nutzt freie Bibliotheken anderer, darunter libheif und GStreamer mit FFmpeg (LGPL), und lädt auf Wunsch ExifTool und die Ästhetik-Modelle (V2.5 unter AGPL). Ihre Lizenztexte liegen dem Programm bei.",
    about_third_party_link: "Alle Drittanbieter und Lizenzen",
    help_tips: [
        (
            "Aussortieren in zwei Durchgängen",
            &[
                "Erst zügig blättern (Leertaste) und Misslungenes mit X ablehnen – ohne lange zu überlegen.",
                "Dann „ohne ×“ in der Filterleiste: Die Abgelehnten sind weg, jetzt mit 1–5 bewerten.",
                "Zum Schluss Aktion › „Abgelehnte löschen“ (Strg+M).",
            ],
        ),
        (
            "Serien und Vergleich",
            &[
                "Nach Aufnahmezeit sortiert stehen Serien zusammen, das schärfste Foto zuerst.",
                "C zeigt zwei Fotos nebeneinander; A behält das linke, D das rechte, das andere wird abgelehnt.",
                "M zeigt nur Fotos, die dem aktuellen ähneln.",
                "Zwei Kameras mit verschiedener Uhrzeit: zwei gleichzeitige Fotos vergleichen, dann Dieses Foto › „Rechte Kamera an linke angleichen“.",
            ],
        ),
        (
            "Die besten Fotos",
            &[
                "Im ersten Feld der Filterleiste „Top 50 Fotos“ wählen: Cerno schlägt die besten vor, aus jeder Serie zuerst eines.",
                "Ästhetik und Schärfe unter dem Foto helfen beim Entscheiden – die Sterne vergibst du.",
            ],
        ),
        (
            "Farbmarken",
            &[
                "Farben haben in Cerno keine feste Bedeutung. Zwei übliche Lesarten:",
                "Arbeitsstand: Rot prüfen · Gelb bearbeiten · Grün fertig · Blau exportiert",
                "Verwendung: Rot Kunde · Gelb Social Media · Grün Portfolio · Blau Druck",
                "6–9 setzen Rot bis Blau, die Filterleiste zeigt eine Farbe.",
            ],
        ),
        (
            "Gelöscht ist nicht weg",
            &[
                "Gelöschte Fotos kommen in den versteckten Ordner .originals neben den Fotos.",
                "Das Papierkorb-Kästchen in der Filterleiste zeigt sie, Strg+Z legt eins zurück.",
                "Vor dem ersten Ausrichten, Zuschneiden oder Drehen behält Cerno das Original – Strg+Z holt es zurück.",
            ],
        ),
        (
            "Die Vorhersage",
            &[
                "Cerno lernt aus deinen Sternen, abgelehnten und gelöschten Fotos, was dir gefällt.",
                "Bei Fotos ohne Sterne zeigt es seine Vermutung als leicht gefüllte Sterne – vergeben musst du sie selbst.",
                "Nach Vorhersage sortiert stehen die Fotos vorn, die dir wahrscheinlich gefallen.",
            ],
        ),
    ],
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
        "Bewerten",
        "Aussortieren",
        "Video",
        "Ansicht",
        "Leisten",
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
        ("Strg+U", "Unterordner einlesen – an / aus"),
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
        (
            "6 – 9",
            "Farbmarke: Rot, Gelb, Grün, Blau – noch einmal entfernt sie",
        ),
        ("Umschalt+6 – 9", "Diese Farbe setzen und zum nächsten Foto"),
        ("B", "Beschreibung: Kommentar und Stichwörter bearbeiten"),
    ],
    help_cull: [
        (
            "C",
            "Vergleichen: dieses Foto links festhalten, rechts blättern",
        ),
        (
            "A, D",
            "Vergleichen: links / rechts behalten – das andere wird abgelehnt, der Vergleich endet",
        ),
        (
            "Umschalt+C",
            "Viereransicht: vier Fotos auf einmal, der Rahmen ist das aktuelle – ↑ ↓ eine Reihe",
        ),
        ("M", "Nur ähnliche Fotos zeigen – noch einmal: wieder alle"),
        (
            "Entf",
            "Löschen: nach 5 Sekunden in den versteckten Ordner .originals – nichts geht verloren",
        ),
        ("Esc, Strg+Z", "Fotos zurückholen, die aufs Löschen warten"),
    ],
    help_video: [
        (
            "Leertaste",
            "Abspielen / anhalten (Umschalt+Leertaste: nächstes Foto)",
        ),
        ("Alt+←, Alt+→", "5 s zurück / vor"),
        (",, .", "Ein Bild zurück / vor (angehalten)"),
        ("↑, ↓", "Lautstärke"),
    ],
    help_view: [
        ("Z, Doppelklick", "Ganzes Foto ↔ 100 %"),
        ("Strg+0, Strg+1", "Ganzes Foto / 100 %"),
        ("+, −, Mausrad", "Hinein- / herauszoomen – auch mit Strg"),
        ("Ziehen", "Gezoomtes Foto verschieben"),
        (
            "O",
            "Overlay: scharfe Kanten → Über- und Unterbelichtung → aus",
        ),
        (
            "F7",
            "Raster aller Fotos: ↑ ↓ eine Zeile, + − Größe, Enter öffnet das Foto",
        ),
        ("F, F11", "Vollbild"),
    ],
    help_panels: [
        (
            "T",
            "Filterleiste: sortieren und filtern – nach Sternen, Farben, Schärfe, Personen",
        ),
        ("Tab", "Detailansicht"),
        ("F6", "Filmstreifen"),
        (
            "Umschalt+Tab",
            "Filterleiste, Details und Filmstreifen zusammen",
        ),
        (
            "G, Umschalt+G",
            "Gesichter: im Detailbereich (G) oder alle groß (Umschalt+G) – Klick zoomt hin",
        ),
    ],
    help_edit: [
        (
            "S",
            "Ausrichten: Raster, Mausrad und Pfeile drehen, Umschalt feiner",
        ),
        (
            "R",
            "Ausschnitt: Rahmen aufziehen oder mit Pfeilen und +/− setzen, A wechselt das Format, X dreht Quer/Hoch",
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
            "Original zurückholen – es bleibt in .originals neben dem Foto; ein gelöschtes Foto zurücklegen",
        ),
        (
            "E",
            "In einem anderen Programm bearbeiten – im gemerkten, oder eins wählen",
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
        ("Esc", "Schritt zurück: Zoom, Vergleich, Raster, Vollbild"),
    ],
};
