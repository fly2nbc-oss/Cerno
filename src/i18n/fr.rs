use super::{DateStyle, Texts};

pub static TEXTS: Texts = Texts {
    date_style: DateStyle::DayMonthYear('/'),
    compass: ["N", "S", "E", "O"],
    size_units: ["Ko", "Mo", "Go"],
    key_ctrl: "Ctrl",
    key_shift: "Maj",
    key_delete: "Suppr",

    sort: |key| format!("Tri : {key}"),
    filter_stars: |n| {
        if n == 1 {
            "1 étoile".to_owned()
        } else {
            format!("{n} étoiles")
        }
    },
    filter_blurry: "Floues",
    filter_blurry_tooltip: "Parmi les 20 % de photos les plus floues de ce dossier, et nettement floues",
    filter_duplicate: "Doublons",
    filter_duplicate_tooltip: "Chaque photo sauf le premier chemin identique",
    filter_people: "Avec des personnes",
    filter_no_people: "Sans personnes",
    filter_people_tooltip: "Repérées à leur visage : les personnes de dos ou toutes petites dans l'image ne comptent pas.",
    filter_deleted: "Supprimées",
    filter_deleted_tooltip: "Photos supprimées de ce dossier. Elles se trouvent dans le dossier caché .originals – Ctrl+Z en remet une en place.",
    filter_deleted_none: "Aucune photo supprimée dans ce dossier",
    filter_hide_rejected: "Masquer les rejetées",
    filter_without: "sans",
    filter_hide_rejected_tooltip: "Les photos rejetées disparaissent, toutes les autres restent. × montre au contraire seulement les rejetées.",
    filter_clear: "Tout afficher",
    media_all: "Photos et vidéos",
    media_photos: "Photos seulement",
    media_videos: "Vidéos seulement",
    media_no_videos: "Ce dossier ne contient pas de vidéos",
    top_photos: |n| format!("Top {n} photos"),
    top_purposes: [
        "Temps forts",
        "Aperçu",
        "Diaporama",
        "Livre photo",
        "Galerie",
    ],
    top_tooltip: "Photos, vidéos ou les deux – ou seulement les meilleures photos, selon vos étoiles (sinon la prédiction), l'esthétique et la netteté, la meilleure de chaque série d'abord. Les photos rejetées ou floues et les doublons ne comptent pas. La sélection reste jusqu'à ce qu'un filtre change ou que vous choisissiez « Actualiser l'ordre ». Rien n'est modifié dans les photos.",
    menu_top: "Meilleures photos",
    bulk_delete_top: "Pas avec Top – ce sont les meilleures photos",
    filter_none_active: "Aucun filtre actif",
    filter_similar: "≈ Semblables",
    filter_similar_to: |name| format!("≈ comme {name}"),
    menu_similar: "Photos semblables",
    menu_similar_to: |name| format!("Semblables à {name}"),
    similar_tooltip: |percent| {
        format!(
            "Seulement les photos qui ressemblent à la photo actuelle (dès {percent:.0} %) – touche M"
        )
    },
    similar_fact: |percent| format!("Ressemblance {percent:.0} %"),
    similar_needs_model: "Les photos semblables ont besoin du modèle d'esthétique (CLIP)",
    similar_not_analysed: "Cette photo n'est pas encore analysée",
    similar_none: |percent| format!("Aucune photo semblable (dès {percent:.0} %)"),
    refresh_order: "Actualiser l'ordre",
    refresh_order_tooltip: "De nouveaux scores ont été calculés depuis le tri et le filtrage",
    analyzing_progress: |done, total| format!("Analyse {done} / {total}"),
    enable_aesthetics: "Activer l'esthétique…",
    enable_aesthetics_tooltip: |size| {
        format!("Télécharge une seule fois les modèles d'image pour l'esthétique ({size})")
    },
    add_v25: "Charger V2.5…",
    add_v25_tooltip: |size| {
        format!(
            "Télécharge une seule fois le second modèle d'esthétique (SigLIP + V2.5, {size}) – l'esthétique devient la moyenne des deux modèles"
        )
    },
    downloading_model: |percent| format!("Téléchargement du modèle {percent:.0} %"),
    aesthetics_loading: "Esthétique : chargement du modèle…",
    aesthetics_failed: "Esthétique : échec",

    sort_name: "Nom",
    sort_rating: "Étoiles",
    sort_aesthetics: "Esthétique",
    sort_personal: "Prédiction",
    sort_sharpness: "Netteté",
    sort_taken: "Date de prise",
    filter_unrated: "Sans étoiles",

    filter_rejected: "Rejetées",
    actions: "Action",
    actions_tooltip: "Copier, déplacer ou supprimer les photos affichées",
    selection_delete: "Supprimer",
    bulk_copy: |n| {
        format!(
            "Copier vers … ({n} {})",
            if n == 1 { "photo" } else { "photos" }
        )
    },
    bulk_move: |n| {
        format!(
            "Déplacer vers … ({n} {})",
            if n == 1 { "photo" } else { "photos" }
        )
    },
    bulk_delete: |n| {
        format!(
            "Supprimer ({n} {})",
            if n == 1 { "photo" } else { "photos" }
        )
    },
    bulk_delete_hint: "Toutes les photos que le filtre affiche. Après 5 secondes, elles vont dans le dossier caché .originals à côté d'elles – rien n'est supprimé définitivement, Esc les ramène.",
    delete_rejected_hint: "Toutes les photos rejetées du dossier, même celles que le filtre masque. Après 5 secondes, elles vont dans le dossier caché .originals – rien n'est supprimé définitivement.",
    bulk_restore: |n| {
        format!(
            "Remettre en place ({n} {})",
            if n == 1 { "photo" } else { "photos" }
        )
    },
    bulk_restore_hint: "Toutes les photos supprimées que montre le filtre retournent dans leur dossier. Si le nom y est déjà pris, la photo reçoit un numéro – rien n'est écrasé.",
    photos_shown: |shown, total| format!("{shown} sur {total} photos"),
    photos_count: |n| format!("{n} {}", if n == 1 { "photo" } else { "photos" }),
    photos_badge_tooltip: "Le nombre de photos que le filtre affiche – « Action » agit sur exactement celles-ci.",
    label_red: "Rouge",
    label_yellow: "Jaune",
    label_green: "Vert",
    label_blue: "Bleu",
    label_purple: "Violet",
    meter_aesthetics_tooltip: "Esthétique : moyenne de LAION et V2.5 sur une échelle fixe – une photo a la même valeur dans chaque dossier. La netteté, elle, compare avec les autres photos du dossier.\nValeurs séparées : panneau des détails (Tab)",
    meter_sharpness: "Netteté",
    meter_eyes: "Yeux",
    probably_blurry: "sans doute floue",
    analyzing: "Analyse en cours…",
    saving: "Enregistrement…",
    auto_advance_on: "Avance auto",
    series_position: |index, len| format!("Série {index} / {len}"),
    duplicate_of: |name| format!("Doublon de {name}"),
    rejected: "Rejetée",
    filmstrip_video: "Vidéo",
    star_tooltip: |n| format!("{n} ★ – touche {n}"),
    personal_hint: |stars| {
        format!(
            "Prédiction : {stars:.1} ★ – les étoiles que Cerno pense que vous donneriez. Pas encore votre note"
        )
    },
    zoom: |percent| format!("Zoom {percent:.0} %"),
    zoom_preview: |percent| format!("Zoom {percent:.0} % de l'aperçu"),
    digital_zoom: |ratio| format!("Zoom numérique {ratio:.1}×"),
    button_toolbar: "Barre de filtres",
    button_details: "Panneau de détails",
    button_filmstrip: "Pellicule",
    button_help: "Aide",
    button_menu: "Menu",
    button_language: |name| format!("Langue : {name}"),

    cmd_all_panels: "Barre de filtres, détails et pellicule",
    cmd_fullscreen: "Plein écran",
    cmd_compare: "Comparer",
    cmd_similar: "N'afficher que les photos semblables",
    cmd_zoom: "Photo entière ↔ 100 %",
    menu_overlay: "Superposition",
    overlay_off: "Désactivée",
    overlay_sharpness: "Contours nets",
    overlay_exposure: "Hautes lumières et ombres écrêtées",
    overlay_fact_sharpness: "Superposition : netteté",
    overlay_fact_exposure: "Superposition : exposition",
    overlay_hint_sharpness: "Netteté : le violet marque les contours les plus nets de la photo",
    overlay_hint_exposure: "Exposition : rouge = brûlé, bleu = bouché",
    overlay_hint_off: "Superposition désactivée",
    overlay_show_on_photo: "Afficher sur la photo (O)",
    cmd_grid: "Grille",
    cmd_reject: "Rejeter",
    cmd_description: "Commentaire et mots-clés",
    cmd_delete_rejected: |n| {
        format!(
            "Supprimer les rejetées ({n} {})",
            if n == 1 { "photo" } else { "photos" }
        )
    },
    cmd_auto_advance: "Avancer automatiquement",
    cmd_subfolders: "Inclure les sous-dossiers",
    subfolders_on: "Sous-dossiers inclus",
    subfolders_off: "Ce dossier seulement, sans sous-dossiers",
    menu_sort: "Trier",
    menu_filter: "Filtre",
    menu_view: "Affichage",
    menu_labels: "Couleurs",
    menu_external: "Modifier ailleurs",
    external_other: "Autre programme …",
    external_chooser: "« Ouvrir avec » du système …",
    external_default: "Ouvrir avec le programme par défaut",
    external_pick_title: "Choisir un programme pour modifier",
    external_opened: |name| {
        format!(
            "Ouvert dans {name} – après l'enregistrement, Cerno affiche la nouvelle version ; l'original reste dans .originals"
        )
    },
    external_reloaded: |name| format!("{name} a été enregistré ailleurs – rechargé"),
    external_failed: |err| format!("Pas ouvert : {err}"),
    menu_language: "Langue",
    menu_models: "Modèles et données",
    menu_this_photo: "Cette photo",
    menu_visible: "Photos affichées",
    menu_settings: "Réglages",
    menu_stars: "Étoiles",
    label_none: "Sans couleur",
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
            "1 photo va dans le dossier caché .originals   ·   Esc pour annuler".to_owned()
        } else {
            format!("{n} photos vont dans le dossier caché .originals   ·   Esc pour annuler")
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
    transfer_progress: |moved, at, total, sizes, file| {
        let verb = if moved { "Déplacement de" } else { "Copie de" };
        let mut text = format!("{verb} {at} / {total} photos – {sizes}");
        if !file.is_empty() {
            text.push_str(&format!(" – {file}"));
        }
        text
    },
    download_title: "Activer l'évaluation esthétique",
    download_text: |clip, v25, size| {
        let mut text = String::from("Il manque pour l'évaluation esthétique :\n");
        if clip {
            text.push_str(
                "\n• CLIP ViT-L/14 – depuis Hugging Face (Xenova/clip-vit-large-patch14)",
            );
        }
        if v25 {
            text.push_str("\n• SigLIP + Aesthetic Predictor V2.5 – depuis la release GitHub models-1 de Cerno (la partie V2.5 est sous AGPL-3.0)");
        }
        text.push_str(&format!(
            "\n\nTélécharger maintenant ({size}) ? Les fichiers vont dans le dossier de données de Cerno et ne sont téléchargés qu'une fois ; un téléchargement interrompu reprend la fois suivante."
        ));
        text
    },
    btn_download: "Télécharger",
    btn_cancel: "Annuler",
    btn_close: "Fermer (Échap)",
    aesthetics_offer: |size| {
        format!(
            "L'évaluation esthétique a besoin de modèles d'image ({size}) : Menu → Modèles et données."
        )
    },
    v25_offer: |size| {
        format!(
            "Une esthétique plus fiable avec le second modèle V2.5 ({size}) : Menu → Modèles et données."
        )
    },

    section_aesthetics: "Esthétique",
    section_sharpness: "Netteté (dans le dossier)",
    section_exposure: "Exposition",
    section_attributes: "Critères CLIP",
    row_aesthetics: "Esthétique (moyenne)",
    section_histogram: "Histogramme",
    section_file: "Fichier",
    row_size: "Taille",
    row_load_time: "Temps de chargement",
    row_file_size: "Taille du fichier",
    row_jpeg_quality: "Qualité JPEG",
    explain_jpeg_quality: "Estimée d'après les tables de quantification du fichier – la qualité elle-même n'y est pas enregistrée. 4:2:0 : couleur en demi-résolution (courant pour les appareils), 4:4:4 : en pleine résolution.",
    explain_raw_preview: "Pour les fichiers RAW, Cerno montre le JPEG intégré par l'appareil – aucun développement RAW. Couleurs et exposition sont celles de l'appareil, et 100 % correspond à la taille de cet aperçu, pas à celle du capteur. L'histogramme et l'exposition mesurent l'aperçu.",
    row_container: "Conteneur",
    row_duration: "Durée",
    row_video: "Vidéo",
    row_frame_rate: "Images/s",
    row_video_bitrate: "Débit vidéo",
    row_audio: "Audio",
    row_audio_bitrate: "Débit audio",
    row_bitrate: "Débit total",
    no_audio: "pas de piste audio",
    variable_frame_rate: "variable",
    channels: |n| match n {
        1 => "Mono".to_owned(),
        2 => "Stéréo".to_owned(),
        6 => "5.1".to_owned(),
        8 => "7.1".to_owned(),
        n => format!("{n} canaux"),
    },
    row_location: "Lieu",
    tab_values: "Valeurs",
    tab_description: "Description",
    tab_faces: "Visages",
    section_comment: "Commentaire",
    section_keywords: "Mots-clés",
    comment_hint: "Écrire un commentaire …",
    keyword_hint: "Ajouter un mot-clé …",
    keyword_remove: "Retirer",
    description_note: "Entrée ajoute un mot-clé, Échap quitte le champ. Le commentaire et les mots-clés sont écrits dans le fichier (IPTC et XMP), là où Windows, Lightroom et digiKam les lisent.",
    description_waiting: "Lecture …",
    row_laion: "LAION (CLIP)",
    row_v25: "V2.5 (SigLIP)",
    row_personal: "Prédiction",
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
    explain_laion: "La beauté de la photo aux yeux d'une IA qui a appris de nombreuses notes données par des personnes. Elle aime surtout les personnes, les portraits et les plats.",
    explain_v25: "Une IA plus récente pour la même question, meilleure avec les photos du quotidien. Elle aime surtout les paysages, l'eau et les vues aériennes.",
    explain_personal: "Les étoiles que vous donneriez selon Cerno. Elle apprend de vos propres étoiles et des photos que vous rejetez ou supprimez.",
    explain_aesthetics: "La valeur sous la photo : la moyenne des deux IA ci-dessous, de 0 % (peu attrayante) à 100 % (très attrayante). La plupart des photos se situent entre 40 et 60 %, à partir de 80 % c'est très bon. L'échelle est fixe – une photo a la même valeur dans chaque dossier.",
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
    explain_models: "Où tourne chaque IA : DirectML = carte graphique, CPU = processeur. ± indique de combien la prédiction se trompe en général.",
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
    model_removing: "suppression…",
    model_failed: "échec",
    model_faces: "Visages",
    model_personal: "Prédiction",
    taste_trained: |n, error| format!("{n} photos, ±{error:.1} ★"),
    taste_photos: |n| format!("{n} photos"),
    taste_untrained: "pas encore entraîné",
    taste_sources: |stars, rejected, deleted| {
        format!(
            "Apprise de {stars} photos avec étoiles, {rejected} rejetées et {deleted} supprimées – les photos rejetées et supprimées comptent pour 0 ★."
        )
    },
    btn_reset_taste: "Réinitialiser la prédiction",
    btn_delete_models: "Supprimer les modèles",
    models_deleted: "Modèles supprimés",
    models_downloaded: "Modèles téléchargés – l'esthétique est en cours de calcul",
    download_failed: |err| {
        format!("Échec du téléchargement : {err} – un nouvel essai reprend là où il s'est arrêté")
    },
    copy_models_path: "Copier le chemin",
    models_path_copied: "Chemin copié",
    confirm_reset_taste_title: "Réinitialiser la prédiction ?",
    confirm_reset_taste_text: "Cerno oubliera ce qu'il a appris de vos étoiles et suppressions. Les étoiles dans les fichiers photo restent inchangées.",
    confirm_delete_models_title: "Supprimer les modèles téléchargés ?",
    confirm_delete_models_text: |size| {
        format!(
            "Supprime les fichiers de modèles téléchargés du disque ({size}). Les scores enregistrés restent dans la base ; les modèles pourront être téléchargés à nouveau."
        )
    },

    cmd_straighten: "Redresser",
    cmd_rotate_ccw: "Pivoter de 90° vers la gauche",
    cmd_rotate_cw: "Pivoter de 90° vers la droite",
    cmd_crop: "Recadrer",
    cmd_undo: "Annuler la retouche",
    edit_not_jpeg: "Redressement, recadrage, rotation et Ctrl+Z uniquement pour les JPEG.",
    video_play_hint: "Lire (Espace)",
    video_play_pause: "Lecture / pause (Espace)",
    video_mute: "Son activé / coupé",
    video_volume: "Volume (↑ ↓)",
    video_no_zoom: "Les vidéos ne se zooment pas",
    video_no_compare: "Les vidéos ne se comparent pas",
    video_no_sound: "Pas de son : Cerno n'a trouvé aucune sortie audio – la vidéo est lue sans le son",
    video_no_ffmpeg: "Pas d'aperçu : Cerno a besoin de ffmpeg (p. ex. winget install Gyan.FFmpeg)",
    video_play_failed: |err| format!("Impossible de lire la vidéo : {err}"),
    edit_writing: "Écriture de la photo…",
    edit_cancelled: "Une autre photo est affichée – retouche annulée",
    busy_editing: "La retouche est encore ouverte – Entrée applique, Échap annule",
    busy_copying: "Cette photo est en cours de copie – possible dans un instant",
    busy_moving: "Cette photo est en cours de déplacement",
    busy_deleted: "Photo supprimée – remettez-la d'abord en place (Ctrl+Z)",
    edit_needs_index: "La retouche a besoin de l'index, qui n'a pas pu être ouvert",
    edit_reencoded: "JPEG réencodé – Ctrl+Z restaure l'original.",
    undo_done: "Original restauré",
    undo_nothing: "Aucun original conservé pour cette photo",
    cmd_restore: "Remettre en place",
    restored: |n, renamed, name| match (n, renamed) {
        (1, 0) => format!("Remise en place : {name}"),
        (1, _) => format!("Remise en place sous le nom {name} – le nom était pris"),
        (n, 0) => format!("{n} photos remises en place"),
        (n, r) => format!("{n} photos remises en place, dont {r} sous un nouveau nom"),
    },
    restore_failed: |n, name, err| {
        format!("Impossible de remettre en place {n} photo(s) – {name} : {err}")
    },
    deleted_mark: "Supprimée",
    faces_loading: "Recherche des visages…",
    faces_unknown: "Pas encore analysée – les visages suivront.",
    faces_none: "Aucun visage détecté",
    faces_only_small: "Seulement de petits visages – trop petits pour juger",
    face_eyes_blurry: "Yeux probablement flous",
    faces_zoom_hint: "Cliquer pour zoomer sur ce visage",
    faces_grid_hint: "Un clic ou son numéro (1–9) zoome sur un visage · Échap ferme",
    cmd_faces: "Visages",
    cmd_face_grid: "Tous les visages en grand",
    faces_small: |n| {
        format!(
            "+ {n} {} – trop petits pour juger",
            if n == 1 {
                "petit visage"
            } else {
                "petits visages"
            }
        )
    },
    face_number: |n| format!("Visage {n}"),
    raw_preview_fact: "Aperçu RAW",
    preview_word: "aperçu",
    edit_failed: |detail| format!("Pas enregistré : {detail}"),
    edit_hint_straighten: "Molette ou ←/→ pour tourner, Maj plus fin · Entrée applique, Échap annule",
    edit_hint_crop: "Flèches : déplacer · +/− : taille, Maj plus fin · A : format · X : paysage/portrait · Entrée applique, Échap annule",
    ratio_original: "Original",
    crop_landscape: "Paysage",
    crop_portrait: "Portrait",

    help_title: "Aide",
    help_intro: "Cerno affiche vos photos instantanément et vous aide à faire le tri. Étoiles et couleurs vont dans le fichier photo, dont la date ne change pas ; tout le reste reste dans la base de données de Cerno.",
    help_drop: "Déposez un dossier ou une photo sur la fenêtre, ou appuyez sur Ctrl+O.",
    help_close: "Esc, H ou F1 ferme cette page",
    help_tab_keys: "Raccourcis",
    help_tab_tips: "Conseils",
    help_pages_hint: "←/→ change de page",
    help_tips: [
        (
            "Trier en deux passes",
            &[
                "D'abord parcourir vite (Espace) et rejeter les ratés avec X – sans trop réfléchir.",
                "Puis « sans × » dans la barre de filtres : les rejetées disparaissent, noter alors de 1 à 5.",
                "Pour finir, Action › « Supprimer les rejetées » (Ctrl+M).",
            ],
        ),
        (
            "Séries et comparaison",
            &[
                "Triées par date de prise de vue, les séries restent ensemble, la photo la plus nette en premier.",
                "C montre deux photos côte à côte ; A garde celle de gauche, D celle de droite, l'autre est rejetée.",
                "M ne montre que les photos qui ressemblent à la photo actuelle.",
            ],
        ),
        (
            "Les meilleures photos",
            &[
                "Choisir « Top 50 photos » dans la première case de la barre de filtres : Cerno propose les meilleures, d'abord une par série.",
                "L'esthétique et la netteté sous la photo aident à décider – les étoiles, c'est vous qui les donnez.",
            ],
        ),
        (
            "Étiquettes de couleur",
            &[
                "Les couleurs n'ont pas de sens fixe dans Cerno. Deux lectures courantes :",
                "Avancement : rouge à vérifier · jaune à retoucher · vert terminé · bleu exporté",
                "Usage : rouge client · jaune réseaux sociaux · vert portfolio · bleu impression",
                "6–9 posent rouge à bleu ; la barre de filtres montre une couleur.",
            ],
        ),
        (
            "Supprimé n'est pas perdu",
            &[
                "Les photos supprimées vont dans le dossier caché .originals à côté des photos.",
                "La case corbeille de la barre de filtres les montre ; Ctrl+Z en remet une en place.",
                "Avant le premier redressement, recadrage ou rotation, Cerno garde l'original – Ctrl+Z le restaure.",
            ],
        ),
        (
            "La prédiction",
            &[
                "Cerno apprend de vos étoiles et de vos photos rejetées et supprimées ce qui vous plaît.",
                "Sur les photos sans étoiles, il montre son estimation en étoiles légèrement remplies – c'est toujours à vous de les donner.",
                "Triées par prédiction, les photos qui vous plairont probablement viennent en premier.",
            ],
        ),
    ],
    welcome_intro: "Voir, noter et trier vos photos sans attendre – les étoiles vont dans le fichier, sa date reste.",
    welcome_keys: [
        ("←, →", "Photo précédente / suivante"),
        ("1 – 5", "Donner des étoiles"),
        ("X", "Rejeter"),
        ("Suppr", "Supprimer – Échap la ramène"),
        ("Ctrl+K", "Menu avec toutes les fonctions"),
    ],
    welcome_more: "Tous les raccourcis : H",
    help_sections: [
        "Parcourir",
        "Noter",
        "Trier",
        "Vidéo",
        "Affichage",
        "Panneaux",
        "Retouche",
        "Divers",
    ],
    help_browse: [
        (
            "→, Espace, Pg suiv",
            "Photo suivante (maintenir pour faire défiler)",
        ),
        ("←, Retour arrière, Pg préc", "Photo précédente"),
        ("Début, Fin", "Première / dernière photo"),
        ("Molette", "Sur la pellicule : faire défiler les photos"),
        ("Ctrl+O", "Ouvrir un dossier (ou le déposer sur la fenêtre)"),
        ("Ctrl+U", "Inclure les sous-dossiers – oui / non"),
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
        (
            "6 – 9",
            "Couleur : rouge, jaune, vert, bleu – une seconde fois l'enlève",
        ),
        (
            "Maj+6 – 9",
            "Poser cette couleur et passer à la photo suivante",
        ),
        (
            "B",
            "Description : modifier le commentaire et les mots-clés",
        ),
    ],
    help_cull: [
        (
            "C",
            "Comparer : fixer cette photo à gauche, parcourir à droite",
        ),
        (
            "A, D",
            "Comparer : garder la gauche / la droite – l'autre est rejetée, la comparaison se termine",
        ),
        (
            "M",
            "N'afficher que les photos semblables – encore : toutes",
        ),
        (
            "Suppr",
            "Supprimer : va dans le dossier caché .originals après 5 secondes – rien n'est perdu",
        ),
        ("Esc", "Récupérer les photos en attente de suppression"),
    ],
    help_video: [
        (
            "Espace",
            "Lire / mettre en pause (Maj+Espace : photo suivante)",
        ),
        ("Alt+←, Alt+→", "5 s en arrière / en avant"),
        (",, .", "Une image en arrière / en avant (en pause)"),
        ("↑, ↓", "Volume"),
    ],
    help_view: [
        ("Z, Double-clic", "Photo entière ↔ 100 %"),
        ("Ctrl+0, Ctrl+1", "Photo entière / 100 %"),
        ("+, −, Molette", "Zoom avant / arrière – aussi avec Ctrl"),
        ("Glisser", "Déplacer la photo zoomée"),
        (
            "O",
            "Superposition : contours nets → lumières et ombres écrêtées → désactivée",
        ),
        (
            "F7",
            "Grille de toutes les photos : ↑ ↓ une ligne, + − taille, Entrée ouvre la photo",
        ),
        ("F, F11", "Plein écran"),
    ],
    help_panels: [
        (
            "T",
            "Barre de filtres : trier et filtrer – par étoiles, couleurs, netteté, personnes",
        ),
        ("Tab", "Panneau de détails"),
        ("F6", "Pellicule"),
        ("Maj+Tab", "Barre de filtres, détails et pellicule ensemble"),
        (
            "G, Maj+G",
            "Visages : dans le panneau de détails (G) ou tous en grand (Maj+G) – un clic zoome dessus",
        ),
    ],
    help_edit: [
        (
            "S",
            "Redresser : grille, molette et flèches tournent, Maj plus fin",
        ),
        (
            "R",
            "Recadrer : tracer un cadre ou le placer avec les flèches et +/−, A change le format, X bascule paysage/portrait",
        ),
        (
            "Entrée, Échap",
            "Appliquer ou annuler redressement et recadrage",
        ),
        (
            "Ctrl+←, Ctrl+→",
            "Pivoter de 90° – sans perte, via l'orientation JPEG",
        ),
        (
            "Ctrl+Z",
            "Récupérer l'original – il reste dans .originals à côté de la photo ; remettre en place une photo supprimée",
        ),
        (
            "E",
            "Modifier dans un autre programme – celui retenu, ou en choisir un",
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
        (
            "Esc",
            "Revenir en arrière : zoom, comparaison, grille, plein écran",
        ),
    ],
};
