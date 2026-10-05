use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('/'),
    compass: ["N", "S", "E", "O"],
    size_units: ["KB", "MB", "GB"],
    key_ctrl: "Ctrl",
    key_shift: "Maiusc",
    key_delete: "Canc",

    sort: |key| format!("Ordina: {key}"),
    filter_stars: |n| {
        if n == 1 {
            "1 stella".to_owned()
        } else {
            format!("{n} stelle")
        }
    },
    filter_blurry: "Sfocate",
    filter_blurry_tooltip: "Tra il 20 % più sfocato di questa cartella e chiaramente morbide",
    filter_duplicate: "Duplicati",
    filter_duplicate_tooltip: "Ogni foto tranne il primo percorso identico",
    filter_people: "Con persone",
    filter_no_people: "Senza persone",
    filter_people_tooltip: "Riconosciute dal volto: le persone di spalle o molto piccole nell'immagine non contano.",
    filter_deleted: "Eliminate",
    filter_deleted_tooltip: "Foto eliminate in questa cartella. Si trovano nella cartella nascosta .originals: Ctrl+Z ne rimette a posto una.",
    filter_deleted_none: "Nessuna foto eliminata in questa cartella",
    filter_hide_rejected: "Nascondi rifiutate",
    filter_without: "senza",
    filter_hide_rejected_tooltip: "Le foto rifiutate spariscono, tutte le altre restano. × invece mostra solo le rifiutate.",
    menu_name_list: "Per elenco di file …",
    name_list_title: "Filtra per elenco di file",
    name_list_intro: "Incolla i nomi di file o i numeri scelti da un cliente, uno per riga o separati da virgole o punti e virgola. Maiuscole ed estensione non contano; un numero trova le cifre con cui finisce un nome.",
    name_list_hint: "IMG_0345, IMG_0351 …",
    name_list_missing: "Non trovati:",
    name_list_ambiguous: "In più cartelle (prese tutte):",
    name_list_apply: "Applica",
    name_list_chip_tooltip: "Solo le foto dell'elenco incollato: un clic le mostra di nuovo tutte",
    name_list_found: |found, total| format!("{found} di {total} trovati"),
    name_list_chip: |found, total| format!("Elenco {found}/{total}  ×"),
    filter_clear: "Mostra tutte",
    media_all: "Foto e video",
    media_photos: "Solo foto",
    media_videos: "Solo video",
    media_no_videos: "Questa cartella non contiene video",
    top_photos: |n| format!("Top {n} foto"),
    top_purposes: [
        "In evidenza",
        "Anteprima",
        "Presentazione",
        "Fotolibro",
        "Galleria",
    ],
    top_tooltip: "Foto, video o entrambi – oppure solo le foto migliori, secondo le tue stelle (altrimenti la previsione), l'estetica e la nitidezza, prima la migliore di ogni serie. Le foto scartate o sfocate e i duplicati non contano. La scelta resta finché cambia un filtro o scegli «Aggiorna ordine». Nelle foto non cambia nulla.",
    menu_top: "Foto migliori",
    bulk_delete_top: "Non con Top – sono le foto migliori",
    filter_none_active: "Nessun filtro attivo",
    filter_similar: "≈ Simili",
    filter_similar_to: |name| format!("≈ come {name}"),
    menu_similar: "Foto simili",
    menu_similar_to: |name| format!("Simili a {name}"),
    similar_tooltip: |percent| {
        format!("Solo le foto che somigliano a quella attuale (da {percent:.0} %) – tasto M")
    },
    similar_fact: |percent| format!("Somiglianza {percent:.0} %"),
    similar_needs_model: "Le foto simili richiedono il modello di estetica (CLIP)",
    similar_not_analysed: "Questa foto non è ancora stata analizzata",
    similar_none: |percent| format!("Nessuna foto simile (da {percent:.0} %)"),
    refresh_order: "Aggiorna ordine",
    refresh_order_tooltip: "Dopo l'ordinamento e il filtro sono arrivati nuovi punteggi",
    analyzing_progress: |done, total| format!("Analisi {done} / {total}"),
    enable_aesthetics: "Attiva estetica…",
    enable_aesthetics_tooltip: |size| {
        format!("Scarica una sola volta i modelli di immagini per l'estetica ({size})")
    },
    add_v25: "Carica V2.5…",
    add_v25_tooltip: |size| {
        format!(
            "Scarica una sola volta il secondo modello di estetica (SigLIP + V2.5, {size}): l'estetica diventa la media dei due modelli"
        )
    },
    downloading_model: |percent| format!("Download del modello {percent:.0} %"),
    aesthetics_loading: "Estetica: caricamento modello…",
    aesthetics_failed: "Estetica: errore",

    sort_name: "Nome",
    sort_rating: "Stelle",
    sort_aesthetics: "Estetica",
    sort_personal: "Previsione",
    sort_sharpness: "Nitidezza",
    sort_taken: "Ora di scatto",
    filter_unrated: "Senza stelle",

    filter_rejected: "Rifiutate",
    actions: "Azione",
    actions_tooltip: "Copia, sposta o elimina le foto in vista",
    selection_delete: "Elimina",
    bulk_copy: |n| format!("Copia in … ({n} foto)"),
    bulk_move: |n| format!("Sposta in … ({n} foto)"),
    bulk_delete: |n| format!("Elimina ({n} foto)"),
    bulk_delete_hint: "Tutte le foto che il filtro mostra. Dopo 5 secondi finiscono nella cartella nascosta .originals accanto a loro – niente viene eliminato per sempre, Esc le riporta indietro.",
    delete_rejected_hint: "Tutte le foto rifiutate della cartella, anche quelle che il filtro nasconde. Dopo 5 secondi finiscono nella cartella nascosta .originals – niente viene eliminato per sempre.",
    bulk_restore: |n| format!("Rimetti a posto ({n} foto)"),
    bulk_restore_hint: "Tutte le foto eliminate mostrate dal filtro tornano nella loro cartella. Se il nome è già usato, la foto riceve un numero: niente viene sovrascritto.",
    photos_shown: |shown, total| format!("{shown} di {total} foto"),
    photos_count: |n| format!("{n} foto"),
    photos_badge_tooltip: "Quante foto mostra ora il filtro – «Azione» agisce proprio su queste.",
    label_red: "Rosso",
    label_yellow: "Giallo",
    label_green: "Verde",
    label_blue: "Blu",
    label_purple: "Viola",
    meter_aesthetics_tooltip: "Estetica: media di LAION e V2.5 su una scala fissa – una foto ha lo stesso valore in ogni cartella. La nitidezza invece confronta con le altre foto della cartella.\nValori separati: pannello dei dettagli (Tab)",
    meter_sharpness: "Nitidezza",
    meter_eyes: "Occhi",
    probably_blurry: "probabilmente sfocata",
    analyzing: "Analisi in corso…",
    saving: "Salvataggio…",
    auto_advance_on: "Avanzamento automatico",
    series_position: |index, len| format!("Serie {index} / {len}"),
    duplicate_of: |name| format!("Duplicato di {name}"),
    rejected: "Rifiutata",
    filmstrip_video: "Video",
    star_tooltip: |n| format!("{n} ★ – tasto {n}"),
    personal_hint: |stars| {
        format!(
            "Previsione: {stars:.1} ★ – le stelle che Cerno pensa daresti. Non è ancora il tuo voto"
        )
    },
    zoom: |percent| format!("Zoom {percent:.0} %"),
    zoom_preview: |percent| format!("Zoom {percent:.0} % dell'anteprima"),
    digital_zoom: |ratio| format!("Zoom digitale {ratio:.1}×"),
    button_toolbar: "Barra dei filtri",
    button_details: "Pannello dettagli",
    button_filmstrip: "Striscia di miniature",
    button_help: "Aiuto",
    button_menu: "Menu",
    button_language: |name| format!("Lingua: {name}"),

    cmd_all_panels: "Barra dei filtri, dettagli e striscia di miniature",
    cmd_fullscreen: "Schermo intero",
    cmd_compare: "Confronta",
    cmd_similar: "Mostra solo foto simili",
    cmd_zoom: "Foto intera ↔ 100 %",
    menu_overlay: "Sovrapposizione",
    overlay_off: "Disattivata",
    overlay_sharpness: "Bordi nitidi",
    overlay_exposure: "Luci e ombre tagliate",
    overlay_fact_sharpness: "Sovrapposizione: nitidezza",
    overlay_fact_exposure: "Sovrapposizione: esposizione",
    overlay_hint_sharpness: "Nitidezza: il viola segna i bordi più nitidi della foto",
    overlay_hint_exposure: "Esposizione: rosso = bruciato, blu = nero chiuso",
    overlay_hint_off: "Sovrapposizione disattivata",
    overlay_show_on_photo: "Mostra sulla foto (O)",
    cmd_grid: "Griglia",
    cmd_reject: "Rifiuta",
    cmd_description: "Commento e parole chiave",
    cmd_delete_rejected: |n| format!("Elimina le rifiutate ({n} foto)"),
    cmd_auto_advance: "Avanza automaticamente",
    cmd_subfolders: "Includi sottocartelle",
    subfolders_on: "Sottocartelle incluse",
    subfolders_off: "Solo questa cartella, senza sottocartelle",
    menu_sort: "Ordina",
    menu_filter: "Filtro",
    menu_view: "Vista",
    menu_labels: "Colori",
    menu_external: "Modifica altrove",
    external_other: "Altro programma …",
    external_chooser: "«Apri con» del sistema …",
    external_default: "Apri con il programma predefinito",
    external_pick_title: "Scegli un programma per modificare",
    external_opened: |name| {
        format!(
            "Aperto in {name} – dopo il salvataggio Cerno mostra la nuova versione; l'originale resta in .originals"
        )
    },
    external_reloaded: |name| format!("{name} è stato salvato altrove – ricaricato"),
    external_failed: |err| format!("Non aperto: {err}"),
    menu_language: "Lingua",
    menu_models: "Modelli e dati",
    menu_this_photo: "Questa foto",
    menu_visible: "Foto visibili",
    menu_settings: "Impostazioni",
    menu_stars: "Stelle",
    label_none: "Senza colore",
    loading: "Caricamento…",
    cannot_show: "Impossibile mostrare questa immagine",
    no_match: "Nessuna foto corrisponde al filtro",
    drop_to_open: "Rilascia per aprire",
    compare_left: "Sinistra",
    compare_right: "Destra",
    compare_left_badge: "S",
    keeps_this: |key| format!("{key} tiene questa"),
    compare_needs_two: "Per confrontare servono almeno due foto",
    deleting: |n| {
        if n == 1 {
            "1 foto va nella cartella nascosta .originals   ·   Esc per annullare".to_owned()
        } else {
            format!("{n} foto vanno nella cartella nascosta .originals   ·   Esc per annullare")
        }
    },
    delete_failed: |n, name, err| format!("Impossibile eliminare {n} foto – {name}: {err}"),
    blurry_tooltip: |eyes, percent| {
        let what = if eyes {
            "occhi più nitidi"
        } else {
            "foto più nitida"
        };
        format!("Probabilmente sfocata: {what} solo del {percent:.0} % di questa cartella")
    },

    db_unavailable: |err| format!("I punteggi non vengono salvati in questa sessione: {err}"),
    cannot_open: |path, err| format!("Impossibile aprire {path}: {err}"),
    no_photos_in: |dir| format!("Nessun file JPEG o HEIC in {dir}"),
    rating_not_saved: |err| format!("Stelle non salvate – {err}"),
    open_folder: "Apri cartella",
    transfer_copy_cmd: "Copia tutto ciò che mostra il filtro attuale in …",
    transfer_move_cmd: "Sposta tutto ciò che mostra il filtro attuale in …",
    transfer_same_folder: "È già la cartella aperta",
    transfer_busy: "Una copia o uno spostamento è già in corso",
    transfer_done: |moved, done, skipped, name, err| {
        let verb = if moved { "spostate" } else { "copiate" };
        let mut text = format!("{done} {verb}, {skipped} saltate");
        if !name.is_empty() {
            text.push_str(&format!(" – {name}: {err}"));
        }
        text
    },
    transfer_progress: |moved, at, total, sizes, file| {
        let verb = if moved { "Spostamento di" } else { "Copia di" };
        let mut text = format!("{verb} {at} / {total} foto – {sizes}");
        if !file.is_empty() {
            text.push_str(&format!(" – {file}"));
        }
        text
    },
    download_title: "Attiva la valutazione estetica",
    download_text: |clip, v25, size| {
        let mut text = String::from("Per la valutazione estetica manca:\n");
        if clip {
            text.push_str("\n• CLIP ViT-L/14 – da Hugging Face (Xenova/clip-vit-large-patch14)");
        }
        if v25 {
            text.push_str("\n• SigLIP + Aesthetic Predictor V2.5 – dalla release GitHub models-1 di Cerno (la parte V2.5 è AGPL-3.0)");
        }
        text.push_str(&format!(
            "\n\nScaricare ora ({size})? I file vanno nella cartella dati di Cerno e vengono scaricati una sola volta; un download interrotto riprende la volta successiva."
        ));
        text
    },
    btn_download: "Scarica",
    btn_cancel: "Annulla",
    btn_close: "Chiudi (Esc)",
    aesthetics_offer: |size| {
        format!(
            "La valutazione estetica richiede modelli di immagini ({size}): Menu → Modelli e dati."
        )
    },
    v25_offer: |size| {
        format!(
            "Estetica più affidabile con il secondo modello V2.5 ({size}): Menu → Modelli e dati."
        )
    },

    section_aesthetics: "Estetica",
    section_sharpness: "Nitidezza (nella cartella)",
    section_exposure: "Esposizione",
    section_attributes: "Caratteristiche CLIP",
    row_aesthetics: "Estetica (media)",
    section_histogram: "Istogramma",
    section_file: "File",
    row_size: "Dimensioni",
    row_load_time: "Tempo di caricamento",
    row_file_size: "Dimensione file",
    row_jpeg_quality: "Qualità JPEG",
    explain_jpeg_quality: "Stimata dalle tabelle di quantizzazione del file: la qualità in sé non vi è salvata. 4:2:0 significa colore a metà risoluzione (comune nelle fotocamere), 4:4:4 a risoluzione piena.",
    explain_raw_preview: "Per i file RAW Cerno mostra il JPEG incorporato dalla fotocamera, senza sviluppo RAW. Colore ed esposizione sono quelli della fotocamera e il 100 % è la dimensione di questa anteprima, non del sensore. Istogramma ed esposizione misurano l'anteprima.",
    row_container: "Contenitore",
    row_duration: "Durata",
    row_video: "Video",
    row_frame_rate: "Fotogrammi/s",
    row_video_bitrate: "Bitrate video",
    row_audio: "Audio",
    row_audio_bitrate: "Bitrate audio",
    row_bitrate: "Bitrate totale",
    no_audio: "nessuna traccia audio",
    variable_frame_rate: "variabile",
    channels: |n| match n {
        1 => "Mono".to_owned(),
        2 => "Stereo".to_owned(),
        6 => "5.1".to_owned(),
        8 => "7.1".to_owned(),
        n => format!("{n} canali"),
    },
    row_location: "Posizione",
    tab_values: "Valori",
    tab_description: "Descrizione",
    tab_faces: "Volti",
    section_comment: "Commento",
    section_keywords: "Parole chiave",
    comment_hint: "Scrivi un commento …",
    keyword_hint: "Aggiungi parola chiave …",
    keyword_remove: "Rimuovi",
    description_note: "Invio aggiunge una parola chiave, Esc esce dal campo. Commento e parole chiave vengono scritti nel file (IPTC e XMP), dove Windows, Lightroom e digiKam li leggono.",
    description_waiting: "Lettura …",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "Previsione",
    row_frame: "Immagine intera",
    row_eyes: "Occhi",
    row_highlights: "Luci bruciate",
    row_shadows: "Ombre chiuse",
    attributes: [
        "Qualità generale",
        "Nitida",
        "Buona luce",
        "Ben composta",
        "Poco rumore",
        "Colorata",
    ],
    explain_laion: "Quanto è bella la foto per un'IA addestrata su molte valutazioni di persone. Apprezza soprattutto persone, ritratti e cibo.",
    explain_v25: "Un'IA più recente per la stessa domanda, migliore con le foto di tutti i giorni. Apprezza soprattutto paesaggi, acqua e riprese aeree.",
    explain_personal: "Le stelle che secondo Cerno daresti tu. Impara dalle tue stelle e dalle foto che elimini.",
    explain_aesthetics: "Il valore sotto la foto: la media delle due IA qui sotto, da 0 % (poco attraente) a 100 % (molto attraente). La maggior parte delle foto sta tra 40 e 60 %, da 80 % in su è molto buona. La scala è fissa – una foto ha lo stesso valore in ogni cartella.",
    explain_frame: "Quanto sono nitide le parti più nitide, rispetto alle altre foto di questa cartella. 80 % significa: più nitida dell'80 % delle altre.",
    explain_eyes: "Nitidezza proprio sugli occhi, se c'è un volto. Nei ritratti conta questa, non lo sfondo.",
    explain_highlights: "Parti completamente bianche, senza più alcun dettaglio. Oltre l'1 % merita un'occhiata.",
    explain_shadows: "Parti completamente nere, senza più alcun dettaglio. Spesso è voluto; oltre il 5 % viene segnalato.",
    explain_attributes: "L'IA confronta la foto con due descrizioni opposte. 50 % significa indecisa, vicino al 100 % chiaramente la prima.",
    explain_attribute: [
        "bella foto – brutta foto",
        "nitida – sfocata",
        "buona luce – cattiva luce",
        "ben composta – mal composta",
        "pulita – con rumore",
        "colorata – spenta",
    ],
    explain_models: "Dove viene eseguita ogni IA: DirectML = scheda grafica, CPU = processore. ± indica di quanto la previsione di solito sbaglia.",
    note_no_embedding: "ancora nessun dato CLIP",
    note_learning: |n, of| format!("in apprendimento – {n} di {of} foto"),
    note_analysing: "analisi in corso…",
    note_no_face: "nessun volto",
    note_faces_too_small: |n| {
        if n == 1 {
            "1 volto, troppo piccolo".to_owned()
        } else {
            format!("{n} volti, troppo piccoli")
        }
    },
    note_needs_clip: "serve il modello CLIP",
    model_missing: "non installato",
    model_downloading: |percent| format!("download {percent:.0} %"),
    model_ready: "pronto",
    model_loading: "caricamento…",
    model_removing: "rimozione…",
    model_failed: "errore",
    model_faces: "Volti",
    model_personal: "Previsione",
    taste_trained: |n, error| format!("{n} foto, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} foto"),
    taste_untrained: "non ancora addestrato",
    taste_sources: |stars, rejected, deleted| {
        format!(
            "Appresa da {stars} foto con stelle, {rejected} rifiutate e {deleted} eliminate – le foto rifiutate ed eliminate contano come 0 ★."
        )
    },
    btn_reset_taste: "Reimposta la previsione",
    btn_delete_models: "Elimina modelli",
    models_deleted: "Modelli eliminati",
    models_downloaded: "Modelli scaricati: ora viene calcolata l'estetica",
    download_failed: |err| {
        format!("Download non riuscito: {err}. Un nuovo tentativo riprende da dove si era fermato")
    },
    copy_models_path: "Copia percorso",
    models_path_copied: "Percorso copiato",
    confirm_reset_taste_title: "Reimpostare la previsione?",
    confirm_reset_taste_text: "Cerno dimenticherà ciò che ha imparato dalle tue stelle e dalle eliminazioni. Le stelle nei file foto restano invariate.",
    confirm_delete_models_title: "Eliminare i modelli scaricati?",
    confirm_delete_models_text: |size| {
        format!(
            "Rimuove dal disco i file dei modelli scaricati ({size}). I punteggi salvati restano nel database; i modelli possono essere scaricati di nuovo."
        )
    },

    cmd_straighten: "Raddrizza",
    cmd_rotate_ccw: "Ruota di 90° a sinistra",
    cmd_rotate_cw: "Ruota di 90° a destra",
    cmd_crop: "Ritaglio",
    cmd_undo: "Annulla modifica",
    edit_not_jpeg: "Raddrizza, ritaglio, rotazione e Ctrl+Z solo per JPEG.",
    video_play_hint: "Riproduci (Spazio)",
    video_play_pause: "Riproduci / pausa (Spazio)",
    video_mute: "Audio sì / no",
    video_volume: "Volume (↑ ↓)",
    video_no_zoom: "I video non si ingrandiscono",
    video_no_compare: "I video non si possono confrontare",
    video_no_sound: "Nessun audio: Cerno non ha trovato un'uscita audio; il video viene riprodotto senza suono",
    video_no_ffmpeg: "Nessuna anteprima: a Cerno serve ffmpeg (ad es. winget install Gyan.FFmpeg)",
    video_play_failed: |err| format!("Impossibile riprodurre il video: {err}"),
    edit_writing: "Scrittura della foto…",
    edit_cancelled: "È visualizzata un'altra foto – modifica annullata",
    busy_editing: "La modifica è ancora aperta – Invio applica, Esc annulla",
    busy_copying: "La foto è in fase di copia – di nuovo possibile tra poco",
    busy_moving: "La foto è in fase di spostamento",
    busy_deleted: "Foto eliminata: rimettila prima a posto (Ctrl+Z)",
    edit_needs_index: "La modifica richiede l'indice, che non è stato possibile aprire",
    edit_reencoded: "JPEG ricodificato – Ctrl+Z ripristina l'originale.",
    undo_done: "Originale ripristinato",
    undo_nothing: "Nessun originale conservato per questa foto",
    cmd_restore: "Rimetti a posto",
    restored: |n, renamed, name| match (n, renamed) {
        (1, 0) => format!("Rimessa a posto: {name}"),
        (1, _) => format!("Rimessa a posto come {name}: il nome era già usato"),
        (n, 0) => format!("{n} foto rimesse a posto"),
        (n, r) => format!("{n} foto rimesse a posto, {r} con un nuovo nome"),
    },
    restore_failed: |n, name, err| {
        format!("Impossibile rimettere a posto {n} foto – {name}: {err}")
    },
    deleted_mark: "Eliminata",
    faces_loading: "Ricerca dei volti…",
    faces_unknown: "Non ancora analizzata: i volti arriveranno.",
    faces_none: "Nessun volto rilevato",
    faces_only_small: "Solo volti piccoli: troppo piccoli per giudicare",
    face_eyes_blurry: "Occhi probabilmente sfocati",
    faces_zoom_hint: "Clic per ingrandire questo volto",
    faces_grid_hint: "Un clic o il suo numero (1–9) ingrandisce un volto · Esc chiude",
    cmd_faces: "Volti",
    cmd_face_grid: "Tutti i volti in grande",
    faces_small: |n| {
        format!(
            "+ {n} {}: troppo piccoli per giudicare",
            if n == 1 {
                "volto piccolo"
            } else {
                "volti piccoli"
            }
        )
    },
    face_number: |n| format!("Volto {n}"),
    raw_preview_fact: "Anteprima RAW",
    preview_word: "anteprima",
    edit_failed: |detail| format!("Non scritto: {detail}"),
    edit_hint_straighten: "Rotella o ←/→ ruota, Maiusc più fine · Invio applica, Esc annulla",
    edit_hint_crop: "Frecce spostano · +/− dimensione, Maiusc più fine · A: proporzioni · X: orizzontale/verticale · Invio applica, Esc annulla",
    ratio_original: "Originale",
    crop_landscape: "Orizzontale",
    crop_portrait: "Verticale",

    help_title: "Aiuto",
    help_intro: "Cerno mostra subito le tue foto e ti aiuta a fare una selezione. Stelle e colori finiscono nel file della foto, senza cambiarne la data; tutto il resto resta nel database di Cerno.",
    help_drop: "Trascina una cartella o una foto sulla finestra, oppure premi Ctrl+O.",
    help_close: "Esc, H o F1 chiude questa pagina",
    help_tab_keys: "Scorciatoie",
    help_tab_tips: "Suggerimenti",
    help_pages_hint: "←/→ cambia pagina",
    help_tips: [
        (
            "Selezionare in due passate",
            &[
                "Prima scorrere veloce (Spazio) e rifiutare con X ciò che non va, senza pensarci troppo.",
                "Poi «senza ×» nella barra dei filtri: le rifiutate spariscono, ora valutare da 1 a 5.",
                "Infine Azione › «Elimina le rifiutate» (Ctrl+M).",
            ],
        ),
        (
            "Serie e confronto",
            &[
                "Ordinate per data di scatto, le serie restano insieme, la foto più nitida per prima.",
                "C mostra due foto affiancate; A tiene quella a sinistra, D quella a destra, l'altra viene rifiutata.",
                "M mostra solo le foto simili a quella attuale.",
            ],
        ),
        (
            "Le foto migliori",
            &[
                "Scegliere «Top 50 foto» nella prima casella della barra dei filtri: Cerno propone le migliori, prima una per serie.",
                "Estetica e nitidezza sotto la foto aiutano a decidere; le stelle le dai tu.",
            ],
        ),
        (
            "Etichette colore",
            &[
                "In Cerno i colori non hanno un significato fisso. Due letture comuni:",
                "Avanzamento: rosso da controllare · giallo da ritoccare · verde finito · blu esportato",
                "Uso: rosso cliente · giallo social · verde portfolio · blu stampa",
                "6–9 impostano da rosso a blu; la barra dei filtri mostra un colore.",
            ],
        ),
        (
            "Eliminata non è persa",
            &[
                "Le foto eliminate finiscono nella cartella nascosta .originals accanto alle foto.",
                "La casella del cestino nella barra dei filtri le mostra; Ctrl+Z ne rimette a posto una.",
                "Prima del primo raddrizzamento, ritaglio o rotazione Cerno conserva l'originale; Ctrl+Z lo ripristina.",
            ],
        ),
        (
            "La previsione",
            &[
                "Cerno impara dalle tue stelle e dalle foto rifiutate ed eliminate che cosa ti piace.",
                "Sulle foto senza stelle mostra la sua stima con stelle appena riempite; darle resta compito tuo.",
                "Ordinate per previsione, vengono prima le foto che probabilmente ti piaceranno.",
            ],
        ),
    ],
    welcome_intro: "Guarda, valuta e scarta le foto senza attese: le stelle finiscono nel file, la sua data resta.",
    welcome_keys: [
        ("←, →", "Foto precedente / successiva"),
        ("1 – 5", "Assegna le stelle"),
        ("X", "Rifiuta"),
        ("Canc", "Elimina – Esc la recupera"),
        ("Ctrl+K", "Menu con tutte le funzioni"),
    ],
    welcome_more: "Tutte le scorciatoie: H",
    help_sections: [
        "Sfogliare",
        "Valutare",
        "Selezionare",
        "Video",
        "Visualizzare",
        "Pannelli",
        "Modificare",
        "Altro",
    ],
    help_browse: [
        (
            "→, Spazio, Pag giù",
            "Foto successiva (tieni premuto per scorrere)",
        ),
        ("←, Backspace, Pag su", "Foto precedente"),
        ("Home, Fine", "Prima / ultima foto"),
        (
            "Rotellina",
            "Sopra la striscia di miniature: scorri le foto",
        ),
        ("Ctrl+O", "Apri una cartella (o trascinala sulla finestra)"),
        ("Ctrl+U", "Includi sottocartelle – sì / no"),
    ],
    help_rate: [
        (
            "1 – 5",
            "Assegna le stelle – vengono scritte nel file della foto",
        ),
        (
            "Maiusc+1 – 5",
            "Assegna le stelle e passa alla foto successiva",
        ),
        ("0", "Rimuovi le stelle o il rifiuto"),
        (
            "X, Maiusc+X",
            "Rifiuta – annotato nel file, non viene eliminato nulla; con Maiusc passa alla foto successiva",
        ),
        (
            "6 – 9",
            "Colore: rosso, giallo, verde, blu – di nuovo lo toglie",
        ),
        (
            "Maiusc+6 – 9",
            "Imposta il colore e vai alla foto successiva",
        ),
        ("B", "Descrizione: modifica commento e parole chiave"),
    ],
    help_cull: [
        (
            "C",
            "Confronta: fissa questa foto a sinistra, sfoglia a destra",
        ),
        (
            "A, D",
            "Confronta: tieni la sinistra / la destra – l'altra viene rifiutata e il confronto termina",
        ),
        ("M", "Mostra solo foto simili – di nuovo: tutte"),
        (
            "Canc",
            "Elimina: va nella cartella nascosta .originals dopo 5 secondi – non si perde nulla",
        ),
        ("Esc", "Recupera le foto in attesa di essere eliminate"),
    ],
    help_video: [
        (
            "Spazio",
            "Riproduci / metti in pausa (Maiusc+Spazio: foto successiva)",
        ),
        ("Alt+←, Alt+→", "5 s indietro / avanti"),
        (",, .", "Un fotogramma indietro / avanti (in pausa)"),
        ("↑, ↓", "Volume"),
    ],
    help_view: [
        ("Z, Doppio clic", "Foto intera ↔ 100 %"),
        ("Ctrl+0, Ctrl+1", "Foto intera / 100 %"),
        ("+, −, Rotellina", "Ingrandisci / riduci – anche con Ctrl"),
        ("Trascina", "Sposta la foto ingrandita"),
        (
            "O",
            "Sovrapposizione: bordi nitidi → luci e ombre tagliate → disattivata",
        ),
        (
            "F7",
            "Griglia di tutte le foto: ↑ ↓ una riga, + − dimensione, Invio apre la foto",
        ),
        ("F, F11", "Schermo intero"),
    ],
    help_panels: [
        (
            "T",
            "Barra dei filtri: ordina e filtra – per stelle, colori, nitidezza, persone",
        ),
        ("Tab", "Pannello dettagli"),
        ("F6", "Striscia di miniature"),
        (
            "Maiusc+Tab",
            "Barra dei filtri, dettagli e striscia di miniature insieme",
        ),
        (
            "G, Maiusc+G",
            "Volti: nel pannello dei dettagli (G) o tutti in grande (Maiusc+G); un clic ingrandisce",
        ),
    ],
    help_edit: [
        (
            "S",
            "Raddrizza: griglia, rotellina e frecce ruotano, Maiusc più fine",
        ),
        (
            "R",
            "Ritaglio: traccia una cornice o regolala con le frecce e +/−, A cambia il formato, X scambia orizzontale/verticale",
        ),
        ("Invio, Esc", "Applica o annulla raddrizzamento e ritaglio"),
        (
            "Ctrl+←, Ctrl+→",
            "Ruota di 90° – senza perdita, tramite l'orientamento JPEG",
        ),
        (
            "Ctrl+Z",
            "Recupera l'originale – resta in .originals accanto alla foto; rimetti a posto una foto eliminata",
        ),
        (
            "E",
            "Modifica in un altro programma – quello memorizzato o scegline uno",
        ),
    ],
    help_more: [
        ("Ctrl+K", "Menu: tutte le funzioni"),
        ("Ctrl+M", "Azione: copia, sposta o elimina le foto in vista"),
        ("Ctrl+L", "Cambia lingua"),
        ("H, F1, ?", "Questa guida"),
        (
            "Esc",
            "Torna indietro: zoom, confronto, griglia, schermo intero",
        ),
    ],
};
