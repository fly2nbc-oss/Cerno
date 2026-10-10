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
    filter_incomplete: "Incomplètes",
    filter_incomplete_tooltip: "JPEG dont le fichier s'arrête avant la fin de l'image – coupé pendant une copie ou un téléchargement ; la partie manquante est grise",
    filter_people: "Avec des personnes",
    filter_no_people: "Sans personnes",
    filter_people_tooltip: "Repérées à leur visage : les personnes de dos ou toutes petites dans l'image ne comptent pas.",
    filter_deleted: "Supprimées",
    filter_deleted_tooltip: "Photos supprimées de ce dossier. Elles se trouvent dans le dossier caché .originals – Ctrl+Z en remet une en place.",
    filter_deleted_none: "Aucune photo supprimée dans ce dossier",
    filter_hide_rejected: "Masquer les rejetées",
    filter_without: "sans",
    filter_hide_rejected_tooltip: "Les photos rejetées disparaissent, toutes les autres restent. × montre au contraire seulement les rejetées.",
    menu_name_list: "Par liste de fichiers …",
    name_list_title: "Filtrer par liste de fichiers",
    name_list_intro: "Coller les noms de fichiers ou numéros choisis par un client – un par ligne, ou séparés par des virgules ou des points-virgules. Casse et extension sont sans importance ; un numéro trouve les chiffres qui terminent un nom.",
    name_list_hint: "IMG_0345, IMG_0351 …",
    name_list_missing: "Introuvables :",
    name_list_ambiguous: "Dans plusieurs dossiers (tous pris) :",
    name_list_apply: "Appliquer",
    name_list_chip_tooltip: "Seulement les photos de la liste collée – un clic les montre toutes",
    name_list_found: |found, total| format!("{found} sur {total} trouvés"),
    name_list_chip: |found, total| format!("Liste {found}/{total}  ×"),
    align_camera_hint: "Décale l'heure de prise de vue de toutes les photos de l'appareil de droite dans ce dossier, pour que la photo de droite ait été prise au même moment que celle de gauche. Seulement dans l'index – les fichiers gardent leur heure.",
    align_needs_compare: "Comparer d'abord deux photos d'appareils différents (C)",
    align_no_time: "Il manque l'appareil ou l'heure de prise de vue à l'une des deux photos",
    align_same_camera: "Les deux photos viennent du même appareil",
    menu_camera_time: "Heure de l'appareil …",
    camera_time_title: "Heure de l'appareil",
    camera_time_intro: "Si l'horloge d'un appareil était fausse, un décalage déplace l'heure de prise de vue de toutes ses photos dans ce dossier – pour le tri par date de prise de vue, les séries et l'affichage. Seulement dans l'index ; les fichiers gardent leur heure.",
    camera_time_reset: "Réinitialiser",
    camera_time_invalid: "Décalage au format +h:mm:ss ou −h:mm:ss, p. ex. +1:30:00",
    camera_time_apply: "Appliquer",
    camera_time_none: "Aucune photo avec appareil et heure de prise de vue dans ce dossier pour l'instant.",
    camera_time_applied: "Heure de l'appareil appliquée",
    camera_aligned: |camera, offset| {
        format!("{camera} : heure de prise de vue décalée de {offset}")
    },
    camera_time_photos: |n| {
        if n == 1 {
            "1 photo".into()
        } else {
            format!("{n} photos")
        }
    },
    camera_time_tooltip: |taken, offset| {
        format!("Heure de l'appareil {taken} – décalée de {offset}")
    },
    cmd_pairs: "RAW+JPG comme une seule photo",
    pairs_hint: "Un RAW et un JPG du même nom apparaissent comme une seule photo – le JPG est affiché. Étoiles, couleur et description vont dans les deux fichiers, copier, déplacer et supprimer les prennent tous les deux ; les retouches ne modifient que le JPG.",
    cmd_update_check: "Rechercher les mises à jour",
    update_check_hint: "Une requête par jour à GitHub – rien sur vous ni sur vos photos",
    update_available: |v| format!("Cerno {v} est disponible – Aide › À propos"),
    cmd_update_download: |v| format!("Télécharger Cerno {v} …"),
    update_ask_title: "Rechercher les mises à jour ?",
    update_ask_text: "Cerno doit-il vérifier une fois par jour s'il existe une nouvelle version ? Rien sur vous ni sur vos photos n'est envoyé. Vous pourrez le changer plus tard dans les Réglages.",
    btn_update_yes: "Oui",
    btn_update_no: "Non",
    pairs_on: "RAW+JPG apparaissent comme une seule photo",
    pairs_off: "RAW et JPG apparaissent séparément",
    pair_badge: "RAW+JPG",
    pair_edit_jpeg_only: "Ne modifie que le JPG – le RAW reste intact.",
    pair_raw_marks: |marks| format!("RAW : {marks}"),
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
    bulk_delete_top: "Pas avec Top – ce sont les meilleures photos",
    filter_similar: "≈ Semblables",
    filter_similar_to: |name| format!("≈ comme {name}"),
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
    photos_badge_tooltip: "Le nombre de photos que le filtre affiche – « Photos affichées » de la barre de menu agit sur exactement celles-ci.",
    label_red: "Rouge",
    label_yellow: "Jaune",
    label_green: "Vert",
    label_blue: "Bleu",
    label_purple: "Violet",
    meter_aesthetics_tooltip: "Esthétique : moyenne de LAION et V2.5 sur une échelle fixe – une photo a la même valeur dans chaque dossier. La netteté, elle, compare avec les autres photos du dossier.\nValeurs séparées : panneau des détails (Tab)",
    meter_sharpness: "Netteté",
    meter_eyes: "Yeux",
    probably_blurry: "sans doute floue",
    incomplete_fact: "Fichier incomplet",
    incomplete_tooltip: "Le fichier s'arrête avant la fin de l'image – la partie manquante est grise. Vérifiez l'original, par exemple sur la carte mémoire.",
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
    view_photo: "Photo",
    view_grid: "Grille",
    view_faces: "Visages",
    faces_video: "Les vidéos ne sont pas analysées pour les visages",
    filmstrip_in_grid: "Inutile dans la grille – F7 revient à la photo",
    faces_button: |n| {
        if n == 1 {
            "Afficher 1 visage en grand (G)".to_owned()
        } else {
            format!("Afficher {n} visages en grand (G)")
        }
    },
    button_side_bar: "Barre de menu",
    bar_rotate: "Pivoter",
    bar_colour: "Couleur",
    bar_stars: "Étoiles",
    bar_align_camera: "Caler les appareils",
    bar_overlay_sharpness: "Netteté",
    bar_overlay_exposure: "Exposition",
    bar_none_rejected: "Aucune photo du dossier n'est rejetée.",
    bar_none_deleted: "Aucune photo supprimée affichée : la case corbeille de la barre de filtres les montre.",
    button_language: |name| format!("Langue : {name}"),

    cmd_fullscreen: "Plein écran",
    cmd_compare: "Comparer",
    cmd_quad: "Vue à quatre",
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
    cmd_reject: "Rejeter",
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
    menu_view: "Affichage",
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
    writer_stopped: "Les marques ne sont plus écrites dans les fichiers – erreur interne (crash.log dans le dossier de données). Veuillez redémarrer Cerno.",
    internal_error: "Erreur interne – la commande a été interrompue, Cerno continue (détails dans crash.log du dossier de données).",
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
            "L'évaluation esthétique a besoin de modèles d'image ({size}) : Barre de menu › Réglages › Modèles et données."
        )
    },
    exiftool_offer: |size| {
        format!(
            "Cerno enregistre étoiles et couleurs avec ExifTool ({size}) : Barre de menu › Réglages › Modèles et données."
        )
    },
    exiftool_title: "Télécharger ExifTool",
    exiftool_text: |size| {
        format!(
            "Cerno écrit les étoiles, les couleurs, les commentaires et les retouches dans les photos avec ExifTool – l'outil libre de Phil Harvey.\n\nLe télécharger maintenant depuis la source officielle (SourceForge, {size}) ? Il va dans le dossier de données de Cerno ; un téléchargement interrompu reprend la prochaine fois."
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
    row_complete: "Complet",
    incomplete_value: "non – la fin manque",
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
    explain_personal: "Les étoiles que vous donneriez selon Cerno. Elle apprend de vos propres étoiles et des photos que vous rejetez.",
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
    exiftool_found: |version, downloaded| {
        let from = if downloaded {
            "téléchargé par Cerno"
        } else {
            "installé"
        };
        if version.is_empty() {
            from.to_owned()
        } else {
            format!("{version} · {from}")
        }
    },
    exiftool_absent: "absent – sans ExifTool, ni étoiles ni couleurs",
    exiftool_old_state: |version| format!("{version} – trop ancien (12.24 ou plus récent requis)"),
    exiftool_downloading: |percent| format!("téléchargement … {percent:.0} %"),
    btn_exiftool: |size| format!("Télécharger ExifTool ({size})"),
    exiftool_outdated: |version, newer| {
        format!("{version} · téléchargé par Cerno – {newer} est disponible")
    },
    btn_exiftool_update: |newer, size| format!("Télécharger ExifTool {newer} ({size})"),
    exiftool_update_hint: |version, newer| {
        format!(
            "ExifTool {newer} est disponible (Cerno utilise {version}) : Barre de menu (F10) › Réglages › Modèles et données."
        )
    },
    taste_sources: |stars, rejected| {
        format!(
            "Apprise de {stars} photos avec étoiles et {rejected} rejetées – les photos rejetées comptent pour 0 ★, les supprimées pas du tout."
        )
    },
    btn_reset_taste: "Réinitialiser la prédiction",
    btn_delete_models: "Supprimer les modèles",
    models_deleted: "Modèles supprimés",
    models_downloaded: "Modèles téléchargés – l'esthétique est en cours de calcul",
    download_failed: |err| {
        format!("Échec du téléchargement : {err} – un nouvel essai reprend là où il s'est arrêté")
    },
    exiftool_ready: "ExifTool est prêt – étoiles et couleurs sont enregistrées désormais",
    exiftool_failed: |err| {
        format!("Impossible de télécharger ExifTool : {err} – un nouvel essai reprend là")
    },
    copy_path: "Copier le chemin",
    path_copied: "Chemin copié",
    confirm_reset_taste_title: "Réinitialiser la prédiction ?",
    confirm_reset_taste_text: "Cerno oubliera ce qu'il a appris de vos étoiles et de vos photos rejetées. Les étoiles dans les fichiers photo restent inchangées.",
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
    video_play_failed: |err| format!("Impossible de lire la vidéo : {err}"),
    edit_writing: "Écriture de la photo…",
    edit_cancelled: "Une autre photo est affichée – retouche annulée",
    busy_editing: "La retouche est encore ouverte – Entrée applique, Échap annule",
    busy_copying: "Cette photo est en cours de copie – possible dans un instant",
    busy_moving: "Cette photo est en cours de déplacement",
    busy_deleted: "Photo supprimée – remettez-la d'abord en place (Ctrl+Z)",
    edit_needs_index: "La retouche a besoin de l'index, qui n'a pas pu être ouvert",
    exiftool_missing: "Les étoiles, les couleurs et les retouches ont besoin d'ExifTool – Barre de menu › Réglages › Modèles et données",
    exiftool_too_old: "L'ExifTool installé est trop ancien (12.24 ou plus récent requis) – Barre de menu › Réglages › Modèles et données",
    exiftool_loading: "Téléchargement d'ExifTool – étoiles et couleurs fonctionnent dans un instant",
    exiftool_install: |command| {
        match command {
        Some(command) => format!("Les étoiles, les couleurs et les retouches ont besoin d'ExifTool. Installation : {command}"),
        None => "Les étoiles, les couleurs et les retouches ont besoin d'ExifTool. Installez-le avec le gestionnaire de paquets (paquet « exiftool » ou « perl-image-exiftool »).".to_owned(),
    }
    },
    edit_reencoded: "JPEG réencodé – Ctrl+Z restaure l'original.",
    undo_done: "Original restauré",
    undo_nothing: "Aucun original conservé pour cette photo",
    undo_mark_row: |what, name| format!("Annuler {what} – {name}"),
    undo_what_stars: "les étoiles",
    undo_what_reject: "le rejet",
    undo_what_colour: "la couleur",
    undo_what_edit: "la retouche",
    undo_mark_done: |name, value| format!("Annulé – {name} : {value}"),
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
    faces_zoom_hint: "Cliquer pour zoomer sur ce visage",
    faces_grid_hint: "Un clic ou son numéro (1–9) zoome sur un visage · Échap ferme",
    cmd_face_grid: "Tous les visages en grand",
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
    help_tab_about: "À propos",
    about_intro: "Cerno affiche les photos sans attendre et aide à faire le tri – en local, sans compte ni cloud. Logiciel libre : le code source est ouvert.",
    about_version: "Version",
    about_license: "Licence",
    about_source: "Code source",
    about_bugs: "Signaler un problème",
    about_bugs_text: "Quelque chose ne fonctionne pas ? Décrivez-le dans un ticket GitHub – il faut pour cela un compte GitHub gratuit. La version et le système sont déjà remplis ; photos et noms de fichiers ne sont pas nécessaires. Si Cerno s'est fermé brutalement, joignez le fichier crash.log du dossier de données.",
    about_bug_link: "Le signaler sur GitHub",
    about_open_data: "Ouvrir le dossier de données",
    about_wishes: "Idées",
    about_wishes_text: "Une idée qui rend le tri avec Cerno plus simple ou plus rapide ? Proposez-la – également avec un compte GitHub.",
    about_wish_link: "La proposer sur GitHub",
    about_third_party: "Tiers",
    about_third_party_text: "Cerno utilise des bibliothèques libres d'autres auteurs, dont libheif et GStreamer avec FFmpeg (LGPL), et télécharge sur demande ExifTool et les modèles d'esthétique (V2.5 sous AGPL). Leurs licences sont fournies avec le programme.",
    about_third_party_link: "Tous les tiers et leurs licences",
    about_updates: "Mises à jour",
    about_updates_text: "Si vous l'autorisez, Cerno demande une fois par jour à GitHub s'il existe une version plus récente – rien sur vous ni sur vos photos n'est envoyé, rien n'est téléchargé ni installé. À activer ou désactiver dans Réglages › Rechercher les mises à jour. Les modèles et ExifTool ne sont téléchargés que si vous le demandez.",
    update_off: "La recherche de mises à jour est désactivée.",
    update_never: "Pas encore vérifié.",
    update_checking: "Vérification …",
    update_current: "Cerno est à jour.",
    update_failed: "GitHub est injoignable – réessayez plus tard.",
    update_newer: |v| format!("Cerno {v} est disponible."),
    update_check_now: "Vérifier maintenant",
    help_tips: [
        (
            "Trier en deux passes",
            &[
                "D'abord parcourir vite (Espace) et rejeter les ratés avec X – sans trop réfléchir.",
                "Puis « sans × » dans la barre de filtres : les rejetées disparaissent, noter alors de 1 à 5.",
                "Pour finir, barre de menu (F10) › Photos affichées › « Supprimer les rejetées ».",
            ],
        ),
        (
            "Séries et comparaison",
            &[
                "Triées par date de prise de vue, les séries restent ensemble, la photo la plus nette en premier.",
                "C montre deux photos côte à côte ; A garde celle de gauche, D celle de droite, l'autre est rejetée.",
                "M ne montre que les photos qui ressemblent à la photo actuelle.",
                "Deux appareils à l'heure différente : comparer deux photos prises au même moment, puis Cette photo › « Caler l'appareil de droite sur celui de gauche ».",
                "Maj+C affiche quatre photos à la fois : le cadre est la photo actuelle, ↑ ↓ le déplacent d'une rangée.",
            ],
        ),
        (
            "Les meilleures photos",
            &[
                "Choisir « Top 50 photos » dans la première case de la barre de filtres : Cerno propose les meilleures, d'abord une par série.",
                "L'esthétique et la netteté sous la photo aident à décider – les étoiles, c'est vous qui les donnez.",
                "Filtre › Par liste de fichiers … : collez les noms choisis par un client – seules ces photos restent.",
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
                "Cerno apprend de vos étoiles et de vos photos rejetées ce qui vous plaît – les photos supprimées ne comptent pas : souvent, c'était juste une de trop parmi des photos semblables.",
                "Sur les photos sans étoiles, il montre son estimation en étoiles légèrement remplies – c'est toujours à vous de les donner.",
                "Triées par prédiction, les photos qui vous plairont probablement viennent en premier.",
            ],
        ),
        (
            "Bon à savoir",
            &[
                "La molette sur la pellicule fait défiler les photos ; sur la photo, elle zoome.",
                "Déposez un dossier sur la fenêtre pour l'ouvrir. Maintenez → pour faire défiler les photos.",
                "Dans la grille (F7), ↑ ↓ changent de ligne, + − la taille, Entrée ouvre la photo.",
                "Redresser (S) : la molette et les flèches tournent, Maj plus fin. Recadrer (R) : tracez un cadre ou déplacez-le avec les flèches, +/− change sa taille, A le format, X bascule paysage/portrait.",
                "La barre de menu (F10 ou le bouton en bas à droite) réunit ce qui agit sur cette photo, les photos affichées et la vue, plus les réglages – tout à la souris.",
                "Dans la description, Entrée place le curseur dans le champ des mots-clés.",
            ],
        ),
    ],
    welcome_intro: "Voir, noter et trier vos photos sans attendre – les étoiles vont dans le fichier, sa date reste.",
    welcome_keys: [
        ("←, →", "Photo précédente / suivante"),
        ("1 – 5", "Donner des étoiles"),
        ("X", "Rejeter"),
        ("Suppr", "Supprimer – Échap la ramène"),
        ("F10", "Barre de menu (bouton en bas à droite)"),
    ],
    welcome_more: "Tous les raccourcis : H",
    setup_line: |exiftool, aesthetics| {
        format!(
            "Configuration : ExifTool {} · esthétique {} – Barre de menu (F10) › Réglages › Modèles et données",
            if exiftool { "prêt" } else { "manquant" },
            if aesthetics { "prête" } else { "manquante" },
        )
    },
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
        ("→, Espace, Pg suiv", "Photo suivante"),
        ("←, Retour arrière, Pg préc", "Photo précédente"),
        ("Début, Fin", "Première / dernière photo"),
        ("Ctrl+O", "Ouvrir un dossier"),
        ("Ctrl+U", "Sous-dossiers oui / non"),
    ],
    help_rate: [
        ("1 – 5", "Étoiles"),
        ("0", "Sans étoiles, non rejetée"),
        ("X", "Rejeter"),
        ("6 – 9", "Rouge, jaune, vert, bleu"),
        ("Maj+…", "Idem, puis photo suivante"),
        ("Ctrl+Z", "Annuler"),
    ],
    help_cull: [
        ("C", "Comparer deux photos"),
        ("A, D", "Garder la gauche / la droite"),
        ("Maj+C", "Quatre photos à la fois"),
        ("M", "Photos semblables seulement"),
        ("Suppr", "Supprimer (vers .originals)"),
        ("Esc", "Récupérer les supprimées"),
    ],
    help_video: [
        ("Espace", "Lire / mettre en pause"),
        ("Maj+Espace", "Photo suivante"),
        ("Alt+←, Alt+→", "5 s en arrière / en avant"),
        (",, .", "Une image en arrière / avant"),
        ("↑, ↓", "Volume"),
    ],
    help_view: [
        ("Z, Double-clic", "Photo entière ↔ 100 %"),
        ("Ctrl+0, Ctrl+1", "Photo entière / 100 %"),
        ("+, −, Molette", "Zoom avant / arrière"),
        ("Glisser", "Déplacer la photo zoomée"),
        ("O", "Contrôle : netteté, écrêtage"),
        ("F7", "Grille de toutes les photos"),
        ("G", "Tous les visages en grand"),
        ("F, F11", "Plein écran"),
    ],
    help_panels: [
        ("F10", "Barre de menu"),
        ("T", "Barre de filtres"),
        ("Tab", "Panneau de détails"),
        ("F6", "Pellicule"),
        ("Maj+Tab", "Les quatre panneaux"),
        ("Ctrl+Tab", "Onglet suivant des détails"),
    ],
    help_edit: [
        ("S", "Redresser"),
        ("R", "Recadrer"),
        ("Entrée, Échap", "Appliquer / annuler"),
        ("Ctrl+←, Ctrl+→", "Pivoter de 90°"),
        ("E", "Modifier dans un autre programme"),
    ],
    help_more: [
        ("Ctrl+L", "Langue"),
        ("H, F1, ?", "Cette aide"),
        ("Esc", "Revenir en arrière"),
    ],
};
