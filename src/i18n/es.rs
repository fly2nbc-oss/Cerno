use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('/'),
    compass: ["N", "S", "E", "O"],

    open: "Abrir…",
    open_tooltip: "Abrir carpeta (Ctrl+O)",
    photos: |n| {
        if n == 1 {
            "1 foto".to_owned()
        } else {
            format!("{n} fotos")
        }
    },
    photos_shown: |shown, total| format!("{shown} de {total} fotos"),
    sort: |key| format!("Orden: {key}"),
    show: |filter| format!("Mostrar: {filter}"),
    hide_blurry: "Ocultar borrosas",
    hide_blurry_tooltip: "Oculta las fotos más borrosas (el 20 % de esta carpeta)",
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
    sort_personal: "Gusto personal",
    sort_sharpness: "Nitidez",
    filter_all: "Todas",
    filter_five: "5 estrellas",
    filter_at_least: |n| format!("{n}+ estrellas"),
    filter_unrated: "Sin estrellas",

    meter_aesthetics: "Estética",
    meter_aesthetics_tooltip: "LAION / V2.5 / tu gusto personal (estrellas)\nEscala 1–10, la mayoría de las fotos 4–6. – = aún no disponible",
    meter_sharpness: "Nitidez",
    meter_eyes: "Ojos",
    probably_blurry: "probablemente borrosa",
    analyzing: "Analizando…",
    saving: "Guardando…",
    star_tooltip: |n| format!("{n} ★ – tecla {n}"),
    zoom: |percent| format!("Zoom {percent:.0} %"),
    digital_zoom: |ratio| format!("Zoom digital {ratio:.1}×"),
    map_tooltip: |place| format!("{place}\nHaz clic para abrirlo en Google Maps"),
    button_toolbar: "Barra superior (B)",
    button_details: "Panel de detalles (P)",
    button_filmstrip: "Tira de miniaturas (T)",
    button_help: "Ayuda (H)",
    button_language: |name| format!("Idioma: {name} (L)"),

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
    download_title: "Activar la puntuación estética",
    download_text: |gb| {
        format!(
            "Cerno necesita el modelo de imagen CLIP ViT-L/14 para puntuar la estética.\n\n\
             ¿Descargarlo ahora de Hugging Face (Xenova/clip-vit-large-patch14, {gb:.1} GB)? \
             Se guarda en la carpeta de datos de Cerno y solo se descarga una vez."
        )
    },

    section_aesthetics: "Estética",
    section_sharpness: "Nitidez (en la carpeta)",
    section_exposure: "Exposición",
    section_attributes: "Atributos CLIP",
    section_models: "Modelos",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "Gusto personal",
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
    explain_laion: "Lo bonita que le parece la foto a una IA entrenada con muchas valoraciones de personas. 1–10; la mayoría de las fotos obtienen 4–6, más de 6 es muy bueno.",
    explain_v25: "Una IA más reciente para la misma pregunta, mejor con fotos del día a día. Misma escala 1–10.",
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
    explain_models: "Dónde se ejecuta cada IA: DirectML = tarjeta gráfica, CPU = procesador. ± indica cuánto suele desviarse tu modelo de gusto.",
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
    model_personal: "Gusto",
    taste_trained: |n, error| format!("{n} fotos, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} fotos"),
    taste_untrained: "aún sin entrenar",

    help_title: "Ayuda",
    help_intro: "Cerno muestra tus fotos al instante y te ayuda a seleccionarlas. Las estrellas se guardan directamente en el archivo de la foto, así que otros programas también las ven – la fecha del archivo no cambia. Todo lo demás se queda en la base de datos propia de Cerno. La nitidez y la belleza se valoran automáticamente en segundo plano; P muestra todos los valores con una explicación.",
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
        ("0", "Quitar las estrellas"),
        ("Supr", "Eliminar: va a la papelera tras 5 segundos"),
        ("Esc", "Recuperar las fotos que esperan a ser eliminadas"),
        (
            "C",
            "Comparar: fijar esta foto a la izquierda, navegar a la derecha",
        ),
        (
            "A, D",
            "Comparar: conservar la izquierda / la derecha – la otra se elimina",
        ),
    ],
    help_view: [
        ("Z, Doble clic", "Foto completa ↔ 100 %"),
        ("+, −, Rueda del ratón", "Acercar / alejar"),
        ("Arrastrar", "Mover la foto ampliada"),
        ("F11, F", "Pantalla completa"),
        ("B", "Mostrar u ocultar la barra superior"),
        ("P", "Panel de detalles con todos los valores"),
        ("T", "Tira de miniaturas"),
        (
            "I",
            "Barra superior, detalles y tira de miniaturas a la vez",
        ),
    ],
    help_more: [
        ("L", "Cambiar idioma"),
        ("H, F1", "Esta ayuda"),
        (
            "Esc",
            "Volver atrás: zoom, modo comparación, pantalla completa",
        ),
    ],
};
