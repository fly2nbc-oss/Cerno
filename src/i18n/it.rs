use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('/'),
    compass: ["N", "S", "E", "O"],
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
    filter_clear: "Mostra tutte",
    media_all: "Foto e video",
    media_photos: "Solo foto",
    media_videos: "Solo video",
    media_no_videos: "Questa cartella non contiene video",
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
    enable_aesthetics_tooltip: "Scarica una sola volta il modello di immagini CLIP (1.2 GB)",
    downloading_model: |percent| format!("Download del modello {percent:.0} %"),
    aesthetics_loading: "Estetica: caricamento modello…",
    aesthetics_failed: "Estetica: errore",

    sort_name: "Nome",
    sort_rating: "Stelle",
    sort_aesthetics: "Estetica",
    sort_personal: "Per te",
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
            "Per te: {stars:.1} ★ – le stelle che Cerno pensa daresti. Non è ancora il tuo voto"
        )
    },
    zoom: |percent| format!("Zoom {percent:.0} %"),
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
    label_none: "Nessun colore",
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
    download_title: "Attiva la valutazione estetica",
    download_text: |gb| {
        format!(
            "Per valutare l'estetica, Cerno ha bisogno del modello di immagini CLIP ViT-L/14.\n\n\
             Vuoi scaricarlo ora da Hugging Face (Xenova/clip-vit-large-patch14, {gb:.1} GB)? \
             Viene salvato nella cartella dati di Cerno e scaricato una sola volta."
        )
    },
    btn_download: "Scarica",
    btn_cancel: "Annulla",
    btn_close: "Chiudi (Esc)",
    aesthetics_offer: "La valutazione estetica richiede un modello (1.2 GB): Menu → Modelli e dati.",

    section_aesthetics: "Estetica",
    section_sharpness: "Nitidezza (nella cartella)",
    section_exposure: "Esposizione",
    section_attributes: "Caratteristiche CLIP",
    row_aesthetics: "Estetica (media)",
    section_histogram: "Istogramma",
    section_file: "File",
    row_size: "Dimensioni",
    row_load_time: "Tempo di caricamento",
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
    row_personal: "Per te",
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
    explain_models: "Dove viene eseguita ogni IA: DirectML = scheda grafica, CPU = processore. ± indica di quanto «Per te» di solito sbaglia.",
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
    model_personal: "Per te",
    taste_trained: |n, error| format!("{n} foto, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} foto"),
    taste_untrained: "non ancora addestrato",
    btn_reset_taste: "Reimposta Per te",
    btn_delete_models: "Elimina modelli",
    models_deleted: "Modelli eliminati",
    copy_models_path: "Copia percorso",
    models_path_copied: "Percorso copiato",
    confirm_reset_taste_title: "Reimpostare Per te?",
    confirm_reset_taste_text: "Cerno dimenticherà ciò che ha imparato dalle tue stelle e dalle eliminazioni. Le stelle nei file foto restano invariate.",
    confirm_delete_models_title: "Eliminare i modelli scaricati?",
    confirm_delete_models_text: "Rimuove i file dei modelli CLIP e SigLIP dal disco (circa 3 GB). I punteggi salvati restano nel database; l'estetica può essere scaricata di nuovo.",

    cmd_straighten: "Raddrizza",
    cmd_rotate_ccw: "Ruota di 90° a sinistra",
    cmd_rotate_cw: "Ruota di 90° a destra",
    cmd_crop: "Ritaglio",
    cmd_undo: "Annulla modifica",
    edit_not_jpeg: "Raddrizza, ritaglio, rotazione e Ctrl+Z solo per JPEG.",
    video_play_hint: "Riproduci (Invio)",
    video_no_ffmpeg: "Nessuna anteprima: a Cerno serve ffmpeg (ad es. winget install Gyan.FFmpeg)",
    video_play_failed: |err| format!("Impossibile riprodurre il video: {err}"),
    video_played_instead: |ext, name| {
        format!(
            "Windows apre i file .{ext} con Cerno – il video parte quindi in {name}. Cambia il lettore predefinito in Impostazioni di Windows › App predefinite."
        )
    },
    video_choose_player: |ext| {
        format!(
            "Windows apre i file .{ext} con Cerno – scegli un lettore. Cambia il lettore predefinito in Impostazioni di Windows › App predefinite."
        )
    },
    edit_writing: "Scrittura della foto…",
    edit_cancelled: "È visualizzata un'altra foto – modifica annullata",
    busy_editing: "La modifica è ancora aperta – Invio applica, Esc annulla",
    busy_copying: "La foto è in fase di copia – di nuovo possibile tra poco",
    busy_moving: "La foto è in fase di spostamento",
    edit_needs_index: "La modifica richiede l'indice, che non è stato possibile aprire",
    edit_reencoded: "JPEG ricodificato – Ctrl+Z ripristina l'originale.",
    undo_done: "Originale ripristinato",
    undo_nothing: "Nessun originale conservato per questa foto",
    edit_failed: |detail| format!("Non scritto: {detail}"),
    edit_hint_straighten: "Rotella o ←/→ ruota, Maiusc più fine · Invio applica, Esc annulla",
    edit_hint_crop: "A: proporzioni · X: orizzontale/verticale · Invio applica, Esc annulla",
    ratio_original: "Originale",
    crop_landscape: "Orizzontale",
    crop_portrait: "Verticale",

    help_title: "Aiuto",
    help_intro: "Cerno mostra subito le tue foto e ti aiuta a fare una selezione. Stelle e colori finiscono nel file della foto, senza cambiarne la data; tutto il resto resta nel database di Cerno.",
    help_drop: "Trascina una cartella o una foto sulla finestra, oppure premi Ctrl+O.",
    help_close: "Esc, H o F1 chiude questa pagina",
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
        "Valutare e selezionare",
        "Visualizzare",
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
            "Canc",
            "Elimina: va nella cartella nascosta .originals dopo 5 secondi – non si perde nulla",
        ),
        ("Esc", "Recupera le foto in attesa di essere eliminate"),
        (
            "C",
            "Confronta: fissa questa foto a sinistra, sfoglia a destra",
        ),
        (
            "A, D",
            "Confronta: tieni la sinistra / la destra – l'altra viene rifiutata e il confronto termina",
        ),
        (
            "6 – 9",
            "Colore: rosso, giallo, verde, blu – di nuovo lo toglie",
        ),
        (
            "Maiusc+6 – 9",
            "Imposta il colore e vai alla foto successiva",
        ),
    ],
    help_view: [
        ("Z, Doppio clic", "Foto intera ↔ 100 %"),
        ("+, −, Rotellina", "Ingrandisci / riduci"),
        ("Trascina", "Sposta la foto ingrandita"),
        ("F, F11", "Schermo intero"),
        ("T", "Barra dei filtri: ordina e filtra"),
        ("Tab", "Pannello dettagli"),
        ("F6", "Striscia di miniature"),
        ("M", "Mostra solo foto simili – di nuovo: tutte"),
        (
            "O",
            "Sovrapposizione: bordi nitidi → luci e ombre tagliate → disattivata",
        ),
        (
            "F7",
            "Griglia di tutte le foto: ↑ ↓ una riga, + − dimensione, Invio apre la foto",
        ),
        (
            "Maiusc+Tab",
            "Barra dei filtri, dettagli e striscia di miniature insieme",
        ),
        ("Invio", "Riproduci un video nel lettore predefinito"),
        ("B", "Descrizione: modifica commento e parole chiave"),
    ],
    help_edit: [
        (
            "S",
            "Raddrizza: griglia, rotellina e frecce ruotano, Maiusc più fine",
        ),
        (
            "R",
            "Ritaglio: traccia una cornice, A cambia il formato, X scambia orizzontale/verticale",
        ),
        ("Invio, Esc", "Applica o annulla raddrizzamento e ritaglio"),
        (
            "Ctrl+←, Ctrl+→",
            "Ruota di 90° – senza perdita, tramite l'orientamento JPEG",
        ),
        (
            "Ctrl+Z",
            "Recupera l'originale – resta in .originals accanto alla foto",
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
