; English texts of cargo-packager's NSIS installer: its own English.nsh (0.11.8,
; Apache-2.0 OR MIT), here so the welcome text below can be added.
; UTF-8 with BOM: NSIS reads a file without BOM in the system code page.
LangString addOrReinstall ${LANG_ENGLISH} "Add/Reinstall components"
LangString alreadyInstalled ${LANG_ENGLISH} "Already Installed"
LangString alreadyInstalledLong ${LANG_ENGLISH} "${PRODUCTNAME} ${VERSION} is already installed. Select the operation you want to perform and click Next to continue."
LangString appRunning ${LANG_ENGLISH} "${PRODUCTNAME} is running! Please close it first then try again."
LangString appRunningOkKill ${LANG_ENGLISH} "${PRODUCTNAME} is running!$\nClick OK to kill it"
LangString chooseMaintenanceOption ${LANG_ENGLISH} "Choose the maintenance option to perform."
LangString choowHowToInstall ${LANG_ENGLISH} "Choose how you want to install ${PRODUCTNAME}."
LangString createDesktop ${LANG_ENGLISH} "Create desktop shortcut"
LangString dontUninstall ${LANG_ENGLISH} "Do not uninstall"
LangString dontUninstallDowngrade ${LANG_ENGLISH} "Do not uninstall (Downgrading without uninstall is disabled for this installer)"
LangString failedToKillApp ${LANG_ENGLISH} "Failed to kill ${PRODUCTNAME}. Please close it first then try again"
LangString installingWebview2 ${LANG_ENGLISH} "Installing WebView2..."
LangString newerVersionInstalled ${LANG_ENGLISH} "A newer version of ${PRODUCTNAME} is already installed! It is not recommended that you install an older version. If you really want to install this older version, it's better to uninstall the current version first. Select the operation you want to perform and click Next to continue."
LangString older ${LANG_ENGLISH} "older"
LangString olderOrUnknownVersionInstalled ${LANG_ENGLISH} "An $R4 version of ${PRODUCTNAME} is installed on your system. It's recommended that you uninstall the current version before installing. Select the operation you want to perform and click Next to continue."
LangString silentDowngrades ${LANG_ENGLISH} "Downgrades are disabled for this installer, can't proceed with the silent installer, please use the graphical interface installer instead.$\n"
LangString unableToUninstall ${LANG_ENGLISH} "Unable to uninstall!"
LangString uninstallApp ${LANG_ENGLISH} "Uninstall ${PRODUCTNAME}"
LangString uninstallBeforeInstalling ${LANG_ENGLISH} "Uninstall before installing"
LangString unknown ${LANG_ENGLISH} "unknown"
LangString deleteAppData ${LANG_ENGLISH} "Delete the application data"
; The welcome page names the update check (shown during installation). MUI defines this
; string first; this later definition wins (NSIS warns 6030, no error).
LangString MUI_TEXT_WELCOME_INFO_TEXT ${LANG_ENGLISH} "This wizard installs $(^NameDA) for your user account.$\r$\n$\r$\nPrivacy: once a day Cerno asks GitHub whether there is a newer version – nothing about you or your photos is sent. You can turn it off under Settings › Check for updates. Models and ExifTool are downloaded only when you ask.$\r$\n$\r$\n$_CLICK"
