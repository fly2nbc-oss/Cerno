use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('/'),
    compass: ["N", "S", "E", "O"],
    key_ctrl: "Ctrl",
    key_shift: "Maj",

    sort: |key| format!("Tri : {key}"),
    filter_stars: |n| {
        if n == 1 {
            "1 étoile".to_owned()
        } else {
            format!("{n} étoiles")
        }
    },
    filter_blurry: "Floues",
    filter_blurry_tooltip: "Les 20 % de photos les plus floues de ce dossier",
    filter_duplicate: "Doublons",
    filter_duplicate_tooltip: "Chaque photo sauf le premier chemin identique",
    filter_clear: "Tout afficher",
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
    sort_personal: "Pour vous",
    sort_sharpness: "Netteté",
    sort_taken: "Date de prise",
    filter_unrated: "Sans étoiles",

    filter_rejected: "Rejetées",
    actions: "Action",
    actions_tooltip: "Copier, déplacer ou supprimer les photos affichées",
    selection_delete: "Supprimer",
    confirm_copy_title: "Copier la sélection ?",
    confirm_copy: |n| format!("Copier {n} photos vers un autre dossier ?"),
    confirm_move_title: "Déplacer la sélection ?",
    confirm_move: |n| format!("Déplacer {n} photos vers un autre dossier ?"),
    confirm_delete_selection_title: "Supprimer la sélection ?",
    confirm_delete_selection: |n| {
        format!(
            "Mettre {n} photos à la corbeille ? Échap les ramène tant que le compte à rebours tourne."
        )
    },
    label_red: "Rouge",
    label_yellow: "Jaune",
    label_green: "Vert",
    label_blue: "Bleu",
    label_purple: "Violet",
    meter_aesthetics: "Esthétique",
    meter_aesthetics_tooltip: "L et V : esthétique (LAION / V2.5). ★ : Pour vous – les étoiles que Cerno pense que vous donneriez. Tous de 0 à 5\n– = pas encore disponible",
    meter_sharpness: "Netteté",
    meter_eyes: "Yeux",
    probably_blurry: "sans doute floue",
    analyzing: "Analyse en cours…",
    saving: "Enregistrement…",
    auto_advance_on: "Avance auto",
    series_position: |index, len| format!("Série {index} / {len}"),
    series_more: |n| format!("+{n}"),
    duplicate_of: |name| format!("Doublon de {name}"),
    rejected: "Rejetée",
    star_tooltip: |n| format!("{n} ★ – touche {n}"),
    zoom: |percent| format!("Zoom {percent:.0} %"),
    digital_zoom: |ratio| format!("Zoom numérique {ratio:.1}×"),
    map_tooltip: |place| format!("{place}\nCliquez pour ouvrir dans Google Maps"),
    button_toolbar: "Trier et filtrer",
    button_details: "Panneau de détails",
    button_filmstrip: "Pellicule",
    button_help: "Aide",
    button_menu: "Menu",
    button_language: |name| format!("Langue : {name}"),

    cmd_explanations: "Déplier ou replier toutes les explications du panneau de détails",
    cmd_all_panels: "Trier et filtrer, détails et pellicule",
    cmd_fullscreen: "Plein écran",
    cmd_compare: "Comparer",
    cmd_zoom: "Photo entière ↔ 100 %",
    cmd_first: "Première photo",
    cmd_last: "Dernière photo",
    cmd_language: |name| format!("Langue : {name}"),
    cmd_reject: "Rejeter",
    cmd_delete_rejected: |n| format!("Supprimer les photos rejetées ({n})"),
    cmd_auto_advance: "Avancer automatiquement",
    cmd_subfolders: "Inclure les sous-dossiers",
    cmd_best_of_series: "Meilleure de chaque série",
    cmd_label: |name| format!("Couleur : {name}"),
    menu_sort: "Trier",
    menu_filter: "Filtre",
    menu_view: "Affichage",
    menu_edit: "Modifier",
    menu_photo: "Photo",
    menu_labels: "Couleurs",
    menu_language: "Langue",
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
    transfer_copy: "Copier",
    transfer_move: "Déplacer",
    transfer_copy_cmd: "Copier tout ce que le filtre actuel affiche vers …",
    transfer_move_cmd: "Déplacer tout ce que le filtre actuel affiche vers …",
    transfer_same_folder: "C'est déjà le dossier ouvert",
    transfer_busy: "Une copie ou un déplacement est déjà en cours",
    transfer_done: |moved, done, skipped, name, err| {
        let verb = if moved { "déplacées" } else { "copiées" };
        let mut text = format!("{done} {verb}, {skipped} ignorées");
        if !name.is_empty() {
            text.push_str(&format!(" – {name} : {err}"));
        }
        text
    },
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
    section_histogram: "Histogramme",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "Pour vous",
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
    explain_laion: "La beauté de la photo aux yeux d'une IA qui a appris de nombreuses notes données par des personnes, convertie en étoiles de 0 à 5. La plupart des photos obtiennent 2 à 3, à partir de 4 c'est très bon.",
    explain_v25: "Une IA plus récente pour la même question, meilleure avec les photos du quotidien. Même échelle d'étoiles.",
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
    explain_models: "Où tourne chaque IA : DirectML = carte graphique, CPU = processeur. ± indique de combien « Pour vous » se trompe en général.",
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
    model_personal: "Pour vous",
    taste_trained: |n, error| format!("{n} photos, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} photos"),
    taste_untrained: "pas encore entraîné",
    btn_reset_taste: "Réinitialiser Pour vous",
    btn_delete_models: "Supprimer les modèles",
    copy_models_path: "Copier le chemin",
    models_path_copied: "Chemin copié",
    confirm_reset_taste_title: "Réinitialiser Pour vous ?",
    confirm_reset_taste_text: "Cerno oubliera ce qu'il a appris de vos étoiles et suppressions. Les étoiles dans les fichiers photo restent inchangées.",
    confirm_delete_models_title: "Supprimer les modèles téléchargés ?",
    confirm_delete_models_text: "Supprime les fichiers des modèles CLIP et SigLIP du disque (environ 3 Go). Les scores enregistrés restent dans la base ; l'esthétique pourra être téléchargée à nouveau.",

    cmd_straighten: "Redresser",
    cmd_rotate_ccw: "Pivoter de 90° vers la gauche",
    cmd_rotate_cw: "Pivoter de 90° vers la droite",
    cmd_crop: "Recadrer",
    edit_not_jpeg: "Redressement et recadrage uniquement pour les JPEG.",
    edit_writing: "Écriture de la photo…",
    edit_reencoded: "JPEG réencodé. La qualité a été réécrite une fois.",
    edit_failed: |detail| format!("Pas enregistré : {detail}"),
    edit_hint: "Entrée applique, Échap annule",
    ratio_original: "Original",
    crop_landscape: "Paysage",
    crop_portrait: "Portrait",

    help_title: "Aide",
    help_intro: "Cerno affiche vos photos instantanément et vous aide à faire le tri. Les étoiles sont écrites directement dans le fichier photo, pour que les autres logiciels les voient aussi – la date du fichier reste inchangée. Tout le reste est conservé dans la base de données de Cerno. La netteté et la beauté sont évaluées automatiquement en arrière-plan ; Tab ouvre le panneau de détails, I déplie ou replie toutes les explications, Ctrl+K ouvre le menu.",
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
        (
            "Maj+1 – 5",
            "Donner des étoiles et passer à la photo suivante",
        ),
        ("0", "Retirer les étoiles ou le rejet"),
        (
            "X, Maj+X",
            "Rejeter – noté dans le fichier, rien n'est supprimé ; avec Maj, passer à la photo suivante",
        ),
        ("Suppr", "Supprimer : va dans la corbeille après 5 secondes"),
        ("Esc", "Récupérer les photos en attente de suppression"),
        (
            "C",
            "Comparer : fixer cette photo à gauche, parcourir à droite",
        ),
        (
            "A, D",
            "Comparer : garder la gauche / la droite – l'autre est rejetée",
        ),
        (
            "6 – 9",
            "Couleur : rouge, jaune, vert, bleu – une seconde fois l'enlève",
        ),
        (
            "Maj+6 – 9",
            "Poser cette couleur et passer à la photo suivante",
        ),
    ],
    help_view: [
        ("Z, Double-clic", "Photo entière ↔ 100 %"),
        ("+, −, Molette", "Zoom avant / arrière"),
        ("Glisser", "Déplacer la photo zoomée"),
        ("F11", "Plein écran"),
        ("F", "Trier et filtrer"),
        ("Tab", "Panneau de détails"),
        ("I", "Détails : déplier ou replier toutes les explications"),
        ("F6", "Pellicule"),
        ("Maj+Tab", "Trier et filtrer, détails et pellicule ensemble"),
        (
            "S",
            "Redresser : grille, molette et flèches tournent, Maj plus fin, Entrée applique",
        ),
        (
            "Ctrl+←, Ctrl+→",
            "Pivoter de 90° – sans perte, via l'orientation JPEG",
        ),
        (
            "R",
            "Recadrer : tracer un cadre, X bascule paysage/portrait, A change le format",
        ),
        (
            "Entrée, Échap",
            "Appliquer ou annuler redressement et recadrage",
        ),
    ],
    help_more: [
        ("Ctrl+K", "Menu : toutes les fonctions"),
        (
            "Ctrl+M",
            "Action : copier, déplacer ou supprimer les photos affichées",
        ),
        ("Ctrl+L", "Changer de langue"),
        ("H, F1, ?", "Cette aide"),
        ("Esc", "Revenir en arrière : zoom, comparaison, plein écran"),
    ],
};
