use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('/'),
    compass: ["N", "S", "E", "O"],

    open: "Ouvrir…",
    open_tooltip: "Ouvrir un dossier (Ctrl+O)",
    photos: |n| {
        if n == 1 {
            "1 photo".to_owned()
        } else {
            format!("{n} photos")
        }
    },
    photos_shown: |shown, total| format!("{shown} sur {total} photos"),
    sort: |key| format!("Tri : {key}"),
    show: |filter| format!("Afficher : {filter}"),
    hide_blurry: "Masquer les floues",
    hide_blurry_tooltip: "Masque les 20 % de photos les plus floues de ce dossier",
    refresh_order: "Actualiser l'ordre",
    refresh_order_tooltip: "De nouveaux scores ont été calculés depuis le tri et le filtrage",
    analyzing_progress: |done, total| format!("Analyse {done} / {total}"),
    analyzed: |total| format!("{total} analysées"),
    enable_aesthetics: "Activer l'esthétique…",
    enable_aesthetics_tooltip: "Télécharge une seule fois le modèle d'image CLIP (1.2 Go)",
    downloading_model: |percent| format!("Téléchargement du modèle {percent:.0} %"),
    aesthetics_ready: "Esthétique : prête",
    aesthetics_loading: "Esthétique : chargement du modèle…",
    aesthetics_backend: |backend| format!("Esthétique : {backend}"),
    aesthetics_backend_tooltip: "Où tourne le modèle d'esthétique (DirectML = carte graphique)",
    aesthetics_failed: "Esthétique : échec",

    sort_name: "Nom",
    sort_rating: "Étoiles",
    sort_laion: "Esthétique (LAION)",
    sort_v25: "Esthétique (V2.5)",
    sort_personal: "Goût personnel",
    sort_sharpness: "Netteté",
    filter_all: "Toutes",
    filter_five: "5 étoiles",
    filter_at_least: |n| {
        if n == 1 {
            "1 étoile ou plus".to_owned()
        } else {
            format!("{n} étoiles ou plus")
        }
    },
    filter_unrated: "Sans étoiles",

    meter_aesthetics: "Esthétique",
    meter_aesthetics_tooltip: "LAION / V2.5 / votre goût personnel (étoiles)\nÉchelle de 1 à 10, la plupart des photos entre 4 et 6. – = pas encore disponible",
    meter_sharpness: "Netteté",
    meter_eyes: "Yeux",
    probably_blurry: "sans doute floue",
    analyzing: "Analyse en cours…",
    saving: "Enregistrement…",
    star_tooltip: |n| format!("{n} ★ – touche {n}"),
    zoom: |percent| format!("Zoom {percent:.0} %"),
    digital_zoom: |ratio| format!("Zoom numérique {ratio:.1}×"),
    map_tooltip: |place| format!("{place}\nCliquez pour ouvrir dans Google Maps"),
    button_toolbar: "Barre du haut (B)",
    button_details: "Panneau de détails (P)",
    button_filmstrip: "Pellicule (T)",
    button_help: "Aide (H)",
    button_language: |name| format!("Langue : {name} (L)"),

    loading: "Chargement…",
    cannot_show: "Impossible d'afficher cette image",
    no_match: "Aucune photo ne correspond au filtre",
    drop_to_open: "Relâchez pour ouvrir",
    compare_left: "Gauche",
    compare_right: "Droite",
    compare_left_badge: "G",
    keeps_this: |key| format!("{key} garde celle-ci"),
    compare_needs_two: "Il faut au moins deux photos pour comparer",
    deleting: |n| {
        if n == 1 {
            "1 photo en cours de suppression   ·   Esc pour annuler".to_owned()
        } else {
            format!("{n} photos en cours de suppression   ·   Esc pour annuler")
        }
    },
    delete_failed: |n, name, err| format!("Impossible de supprimer {n} photo(s) – {name} : {err}"),
    blurry_tooltip: |eyes, percent| {
        let what = if eyes {
            "les yeux sont plus nets"
        } else {
            "la photo est plus nette"
        };
        format!("Sans doute floue : {what} que seulement {percent:.0} % des photos de ce dossier")
    },

    db_unavailable: |err| {
        format!("Les scores ne sont pas enregistrés pendant cette session : {err}")
    },
    cannot_open: |path, err| format!("Impossible d'ouvrir {path} : {err}"),
    no_photos_in: |dir| format!("Aucun fichier JPEG ou HEIC dans {dir}"),
    rating_not_saved: |err| format!("Étoiles non enregistrées – {err}"),
    open_folder: "Ouvrir un dossier",
    download_title: "Activer l'évaluation esthétique",
    download_text: |gb| {
        format!(
            "Pour évaluer l'esthétique, Cerno a besoin du modèle d'image CLIP ViT-L/14.\n\n\
             Voulez-vous le télécharger maintenant depuis Hugging Face \
             (Xenova/clip-vit-large-patch14, {gb:.1} Go) ? \
             Il est enregistré dans le dossier de données de Cerno et téléchargé une seule fois."
        )
    },

    section_aesthetics: "Esthétique",
    section_sharpness: "Netteté (dans le dossier)",
    section_exposure: "Exposition",
    section_attributes: "Critères CLIP",
    section_models: "Modèles",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "Goût personnel",
    row_frame: "Image entière",
    row_eyes: "Yeux",
    row_highlights: "Hautes lumières brûlées",
    row_shadows: "Ombres bouchées",
    attributes: [
        "Qualité globale",
        "Nette",
        "Bonne lumière",
        "Bien composée",
        "Peu de bruit",
        "Colorée",
    ],
    explain_laion: "La beauté de la photo aux yeux d'une IA qui a appris de nombreuses notes données par des personnes. De 1 à 10 : la plupart des photos obtiennent entre 4 et 6, au-dessus de 6, c'est très bon.",
    explain_v25: "Une IA plus récente pour la même question, meilleure avec les photos du quotidien. Même échelle de 1 à 10.",
    explain_personal: "Les étoiles que vous donneriez selon Cerno. Il apprend de vos propres étoiles et des photos que vous supprimez.",
    explain_frame: "La netteté des zones les plus nettes, comparée aux autres photos de ce dossier. 80 % signifie plus nette que 80 % d'entre elles.",
    explain_eyes: "La netteté au niveau des yeux, s'il y a un visage. Pour un portrait, c'est elle qui compte, pas l'arrière-plan.",
    explain_highlights: "Les zones d'un blanc pur, où il ne reste aucun détail. Au-delà de 1 %, mieux vaut vérifier.",
    explain_shadows: "Les zones d'un noir pur, où il ne reste aucun détail. C'est souvent voulu ; au-delà de 5 %, c'est signalé.",
    explain_attributes: "L'IA compare la photo à deux descriptions opposées. 50 % signifie indécis, près de 100 % clairement la première.",
    explain_attribute: [
        "bonne photo – mauvaise photo",
        "nette – floue",
        "bonne lumière – mauvaise lumière",
        "bien composée – mal composée",
        "propre – bruitée",
        "colorée – terne",
    ],
    explain_models: "Où tourne chaque IA : DirectML = carte graphique, CPU = processeur. ± indique de combien votre modèle de goût se trompe en général.",
    note_no_embedding: "pas encore de données CLIP",
    note_learning: |n, of| format!("apprentissage – {n} sur {of} photos"),
    note_analysing: "analyse en cours…",
    note_no_face: "aucun visage",
    note_faces_too_small: |n| {
        if n == 1 {
            "1 visage, trop petit".to_owned()
        } else {
            format!("{n} visages, trop petits")
        }
    },
    note_needs_clip: "nécessite le modèle CLIP",
    model_missing: "non installé",
    model_downloading: |percent| format!("téléchargement {percent:.0} %"),
    model_ready: "prêt",
    model_loading: "chargement…",
    model_failed: "échec",
    model_faces: "Visages",
    model_personal: "Goût",
    taste_trained: |n, error| format!("{n} photos, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} photos"),
    taste_untrained: "pas encore entraîné",

    help_title: "Aide",
    help_intro: "Cerno affiche vos photos instantanément et vous aide à faire le tri. Les étoiles sont écrites directement dans le fichier photo, pour que les autres logiciels les voient aussi – la date du fichier reste inchangée. Tout le reste est conservé dans la base de données de Cerno. La netteté et la beauté sont évaluées automatiquement en arrière-plan ; P affiche toutes les valeurs avec une explication.",
    help_drop: "Déposez un dossier ou une photo sur la fenêtre, ou appuyez sur Ctrl+O.",
    help_close: "Esc, H ou F1 ferme cette page",
    help_sections: ["Parcourir", "Noter et trier", "Affichage", "Divers"],
    help_browse: [
        (
            "→, Espace, Pg suiv",
            "Photo suivante (maintenir pour faire défiler)",
        ),
        ("←, Retour arrière, Pg préc", "Photo précédente"),
        ("Début, Fin", "Première / dernière photo"),
        ("Molette", "Sur la pellicule : faire défiler les photos"),
        ("Ctrl+O", "Ouvrir un dossier (ou le déposer sur la fenêtre)"),
    ],
    help_rate: [
        (
            "1 – 5",
            "Donner des étoiles – écrites dans le fichier photo",
        ),
        ("0", "Retirer les étoiles"),
        ("Suppr", "Supprimer : va dans la corbeille après 5 secondes"),
        ("Esc", "Récupérer les photos en attente de suppression"),
        (
            "C",
            "Comparer : fixer cette photo à gauche, parcourir à droite",
        ),
        (
            "A, D",
            "Comparer : garder la gauche / la droite – l'autre est supprimée",
        ),
    ],
    help_view: [
        ("Z, Double-clic", "Photo entière ↔ 100 %"),
        ("+, −, Molette", "Zoom avant / arrière"),
        ("Glisser", "Déplacer la photo zoomée"),
        ("F11, F", "Plein écran"),
        ("B", "Afficher ou masquer la barre du haut"),
        ("P", "Panneau de détails avec toutes les valeurs"),
        ("T", "Pellicule"),
        ("I", "Barre du haut, détails et pellicule ensemble"),
    ],
    help_more: [
        ("L", "Changer de langue"),
        ("H, F1", "Cette aide"),
        ("Esc", "Revenir en arrière : zoom, comparaison, plein écran"),
    ],
};
