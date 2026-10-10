; French texts of cargo-packager's NSIS installer: its own French.nsh (0.11.8,
; Apache-2.0 OR MIT), here so the welcome text below can be added.
; UTF-8 with BOM: NSIS reads a file without BOM in the system code page.
LangString addOrReinstall ${LANG_FRENCH} "Ajouter/Réinstaller un composant."
LangString alreadyInstalled ${LANG_FRENCH} "Déja installé."
LangString alreadyInstalledLong ${LANG_FRENCH} "${PRODUCTNAME} ${VERSION} est déja installé. Sélectionnez l'opération que vous souhaitez effectuer, puis cliquez sur Suivant pour continuer."
LangString appRunning ${LANG_FRENCH} "${PRODUCTNAME} est en cours d'exécution. Veuillez fermer l'application avant de réessayer."
LangString appRunningOkKill ${LANG_FRENCH} "${PRODUCTNAME} est en cours d'exécution.$\nCliquez sur OK pour fermer l'application."
LangString chooseMaintenanceOption ${LANG_FRENCH} "Veuillez choisir l'option de maintenance à effectuer."
LangString choowHowToInstall ${LANG_FRENCH} "Veuillez choisir l'emplacement d'installation de ${PRODUCTNAME}."
LangString createDesktop ${LANG_FRENCH} "Créer un raccourci sur le bureau."
LangString dontUninstall ${LANG_FRENCH} "Ne pas désinstaller"
LangString dontUninstallDowngrade ${LANG_FRENCH} "Ne pas désinstaller (revenir à une ancienne version sans désinstallation est désactivé pour cet installateur)"
LangString failedToKillApp ${LANG_FRENCH} "La fermeture de ${PRODUCTNAME} a échoué. Veuillez fermer l'application et réessayer."
LangString installingWebview2 ${LANG_FRENCH} "Installation de WebView2..."
LangString newerVersionInstalled ${LANG_FRENCH} "Une version plus récente de ${PRODUCTNAME} est déja installée. Il n'est pas recommandé d'installer une ancienne version. Si vous souhaitez installer cette ancienne version, il est conseillé de désinstaller la version courante en premier. Veuillez sélectionner l'opération que vous souhaitez effectuer, puis cliquez sur Suivant pour continer."
LangString older ${LANG_FRENCH} "ancien"
LangString olderOrUnknownVersionInstalled ${LANG_FRENCH} "La version $R4 de ${PRODUCTNAME} est installée sur le système. Il est recommandé de désinstaller la version actuelle avant d'installer celle-ci. Sélectionnez l'opération que vous souhaitez effectuer, puis cliquez sur Suivant pour continuer."
LangString silentDowngrades ${LANG_FRENCH} "Revenir à une version antérieure est désactivé pour cet installateur. Impossible de continuer avec l'installation silencieuse, veuillez utiliser l'interface graphique à la place.$\n"
LangString unableToUninstall ${LANG_FRENCH} "Impossible de désinstaller le programme !"
LangString uninstallApp ${LANG_FRENCH} "Désinstaller ${PRODUCTNAME}"
LangString uninstallBeforeInstalling ${LANG_FRENCH} "Désinstaller avant d'installer"
LangString unknown ${LANG_FRENCH} "inconnu"
LangString deleteAppData ${LANG_FRENCH} "Supprimer les données de l'application"
; The welcome page names the update check (shown during installation). MUI defines this
; string first; this later definition wins (NSIS warns 6030, no error).
LangString MUI_TEXT_WELCOME_INFO_TEXT ${LANG_FRENCH} "Cet assistant installe $(^NameDA) pour votre compte utilisateur.$\r$\n$\r$\nConfidentialité : une fois par jour, Cerno demande à GitHub s'il existe une version plus récente – rien sur vous ni sur vos photos n'est envoyé. Vous pouvez le désactiver dans Réglages › Rechercher les mises à jour. Les modèles et ExifTool ne sont téléchargés que si vous le demandez.$\r$\n$\r$\n$_CLICK"
