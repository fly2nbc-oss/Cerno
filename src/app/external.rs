//! Editing the current photo in another program: the remembered one (`E`), one the system
//! offers for its type, one picked by hand, or the system's chooser. The first original goes
//! to `.originals` before, and the photo reloads as soon as the other program saves it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::Result;
use eframe::egui;

use crate::db::{Db, FileStamp};
use crate::external::{self, Editor};
use crate::filelock::FileLocks;
use crate::i18n;
use crate::library;
use crate::loader::Lookup;

use super::CernoApp;
use super::gate::Change;
use super::notice::Notice;

/// Setting that holds the remembered program.
pub(super) const SETTING: &str = "external_editor";
/// How long a photo opened elsewhere is watched for saves.
const WATCH_FOR: Duration = Duration::from_secs(2 * 60 * 60);
/// How often it is looked at.
const CHECK_EVERY: Duration = Duration::from_secs(1);
/// The system's programs are asked this long after the first photo shows (`poll_editors`).
const EDITORS_AFTER: Duration = Duration::from_secs(2);

/// A photo opened in another program, and how it looked then.
pub(super) struct Watched {
    path: PathBuf,
    stamp: Option<FileStamp>,
    /// Cerno's own writes (a star) change the file too; they bump this.
    generation: u64,
    until: Instant,
}

/// A program being started (`None`: the system's chooser), off the UI thread: keeping the
/// original copies the whole file and waits for the file lock.
pub(super) struct Launched {
    path: PathBuf,
    editor: Option<Editor>,
    /// The file's stamp and write generation just before the program got it.
    result: Result<(Option<FileStamp>, u64)>,
}

impl CernoApp {
    /// `E`: the remembered program – without one, the menu at "Edit elsewhere".
    pub(super) fn edit_elsewhere(&mut self) {
        if self.view.get(self.current).is_none() {
            return;
        }
        match self.external_editor.clone() {
            Some(editor) => self.open_in(editor),
            None => {
                self.prepare_editors();
                self.palette = Some(self.menu_at_editors());
            }
        }
    }

    /// Asks the system for the current photo's programs on a thread, a moment after the first
    /// photo shows: the shell can take a while, and the first menu or `E` would wait for it.
    pub(super) fn poll_editors(&mut self, ctx: &egui::Context) {
        if let Some(coming) = &self.editors_coming {
            match coming.try_recv() {
                Ok((extension, editors)) => {
                    self.editors.entry(extension).or_insert(editors);
                    self.editors_coming = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.editors_coming = None,
            }
            return;
        }
        let Some(shown) = self.first_photo.filter(|_| !self.editors_asked) else {
            return;
        };
        let waited = shown.elapsed();
        if waited < EDITORS_AFTER {
            ctx.request_repaint_after(EDITORS_AFTER - waited);
            return;
        }
        self.editors_asked = true;
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        let wanted = extension(&path);
        if self.editors.contains_key(&wanted) {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("cerno-editors".into())
            .spawn(move || {
                let _ = tx.send((wanted, external::editors_for(&path)));
                ctx.request_repaint();
            });
        if spawned.is_ok() {
            self.editors_coming = Some(rx);
        }
    }

    /// Asks the system once per file type which programs it offers.
    pub(super) fn prepare_editors(&mut self) {
        let Some(path) = self.view.get(self.current) else {
            return;
        };
        self.editors
            .entry(extension(path))
            .or_insert_with(|| external::editors_for(path));
    }

    /// The programs for the current photo's type, as asked (see `prepare_editors`).
    pub(super) fn editors_for_current(&self) -> &[Editor] {
        self.view
            .get(self.current)
            .and_then(|path| self.editors.get(&extension(path)))
            .map_or(&[], Vec::as_slice)
    }

    /// One of the system's programs, by its place in `editors_for_current`.
    pub(super) fn open_in_listed(&mut self, index: usize) {
        if let Some(editor) = self.editors_for_current().get(index).cloned() {
            self.open_in(editor);
        }
    }

    /// Opens the current photo in `editor`; once it started, `E` means that program.
    pub(super) fn open_in(&mut self, editor: Editor) {
        self.launch(Some(editor));
    }

    /// "Other program …": an executable picked in the file dialog, remembered like the others.
    pub(super) fn pick_editor(&mut self) {
        let mut dialog = rfd::FileDialog::new().set_title(i18n::t().external_pick_title);
        if cfg!(windows) {
            dialog = dialog.add_filter("exe", &["exe"]);
        }
        if let Some(program) = dialog.pick_file() {
            self.open_in(Editor::program(&program));
        }
    }

    /// The system's own chooser – what is picked there is not remembered.
    pub(super) fn edit_with_chooser(&mut self) {
        self.launch(None);
    }

    /// One action at a time, then a worker keeps the first original in `.originals` (without
    /// that copy the program does not start) and starts the program.
    fn launch(&mut self, editor: Option<Editor>) {
        let Some(path) = self.view.get(self.current).cloned() else {
            return;
        };
        if self.launching.is_some() || !self.allowed(Change::External, Some(&path)) {
            return;
        }
        // A video playing here would keep its file open in the other program's way.
        self.stop_video();
        let (tx, rx) = mpsc::channel();
        let db = Arc::clone(&self.db);
        let files = Arc::clone(&self.files);
        std::thread::spawn(move || {
            let result = keep_and_start(&db, &files, &path, editor.as_ref());
            let _ = tx.send(Launched {
                path,
                editor,
                result,
            });
        });
        self.launching = Some(rx);
    }

    /// The started program (or why not), then the photos opened elsewhere: for one another
    /// program has saved, the index forgets the old version and the writer puts back the marks
    /// the save dropped; then it is reloaded and analysed again. Cerno's own writes are told
    /// apart by the write generation.
    pub(super) fn poll_external(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.launching {
            match rx.try_recv() {
                Ok(launched) => {
                    self.launching = None;
                    self.launched(launched);
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
                Err(TryRecvError::Disconnected) => self.launching = None,
            }
        }
        if self.watched.is_empty() {
            return;
        }
        if self.external_checked.elapsed() < CHECK_EVERY {
            ctx.request_repaint_after(CHECK_EVERY);
            return;
        }
        self.external_checked = Instant::now();
        let now = Instant::now();
        self.watched.retain(|w| w.until > now);
        let mut saved = Vec::new();
        for watched in &mut self.watched {
            // Mid-write the file already differs while the generation is still the old one:
            // a busy photo waits, and a write that started or ended meanwhile counts as ours.
            let generation = self.files.generation(&watched.path);
            let stamp = FileStamp::of(&watched.path).ok();
            if self.files.busy(&watched.path) || self.files.generation(&watched.path) != generation
            {
                continue;
            }
            if stamp.is_none() || stamp == watched.stamp {
                continue;
            }
            if generation == watched.generation {
                saved.push(watched.path.clone());
            }
            watched.stamp = stamp;
            watched.generation = generation;
        }
        for path in saved {
            self.saved_elsewhere(&path);
        }
        ctx.request_repaint_after(CHECK_EVERY);
    }

    fn launched(&mut self, launched: Launched) {
        let Launched {
            path,
            editor,
            result,
        } = launched;
        let t = i18n::t();
        match result {
            Ok((stamp, generation)) => {
                self.watched.retain(|w| w.path != path);
                self.watched.push(Watched {
                    path,
                    stamp,
                    generation,
                    until: Instant::now() + WATCH_FOR,
                });
                if let Some(editor) = editor {
                    self.db.put_setting(SETTING, &editor.to_setting());
                    self.notice = Some(Notice::hint((t.external_opened)(&editor.name)));
                    self.external_editor = Some(editor);
                }
            }
            Err(err) => {
                self.notice = Some(Notice::error((t.external_failed)(&format!("{err:#}"))));
            }
        }
    }

    /// Another program saved `path`: the writer gets the marks Cerno showed until now (read
    /// before anything reloads), and only once they are back is the photo reloaded
    /// (`reload_saved`) – a reload first would show the file without them.
    fn saved_elsewhere(&mut self, path: &Path) {
        let image = self
            .view
            .iter()
            .position(|candidate| candidate == path)
            .and_then(|index| match self.loader.get(index) {
                Lookup::Ready(image) => Some(image),
                _ => None,
            });
        let rating = self.rating_of(path, image.as_deref());
        let label = self.label_of(path, image.as_deref());
        let description = self
            .description_of(path, image.as_deref())
            .unwrap_or_default();
        if let Err(err) = self.db.forget_file(&path.to_string_lossy()) {
            log::warn!("index: {err:#}");
        }
        log::info!("changed elsewhere: {}", path.display());
        self.writer
            .keep_marks(path.to_path_buf(), rating, label, description);
    }

    /// The writer is done with a photo another program saved: the new version is shown and
    /// analysed again.
    pub(super) fn reload_saved(&mut self, path: &Path) {
        self.refresh_edited(path);
        let name = library::file_name_lossy(path);
        self.notice = Some(Notice::hint((i18n::t().external_reloaded)(&name)));
    }
}

/// Keeps the first original, notes how the file looks, then starts the program.
fn keep_and_start(
    db: &Db,
    files: &FileLocks,
    path: &Path,
    editor: Option<&Editor>,
) -> Result<(Option<FileStamp>, u64)> {
    let seen = {
        let _held = files.hold(path);
        crate::originals::keep(db, path)?;
        (FileStamp::of(path).ok(), files.generation(path))
    };
    match editor {
        Some(editor) => external::open(editor, path)?,
        None => external::choose(path)?,
    }
    Ok(seen)
}

/// The lower-case extension that keys `CernoApp::editors`.
pub(super) fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}
