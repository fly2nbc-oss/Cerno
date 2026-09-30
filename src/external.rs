//! Editing a photo in another program: the ones the system registers for its type ("Open
//! with" on Windows, `.desktop` entries on Linux), a program picked by hand, or the system's own
//! chooser. Cerno only starts them; the file stays where it is, and the caller keeps the first
//! original and reloads the photo once it changes.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context as _, Result, bail};

/// A program to open photos with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    /// As the menu shows it.
    pub name: String,
    /// How it is found again: the system's name for it (a program path, a Store app's id, a
    /// `.desktop` file) or a program picked by hand.
    pub id: String,
}

impl Editor {
    /// As stored in the settings: `name`, tab, `id`.
    pub fn to_setting(&self) -> String {
        format!("{}\t{}", self.name, self.id)
    }

    pub fn from_setting(text: &str) -> Option<Self> {
        let (name, id) = text.split_once('\t')?;
        (!name.is_empty() && !id.is_empty()).then(|| Self {
            name: name.to_owned(),
            id: id.to_owned(),
        })
    }

    /// A program picked by hand: its file name without extension as the name.
    pub fn program(path: &Path) -> Self {
        Self {
            name: path.file_stem().map_or_else(
                || path.display().to_string(),
                |s| s.to_string_lossy().into(),
            ),
            id: path.to_string_lossy().into_owned(),
        }
    }
}

/// The programs registered for the file type of `path`, as the system recommends them.
pub fn editors_for(path: &Path) -> Vec<Editor> {
    let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
        return Vec::new();
    };
    let mut editors = platform::editors_for(&extension.to_ascii_lowercase());
    let mut seen = std::collections::HashSet::new();
    editors.retain(|e| seen.insert(e.id.to_lowercase()));
    editors
}

/// Opens `path` in `editor`: through the system when it is registered for the type, else – a
/// program picked by hand, or registered for other types only – with the photo as argument.
pub fn open(editor: &Editor, path: &Path) -> Result<()> {
    if platform::open_registered(&editor.id, path)? {
        return Ok(());
    }
    let program = Path::new(&editor.id);
    if program.is_file() {
        Command::new(program)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("cannot start {}", program.display()))?;
        return Ok(());
    }
    bail!("{} cannot open this file type", editor.name)
}

/// The system's own chooser (Windows "Open with"; the default program elsewhere). What is
/// picked there is not remembered. Returns at once; the chooser stays open on its own.
pub fn choose(path: &Path) -> Result<()> {
    platform::choose(path)
}

/// Whether the system would open `path` with Cerno itself: once "Open with" chose Cerno for
/// the type, Windows can treat it as the default. Handing a video to the system then only
/// starts Cerno again.
pub fn opens_with_cerno(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    match (
        platform::default_program(&extension.to_ascii_lowercase()),
        std::env::current_exe(),
    ) {
        (Some(program), Ok(own)) => is_cerno(&program, &own),
        _ => false,
    }
}

/// Whether `program` (a path, as `Editor::id` holds one) is a Cerno: any file named like the
/// running one, so an installed copy counts next to a build run from the source tree.
pub fn is_cerno(program: &Path, own: &Path) -> bool {
    match (program.file_name(), own.file_name()) {
        (Some(name), Some(own)) => name.eq_ignore_ascii_case(own),
        _ => false,
    }
}

/// The first program the system offers for `path` that is not Cerno.
pub fn other_than_cerno(editors: &[Editor]) -> Option<&Editor> {
    let own = std::env::current_exe().unwrap_or_default();
    editors
        .iter()
        .find(|editor| !is_cerno(Path::new(&editor.id), &own))
}

#[cfg(windows)]
mod platform {
    use std::path::Path;

    use anyhow::{Result, anyhow, bail};
    use windows::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoTaskMemFree,
        CoUninitialize, IDataObject,
    };
    use windows::Win32::UI::Shell::{
        ASSOC_FILTER_RECOMMENDED, ASSOCF_NOTRUNCATE, ASSOCSTR_EXECUTABLE, AssocQueryStringW,
        BHID_DataObject, IAssocHandler, IShellItem, OAIF_EXEC, OAIF_HIDE_REGISTRATION,
        OPEN_AS_INFO_FLAGS, OPENASINFO, SHAssocEnumHandlers, SHCreateItemFromParsingName,
        SHOpenWithDialog,
    };
    use windows::core::{HSTRING, PCWSTR, PWSTR};

    use super::Editor;

    /// Runs `job` on a thread of its own with a single-threaded COM apartment – the UI thread's
    /// COM state (winit, the file dialogs) stays untouched.
    fn with_com<T: Send + 'static>(job: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
        std::thread::spawn(move || {
            // SAFETY: plain COM set-up, balanced by `CoUninitialize` on this same thread.
            let init =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
            if init.is_err() {
                bail!("COM: {init:?}");
            }
            let result = job();
            // SAFETY: matches the successful `CoInitializeEx` above.
            unsafe { CoUninitialize() };
            result
        })
        .join()
        .map_err(|_| anyhow!("the COM thread crashed"))?
    }

    /// A string the shell allocated, copied and freed.
    fn take(text: PWSTR) -> String {
        // SAFETY: the shell hands out a NUL-terminated string from `CoTaskMemAlloc`, ours to free.
        unsafe {
            let copy = text.to_string().unwrap_or_default();
            CoTaskMemFree(Some(text.0 as *const _));
            copy
        }
    }

    fn handlers(extension: &str) -> Result<Vec<IAssocHandler>> {
        let extension = HSTRING::from(format!(".{extension}"));
        // SAFETY: a valid, NUL-terminated extension; the enumerator is released on drop.
        let list = unsafe { SHAssocEnumHandlers(&extension, ASSOC_FILTER_RECOMMENDED) }?;
        let mut handlers = Vec::new();
        loop {
            let mut one = [None];
            let mut fetched = 0u32;
            // SAFETY: room for exactly one handler; S_FALSE with 0 fetched ends the list.
            let next = unsafe { list.Next(&mut one, Some(&mut fetched)) };
            if next.is_err() || fetched == 0 {
                break;
            }
            handlers.extend(one[0].take());
        }
        Ok(handlers)
    }

    pub fn editors_for(extension: &str) -> Vec<Editor> {
        let owned = extension.to_owned();
        with_com(move || {
            let mut editors = Vec::new();
            for handler in handlers(&owned)? {
                // SAFETY: both strings are freed by `take`.
                let id = unsafe { handler.GetName() }.map(take);
                let name = unsafe { handler.GetUIName() }.map(take);
                if let (Ok(id), Ok(name)) = (id, name)
                    && !id.is_empty()
                {
                    editors.push(Editor { name, id });
                }
            }
            Ok(editors)
        })
        .unwrap_or_else(|err| {
            log::warn!("programs for .{extension}: {err:#}");
            Vec::new()
        })
    }

    /// Finds `id` among the programs registered for the file's type and lets it open the file
    /// (Store apps included). `false` when it is not registered for this type.
    pub fn open_registered(id: &str, path: &Path) -> Result<bool> {
        let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
            return Ok(false);
        };
        let (extension, id, path) = (
            extension.to_ascii_lowercase(),
            id.to_owned(),
            path.to_owned(),
        );
        with_com(move || {
            for handler in handlers(&extension)? {
                // SAFETY: freed by `take`.
                let name = unsafe { handler.GetName() }.map(take).unwrap_or_default();
                if !name.eq_ignore_ascii_case(&id) {
                    continue;
                }
                let file = HSTRING::from(path.as_os_str());
                // SAFETY: a valid path; the item and its data object are released on drop.
                let item: IShellItem = unsafe { SHCreateItemFromParsingName(&file, None) }?;
                let data: IDataObject = unsafe { item.BindToHandler(None, &BHID_DataObject) }?;
                unsafe { handler.Invoke(&data) }?;
                return Ok(true);
            }
            Ok(false)
        })
    }

    /// The program file Windows opens the type with by default; `None` for a Store app (it
    /// has no program file) or when nothing is registered.
    pub fn default_program(extension: &str) -> Option<std::path::PathBuf> {
        let assoc = HSTRING::from(format!(".{extension}"));
        let verb = HSTRING::from("open");
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        // SAFETY: `buffer` has room for `len` characters; on success the path in it ends
        // with a NUL.
        let result = unsafe {
            AssocQueryStringW(
                ASSOCF_NOTRUNCATE,
                ASSOCSTR_EXECUTABLE,
                &assoc,
                &verb,
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut len,
            )
        };
        if result.is_err() {
            return None;
        }
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        let text = String::from_utf16_lossy(&buffer[..end]);
        (!text.is_empty()).then(|| std::path::PathBuf::from(text))
    }

    /// Windows' "Open with" dialog, on a thread of its own so the window keeps drawing. The
    /// "always use" box is hidden: Cerno never changes the system's defaults.
    pub fn choose(path: &Path) -> Result<()> {
        let path = path.to_owned();
        std::thread::spawn(move || {
            let result = with_com(move || {
                let file = HSTRING::from(path.as_os_str());
                let info = OPENASINFO {
                    pcszFile: PCWSTR(file.as_ptr()),
                    pcszClass: PCWSTR::null(),
                    oaifInFlags: OPEN_AS_INFO_FLAGS(OAIF_EXEC.0 | OAIF_HIDE_REGISTRATION.0),
                };
                // SAFETY: `file` outlives the modal call; a cancel is just an error result.
                unsafe { SHOpenWithDialog(None, &info) }?;
                Ok(())
            });
            if let Err(err) = result {
                log::info!("open with: {err:#}");
            }
        });
        Ok(())
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    use anyhow::{Context as _, Result};

    use super::{Editor, desktop_entry, exec_args, mime_of};

    fn application_dirs() -> Vec<PathBuf> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
        let data_dirs = std::env::var("XDG_DATA_DIRS")
            .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_owned());
        let mut dirs: Vec<PathBuf> = data_home.into_iter().collect();
        dirs.extend(data_dirs.split(':').map(PathBuf::from));
        dirs.push(PathBuf::from("/var/lib/flatpak/exports/share"));
        if let Some(home) = &home {
            dirs.push(home.join(".local/share/flatpak/exports/share"));
        }
        dirs.into_iter().map(|d| d.join("applications")).collect()
    }

    pub fn editors_for(extension: &str) -> Vec<Editor> {
        let Some(mime) = mime_of(extension) else {
            return Vec::new();
        };
        let mut editors = Vec::new();
        // The first directory wins: a user's entry overrides (or hides) the system's one.
        let mut seen = std::collections::HashSet::new();
        for dir in application_dirs() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for file in entries.filter_map(Result::ok).map(|e| e.path()) {
                if file.extension().is_none_or(|e| e != "desktop") {
                    continue;
                }
                if !file
                    .file_name()
                    .is_some_and(|name| seen.insert(name.to_owned()))
                {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&file) else {
                    continue;
                };
                if let Some(entry) = desktop_entry(&text)
                    && entry.mime.iter().any(|m| m == mime)
                {
                    editors.push(Editor {
                        name: entry.name,
                        id: file.to_string_lossy().into_owned(),
                    });
                }
            }
        }
        editors.sort_by_key(|a| a.name.to_lowercase());
        editors
    }

    pub fn open_registered(id: &str, path: &Path) -> Result<bool> {
        if !id.ends_with(".desktop") {
            return Ok(false);
        }
        let text = std::fs::read_to_string(id).context("cannot read the program entry")?;
        let entry = desktop_entry(&text).context("not an application entry")?;
        let args = exec_args(&entry.exec, path);
        let (program, rest) = args.split_first().context("empty Exec line")?;
        Command::new(program)
            .args(rest)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("cannot start {program}"))?;
        Ok(true)
    }

    /// `xdg-open` picks the default itself, and `cerno.desktop` lists no video types.
    pub fn default_program(_extension: &str) -> Option<PathBuf> {
        None
    }

    /// No chooser here: the default program.
    pub fn choose(path: &Path) -> Result<()> {
        Command::new("xdg-open")
            .arg(path)
            .stdin(Stdio::null())
            .spawn()
            .map(drop)
            .context("cannot start xdg-open")
    }
}

/// The MIME type a `.desktop` entry lists for a photo extension (Linux).
#[cfg_attr(windows, allow(dead_code))]
fn mime_of(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "jpg" | "jpeg" | "jpe" | "jfif" => "image/jpeg",
        "heic" | "heif" | "hif" => "image/heif",
        "png" => "image/png",
        "tif" | "tiff" => "image/tiff",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => return None,
    })
}

/// The parts of a `.desktop` file Cerno needs.
#[cfg_attr(windows, allow(dead_code))]
#[derive(Debug, PartialEq)]
struct DesktopEntry {
    name: String,
    exec: String,
    mime: Vec<String>,
}

/// An application entry that shows in menus; `None` for hidden ones, links and broken files.
#[cfg_attr(windows, allow(dead_code))]
fn desktop_entry(text: &str) -> Option<DesktopEntry> {
    let mut in_entry = false;
    let (mut name, mut exec, mut mime, mut kind) = (None, None, Vec::new(), None);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "Name" => name = Some(value.trim().to_owned()),
            "Exec" => exec = Some(value.trim().to_owned()),
            "Type" => kind = Some(value.trim().to_owned()),
            "MimeType" => {
                mime = value
                    .split(';')
                    .map(str::trim)
                    .filter(|m| !m.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            "NoDisplay" | "Hidden" if value.trim() == "true" => return None,
            _ => {}
        }
    }
    (kind.as_deref() == Some("Application")).then_some(())?;
    Some(DesktopEntry {
        name: name?,
        exec: exec?,
        mime,
    })
}

/// The `Exec` line as program and arguments, with the photo for `%f` / `%F` / `%u` / `%U`
/// (appended when the line has none) and every other field code left out. Quoting as the
/// desktop entry specification has it: double quotes, backslash escapes inside them.
#[cfg_attr(windows, allow(dead_code))]
fn exec_args(exec: &str, path: &Path) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut any) = (false, false);
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            '\\' if quoted => word.extend(chars.next()),
            ' ' | '\t' if !quoted => {
                if any || !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
                any = false;
            }
            c => word.push(c),
        }
    }
    if any || !word.is_empty() {
        words.push(word);
    }
    let file = path.to_string_lossy().into_owned();
    let mut placed = false;
    let mut args = Vec::new();
    for word in words {
        match word.as_str() {
            "%f" | "%F" | "%u" | "%U" => {
                args.push(file.clone());
                placed = true;
            }
            w if w.len() == 2 && w.starts_with('%') => {}
            w => args.push(w.replace("%%", "%")),
        }
    }
    if !placed {
        args.push(file);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Any `cerno.exe` is Cerno – the installed one next to a build from the source tree –
    /// and a Store app's id is never a program file.
    #[test]
    fn cerno_is_recognised_by_its_file_name() {
        let own = Path::new(r"C:\src\Cerno\target\release\cerno.exe");
        assert!(is_cerno(
            Path::new(r"C:\Users\x\AppData\Local\Cerno\cerno.exe"),
            own
        ));
        assert!(is_cerno(Path::new(r"D:\Tools\CERNO.EXE"), own));
        assert!(!is_cerno(Path::new(r"C:\Program Files\VLC\vlc.exe"), own));
        assert!(!is_cerno(
            Path::new("Microsoft.ZuneVideo_8wekyb3d8bbwe!Microsoft.ZuneVideo"),
            own
        ));
        assert!(!is_cerno(Path::new(""), own));
    }

    /// A video that would open in Cerno plays in the next program the system offers.
    #[test]
    fn the_player_is_the_first_program_that_is_not_cerno() {
        let own = std::env::current_exe().expect("the test binary has a path");
        let editor = |name: &str, id: &str| Editor {
            name: name.to_owned(),
            id: id.to_owned(),
        };
        // The test binary stands in for Cerno: same file name.
        let cerno = editor("Cerno", &own.to_string_lossy());
        let films = editor(
            "Films & TV",
            "Microsoft.ZuneVideo_8wekyb3d8bbwe!Microsoft.ZuneVideo",
        );
        let offered = [cerno.clone(), films.clone()];
        assert_eq!(other_than_cerno(&offered), Some(&films));
        assert_eq!(other_than_cerno(&[cerno]), None);
    }

    #[test]
    fn the_choice_is_stored_as_name_and_id() {
        let editor = Editor {
            name: "Affinity Photo 2".to_owned(),
            id: r"C:\Program Files\Affinity\Photo 2\Photo.exe".to_owned(),
        };
        assert_eq!(Editor::from_setting(&editor.to_setting()), Some(editor));
        assert_eq!(Editor::from_setting("no tab"), None);
        assert_eq!(Editor::from_setting("\tid"), None);
        // Forward slashes: a separator on Windows and Linux alike.
        let picked = Editor::program(Path::new("D:/Tools/GIMP 3/gimp.exe"));
        assert_eq!(picked.name, "gimp");
    }

    #[test]
    fn desktop_entries_for_image_programs() {
        let gimp = "[Desktop Entry]\nType=Application\nName=GNU Image Manipulation Program\n\
            Exec=gimp-3.0 %U\nMimeType=image/jpeg;image/png;\n\n[Desktop Action new]\nName=New";
        let entry = desktop_entry(gimp).expect("an application");
        assert_eq!(entry.name, "GNU Image Manipulation Program");
        assert_eq!(entry.mime, ["image/jpeg", "image/png"]);
        assert_eq!(
            desktop_entry("[Desktop Entry]\nType=Link\nName=x\nExec=y"),
            None
        );
        assert_eq!(
            desktop_entry("[Desktop Entry]\nType=Application\nName=x\nExec=y\nNoDisplay=true"),
            None
        );
        assert_eq!(mime_of("jpg"), Some("image/jpeg"));
        assert_eq!(mime_of("cr3"), None);
    }

    #[test]
    fn exec_lines_get_the_photo() {
        let photo = Path::new("/home/a/Fotos/Grüße 1.jpg");
        assert_eq!(
            exec_args("gimp-3.0 %U", photo),
            ["gimp-3.0", "/home/a/Fotos/Grüße 1.jpg"]
        );
        assert_eq!(
            exec_args(
                r#"flatpak run --command=krita "org.kde.krita" %F --nosplash %i"#,
                photo
            ),
            [
                "flatpak",
                "run",
                "--command=krita",
                "org.kde.krita",
                "/home/a/Fotos/Grüße 1.jpg",
                "--nosplash"
            ]
        );
        assert_eq!(
            exec_args(r#""/opt/My Editor/run" --x"#, photo),
            ["/opt/My Editor/run", "--x", "/home/a/Fotos/Grüße 1.jpg"],
            "no field code: the photo goes last"
        );
    }

    /// The real system list; on Windows every machine offers something for JPEG (Photos,
    /// Paint). Only checks that asking does not fail.
    #[test]
    fn asking_the_system_works() {
        let editors = editors_for(Path::new("x.jpg"));
        eprintln!("programs for .jpg: {editors:?}");
        assert!(editors_for(Path::new("no-extension")).is_empty());
    }
}
