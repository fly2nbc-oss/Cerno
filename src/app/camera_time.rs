//! Camera clocks set right: the offsets of the open folder (`Db::camera_offsets`, added to
//! the capture time in `facts`), *This photo ▸ Match right camera to left* in compare mode and
//! the card *Visible photos ▸ Camera time …*. Only the index changes, never a file.

use std::collections::HashMap;
use std::path::Path;

use eframe::egui::{self, Rect};

use crate::camera_time;
use crate::i18n;
use crate::ui::camera_time as card_ui;

use super::CernoApp;
use super::layer::Layer;
use super::notice::Notice;

#[derive(Default)]
pub(super) struct CameraTime {
    /// Camera id → milliseconds added to its photos' capture times, in the open folder.
    offsets: HashMap<u64, i64>,
}

/// The card, while it is open (`Layer::CameraTime`).
pub(super) struct Card {
    /// The camera of each row.
    cameras: Vec<u64>,
    rows: Vec<card_ui::Row>,
}

impl CameraTime {
    pub(super) fn offset(&self, camera: Option<u64>) -> i64 {
        camera
            .and_then(|camera| self.offsets.get(&camera))
            .copied()
            .unwrap_or(0)
    }
}

/// The two photos of a comparison, as far as their clocks go.
struct Pair {
    left_taken: i64,
    left_offset: i64,
    right_taken: i64,
    right_camera: u64,
}

impl CernoApp {
    /// The open folder's offsets; another folder has its own.
    pub(super) fn load_camera_offsets(&mut self) {
        self.layer.close_if(Layer::is_camera_time);
        let folder = self
            .folder
            .dir
            .as_ref()
            .map(|dir| dir.to_string_lossy().into_owned());
        self.camera_time.offsets = match folder.map(|folder| self.db.camera_offsets(&folder)) {
            Some(Ok(offsets)) => offsets,
            Some(Err(err)) => {
                log::warn!("cannot read the camera offsets: {err:#}");
                HashMap::new()
            }
            None => HashMap::new(),
        };
    }

    /// What the camera of `path` is set right by, if anything.
    pub(super) fn time_offset_of(&self, path: &Path) -> Option<i64> {
        let offset = self.camera_time.offset(self.board.get(path)?.camera);
        (offset != 0).then_some(offset)
    }

    /// Both photos of the comparison with their camera and time, from different cameras –
    /// or why not.
    fn pair(&self) -> Result<Pair, &'static str> {
        let t = i18n::t();
        let (Some(left), Some(right)) = (self.viewer.pinned.as_ref(), self.view.get(self.current))
        else {
            return Err(t.align_needs_compare);
        };
        let known = |path: &Path| {
            let known = self.board.get(path)?;
            Some((known.camera?, known.taken_ms?))
        };
        let (Some((left_camera, left_taken)), Some((right_camera, right_taken))) =
            (known(left), known(right))
        else {
            return Err(t.align_no_time);
        };
        if left_camera == right_camera {
            return Err(t.align_same_camera);
        }
        Ok(Pair {
            left_taken,
            left_offset: self.camera_time.offset(Some(left_camera)),
            right_taken,
            right_camera,
        })
    }

    /// Why *Match right camera to left* is greyed out, if it is.
    pub(super) fn align_block(&self) -> Option<&'static str> {
        self.pair().err()
    }

    /// Compare mode: the right photo's camera gets the offset that makes it as old as the left
    /// one – both were taken at the same moment.
    pub(super) fn align_right_camera(&mut self, ctx: &egui::Context) {
        let pair = match self.pair() {
            Ok(pair) => pair,
            Err(reason) => {
                self.notice = Some(Notice::hint(reason));
                return;
            }
        };
        let offset = camera_time::aligned(pair.left_taken, pair.left_offset, pair.right_taken);
        if self.store_camera_offset(pair.right_camera, offset) {
            let name = self
                .board
                .camera_name(pair.right_camera)
                .unwrap_or_default();
            self.notice = Some(Notice::hint((i18n::t().camera_aligned)(
                &name,
                &camera_time::format_offset(offset),
            )));
            self.rebuild_view(ctx, None);
        }
    }

    /// Writes one camera's offset to the index and uses it from now on.
    fn store_camera_offset(&mut self, camera: u64, offset: i64) -> bool {
        let Some(dir) = &self.folder.dir else {
            return false;
        };
        if let Err(err) = self
            .db
            .set_camera_offset(&dir.to_string_lossy(), camera, offset)
        {
            log::warn!("cannot save the camera offset: {err:#}");
            self.notice = Some(Notice::error(format!("{err:#}")));
            return false;
        }
        if offset == 0 {
            self.camera_time.offsets.remove(&camera);
        } else {
            self.camera_time.offsets.insert(camera, offset);
        }
        true
    }

    /// *Visible photos ▸ Camera time …*: every camera of the folder whose photos have a
    /// capture time, the most photos first, with its offset.
    pub(super) fn open_camera_time(&mut self) {
        let mut counts: HashMap<u64, usize> = HashMap::new();
        for path in self.folder.all.iter() {
            if let Some(known) = self.board.get(path)
                && let (Some(camera), Some(_)) = (known.camera, known.taken_ms)
            {
                *counts.entry(camera).or_default() += 1;
            }
        }
        let mut rows: Vec<(u64, card_ui::Row)> = counts
            .into_iter()
            .map(|(camera, count)| {
                let offset = self.camera_time.offset(Some(camera));
                let row = card_ui::Row {
                    name: self.board.camera_name(camera).unwrap_or_default(),
                    count,
                    text: if offset == 0 {
                        String::new()
                    } else {
                        camera_time::format_offset(offset)
                    },
                };
                (camera, row)
            })
            .collect();
        rows.sort_by(|(_, a), (_, b)| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
        let (cameras, rows) = rows.into_iter().unzip();
        self.layer = Layer::CameraTime(Card { cameras, rows });
    }

    /// The card, while open; Apply stores what changed and sorts again.
    pub(super) fn draw_camera_time_card(&mut self, ctx: &egui::Context, window: Rect) {
        let Layer::CameraTime(card) = &mut self.layer else {
            return;
        };
        let out = card_ui::show(ctx, window, &mut card.rows);
        if out.apply
            && let Layer::CameraTime(card) = std::mem::take(&mut self.layer)
        {
            let mut changed = false;
            for (camera, row) in card.cameras.into_iter().zip(card.rows) {
                let offset = camera_time::parse_offset(&row.text).unwrap_or(0);
                if offset != self.camera_time.offset(Some(camera)) {
                    changed |= self.store_camera_offset(camera, offset);
                }
            }
            if changed {
                self.notice = Some(Notice::hint(i18n::t().camera_time_applied));
                self.rebuild_view(ctx, None);
            }
        } else if out.close {
            self.layer = Layer::None;
        }
    }
}
