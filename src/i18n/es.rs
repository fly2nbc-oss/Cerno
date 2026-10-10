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
    filter_incomplete: "Incompletas",
    filter_incomplete_tooltip: "JPEG cuyo archivo termina antes que la imagen – cortado al copiar o descargar; la parte que falta se ve gris",
    filter_people: "Con personas",
    filter_no_people: "Sin personas",
    filter_people_tooltip: "Detectadas por la cara: las personas de espaldas o muy pequeñas en la imagen no cuentan.",
    filter_deleted: "Eliminadas",
    filter_deleted_tooltip: "Fotos eliminadas de esta carpeta. Están en la carpeta oculta .originals: Ctrl+Z devuelve una a su sitio.",
    filter_deleted_none: "No hay fotos eliminadas en esta carpeta",
    filter_hide_rejected: "Ocultar rechazadas",
    filter_without: "sin",
    filter_hide_rejected_tooltip: "Las fotos rechazadas desaparecen y las demás se quedan. ×, en cambio, muestra solo las rechazadas.",
    menu_name_list: "Por lista de archivos …",
    name_list_title: "Filtrar por lista de archivos",
    name_list_intro: "Pega los nombres de archivo o números que eligió un cliente, uno por línea o separados por comas o puntos y coma. Mayúsculas y extensión dan igual; un número encuentra las cifras con las que termina un nombre.",
    name_list_hint: "IMG_0345, IMG_0351 …",
    name_list_missing: "No encontrados:",
    name_list_ambiguous: "En varias carpetas (se toman todas):",
    name_list_apply: "Aplicar",
    name_list_chip_tooltip: "Solo las fotos de la lista pegada: un clic vuelve a mostrar todas",
    name_list_found: |found, total| format!("{found} de {total} encontrados"),
    name_list_chip: |found, total| format!("Lista {found}/{total}  ×"),
    align_camera_hint: "Desplaza la hora de captura de todas las fotos de la cámara derecha en esta carpeta, para que la foto derecha se tomara en el mismo momento que la izquierda. Solo en el índice: los archivos conservan su hora.",
    align_needs_compare: "Primero compara dos fotos de cámaras distintas (C)",
    align_no_time: "A una de las dos fotos le falta la cámara o la hora de captura",
    align_same_camera: "Las dos fotos son de la misma cámara",
    menu_camera_time: "Hora de la cámara …",
    camera_time_title: "Hora de la cámara",
    camera_time_intro: "Si el reloj de una cámara iba mal, un desfase mueve la hora de captura de todas sus fotos en esta carpeta, para ordenar por hora de captura, las series y lo que se muestra. Solo en el índice; los archivos conservan su hora.",
    camera_time_reset: "Restablecer",
    camera_time_invalid: "Desfase como +h:mm:ss o −h:mm:ss, p. ej. +1:30:00",
    camera_time_apply: "Aplicar",
    camera_time_none: "Aún no hay fotos con cámara y hora de captura en esta carpeta.",
    camera_time_applied: "Hora de la cámara aplicada",
    camera_aligned: |camera, offset| format!("{camera}: hora de captura desplazada {offset}"),
    camera_time_photos: |n| {
        if n == 1 {
            "1 foto".into()
        } else {
            format!("{n} fotos")
        }
    },
    camera_time_tooltip: |taken, offset| format!("Hora de la cámara {taken}, ajustada {offset}"),
    cmd_pairs: "RAW+JPG como una sola foto",
    pairs_hint: "Un RAW y un JPG con el mismo nombre aparecen como una sola foto: se muestra el JPG. Estrellas, color y descripción van a los dos archivos, copiar, mover y eliminar se llevan ambos; las ediciones solo cambian el JPG.",
    cmd_update_check: "Buscar actualizaciones",
    update_check_hint: "Una consulta al día a GitHub – nada sobre ti ni sobre tus fotos",
    update_available: |v| format!("Cerno {v} está disponible – Ayuda › Acerca de Cerno"),
    cmd_update_download: |v| format!("Descargar Cerno {v} …"),
    update_ask_title: "¿Buscar actualizaciones?",
    update_ask_text: "¿Debe Cerno comprobar una vez al día si hay una versión nueva? No se envía nada sobre ti ni sobre tus fotos. Puedes cambiarlo más tarde en Configuración.",
    btn_update_yes: "Sí",
    btn_update_no: "No",
    pairs_on: "RAW+JPG aparecen como una sola foto",
    pairs_off: "RAW y JPG aparecen por separado",
    pair_badge: "RAW+JPG",
    pair_edit_jpeg_only: "Solo cambia el JPG: el RAW queda intacto.",
    pair_raw_marks: |marks| format!("RAW: {marks}"),
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
    bulk_delete_top: "No con Top – son las mejores fotos",
    filter_similar: "≈ Parecidas",
    filter_similar_to: |name| format!("≈ como {name}"),
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
    photos_badge_tooltip: "Cuántas fotos muestra ahora el filtro – «Fotos visibles» de la barra de menú actúa exactamente sobre estas.",
    label_red: "Rojo",
    label_yellow: "Amarillo",
    label_green: "Verde",
    label_blue: "Azul",
    label_purple: "Morado",
    meter_aesthetics_tooltip: "Estética: media de LAION y V2.5 en una escala fija – una foto tiene el mismo valor en cualquier carpeta. La nitidez, en cambio, compara con las demás fotos de la carpeta.\nValores por separado: panel de detalles (Tab)",
    meter_sharpness: "Nitidez",
    meter_eyes: "Ojos",
    probably_blurry: "probablemente borrosa",
    incomplete_fact: "Archivo incompleto",
    incomplete_tooltip: "El archivo termina antes que la imagen – la parte que falta se ve gris. Revisa el original, por ejemplo en la tarjeta de memoria.",
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
    zoom_preview: |percent| format!("Zoom {percent:.0} % de la vista previa"),
    digital_zoom: |ratio| format!("Zoom digital {ratio:.1}×"),
    button_toolbar: "Barra de filtros",
    button_details: "Panel de detalles",
    button_filmstrip: "Tira de miniaturas",
    button_help: "Ayuda",
    view_photo: "Foto",
    view_grid: "Cuadrícula",
    view_faces: "Caras",
    faces_video: "En los vídeos no se buscan caras",
    filmstrip_in_grid: "No hace falta en la cuadrícula – F7 vuelve a la foto",
    faces_button: |n| {
        if n == 1 {
            "Mostrar 1 cara en grande (G)".to_owned()
        } else {
            format!("Mostrar {n} caras en grande (G)")
        }
    },
    button_side_bar: "Barra de menú",
    bar_rotate: "Girar",
    bar_colour: "Color",
    bar_stars: "Estrellas",
    bar_align_camera: "Igualar cámaras",
    bar_overlay_sharpness: "Nitidez",
    bar_overlay_exposure: "Exposición",
    bar_none_rejected: "Ninguna foto de la carpeta está rechazada.",
    bar_none_deleted: "No se muestran fotos eliminadas: la casilla de la papelera de la barra de filtros las muestra.",
    button_language: |name| format!("Idioma: {name}"),

    cmd_fullscreen: "Pantalla completa",
    cmd_compare: "Comparar",
    cmd_quad: "Vista de cuatro",
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
    cmd_reject: "Rechazar",
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
    menu_view: "Vista",
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
    writer_stopped: "Las marcas ya no se escriben en los archivos – error interno (crash.log en la carpeta de datos). Reinicia Cerno.",
    internal_error: "Error interno – se detuvo el comando, Cerno sigue funcionando (detalles en crash.log de la carpeta de datos).",
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
            "La puntuación estética necesita modelos de imagen ({size}): Barra de menú › Configuración › Modelos y datos."
        )
    },
    exiftool_offer: |size| {
        format!(
            "Cerno guarda estrellas y colores con ExifTool ({size}): Barra de menú › Configuración › Modelos y datos."
        )
    },
    exiftool_title: "Descargar ExifTool",
    exiftool_text: |size| {
        format!(
            "Cerno escribe estrellas, colores, comentarios y ediciones en las fotos con ExifTool, la herramienta libre de Phil Harvey.\n\n¿Descargarla ahora desde la fuente oficial (SourceForge, {size})? Va a la carpeta de datos de Cerno; una descarga interrumpida continúa la próxima vez."
        )
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
    row_file_size: "Tamaño del archivo",
    row_jpeg_quality: "Calidad JPEG",
    explain_jpeg_quality: "Estimada a partir de las tablas de cuantización del archivo: la calidad en sí no se guarda. 4:2:0 significa color a media resolución (habitual en cámaras), 4:4:4 a resolución completa.",
    row_complete: "Completo",
    incomplete_value: "no – falta el final",
    explain_raw_preview: "En los archivos RAW, Cerno muestra el JPEG que incrustó la cámara, sin revelado RAW. El color y la exposición son los de la cámara, y el 100 % es el tamaño de esta vista previa, no el del sensor. El histograma y la exposición miden la vista previa.",
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
    explain_personal: "Las estrellas que Cerno cree que le darías. Aprende de tus propias estrellas y de las fotos que rechazas.",
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
    exiftool_found: |version, downloaded| {
        let from = if downloaded {
            "descargado por Cerno"
        } else {
            "instalado"
        };
        if version.is_empty() {
            from.to_owned()
        } else {
            format!("{version} · {from}")
        }
    },
    exiftool_absent: "falta: sin ExifTool no hay estrellas ni colores",
    exiftool_old_state: |version| {
        format!("{version}: demasiado antiguo (se necesita 12.24 o posterior)")
    },
    exiftool_downloading: |percent| format!("descargando … {percent:.0} %"),
    btn_exiftool: |size| format!("Descargar ExifTool ({size})"),
    exiftool_outdated: |version, newer| {
        format!("{version} · descargado por Cerno – {newer} disponible")
    },
    btn_exiftool_update: |newer, size| format!("Descargar ExifTool {newer} ({size})"),
    exiftool_update_hint: |version, newer| {
        format!(
            "ExifTool {newer} está disponible (Cerno usa {version}): Barra de menú (F10) › Configuración › Modelos y datos."
        )
    },
    taste_sources: |stars, rejected| {
        format!(
            "Aprendida de {stars} fotos con estrellas y {rejected} rechazadas – las rechazadas cuentan como 0 ★, las eliminadas no cuentan."
        )
    },
    btn_reset_taste: "Restablecer la predicción",
    btn_delete_models: "Eliminar modelos",
    models_deleted: "Modelos eliminados",
    models_downloaded: "Modelos descargados: ahora se calcula la estética",
    download_failed: |err| {
        format!("La descarga falló: {err}. Un nuevo intento continúa donde se detuvo")
    },
    exiftool_ready: "ExifTool está listo: ahora se guardan estrellas y colores",
    exiftool_failed: |err| {
        format!("No se pudo descargar ExifTool: {err}. Un nuevo intento continúa desde ahí")
    },
    copy_path: "Copiar ruta",
    path_copied: "Ruta copiada",
    confirm_reset_taste_title: "¿Restablecer la predicción?",
    confirm_reset_taste_text: "Cerno olvidará lo aprendido de tus estrellas y fotos rechazadas. Las estrellas en los archivos de foto no cambian.",
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
    video_play_failed: |err| format!("No se puede reproducir el vídeo: {err}"),
    edit_writing: "Escribiendo la foto…",
    edit_cancelled: "Se muestra otra foto: edición cancelada",
    busy_editing: "La edición sigue abierta: Intro aplica, Esc cancela",
    busy_copying: "Esta foto se está copiando; en un momento vuelve a ser posible",
    busy_moving: "Esta foto se está moviendo",
    busy_deleted: "Foto eliminada: devuélvela primero a su sitio (Ctrl+Z)",
    edit_needs_index: "Editar necesita el índice, que no se pudo abrir",
    exiftool_missing: "Estrellas, colores y edición necesitan ExifTool: Barra de menú › Configuración › Modelos y datos",
    exiftool_too_old: "El ExifTool instalado es demasiado antiguo (se necesita 12.24 o posterior): Barra de menú › Configuración › Modelos y datos",
    exiftool_loading: "Descargando ExifTool: enseguida se podrán poner estrellas y colores",
    exiftool_install: |command| {
        match command {
        Some(command) => format!("Estrellas, colores y edición necesitan ExifTool. Instálalo con: {command}"),
        None => "Estrellas, colores y edición necesitan ExifTool. Instálalo con el gestor de paquetes (paquete «exiftool» o «perl-image-exiftool»).".to_owned(),
    }
    },
    edit_reencoded: "JPEG recodificado – Ctrl+Z recupera el original.",
    undo_done: "Original recuperado",
    undo_nothing: "No se guarda ningún original de esta foto",
    undo_mark_row: |what, name| format!("Deshacer {what} – {name}"),
    undo_what_stars: "las estrellas",
    undo_what_reject: "el rechazo",
    undo_what_colour: "el color",
    undo_what_edit: "la edición",
    undo_mark_done: |name, value| format!("Deshecho – {name}: {value}"),
    cmd_restore: "Devolver a su sitio",
    restored: |n, renamed, name| match (n, renamed) {
        (1, 0) => format!("Devuelta a su sitio: {name}"),
        (1, _) => format!("Devuelta como {name}: el nombre estaba ocupado"),
        (n, 0) => format!("{n} fotos devueltas a su sitio"),
        (n, r) => format!("{n} fotos devueltas a su sitio, {r} con un nombre nuevo"),
    },
    restore_failed: |n, name, err| format!("No se pudieron devolver {n} foto(s) – {name}: {err}"),
    deleted_mark: "Eliminada",
    faces_loading: "Buscando caras…",
    faces_unknown: "Aún sin analizar: las caras llegarán después.",
    faces_none: "No se detectaron caras",
    faces_only_small: "Solo caras pequeñas: demasiado pequeñas para juzgar",
    faces_zoom_hint: "Haz clic para acercar esta cara",
    faces_grid_hint: "Un clic o su número (1–9) acerca una cara · Esc cierra",
    cmd_face_grid: "Todas las caras en grande",
    raw_preview_fact: "Vista previa RAW",
    preview_word: "vista previa",
    edit_failed: |detail| format!("No se ha escrito: {detail}"),
    edit_hint_straighten: "Rueda o ←/→ gira, Mayús más fino · Intro aplica, Esc cancela",
    edit_hint_crop: "Flechas mueven · +/− tamaño, Mayús más fino · A: proporción · X: horizontal/vertical · Intro aplica, Esc cancela",
    ratio_original: "Original",
    crop_landscape: "Horizontal",
    crop_portrait: "Vertical",

    help_title: "Ayuda",
    help_intro: "Cerno muestra tus fotos al instante y te ayuda a seleccionarlas. Las estrellas y los colores se guardan en el archivo, sin cambiar su fecha; todo lo demás queda en la base de datos de Cerno.",
    help_drop: "Arrastra una carpeta o una foto a la ventana, o pulsa Ctrl+O.",
    help_close: "Esc, H o F1 cierra esta página",
    help_tab_keys: "Atajos",
    help_tab_tips: "Consejos",
    help_pages_hint: "←/→ cambia de página",
    help_tab_about: "Acerca de Cerno",
    about_intro: "Cerno muestra las fotos sin esperas y ayuda a seleccionarlas, en local, sin cuenta ni nube. Software libre: el código fuente es abierto.",
    about_version: "Versión",
    about_license: "Licencia",
    about_source: "Código fuente",
    about_bugs: "Informar de un error",
    about_bugs_text: "¿Algo no funciona? Descríbelo en un issue de GitHub; hace falta una cuenta gratuita de GitHub. La versión y el sistema ya están rellenados; no hacen falta fotos ni nombres de archivo. Si Cerno se cerró de golpe, adjunta el archivo crash.log de la carpeta de datos.",
    about_bug_link: "Informar en GitHub",
    about_open_data: "Abrir la carpeta de datos",
    about_wishes: "Ideas",
    about_wishes_text: "¿Una idea que haga más simple o rápido seleccionar fotos con Cerno? Escríbela como propuesta, también con una cuenta de GitHub.",
    about_wish_link: "Proponerla en GitHub",
    about_third_party: "Terceros",
    about_third_party_text: "Cerno usa bibliotecas libres de otros, entre ellas libheif y GStreamer con FFmpeg (LGPL), y descarga a petición ExifTool y los modelos de estética (V2.5 bajo AGPL). Sus licencias acompañan al programa.",
    about_third_party_link: "Todos los terceros y licencias",
    about_updates: "Actualizaciones",
    about_updates_text: "Si lo permites, Cerno pregunta a GitHub una vez al día si hay una versión más reciente – no se envía nada sobre ti ni sobre tus fotos, y no se descarga ni instala nada. Se activa o desactiva en Configuración › Buscar actualizaciones. Los modelos y ExifTool solo se descargan si lo pides.",
    update_off: "La búsqueda de actualizaciones está desactivada.",
    update_never: "Aún no comprobado.",
    update_checking: "Comprobando …",
    update_current: "Cerno está actualizado.",
    update_failed: "No se pudo contactar con GitHub – inténtalo más tarde.",
    update_newer: |v| format!("Cerno {v} está disponible."),
    update_check_now: "Comprobar ahora",
    help_tips: [
        (
            "Seleccionar en dos pasadas",
            &[
                "Primero recorrer rápido (Espacio) y rechazar lo fallido con X, sin pensarlo mucho.",
                "Después «sin ×» en la barra de filtros: las rechazadas desaparecen; ahora valorar de 1 a 5.",
                "Por último, barra de menú (F10) › Fotos visibles › «Eliminar las rechazadas».",
            ],
        ),
        (
            "Series y comparación",
            &[
                "Ordenadas por fecha de captura, las series quedan juntas, la foto más nítida primero.",
                "C muestra dos fotos juntas; A conserva la izquierda, D la derecha, la otra queda rechazada.",
                "M muestra solo las fotos parecidas a la actual.",
                "Dos cámaras con distinta hora: compara dos fotos tomadas en el mismo momento y luego Esta foto › «Igualar la cámara derecha a la izquierda».",
                "Mayús+C muestra cuatro fotos a la vez: el marco es la foto actual, ↑ ↓ lo mueven una fila.",
            ],
        ),
        (
            "Las mejores fotos",
            &[
                "Elegir «Top 50 fotos» en la primera casilla de la barra de filtros: Cerno propone las mejores, primero una de cada serie.",
                "La estética y la nitidez bajo la foto ayudan a decidir; las estrellas las pones tú.",
                "Filtro › Por lista de archivos …: pega los nombres que eligió un cliente – solo quedan esas fotos.",
            ],
        ),
        (
            "Etiquetas de color",
            &[
                "Los colores no tienen un significado fijo en Cerno. Dos lecturas habituales:",
                "Estado: rojo revisar · amarillo editar · verde terminado · azul exportado",
                "Uso: rojo cliente · amarillo redes sociales · verde portfolio · azul impresión",
                "6–9 ponen de rojo a azul; la barra de filtros muestra un color.",
            ],
        ),
        (
            "Eliminada no es perdida",
            &[
                "Las fotos eliminadas van a la carpeta oculta .originals junto a las fotos.",
                "La casilla de la papelera en la barra de filtros las muestra; Ctrl+Z devuelve una a su sitio.",
                "Antes del primer enderezado, recorte o giro, Cerno guarda el original; Ctrl+Z lo recupera.",
            ],
        ),
        (
            "La predicción",
            &[
                "Cerno aprende de tus estrellas y de tus fotos rechazadas lo que te gusta; las eliminadas no cuentan: a menudo solo sobraba una entre muchas parecidas.",
                "En las fotos sin estrellas muestra su estimación como estrellas ligeramente rellenas; ponerlas sigue siendo cosa tuya.",
                "Ordenadas por predicción, primero aparecen las fotos que probablemente te gustarán.",
            ],
        ),
        (
            "Bueno saber",
            &[
                "La rueda del ratón sobre la tira de miniaturas recorre las fotos; sobre la foto, acerca y aleja.",
                "Arrastra una carpeta a la ventana para abrirla. Mantén pulsado → para avanzar sin parar.",
                "En la cuadrícula (F7), ↑ ↓ cambian de fila, + − el tamaño, Intro abre la foto.",
                "Enderezar (S): la rueda y las flechas giran, Mayús más fino. Recorte (R): traza un marco o muévelo con las flechas, +/− cambia su tamaño, A el formato, X cambia horizontal/vertical.",
                "La barra de menú (F10 o el botón abajo a la derecha) reúne lo que actúa sobre esta foto, las fotos visibles y la vista, y los ajustes – todo con el ratón.",
                "En la descripción, Intro lleva el cursor al campo de palabras clave.",
            ],
        ),
    ],
    welcome_intro: "Ver, puntuar y descartar fotos sin esperas: las estrellas van al archivo y su fecha no cambia.",
    welcome_keys: [
        ("←, →", "Foto anterior / siguiente"),
        ("1 – 5", "Dar estrellas"),
        ("X", "Rechazar"),
        ("Supr", "Eliminar – Esc la recupera"),
        ("F10", "Barra de menú (botón inferior derecho)"),
    ],
    welcome_more: "Todos los atajos: H",
    setup_line: |exiftool, aesthetics| {
        format!(
            "Configuración: ExifTool {} · estética {} – Barra de menú (F10) › Configuración › Modelos y datos",
            if exiftool { "listo" } else { "falta" },
            if aesthetics { "lista" } else { "falta" },
        )
    },
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
        ("→, Espacio, Av Pág", "Foto siguiente"),
        ("←, Retroceso, Re Pág", "Foto anterior"),
        ("Inicio, Fin", "Primera / última foto"),
        ("Ctrl+O", "Abrir una carpeta"),
        ("Ctrl+U", "Subcarpetas sí / no"),
    ],
    help_rate: [
        ("1 – 5", "Estrellas"),
        ("0", "Sin estrellas, sin rechazo"),
        ("X", "Rechazar"),
        ("6 – 9", "Rojo, amarillo, verde, azul"),
        ("Mayús+…", "Igual, y pasar a la siguiente"),
        ("Ctrl+Z", "Deshacer"),
    ],
    help_cull: [
        ("C", "Comparar dos fotos"),
        ("A, D", "Conservar izquierda / derecha"),
        ("Mayús+C", "Cuatro fotos a la vez"),
        ("M", "Solo fotos parecidas"),
        ("Supr", "Eliminar (a .originals)"),
        ("Esc", "Recuperar las eliminadas"),
    ],
    help_video: [
        ("Espacio", "Reproducir / pausar"),
        ("Mayús+Espacio", "Foto siguiente"),
        ("Alt+←, Alt+→", "5 s atrás / adelante"),
        (",, .", "Un fotograma atrás / adelante"),
        ("↑, ↓", "Volumen"),
    ],
    help_view: [
        ("Z, Doble clic", "Foto completa ↔ 100 %"),
        ("Ctrl+0, Ctrl+1", "Foto completa / 100 %"),
        ("+, −, Rueda del ratón", "Acercar / alejar"),
        ("Arrastrar", "Mover la foto ampliada"),
        ("O", "Revisar: nitidez, recortes"),
        ("F7", "Cuadrícula de todas las fotos"),
        ("G", "Todas las caras en grande"),
        ("F, F11", "Pantalla completa"),
    ],
    help_panels: [
        ("F10", "Barra de menú"),
        ("T", "Barra de filtros"),
        ("Tab", "Panel de detalles"),
        ("F6", "Tira de miniaturas"),
        ("Mayús+Tab", "Los cuatro paneles"),
        ("Ctrl+Tab", "Pestaña siguiente de detalles"),
    ],
    help_edit: [
        ("S", "Enderezar"),
        ("R", "Recorte"),
        ("Intro, Esc", "Aplicar / cancelar"),
        ("Ctrl+←, Ctrl+→", "Girar 90°"),
        ("E", "Editar en otro programa"),
    ],
    help_more: [
        ("Ctrl+L", "Idioma"),
        ("H, F1, ?", "Esta ayuda"),
        ("Esc", "Volver atrás"),
    ],
};
