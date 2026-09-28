use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('/'),
    compass: ["N", "S", "E", "O"],
    key_ctrl: "Ctrl",
    key_shift: "Maiusc",

    sort: |key| format!("Ordina: {key}"),
    filter_stars: |n| {
        if n == 1 {
            "1 stella".to_owned()
        } else {
            format!("{n} stelle")
        }
    },
    filter_blurry: "Sfocate",
    filter_blurry_tooltip: "Il 20 % delle foto più sfocate di questa cartella",
    filter_duplicate: "Duplicati",
    filter_duplicate_tooltip: "Ogni foto tranne il primo percorso identico",
    filter_clear: "Mostra tutte",
    refresh_order: "Aggiorna ordine",
    refresh_order_tooltip: "Dopo l'ordinamento e il filtro sono arrivati nuovi punteggi",
    analyzing_progress: |done, total| format!("Analisi {done} / {total}"),
    analyzed: |total| format!("{total} analizzate"),
    enable_aesthetics: "Attiva estetica…",
    enable_aesthetics_tooltip: "Scarica una sola volta il modello di immagini CLIP (1.2 GB)",
    downloading_model: |percent| format!("Download del modello {percent:.0} %"),
    aesthetics_ready: "Estetica: pronta",
    aesthetics_loading: "Estetica: caricamento modello…",
    aesthetics_backend: |backend| format!("Estetica: {backend}"),
    aesthetics_backend_tooltip: "Dove viene eseguito il modello estetico (DirectML = scheda grafica)",
    aesthetics_failed: "Estetica: errore",

    sort_name: "Nome",
    sort_rating: "Stelle",
    sort_laion: "Estetica (LAION)",
    sort_v25: "Estetica (V2.5)",
    sort_personal: "Per te",
    sort_sharpness: "Nitidezza",
    sort_taken: "Ora di scatto",
    filter_unrated: "Senza stelle",

    filter_rejected: "Rifiutate",
    actions: "Azione",
    actions_tooltip: "Copia, sposta o elimina le foto in vista",
    selection_delete: "Elimina",
    label_red: "Rosso",
    label_yellow: "Giallo",
    label_green: "Verde",
    label_blue: "Blu",
    label_purple: "Viola",
    meter_aesthetics: "Estetica",
    meter_aesthetics_tooltip: "L e V: estetica (LAION / V2.5). ★: Per te – le stelle che Cerno pensa daresti. Tutte da 0 a 5\n– = non ancora disponibile",
    meter_sharpness: "Nitidezza",
    meter_eyes: "Occhi",
    probably_blurry: "probabilmente sfocata",
    analyzing: "Analisi in corso…",
    saving: "Salvataggio…",
    auto_advance_on: "Avanzamento automatico",
    series_position: |index, len| format!("Serie {index} / {len}"),
    series_more: |n| format!("+{n}"),
    duplicate_of: |name| format!("Duplicato di {name}"),
    rejected: "Rifiutata",
    star_tooltip: |n| format!("{n} ★ – tasto {n}"),
    zoom: |percent| format!("Zoom {percent:.0} %"),
    digital_zoom: |ratio| format!("Zoom digitale {ratio:.1}×"),
    map_tooltip: |place| format!("{place}\nClicca per aprire in Google Maps"),
    button_toolbar: "Ordina e filtra",
    button_details: "Pannello dettagli",
    button_filmstrip: "Striscia di miniature",
    button_help: "Aiuto",
    button_menu: "Menu",
    button_language: |name| format!("Lingua: {name}"),

    cmd_explanations: "Espandi o comprimi tutte le spiegazioni nel pannello dettagli",
    cmd_all_panels: "Ordina e filtra, dettagli e striscia di miniature",
    cmd_fullscreen: "Schermo intero",
    cmd_compare: "Confronta",
    cmd_zoom: "Foto intera ↔ 100 %",
    cmd_first: "Prima foto",
    cmd_last: "Ultima foto",
    cmd_language: |name| format!("Lingua: {name}"),
    cmd_reject: "Rifiuta",
    cmd_delete_rejected: |n| format!("Elimina le foto rifiutate ({n})"),
    cmd_auto_advance: "Avanza automaticamente",
    cmd_subfolders: "Includi sottocartelle",
    cmd_best_of_series: "La migliore di ogni serie",
    cmd_label: |name| format!("Colore: {name}"),
    menu_sort: "Ordina",
    menu_filter: "Filtro",
    menu_view: "Vista",
    menu_edit: "Modifica",
    menu_photo: "Foto",
    menu_labels: "Colori",
    menu_language: "Lingua",
    menu_models: "Modelli e dati",
    loading: "Caricamento…",
    cannot_show: "Impossibile mostrare questa immagine",
    no_match: "Nessuna foto corrisponde al filtro",
    drop_to_open: "Rilascia per aprire",
    compare_left: "Sinistra",
    compare_right: "Destra",
    compare_left_badge: "S",
    keeps_this: |key| format!("{key} tiene questa"),
    compare_needs_two: "Per confrontare servono almeno due foto",
    deleting: |n| format!("Eliminazione di {n} foto   ·   Esc per annullare"),
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
    transfer_copy: "Copia",
    transfer_move: "Sposta",
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
    section_histogram: "Istogramma",
    section_file: "File",
    row_size: "Dimensioni",
    row_load_time: "Tempo di caricamento",
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
    explain_laion: "Quanto è bella la foto per un'IA addestrata su molte valutazioni di persone, convertito in stelle da 0 a 5. La maggior parte delle foto ottiene 2–3, da 4 in su è molto buona.",
    explain_v25: "Un'IA più recente per la stessa domanda, migliore con le foto di tutti i giorni. Stessa scala di stelle.",
    explain_personal: "Le stelle che secondo Cerno daresti tu. Impara dalle tue stelle e dalle foto che elimini.",
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
    model_failed: "errore",
    model_faces: "Volti",
    model_personal: "Per te",
    taste_trained: |n, error| format!("{n} foto, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} foto"),
    taste_untrained: "non ancora addestrato",
    btn_reset_taste: "Reimposta Per te",
    btn_delete_models: "Elimina modelli",
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
    edit_not_jpeg: "Raddrizza e ritaglio solo per JPEG.",
    edit_writing: "Scrittura della foto…",
    edit_reencoded: "JPEG ricodificato. La qualità è stata impostata di nuovo una volta.",
    edit_failed: |detail| format!("Non scritto: {detail}"),
    edit_hint: "Invio applica, Esc annulla",
    ratio_original: "Originale",
    crop_landscape: "Orizzontale",
    crop_portrait: "Verticale",

    help_title: "Aiuto",
    help_intro: "Cerno mostra subito le tue foto e ti aiuta a fare una selezione. Le stelle vengono scritte direttamente nel file della foto, così le vedono anche gli altri programmi – la data del file non cambia. Tutto il resto resta nel database di Cerno. Nitidezza e bellezza vengono valutate automaticamente in background; Tab apre il pannello dettagli, I espande o comprime tutte le spiegazioni, Ctrl+K apre il menu.",
    help_drop: "Trascina una cartella o una foto sulla finestra, oppure premi Ctrl+O.",
    help_close: "Esc, H o F1 chiude questa pagina",
    help_sections: [
        "Sfogliare",
        "Valutare e selezionare",
        "Visualizzare",
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
        ("Canc", "Elimina: va nel cestino dopo 5 secondi"),
        ("Esc", "Recupera le foto in attesa di essere eliminate"),
        (
            "C",
            "Confronta: fissa questa foto a sinistra, sfoglia a destra",
        ),
        (
            "A, D",
            "Confronta: tieni la sinistra / la destra – l'altra viene rifiutata",
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
        ("F11", "Schermo intero"),
        ("F", "Ordina e filtra"),
        ("Tab", "Pannello dettagli"),
        ("I", "Dettagli: espandi o comprimi tutte le spiegazioni"),
        ("F6", "Striscia di miniature"),
        (
            "Maiusc+Tab",
            "Ordina e filtra, dettagli e striscia di miniature insieme",
        ),
        (
            "S",
            "Raddrizza: griglia, rotellina e frecce ruotano, Maiusc più fine, Invio applica",
        ),
        (
            "Ctrl+←, Ctrl+→",
            "Ruota di 90° – senza perdita, tramite l'orientamento JPEG",
        ),
        (
            "R",
            "Ritaglio: traccia una cornice, X scambia orizzontale/verticale, A cambia il formato",
        ),
        ("Invio, Esc", "Applica o annulla raddrizzamento e ritaglio"),
    ],
    help_more: [
        ("Ctrl+K", "Menu: tutte le funzioni"),
        ("Ctrl+M", "Azione: copia, sposta o elimina le foto in vista"),
        ("Ctrl+L", "Cambia lingua"),
        ("H, F1, ?", "Questa guida"),
        ("Esc", "Torna indietro: zoom, confronto, schermo intero"),
    ],
};
