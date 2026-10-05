use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('/'),
    compass: ["N", "S", "E", "O"],
    size_units: ["KB", "MB", "GB"],
    key_ctrl: "Ctrl",
    key_shift: "Mayús",
    key_delete: "Supr",

    sort: |key| format!("Orden: {key}"),
    filter_stars: |n| {
        if n == 1 {
            "1 estrella".to_owned()
        } else {
            format!("{n} estrellas")
        }
    },
    filter_blurry: "Borrosas",
    filter_blurry_tooltip: "Entre el 20 % más borroso de esta carpeta y claramente desenfocadas",
    filter_duplicate: "Duplicados",
    filter_duplicate_tooltip: "Cada foto salvo la primera ruta idéntica",
    filter_people: "Con personas",
    filter_no_people: "Sin personas",
    filter_people_tooltip: "Detectadas por la cara: las personas de espaldas o muy pequeñas en la imagen no cuentan.",
    filter_deleted: "Eliminadas",
    filter_deleted_tooltip: "Fotos eliminadas de esta carpeta. Están en la carpeta oculta .originals: Ctrl+Z devuelve una a su sitio.",
    filter_deleted_none: "No hay fotos eliminadas en esta carpeta",
    filter_clear: "Mostrar todas",
    media_all: "Fotos y vídeos",
    media_photos: "Solo fotos",
    media_videos: "Solo vídeos",
    media_no_videos: "Esta carpeta no tiene vídeos",
    top_photos: |n| format!("Top {n} fotos"),
    top_purposes: [
        "Destacadas",
        "Avance",
        "Presentación",
        "Fotolibro",
        "Galería",
    ],
    top_tooltip: "Fotos, vídeos o ambos – o solo las mejores fotos, según tus estrellas (si no, la predicción), la estética y la nitidez, la mejor de cada serie primero. Las fotos rechazadas o borrosas y los duplicados no cuentan. La selección se mantiene hasta que cambie un filtro o elijas «Actualizar orden». En las fotos no cambia nada.",
    menu_top: "Mejores fotos",
    bulk_delete_top: "No con Top – son las mejores fotos",
    filter_none_active: "Ningún filtro activo",
    filter_similar: "≈ Parecidas",
    filter_similar_to: |name| format!("≈ como {name}"),
    menu_similar: "Fotos parecidas",
    menu_similar_to: |name| format!("Parecidas a {name}"),
    similar_tooltip: |percent| {
        format!("Solo las fotos que se parecen a la actual (desde {percent:.0} %) – tecla M")
    },
    similar_fact: |percent| format!("Parecido {percent:.0} %"),
    similar_needs_model: "Las fotos parecidas necesitan el modelo de estética (CLIP)",
    similar_not_analysed: "Esta foto aún no se ha analizado",
    similar_none: |percent| format!("Ninguna foto parecida (desde {percent:.0} %)"),
    refresh_order: "Actualizar orden",
    refresh_order_tooltip: "Hay puntuaciones nuevas desde que ordenaste y filtraste",
    analyzing_progress: |done, total| format!("Analizando {done} / {total}"),
    enable_aesthetics: "Activar estética…",
    enable_aesthetics_tooltip: |size| {
        format!("Descarga una sola vez los modelos de imagen para la estética ({size})")
    },
    add_v25: "Cargar V2.5…",
    add_v25_tooltip: |size| {
        format!(
            "Descarga una sola vez el segundo modelo de estética (SigLIP + V2.5, {size}): la estética pasa a ser la media de ambos modelos"
        )
    },
    downloading_model: |percent| format!("Descargando modelo {percent:.0} %"),
    aesthetics_loading: "Estética: cargando modelo…",
    aesthetics_failed: "Estética: error",

    sort_name: "Nombre",
    sort_rating: "Estrellas",
    sort_aesthetics: "Estética",
    sort_personal: "Predicción",
    sort_sharpness: "Nitidez",
    sort_taken: "Hora de captura",
    filter_unrated: "Sin estrellas",

    filter_rejected: "Rechazadas",
    actions: "Acción",
    actions_tooltip: "Copiar, mover o eliminar las fotos en pantalla",
    selection_delete: "Eliminar",
    bulk_copy: |n| format!("Copiar a … ({n} {})", if n == 1 { "foto" } else { "fotos" }),
    bulk_move: |n| format!("Mover a … ({n} {})", if n == 1 { "foto" } else { "fotos" }),
    bulk_delete: |n| format!("Eliminar ({n} {})", if n == 1 { "foto" } else { "fotos" }),
    bulk_delete_hint: "Todas las fotos que muestra el filtro. A los 5 segundos pasan a la carpeta oculta .originals junto a ellas – nada se borra para siempre, Esc las recupera.",
    delete_rejected_hint: "Todas las fotos rechazadas de la carpeta, también las que el filtro oculta ahora. A los 5 segundos pasan a la carpeta oculta .originals – nada se borra para siempre.",
    bulk_restore: |n| {
        format!(
            "Devolver a su sitio ({n} {})",
            if n == 1 { "foto" } else { "fotos" }
        )
    },
    bulk_restore_hint: "Todas las fotos eliminadas que muestra el filtro vuelven a su carpeta. Si el nombre ya está ocupado, la foto recibe un número: no se sobrescribe nada.",
    photos_shown: |shown, total| format!("{shown} de {total} fotos"),
    photos_count: |n| format!("{n} {}", if n == 1 { "foto" } else { "fotos" }),
    photos_badge_tooltip: "Cuántas fotos muestra ahora el filtro – «Acción» actúa exactamente sobre estas.",
    label_red: "Rojo",
    label_yellow: "Amarillo",
    label_green: "Verde",
    label_blue: "Azul",
    label_purple: "Morado",
    meter_aesthetics_tooltip: "Estética: media de LAION y V2.5 en una escala fija – una foto tiene el mismo valor en cualquier carpeta. La nitidez, en cambio, compara con las demás fotos de la carpeta.\nValores por separado: panel de detalles (Tab)",
    meter_sharpness: "Nitidez",
    meter_eyes: "Ojos",
    probably_blurry: "probablemente borrosa",
    analyzing: "Analizando…",
    saving: "Guardando…",
    auto_advance_on: "Avance automático",
    series_position: |index, len| format!("Serie {index} / {len}"),
    duplicate_of: |name| format!("Duplicado de {name}"),
    rejected: "Rechazada",
    filmstrip_video: "Vídeo",
    star_tooltip: |n| format!("{n} ★ – tecla {n}"),
    personal_hint: |stars| {
        format!(
            "Predicción: {stars:.1} ★ – las estrellas que Cerno cree que darías. Aún no es tu valoración"
        )
    },
    zoom: |percent| format!("Zoom {percent:.0} %"),
    digital_zoom: |ratio| format!("Zoom digital {ratio:.1}×"),
    button_toolbar: "Barra de filtros",
    button_details: "Panel de detalles",
    button_filmstrip: "Tira de miniaturas",
    button_help: "Ayuda",
    button_menu: "Menú",
    button_language: |name| format!("Idioma: {name}"),

    cmd_all_panels: "Barra de filtros, detalles y tira de miniaturas",
    cmd_fullscreen: "Pantalla completa",
    cmd_compare: "Comparar",
    cmd_similar: "Mostrar solo fotos parecidas",
    cmd_zoom: "Foto completa ↔ 100 %",
    menu_overlay: "Superposición",
    overlay_off: "Desactivada",
    overlay_sharpness: "Bordes nítidos",
    overlay_exposure: "Luces y sombras recortadas",
    overlay_fact_sharpness: "Superposición: nitidez",
    overlay_fact_exposure: "Superposición: exposición",
    overlay_hint_sharpness: "Nitidez: el morado marca los bordes más nítidos de la foto",
    overlay_hint_exposure: "Exposición: rojo = quemado, azul = negro empastado",
    overlay_hint_off: "Superposición desactivada",
    overlay_show_on_photo: "Mostrar en la foto (O)",
    cmd_grid: "Cuadrícula",
    cmd_reject: "Rechazar",
    cmd_description: "Comentario y palabras clave",
    cmd_delete_rejected: |n| {
        format!(
            "Eliminar las rechazadas ({n} {})",
            if n == 1 { "foto" } else { "fotos" }
        )
    },
    cmd_auto_advance: "Avanzar automáticamente",
    cmd_subfolders: "Incluir subcarpetas",
    subfolders_on: "Subcarpetas incluidas",
    subfolders_off: "Solo esta carpeta, sin subcarpetas",
    menu_sort: "Ordenar",
    menu_filter: "Filtro",
    menu_view: "Vista",
    menu_labels: "Colores",
    menu_external: "Editar en otro programa",
    external_other: "Otro programa …",
    external_chooser: "«Abrir con» del sistema …",
    external_default: "Abrir con el programa predeterminado",
    external_pick_title: "Elegir un programa para editar",
    external_opened: |name| {
        format!(
            "Abierto en {name}: al guardar, Cerno muestra la nueva versión; el original queda en .originals"
        )
    },
    external_reloaded: |name| format!("{name} se guardó en otro programa: recargado"),
    external_failed: |err| format!("No se abrió: {err}"),
    menu_language: "Idioma",
    menu_models: "Modelos y datos",
    menu_this_photo: "Esta foto",
    menu_visible: "Fotos visibles",
    menu_settings: "Configuración",
    menu_stars: "Estrellas",
    label_none: "Sin color",
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
            "1 foto pasa a la carpeta oculta .originals   ·   Esc para deshacer".to_owned()
        } else {
            format!("{n} fotos pasan a la carpeta oculta .originals   ·   Esc para deshacer")
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
    transfer_progress: |moved, at, total, sizes, file| {
        let verb = if moved { "Moviendo" } else { "Copiando" };
        let mut text = format!("{verb} {at} / {total} fotos – {sizes}");
        if !file.is_empty() {
            text.push_str(&format!(" – {file}"));
        }
        text
    },
    download_title: "Activar la puntuación estética",
    download_text: |clip, v25, size| {
        let mut text = String::from("A la puntuación estética le falta:\n");
        if clip {
            text.push_str("\n• CLIP ViT-L/14 – de Hugging Face (Xenova/clip-vit-large-patch14)");
        }
        if v25 {
            text.push_str("\n• SigLIP + Aesthetic Predictor V2.5 – de la release de GitHub models-1 de Cerno (la parte V2.5 es AGPL-3.0)");
        }
        text.push_str(&format!(
            "\n\n¿Descargar ahora ({size})? Los archivos van a la carpeta de datos de Cerno y solo se descargan una vez; una descarga interrumpida continúa la próxima vez."
        ));
        text
    },
    btn_download: "Descargar",
    btn_cancel: "Cancelar",
    btn_close: "Cerrar (Esc)",
    aesthetics_offer: |size| {
        format!(
            "La puntuación estética necesita modelos de imagen ({size}): Menú → Modelos y datos."
        )
    },
    v25_offer: |size| {
        format!("Estética más fiable con el segundo modelo V2.5 ({size}): Menú → Modelos y datos.")
    },

    section_aesthetics: "Estética",
    section_sharpness: "Nitidez (en la carpeta)",
    section_exposure: "Exposición",
    section_attributes: "Atributos CLIP",
    row_aesthetics: "Estética (media)",
    section_histogram: "Histograma",
    section_file: "Archivo",
    row_size: "Tamaño",
    row_load_time: "Tiempo de carga",
    row_container: "Contenedor",
    row_duration: "Duración",
    row_video: "Vídeo",
    row_frame_rate: "Fotogramas/s",
    row_video_bitrate: "Tasa de vídeo",
    row_audio: "Audio",
    row_audio_bitrate: "Tasa de audio",
    row_bitrate: "Tasa total",
    no_audio: "sin pista de audio",
    variable_frame_rate: "variable",
    channels: |n| match n {
        1 => "Mono".to_owned(),
        2 => "Estéreo".to_owned(),
        6 => "5.1".to_owned(),
        8 => "7.1".to_owned(),
        n => format!("{n} canales"),
    },
    row_location: "Ubicación",
    tab_values: "Valores",
    tab_description: "Descripción",
    section_comment: "Comentario",
    section_keywords: "Palabras clave",
    comment_hint: "Escribir un comentario …",
    keyword_hint: "Añadir palabra clave …",
    keyword_remove: "Quitar",
    description_note: "Intro añade una palabra clave, Esc sale del campo. El comentario y las palabras clave se escriben en el archivo (IPTC y XMP), donde Windows, Lightroom y digiKam los leen.",
    description_waiting: "Leyendo …",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "Predicción",
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
    explain_laion: "Lo bonita que le parece la foto a una IA entrenada con muchas valoraciones de personas. Le gustan sobre todo las personas, los retratos y la comida.",
    explain_v25: "Una IA más reciente para la misma pregunta, mejor con fotos del día a día. Le gustan sobre todo los paisajes, el agua y las tomas aéreas.",
    explain_personal: "Las estrellas que Cerno cree que le darías. Aprende de tus propias estrellas y de las fotos que eliminas.",
    explain_aesthetics: "El valor bajo la foto: la media de las dos IA de abajo, de 0 % (poco atractiva) a 100 % (muy atractiva). La mayoría de las fotos queda entre 40 y 60 %; a partir de 80 % es muy bueno. La escala es fija – una foto tiene el mismo valor en cualquier carpeta.",
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
    explain_models: "Dónde se ejecuta cada IA: DirectML = tarjeta gráfica, CPU = procesador. ± indica cuánto suele desviarse la predicción.",
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
    model_removing: "eliminando…",
    model_failed: "error",
    model_faces: "Caras",
    model_personal: "Predicción",
    taste_trained: |n, error| format!("{n} fotos, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} fotos"),
    taste_untrained: "aún sin entrenar",
    taste_sources: |stars, rejected, deleted| {
        format!(
            "Aprendida de {stars} fotos con estrellas, {rejected} rechazadas y {deleted} eliminadas – las rechazadas y eliminadas cuentan como 0 ★."
        )
    },
    btn_reset_taste: "Restablecer la predicción",
    btn_delete_models: "Eliminar modelos",
    models_deleted: "Modelos eliminados",
    models_downloaded: "Modelos descargados: ahora se calcula la estética",
    download_failed: |err| {
        format!("La descarga falló: {err}. Un nuevo intento continúa donde se detuvo")
    },
    copy_models_path: "Copiar ruta",
    models_path_copied: "Ruta copiada",
    confirm_reset_taste_title: "¿Restablecer la predicción?",
    confirm_reset_taste_text: "Cerno olvidará lo aprendido de tus estrellas y eliminaciones. Las estrellas en los archivos de foto no cambian.",
    confirm_delete_models_title: "¿Eliminar los modelos descargados?",
    confirm_delete_models_text: |size| {
        format!(
            "Quita del disco los archivos de modelos descargados ({size}). Los valores guardados permanecen en la base de datos; los modelos se pueden volver a descargar."
        )
    },

    cmd_straighten: "Enderezar",
    cmd_rotate_ccw: "Girar 90° a la izquierda",
    cmd_rotate_cw: "Girar 90° a la derecha",
    cmd_crop: "Recorte",
    cmd_undo: "Deshacer",
    edit_not_jpeg: "Enderezar, recortar, girar y Ctrl+Z solo funcionan con JPEG.",
    video_play_hint: "Reproducir (Espacio)",
    video_play_pause: "Reproducir / pausa (Espacio)",
    video_mute: "Sonido sí / no",
    video_volume: "Volumen (↑ ↓)",
    video_no_zoom: "Los vídeos no se amplían",
    video_no_compare: "Los vídeos no se pueden comparar",
    video_no_sound: "Sin sonido: Cerno no encontró ninguna salida de audio; el vídeo se reproduce sin sonido",
    video_no_ffmpeg: "Sin vista previa: Cerno necesita ffmpeg (p. ej. winget install Gyan.FFmpeg)",
    video_play_failed: |err| format!("No se puede reproducir el vídeo: {err}"),
    edit_writing: "Escribiendo la foto…",
    edit_cancelled: "Se muestra otra foto: edición cancelada",
    busy_editing: "La edición sigue abierta: Intro aplica, Esc cancela",
    busy_copying: "Esta foto se está copiando; en un momento vuelve a ser posible",
    busy_moving: "Esta foto se está moviendo",
    busy_deleted: "Foto eliminada: devuélvela primero a su sitio (Ctrl+Z)",
    edit_needs_index: "Editar necesita el índice, que no se pudo abrir",
    edit_reencoded: "JPEG recodificado – Ctrl+Z recupera el original.",
    undo_done: "Original recuperado",
    undo_nothing: "No se guarda ningún original de esta foto",
    cmd_restore: "Devolver a su sitio",
    restored: |n, renamed, name| match (n, renamed) {
        (1, 0) => format!("Devuelta a su sitio: {name}"),
        (1, _) => format!("Devuelta como {name}: el nombre estaba ocupado"),
        (n, 0) => format!("{n} fotos devueltas a su sitio"),
        (n, r) => format!("{n} fotos devueltas a su sitio, {r} con un nombre nuevo"),
    },
    restore_failed: |n, name, err| format!("No se pudieron devolver {n} foto(s) – {name}: {err}"),
    deleted_mark: "Eliminada",
    edit_failed: |detail| format!("No se ha escrito: {detail}"),
    edit_hint_straighten: "Rueda o ←/→ gira, Mayús más fino · Intro aplica, Esc cancela",
    edit_hint_crop: "A: proporción · X: horizontal/vertical · Intro aplica, Esc cancela",
    ratio_original: "Original",
    crop_landscape: "Horizontal",
    crop_portrait: "Vertical",

    help_title: "Ayuda",
    help_intro: "Cerno muestra tus fotos al instante y te ayuda a seleccionarlas. Las estrellas y los colores se guardan en el archivo, sin cambiar su fecha; todo lo demás queda en la base de datos de Cerno.",
    help_drop: "Arrastra una carpeta o una foto a la ventana, o pulsa Ctrl+O.",
    help_close: "Esc, H o F1 cierra esta página",
    welcome_intro: "Ver, puntuar y descartar fotos sin esperas: las estrellas van al archivo y su fecha no cambia.",
    welcome_keys: [
        ("←, →", "Foto anterior / siguiente"),
        ("1 – 5", "Dar estrellas"),
        ("X", "Rechazar"),
        ("Supr", "Eliminar – Esc la recupera"),
        ("Ctrl+K", "Menú con todas las funciones"),
    ],
    welcome_more: "Todos los atajos: H",
    help_sections: [
        "Navegar",
        "Valorar",
        "Descartar",
        "Vídeo",
        "Vista",
        "Paneles",
        "Editar",
        "Más",
    ],
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
        ("Ctrl+U", "Incluir subcarpetas – sí / no"),
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
        (
            "6 – 9",
            "Color: rojo, amarillo, verde, azul – otra vez lo quita",
        ),
        ("Mayús+6 – 9", "Poner ese color y pasar a la foto siguiente"),
        (
            "B",
            "Descripción: editar el comentario y las palabras clave",
        ),
    ],
    help_cull: [
        (
            "C",
            "Comparar: fijar esta foto a la izquierda, navegar a la derecha",
        ),
        (
            "A, D",
            "Comparar: conservar la izquierda / la derecha – la otra se rechaza y la comparación termina",
        ),
        ("M", "Mostrar solo fotos parecidas – otra vez: todas"),
        (
            "Supr",
            "Eliminar: va a la carpeta oculta .originals tras 5 segundos – no se pierde nada",
        ),
        ("Esc", "Recuperar las fotos que esperan a ser eliminadas"),
    ],
    help_video: [
        (
            "Espacio",
            "Reproducir / pausar (Mayús+Espacio: foto siguiente)",
        ),
        ("Alt+←, Alt+→", "5 s atrás / adelante"),
        (",, .", "Un fotograma atrás / adelante (en pausa)"),
        ("↑, ↓", "Volumen"),
    ],
    help_view: [
        ("Z, Doble clic", "Foto completa ↔ 100 %"),
        ("Ctrl+0, Ctrl+1", "Foto completa / 100 %"),
        (
            "+, −, Rueda del ratón",
            "Acercar / alejar – también con Ctrl",
        ),
        ("Arrastrar", "Mover la foto ampliada"),
        (
            "O",
            "Superposición: bordes nítidos → luces y sombras recortadas → desactivada",
        ),
        (
            "F7",
            "Cuadrícula de todas las fotos: ↑ ↓ una fila, + − tamaño, Intro abre la foto",
        ),
        ("F, F11", "Pantalla completa"),
    ],
    help_panels: [
        (
            "T",
            "Barra de filtros: ordenar y filtrar – por estrellas, colores, nitidez, personas",
        ),
        ("Tab", "Panel de detalles"),
        ("F6", "Tira de miniaturas"),
        (
            "Mayús+Tab",
            "Barra de filtros, detalles y tira de miniaturas a la vez",
        ),
    ],
    help_edit: [
        (
            "S",
            "Enderezar: cuadrícula, rueda y flechas giran, Mayús más fino",
        ),
        (
            "R",
            "Recorte: trazar un marco, A cambia el formato, X cambia horizontal/vertical",
        ),
        ("Intro, Esc", "Aplicar o cancelar enderezar y recorte"),
        (
            "Ctrl+←, Ctrl+→",
            "Girar 90° – sin pérdida, mediante la orientación JPEG",
        ),
        (
            "Ctrl+Z",
            "Recuperar el original – se queda en .originals junto a la foto",
        ),
        ("E", "Editar en otro programa: el recordado o elegir uno"),
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
            "Volver atrás: zoom, modo comparación, cuadrícula, pantalla completa",
        ),
    ],
};
