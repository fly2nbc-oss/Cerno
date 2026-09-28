use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('/'),
    compass: ["N", "S", "E", "O"],
    key_ctrl: "Ctrl",
    key_shift: "Mayús",

    sort: |key| format!("Orden: {key}"),
    filter_stars: |n| {
        if n == 1 {
            "1 estrella".to_owned()
        } else {
            format!("{n} estrellas")
        }
    },
    filter_blurry: "Borrosas",
    filter_blurry_tooltip: "Las fotos más borrosas (el 20 % de esta carpeta)",
    filter_duplicate: "Duplicados",
    filter_duplicate_tooltip: "Cada foto salvo la primera ruta idéntica",
    filter_clear: "Mostrar todas",
    refresh_order: "Actualizar orden",
    refresh_order_tooltip: "Hay puntuaciones nuevas desde que ordenaste y filtraste",
    analyzing_progress: |done, total| format!("Analizando {done} / {total}"),
    analyzed: |total| format!("{total} analizadas"),
    enable_aesthetics: "Activar estética…",
    enable_aesthetics_tooltip: "Descarga una sola vez el modelo de imagen CLIP (1.2 GB)",
    downloading_model: |percent| format!("Descargando modelo {percent:.0} %"),
    aesthetics_ready: "Estética: lista",
    aesthetics_loading: "Estética: cargando modelo…",
    aesthetics_backend: |backend| format!("Estética: {backend}"),
    aesthetics_backend_tooltip: "Dónde se ejecuta el modelo de estética (DirectML = tarjeta gráfica)",
    aesthetics_failed: "Estética: error",

    sort_name: "Nombre",
    sort_rating: "Estrellas",
    sort_laion: "Estética (LAION)",
    sort_v25: "Estética (V2.5)",
    sort_personal: "Para ti",
    sort_sharpness: "Nitidez",
    sort_taken: "Hora de captura",
    filter_unrated: "Sin estrellas",

    filter_rejected: "Rechazadas",
    actions: "Acción",
    actions_tooltip: "Copiar, mover o eliminar las fotos en pantalla",
    selection_delete: "Eliminar",
    label_red: "Rojo",
    label_yellow: "Amarillo",
    label_green: "Verde",
    label_blue: "Azul",
    label_purple: "Morado",
    meter_aesthetics: "Estética",
    meter_aesthetics_tooltip: "L y V: estética (LAION / V2.5). ★: Para ti – las estrellas que Cerno cree que darías. Todas de 0 a 5\n– = aún no disponible",
    meter_sharpness: "Nitidez",
    meter_eyes: "Ojos",
    probably_blurry: "probablemente borrosa",
    analyzing: "Analizando…",
    saving: "Guardando…",
    auto_advance_on: "Avance automático",
    series_position: |index, len| format!("Serie {index} / {len}"),
    series_more: |n| format!("+{n}"),
    duplicate_of: |name| format!("Duplicado de {name}"),
    rejected: "Rechazada",
    star_tooltip: |n| format!("{n} ★ – tecla {n}"),
    zoom: |percent| format!("Zoom {percent:.0} %"),
    digital_zoom: |ratio| format!("Zoom digital {ratio:.1}×"),
    map_tooltip: |place| format!("{place}\nHaz clic para abrirlo en Google Maps"),
    button_toolbar: "Ordenar y filtrar",
    button_details: "Panel de detalles",
    button_filmstrip: "Tira de miniaturas",
    button_help: "Ayuda",
    button_menu: "Menú",
    button_language: |name| format!("Idioma: {name}"),

    cmd_explanations: "Desplegar o plegar todas las explicaciones del panel de detalles",
    cmd_all_panels: "Ordenar y filtrar, detalles y tira de miniaturas",
    cmd_fullscreen: "Pantalla completa",
    cmd_compare: "Comparar",
    cmd_zoom: "Foto completa ↔ 100 %",
    cmd_reject: "Rechazar",
    cmd_delete_rejected: |n| format!("Eliminar las fotos rechazadas ({n})"),
    cmd_auto_advance: "Avanzar automáticamente",
    cmd_subfolders: "Incluir subcarpetas",
    cmd_best_of_series: "La mejor de cada serie",
    menu_sort: "Ordenar",
    menu_filter: "Filtro",
    menu_view: "Vista",
    menu_edit: "Editar",
    menu_photo: "Foto",
    menu_labels: "Colores",
    menu_language: "Idioma",
    menu_models: "Modelos y datos",
    loading: "Cargando…",
    cannot_show: "No se puede mostrar esta imagen",
    no_match: "Ninguna foto coincide con el filtro",
    drop_to_open: "Suelta para abrir",
    compare_left: "Izquierda",
    compare_right: "Derecha",
    compare_left_badge: "I",
    keeps_this: |key| format!("{key} conserva esta"),
    compare_needs_two: "Para comparar hacen falta al menos dos fotos",
    deleting: |n| {
        if n == 1 {
            "Eliminando 1 foto   ·   Esc para deshacer".to_owned()
        } else {
            format!("Eliminando {n} fotos   ·   Esc para deshacer")
        }
    },
    delete_failed: |n, name, err| format!("No se pudieron eliminar {n} foto(s) – {name}: {err}"),
    blurry_tooltip: |eyes, percent| {
        let what = if eyes {
            "los ojos solo son más nítidos"
        } else {
            "la foto solo es más nítida"
        };
        format!("Probablemente borrosa: {what} que el {percent:.0} % de esta carpeta")
    },

    db_unavailable: |err| format!("Las puntuaciones no se guardan en esta sesión: {err}"),
    cannot_open: |path, err| format!("No se puede abrir {path}: {err}"),
    no_photos_in: |dir| format!("No hay archivos JPEG ni HEIC en {dir}"),
    rating_not_saved: |err| format!("Estrellas no guardadas – {err}"),
    open_folder: "Abrir carpeta",
    transfer_copy: "Copiar",
    transfer_move: "Mover",
    transfer_copy_cmd: "Copia todo lo que muestra el filtro actual a …",
    transfer_move_cmd: "Mueve todo lo que muestra el filtro actual a …",
    transfer_same_folder: "Esa ya es la carpeta abierta",
    transfer_busy: "Ya hay una copia o un movimiento en curso",
    transfer_done: |moved, done, skipped, name, err| {
        let verb = if moved { "movidas" } else { "copiadas" };
        let mut text = format!("{done} {verb}, {skipped} omitidas");
        if !name.is_empty() {
            text.push_str(&format!(" – {name}: {err}"));
        }
        text
    },
    download_title: "Activar la puntuación estética",
    download_text: |gb| {
        format!(
            "Cerno necesita el modelo de imagen CLIP ViT-L/14 para puntuar la estética.\n\n\
             ¿Descargarlo ahora de Hugging Face (Xenova/clip-vit-large-patch14, {gb:.1} GB)? \
             Se guarda en la carpeta de datos de Cerno y solo se descarga una vez."
        )
    },
    btn_download: "Descargar",
    btn_cancel: "Cancelar",
    btn_close: "Cerrar (Esc)",
    aesthetics_offer: "La puntuación estética necesita un modelo (1.2 GB): Menú → Modelos y datos.",

    section_aesthetics: "Estética",
    section_sharpness: "Nitidez (en la carpeta)",
    section_exposure: "Exposición",
    section_attributes: "Atributos CLIP",
    section_histogram: "Histograma",
    section_file: "Archivo",
    row_size: "Tamaño",
    row_load_time: "Tiempo de carga",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "Para ti",
    row_frame: "Imagen completa",
    row_eyes: "Ojos",
    row_highlights: "Luces quemadas",
    row_shadows: "Sombras empastadas",
    attributes: [
        "Calidad general",
        "Nítida",
        "Buena luz",
        "Bien compuesta",
        "Poco ruido",
        "Colorida",
    ],
    explain_laion: "Lo bonita que le parece la foto a una IA entrenada con muchas valoraciones de personas, convertido a estrellas de 0 a 5. La mayoría de las fotos obtiene 2–3; a partir de 4 es muy bueno.",
    explain_v25: "Una IA más reciente para la misma pregunta, mejor con fotos del día a día. Misma escala de estrellas.",
    explain_personal: "Las estrellas que Cerno cree que le darías. Aprende de tus propias estrellas y de las fotos que eliminas.",
    explain_frame: "La nitidez de las zonas más nítidas, comparada con las demás fotos de esta carpeta. 80 % significa más nítida que el 80 % de ellas.",
    explain_eyes: "Nitidez justo en los ojos, si hay una cara. En los retratos cuenta esto, no el fondo.",
    explain_highlights: "Zonas de blanco puro, sin ningún detalle. Más del 1 % merece un vistazo.",
    explain_shadows: "Zonas de negro puro, sin ningún detalle. A menudo es intencionado; más del 5 % se marca.",
    explain_attributes: "La IA compara la foto con dos descripciones opuestas. 50 % significa empate, cerca de 100 % claramente la primera.",
    explain_attribute: [
        "buena foto – mala foto",
        "nítida – borrosa",
        "buena luz – mala luz",
        "bien compuesta – mal compuesta",
        "limpia – con ruido",
        "colorida – apagada",
    ],
    explain_models: "Dónde se ejecuta cada IA: DirectML = tarjeta gráfica, CPU = procesador. ± indica cuánto suele desviarse «Para ti».",
    note_no_embedding: "aún sin datos CLIP",
    note_learning: |n, of| format!("aprendiendo – {n} de {of} fotos"),
    note_analysing: "analizando…",
    note_no_face: "ninguna cara",
    note_faces_too_small: |n| {
        if n == 1 {
            "1 cara, demasiado pequeña".to_owned()
        } else {
            format!("{n} caras, demasiado pequeñas")
        }
    },
    note_needs_clip: "necesita el modelo CLIP",
    model_missing: "no instalado",
    model_downloading: |percent| format!("descargando {percent:.0} %"),
    model_ready: "listo",
    model_loading: "cargando…",
    model_failed: "error",
    model_faces: "Caras",
    model_personal: "Para ti",
    taste_trained: |n, error| format!("{n} fotos, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} fotos"),
    taste_untrained: "aún sin entrenar",
    btn_reset_taste: "Restablecer Para ti",
    btn_delete_models: "Eliminar modelos",
    copy_models_path: "Copiar ruta",
    models_path_copied: "Ruta copiada",
    confirm_reset_taste_title: "¿Restablecer Para ti?",
    confirm_reset_taste_text: "Cerno olvidará lo aprendido de tus estrellas y eliminaciones. Las estrellas en los archivos de foto no cambian.",
    confirm_delete_models_title: "¿Eliminar los modelos descargados?",
    confirm_delete_models_text: "Quita los archivos de los modelos CLIP y SigLIP del disco (unos 3 GB). Los valores guardados permanecen en la base de datos; la estética se puede volver a descargar.",

    cmd_straighten: "Enderezar",
    cmd_rotate_ccw: "Girar 90° a la izquierda",
    cmd_rotate_cw: "Girar 90° a la derecha",
    cmd_crop: "Recorte",
    edit_not_jpeg: "Enderezar y recortar solo funciona con JPEG.",
    edit_writing: "Escribiendo la foto…",
    edit_reencoded: "JPEG recodificado. La calidad se ha fijado de nuevo una vez.",
    edit_failed: |detail| format!("No se ha escrito: {detail}"),
    edit_hint: "Intro aplica, Esc cancela",
    ratio_original: "Original",
    crop_landscape: "Horizontal",
    crop_portrait: "Vertical",

    help_title: "Ayuda",
    help_intro: "Cerno muestra tus fotos al instante y te ayuda a seleccionarlas. Las estrellas se guardan directamente en el archivo de la foto, así que otros programas también las ven – la fecha del archivo no cambia. Todo lo demás se queda en la base de datos propia de Cerno. La nitidez y la belleza se valoran automáticamente en segundo plano; Tab abre el panel de detalles, I despliega o pliega todas las explicaciones, Ctrl+K abre el menú.",
    help_drop: "Arrastra una carpeta o una foto a la ventana, o pulsa Ctrl+O.",
    help_close: "Esc, H o F1 cierra esta página",
    help_sections: ["Navegar", "Valorar y descartar", "Vista", "Más"],
    help_browse: [
        (
            "→, Espacio, Av Pág",
            "Foto siguiente (mantén pulsado para avanzar sin parar)",
        ),
        ("←, Retroceso, Re Pág", "Foto anterior"),
        ("Inicio, Fin", "Primera / última foto"),
        (
            "Rueda del ratón",
            "Sobre la tira de miniaturas: recorrer las fotos",
        ),
        ("Ctrl+O", "Abrir una carpeta (o arrastrarla a la ventana)"),
    ],
    help_rate: [
        (
            "1 – 5",
            "Dar estrellas – se escriben en el archivo de la foto",
        ),
        ("Mayús+1 – 5", "Dar estrellas y pasar a la foto siguiente"),
        ("0", "Quitar las estrellas o el rechazo"),
        (
            "X, Mayús+X",
            "Rechazar – queda anotado en el archivo, no se elimina nada; con Mayús, pasar a la foto siguiente",
        ),
        ("Supr", "Eliminar: va a la papelera tras 5 segundos"),
        ("Esc", "Recuperar las fotos que esperan a ser eliminadas"),
        (
            "C",
            "Comparar: fijar esta foto a la izquierda, navegar a la derecha",
        ),
        (
            "A, D",
            "Comparar: conservar la izquierda / la derecha – la otra se rechaza",
        ),
        (
            "6 – 9",
            "Color: rojo, amarillo, verde, azul – otra vez lo quita",
        ),
        ("Mayús+6 – 9", "Poner ese color y pasar a la foto siguiente"),
    ],
    help_view: [
        ("Z, Doble clic", "Foto completa ↔ 100 %"),
        ("+, −, Rueda del ratón", "Acercar / alejar"),
        ("Arrastrar", "Mover la foto ampliada"),
        ("F11", "Pantalla completa"),
        ("F", "Ordenar y filtrar"),
        ("Tab", "Panel de detalles"),
        ("I", "Detalles: desplegar o plegar todas las explicaciones"),
        ("F6", "Tira de miniaturas"),
        (
            "Mayús+Tab",
            "Ordenar y filtrar, detalles y tira de miniaturas a la vez",
        ),
        (
            "S",
            "Enderezar: cuadrícula, rueda y flechas giran, Mayús más fino, Intro aplica",
        ),
        (
            "Ctrl+←, Ctrl+→",
            "Girar 90° – sin pérdida, mediante la orientación JPEG",
        ),
        (
            "R",
            "Recorte: trazar un marco, X cambia horizontal/vertical, A cambia el formato",
        ),
        ("Intro, Esc", "Aplicar o cancelar enderezar y recorte"),
    ],
    help_more: [
        ("Ctrl+K", "Menú: todas las funciones"),
        (
            "Ctrl+M",
            "Acción: copiar, mover o eliminar las fotos en pantalla",
        ),
        ("Ctrl+L", "Cambiar idioma"),
        ("H, F1, ?", "Esta ayuda"),
        (
            "Esc",
            "Volver atrás: zoom, modo comparación, pantalla completa",
        ),
    ],
};
