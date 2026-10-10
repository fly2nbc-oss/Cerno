//! RAW + JPG as one photo (`crate::pairs`): the switch, the marks the user sets going into the
//! RAW too, and the note in the info bar and the cell where the RAW's own marks differ from
//! the JPEG's. A RAW's marks are known from its XMP sidecar only – the RAW itself is not read,
//! so a pair whose RAW has none (or a DNG, which keeps its marks inside) shows no difference.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use eframe::egui;

use crate::i18n;
use crate::metadata::{self, Description, Label, Rating};
use crate::sidecar;

use super::CernoApp;
use super::notice::Notice;
use super::undo;

/// The saved switch; on unless it was turned off.
pub(super) const SETTING: &str = "raw_jpeg_pairs";

/// What a RAW's sidecar says: its rating and colour.
type Marks = (Rating, Option<Label>);

#[derive(Default)]
pub(super) struct RawMarks {
    known: HashMap<PathBuf, Marks>,
    rx: Option<mpsc::Receiver<HashMap<PathBuf, Marks>>>,
}

impl CernoApp {
    /// A folder opened: the sidecars of its pairs' RAWs are read on a thread.
    pub(super) fn scan_raw_marks(&mut self) {
        self.raw_marks = RawMarks::default();
        let raws: Vec<PathBuf> = self
            .all
            .iter()
            .filter_map(|jpeg| self.pairs.companion(jpeg))
            .filter(|raw| sidecar::applies(raw))
            .map(Path::to_path_buf)
            .collect();
        if raws.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("cerno-raw-marks".into())
            .spawn(move || {
                let found = raws
                    .into_iter()
                    .filter_map(|raw| {
                        let meta = metadata::read(&sidecar::read(&raw)?);
                        Some((raw, (meta.rating.value, meta.label.known())))
                    })
                    .collect();
                let _ = tx.send(found);
            });
        match spawned {
            Ok(_) => self.raw_marks.rx = Some(rx),
            Err(err) => log::warn!("raw marks: {err}"),
        }
    }

    pub(super) fn poll_raw_marks(&mut self) {
        if let Some(found) = self.raw_marks.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.raw_marks.rx = None;
            self.raw_marks.known = found;
        }
    }

    /// The RAW of `path`'s pair holds other marks than the JPEG – which, as a short text
    /// (`RAW: 5 Sterne, Rot`).
    fn raw_marks_differ(&self, path: &Path) -> Option<String> {
        let raw = self.pairs.companion(path)?;
        let (rating, label) = *self.raw_marks.known.get(raw)?;
        let t = i18n::t();
        let mut parts = Vec::new();
        if rating != self.rating_of(path, None) {
            parts.push(undo::rating_word(t, rating));
        }
        if label != self.label_of(path, None) {
            parts.push(undo::label_word(t, label).to_owned());
        }
        (!parts.is_empty()).then(|| (t.pair_raw_marks)(&parts.join(", ")))
    }

    /// What the sidecar of `path`'s RAW says, when `path` is a pair's JPEG and it is known.
    pub(super) fn companion_marks(&self, path: &Path) -> Option<(Rating, Option<Label>)> {
        let raw = self.pairs.companion(path)?;
        self.raw_marks.known.get(raw).copied()
    }

    /// What the info bar and the cell's tooltip say about a pair – `RAW+JPG`, and the RAW's
    /// marks where they differ – and whether they do.
    pub(super) fn pair_note(&self, path: &Path) -> Option<(String, bool)> {
        self.pairs
            .companion(path)
            .or_else(|| self.deleted.companion(path))?;
        let badge = i18n::t().pair_badge;
        Some(match self.raw_marks_differ(path) {
            Some(differ) => (format!("{badge} – {differ}"), true),
            None => (badge.to_owned(), false),
        })
    }

    /// Stars the user set on a pair go into its RAW too: from now on both agree on them.
    pub(super) fn rate_companion(&mut self, path: &Path, rating: Rating) {
        let Some(raw) = self.pairs.companion(path).map(Path::to_path_buf) else {
            return;
        };
        self.writer.set(raw.clone(), rating);
        let label = self.label_of(path, None);
        self.raw_marks
            .known
            .entry(raw)
            .and_modify(|marks| marks.0 = rating)
            .or_insert((rating, label));
    }

    /// A colour the user set on a pair goes into its RAW too.
    pub(super) fn label_companion(&mut self, path: &Path, label: Option<Label>) {
        let Some(raw) = self.pairs.companion(path).map(Path::to_path_buf) else {
            return;
        };
        self.writer.set_label(raw.clone(), label);
        let rating = self.rating_of(path, None);
        self.raw_marks
            .known
            .entry(raw)
            .and_modify(|marks| marks.1 = label)
            .or_insert((rating, label));
    }

    /// A comment and keywords the user set on a pair go into its RAW too.
    pub(super) fn describe_companion(&mut self, path: &Path, description: &Description) {
        if let Some(raw) = self.pairs.companion(path).map(Path::to_path_buf) {
            self.writer.set_description(raw, description.clone());
        }
    }

    /// Settings ▸ *RAW+JPG as one photo* (on by default): the folder opens again, at the
    /// photo shown – a RAW by its JPEG.
    pub(super) fn toggle_pairs(&mut self, ctx: &egui::Context) {
        self.pair_mode = !self.pair_mode;
        self.db.put_flag(SETTING, self.pair_mode);
        let at = self
            .view
            .get(self.current)
            .filter(|path| !self.is_deleted(path))
            .cloned()
            .or_else(|| self.dir.clone());
        if let Some(at) = at {
            self.open(ctx, &at);
        }
        // An error or "no photos here" from opening says more.
        if self.notice.is_none() {
            let t = i18n::t();
            self.notice = Some(Notice::hint(if self.pair_mode {
                t.pairs_on
            } else {
                t.pairs_off
            }));
        }
    }
}
