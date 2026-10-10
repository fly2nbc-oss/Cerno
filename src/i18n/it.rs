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
    filter_incomplete: "Incomplete",
    filter_incomplete_tooltip: "JPEG il cui file finisce prima dell'immagine – troncato durante una copia o un download; la parte mancante è grigia",
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
    align_camera_hint: "Sposta l'ora di scatto di tutte le foto della fotocamera destra in questa cartella, così che la foto a destra risulti scattata nello stesso momento di quella a sinistra. Solo nell'indice: i file mantengono la loro ora.",
    align_needs_compare: "Prima confronta due foto di fotocamere diverse (C)",
    align_no_time: "A una delle due foto manca la fotocamera o l'ora di scatto",
    align_same_camera: "Le due foto vengono dalla stessa fotocamera",
    menu_camera_time: "Ora della fotocamera …",
    camera_time_title: "Ora della fotocamera",
    camera_time_intro: "Se l'orologio di una fotocamera era sbagliato, uno scarto sposta l'ora di scatto di tutte le sue foto in questa cartella, per l'ordinamento per ora di scatto, le serie e la visualizzazione. Solo nell'indice; i file mantengono la loro ora.",
    camera_time_reset: "Reimposta",
    camera_time_invalid: "Scarto come +h:mm:ss o −h:mm:ss, ad es. +1:30:00",
    camera_time_apply: "Applica",
    camera_time_none: "Ancora nessuna foto con fotocamera e ora di scatto in questa cartella.",
    camera_time_applied: "Ora della fotocamera applicata",
    camera_aligned: |camera, offset| format!("{camera}: ora di scatto spostata di {offset}"),
    camera_time_photos: |n| format!("{n} foto"),
    camera_time_tooltip: |taken, offset| {
        format!("Ora della fotocamera {taken}, corretta di {offset}")
    },
    cmd_pairs: "RAW+JPG come una sola foto",
    pairs_hint: "Un RAW e un JPG con lo stesso nome appaiono come una sola foto: si vede il JPG. Stelle, colore e descrizione vanno in entrambi i file, copia, sposta ed elimina li prendono entrambi; le modifiche cambiano solo il JPG.",
    cmd_update_check: "Cerca aggiornamenti",
    update_check_hint: "Una richiesta al giorno a GitHub – niente su di te o sulle tue foto",
    update_available: |v| format!("Cerno {v} è disponibile – Aiuto › Informazioni"),
    cmd_update_download: |v| format!("Scarica Cerno {v} …"),
    update_ask_title: "Cercare aggiornamenti?",
    update_ask_text: "Cerno deve controllare una volta al giorno se c'è una nuova versione? Non viene inviato nulla su di te o sulle tue foto. Puoi cambiarlo più tardi nelle Impostazioni.",
    btn_update_yes: "Sì",
    btn_update_no: "No",
    pairs_on: "RAW+JPG appaiono come una sola foto",
    pairs_off: "RAW e JPG appaiono separati",
    pair_badge: "RAW+JPG",
    pair_edit_jpeg_only: "Modifica solo il JPG: il RAW resta intatto.",
    pair_raw_marks: |marks| format!("RAW: {marks}"),
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
    bulk_delete_top: "Non con Top – sono le foto migliori",
    filter_similar: "≈ Simili",
    filter_similar_to: |name| format!("≈ come {name}"),
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
    photos_badge_tooltip: "Quante foto mostra ora il filtro – «Foto visibili» nella barra dei menu agisce proprio su queste.",
    label_red: "Rosso",
    label_yellow: "Giallo",
    label_green: "Verde",
    label_blue: "Blu",
    label_purple: "Viola",
    meter_aesthetics_tooltip: "Estetica: media di LAION e V2.5 su una scala fissa – una foto ha lo stesso valore in ogni cartella. La nitidezza invece confronta con le altre foto della cartella.\nValori separati: pannello dei dettagli (Tab)",
    meter_sharpness: "Nitidezza",
    meter_eyes: "Occhi",
    probably_blurry: "probabilmente sfocata",
    incomplete_fact: "File incompleto",
    incomplete_tooltip: "Il file finisce prima dell'immagine – la parte mancante è grigia. Controlla l'originale, ad esempio sulla scheda di memoria.",
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
    view_photo: "Foto",
    view_grid: "Griglia",
    view_faces: "Volti",
    faces_video: "Nei video non si cercano volti",
    filmstrip_in_grid: "Non serve nella griglia – F7 torna alla foto",
    faces_button: |n| {
        if n == 1 {
            "Mostra 1 volto in grande (G)".to_owned()
        } else {
            format!("Mostra {n} volti in grande (G)")
        }
    },
    button_side_bar: "Barra dei menu",
    bar_rotate: "Ruota",
    bar_colour: "Colore",
    bar_stars: "Stelle",
    bar_align_camera: "Allinea fotocamere",
    bar_overlay_sharpness: "Nitidezza",
    bar_overlay_exposure: "Esposizione",
    bar_none_rejected: "Nessuna foto della cartella è rifiutata.",
    bar_none_deleted: "Nessuna foto eliminata mostrata: la casella del cestino nella barra dei filtri le mostra.",
    button_language: |name| format!("Lingua: {name}"),

    cmd_fullscreen: "Schermo intero",
    cmd_compare: "Confronta",
    cmd_quad: "Vista a quattro",
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
    cmd_reject: "Rifiuta",
    cmd_delete_rejected: |n| format!("Elimina le rifiutate ({n} foto)"),
    cmd_auto_advance: "Avanza automaticamente",
    cmd_subfolders: "Includi sottocartelle",
    subfolders_on: "Sottocartelle incluse",
    subfolders_off: "Solo questa cartella, senza sottocartelle",
    menu_view: "Vista",
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
    writer_stopped: "I contrassegni non vengono più scritti nei file – errore interno (crash.log nella cartella dei dati). Riavvia Cerno.",
    internal_error: "Errore interno – il comando è stato interrotto, Cerno continua (dettagli in crash.log nella cartella dei dati).",
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
            "La valutazione estetica richiede modelli di immagini ({size}): Barra dei menu › Impostazioni › Modelli e dati."
        )
    },
    exiftool_offer: |size| {
        format!(
            "Cerno salva stelle e colori con ExifTool ({size}): Barra dei menu › Impostazioni › Modelli e dati."
        )
    },
    exiftool_title: "Scarica ExifTool",
    exiftool_text: |size| {
        format!(
            "Cerno scrive stelle, colori, commenti e modifiche nelle foto con ExifTool, lo strumento libero di Phil Harvey.\n\nScaricarlo ora dalla fonte ufficiale (SourceForge, {size})? Va nella cartella dati di Cerno; un download interrotto riprende la volta successiva."
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
    row_complete: "Completo",
    incomplete_value: "no – manca la fine",
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
    explain_personal: "Le stelle che secondo Cerno daresti tu. Impara dalle tue stelle e dalle foto che rifiuti.",
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
    exiftool_found: |version, downloaded| {
        let from = if downloaded {
            "scaricato da Cerno"
        } else {
            "installato"
        };
        if version.is_empty() {
            from.to_owned()
        } else {
            format!("{version} · {from}")
        }
    },
    exiftool_absent: "mancante: senza ExifTool niente stelle né colori",
    exiftool_old_state: |version| format!("{version}: troppo vecchio (serve 12.24 o successivo)"),
    exiftool_downloading: |percent| format!("download in corso … {percent:.0} %"),
    btn_exiftool: |size| format!("Scarica ExifTool ({size})"),
    taste_sources: |stars, rejected| {
        format!(
            "Appresa da {stars} foto con stelle e {rejected} rifiutate – le foto rifiutate contano come 0 ★, quelle eliminate per niente."
        )
    },
    btn_reset_taste: "Reimposta la previsione",
    btn_delete_models: "Elimina modelli",
    models_deleted: "Modelli eliminati",
    models_downloaded: "Modelli scaricati: ora viene calcolata l'estetica",
    download_failed: |err| {
        format!("Download non riuscito: {err}. Un nuovo tentativo riprende da dove si era fermato")
    },
    exiftool_ready: "ExifTool è pronto: ora stelle e colori vengono salvati",
    exiftool_failed: |err| {
        format!("Impossibile scaricare ExifTool: {err}. Un nuovo tentativo riprende da lì")
    },
    copy_path: "Copia percorso",
    path_copied: "Percorso copiato",
    confirm_reset_taste_title: "Reimpostare la previsione?",
    confirm_reset_taste_text: "Cerno dimenticherà ciò che ha imparato dalle tue stelle e dalle foto rifiutate. Le stelle nei file foto restano invariate.",
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
    video_play_failed: |err| format!("Impossibile riprodurre il video: {err}"),
    edit_writing: "Scrittura della foto…",
    edit_cancelled: "È visualizzata un'altra foto – modifica annullata",
    busy_editing: "La modifica è ancora aperta – Invio applica, Esc annulla",
    busy_copying: "La foto è in fase di copia – di nuovo possibile tra poco",
    busy_moving: "La foto è in fase di spostamento",
    busy_deleted: "Foto eliminata: rimettila prima a posto (Ctrl+Z)",
    edit_needs_index: "La modifica richiede l'indice, che non è stato possibile aprire",
    exiftool_missing: "Stelle, colori e modifiche richiedono ExifTool: Barra dei menu › Impostazioni › Modelli e dati",
    exiftool_too_old: "L'ExifTool installato è troppo vecchio (serve 12.24 o successivo): Barra dei menu › Impostazioni › Modelli e dati",
    exiftool_loading: "Download di ExifTool in corso: tra poco si potranno assegnare stelle e colori",
    exiftool_install: |command| {
        match command {
        Some(command) => format!("Stelle, colori e modifiche richiedono ExifTool. Installalo con: {command}"),
        None => "Stelle, colori e modifiche richiedono ExifTool. Installalo con il gestore di pacchetti (pacchetto «exiftool» o «perl-image-exiftool»).".to_owned(),
    }
    },
    edit_reencoded: "JPEG ricodificato – Ctrl+Z ripristina l'originale.",
    undo_done: "Originale ripristinato",
    undo_nothing: "Nessun originale conservato per questa foto",
    undo_mark_row: |what, name| format!("Annulla {what} – {name}"),
    undo_what_stars: "le stelle",
    undo_what_reject: "il rifiuto",
    undo_what_colour: "il colore",
    undo_what_edit: "la modifica",
    undo_mark_done: |name, value| format!("Annullato – {name}: {value}"),
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
    faces_zoom_hint: "Clic per ingrandire questo volto",
    faces_grid_hint: "Un clic o il suo numero (1–9) ingrandisce un volto · Esc chiude",
    cmd_face_grid: "Tutti i volti in grande",
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
    help_tab_about: "Informazioni",
    about_intro: "Cerno mostra le foto senza attese e aiuta a selezionarle, in locale, senza account né cloud. Software libero: il codice sorgente è aperto.",
    about_version: "Versione",
    about_license: "Licenza",
    about_source: "Codice sorgente",
    about_bugs: "Segnala un problema",
    about_bugs_text: "Qualcosa non funziona? Descrivilo in una issue su GitHub: serve un account GitHub gratuito. Versione e sistema sono già compilati; foto e nomi di file non servono. Se Cerno si è chiuso all'improvviso, allega il file crash.log della cartella dati.",
    about_bug_link: "Segnalalo su GitHub",
    about_open_data: "Apri la cartella dati",
    about_wishes: "Idee",
    about_wishes_text: "Un'idea che renda la selezione con Cerno più semplice o più veloce? Scrivila come proposta, sempre con un account GitHub.",
    about_wish_link: "Proponila su GitHub",
    about_third_party: "Terze parti",
    about_third_party_text: "Cerno usa librerie libere di altri, tra cui libheif e GStreamer con FFmpeg (LGPL), e su richiesta scarica ExifTool e i modelli di estetica (V2.5 sotto AGPL). Le loro licenze accompagnano il programma.",
    about_third_party_link: "Tutte le terze parti e le licenze",
    about_updates: "Aggiornamenti",
    about_updates_text: "Se lo permetti, Cerno chiede a GitHub una volta al giorno se esiste una versione più recente – non viene inviato nulla su di te o sulle tue foto, e nulla viene scaricato o installato. Si attiva o disattiva in Impostazioni › Cerca aggiornamenti. I modelli ed ExifTool vengono scaricati solo se lo chiedi.",
    update_off: "La ricerca di aggiornamenti è disattivata.",
    update_never: "Non ancora controllato.",
    update_checking: "Controllo in corso …",
    update_current: "Cerno è aggiornato.",
    update_failed: "GitHub non è raggiungibile – riprova più tardi.",
    update_newer: |v| format!("Cerno {v} è disponibile."),
    update_check_now: "Controlla ora",
    help_tips: [
        (
            "Selezionare in due passate",
            &[
                "Prima scorrere veloce (Spazio) e rifiutare con X ciò che non va, senza pensarci troppo.",
                "Poi «senza ×» nella barra dei filtri: le rifiutate spariscono, ora valutare da 1 a 5.",
                "Infine barra dei menu (F10) › Foto visibili › «Elimina le rifiutate».",
            ],
        ),
        (
            "Serie e confronto",
            &[
                "Ordinate per data di scatto, le serie restano insieme, la foto più nitida per prima.",
                "C mostra due foto affiancate; A tiene quella a sinistra, D quella a destra, l'altra viene rifiutata.",
                "M mostra solo le foto simili a quella attuale.",
                "Due fotocamere con l'ora diversa: confronta due foto scattate nello stesso momento, poi Questa foto › «Allinea la fotocamera destra alla sinistra».",
                "Maiusc+C mostra quattro foto insieme: la cornice è la foto attuale, ↑ ↓ la spostano di una riga.",
            ],
        ),
        (
            "Le foto migliori",
            &[
                "Scegliere «Top 50 foto» nella prima casella della barra dei filtri: Cerno propone le migliori, prima una per serie.",
                "Estetica e nitidezza sotto la foto aiutano a decidere; le stelle le dai tu.",
                "Filtro › Per elenco di file …: incolla i nomi scelti da un cliente – restano solo quelle foto.",
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
                "Cerno impara dalle tue stelle e dalle foto rifiutate che cosa ti piace; quelle eliminate non contano: spesso era solo una di troppo tra tante simili.",
                "Sulle foto senza stelle mostra la sua stima con stelle appena riempite; darle resta compito tuo.",
                "Ordinate per previsione, vengono prima le foto che probabilmente ti piaceranno.",
            ],
        ),
        (
            "Buono a sapersi",
            &[
                "La rotellina sopra la striscia di miniature scorre le foto; sopra la foto ingrandisce e riduce.",
                "Trascina una cartella sulla finestra per aprirla. Tieni premuto → per scorrere le foto.",
                "Nella griglia (F7) ↑ ↓ cambiano riga, + − la dimensione, Invio apre la foto.",
                "Raddrizza (S): rotellina e frecce ruotano, Maiusc più fine. Ritaglio (R): traccia una cornice o spostala con le frecce, +/− ne cambia la dimensione, A il formato, X scambia orizzontale/verticale.",
                "La barra dei menu (F10 o il pulsante in basso a destra) raccoglie ciò che agisce su questa foto, sulle foto visibili e sulla vista, più le impostazioni – tutto con il mouse.",
                "Nella descrizione, Invio porta il cursore nel campo delle parole chiave.",
            ],
        ),
    ],
    welcome_intro: "Guarda, valuta e scarta le foto senza attese: le stelle finiscono nel file, la sua data resta.",
    welcome_keys: [
        ("←, →", "Foto precedente / successiva"),
        ("1 – 5", "Assegna le stelle"),
        ("X", "Rifiuta"),
        ("Canc", "Elimina – Esc la recupera"),
        ("F10", "Barra dei menu (in basso a destra)"),
    ],
    welcome_more: "Tutte le scorciatoie: H",
    setup_line: |exiftool, aesthetics| {
        format!(
            "Configurazione: ExifTool {} · estetica {} – Barra dei menu (F10) › Impostazioni › Modelli e dati",
            if exiftool { "pronto" } else { "manca" },
            if aesthetics { "pronta" } else { "manca" },
        )
    },
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
        ("→, Spazio, Pag giù", "Foto successiva"),
        ("←, Backspace, Pag su", "Foto precedente"),
        ("Home, Fine", "Prima / ultima foto"),
        ("Ctrl+O", "Apri una cartella"),
        ("Ctrl+U", "Sottocartelle sì / no"),
    ],
    help_rate: [
        ("1 – 5", "Stelle"),
        ("0", "Senza stelle, non rifiutata"),
        ("X", "Rifiuta"),
        ("6 – 9", "Rosso, giallo, verde, blu"),
        ("Maiusc+…", "Idem, poi foto successiva"),
        ("Ctrl+Z", "Annulla"),
    ],
    help_cull: [
        ("C", "Confronta due foto"),
        ("A, D", "Tieni sinistra / destra"),
        ("Maiusc+C", "Quattro foto insieme"),
        ("M", "Solo foto simili"),
        ("Canc", "Elimina (in .originals)"),
        ("Esc", "Recupera le eliminate"),
    ],
    help_video: [
        ("Spazio", "Riproduci / pausa"),
        ("Maiusc+Spazio", "Foto successiva"),
        ("Alt+←, Alt+→", "5 s indietro / avanti"),
        (",, .", "Un fotogramma indietro / avanti"),
        ("↑, ↓", "Volume"),
    ],
    help_view: [
        ("Z, Doppio clic", "Foto intera ↔ 100 %"),
        ("Ctrl+0, Ctrl+1", "Foto intera / 100 %"),
        ("+, −, Rotellina", "Ingrandisci / riduci"),
        ("Trascina", "Sposta la foto ingrandita"),
        ("O", "Controllo: nitidezza, tagli"),
        ("F7", "Griglia di tutte le foto"),
        ("G", "Tutti i volti in grande"),
        ("F, F11", "Schermo intero"),
    ],
    help_panels: [
        ("F10", "Barra dei menu"),
        ("T", "Barra dei filtri"),
        ("Tab", "Pannello dettagli"),
        ("F6", "Striscia di miniature"),
        ("Maiusc+Tab", "Tutti e quattro i pannelli"),
        ("Ctrl+Tab", "Scheda successiva dei dettagli"),
    ],
    help_edit: [
        ("S", "Raddrizza"),
        ("R", "Ritaglio"),
        ("Invio, Esc", "Applica / annulla"),
        ("Ctrl+←, Ctrl+→", "Ruota di 90°"),
        ("E", "Modifica in un altro programma"),
    ],
    help_more: [
        ("Ctrl+L", "Lingua"),
        ("H, F1, ?", "Questa guida"),
        ("Esc", "Torna indietro"),
    ],
};
