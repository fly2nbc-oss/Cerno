//! The faces of the current photo, for the details panel's faces tab (`Ctrl+Tab`) and the grid
//! of all faces (`G`). Where they are comes from the index (`Db::faces_of`); a photo analysed
//! before 1.7 has no rows yet, and its faces are looked for once more – only when one of the
//! two shows it, then stored. The pictures are cut from the photo at full size on a thread, for
//! the current photo only; nothing is analysed while browsing.

use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use eframe::egui::{self, ColorImage, TextureOptions, vec2};

use crate::analysis::faces as detection;
use crate::db::{Db, FaceRow};
use crate::filelock::FileLocks;
use crate::ui::details::{DetailsMode, DetailsTab};
use crate::ui::faces::{Crop, Shown};
use crate::ui::viewer;
use crate::{decode, library, metadata, view};

use super::CernoApp;

/// The longest side of a face's picture: the details panel shows it at its full width,
/// sharp on a 150 % display too.
const CROP_SIDE: u32 = 512;
/// A face's picture shows this much more than its box, so hair and chin are in it.
const CROP_MARGIN: f32 = 1.8;
/// The zoom to a face: its box takes this part of the photo area's height.
const ZOOM_SHARE: f32 = 0.45;
/// Never zoomed in further than this (physical pixels per image pixel).
const ZOOM_MAX: f32 = 2.0;
/// The size the analysis works at – a face below `MIN_FACE_WIDTH` there is too small.
const ANALYSIS_SIZE: u32 = 2048;

/// The faces of one photo, cut out: large enough to judge, left to right.
pub(super) struct Found {
    /// In 0..1 of the photo, parallel to `crops`.
    boxes: Vec<[f32; 4]>,
    crops: Vec<Crop>,
    /// How many were too small to judge.
    small: usize,
}

#[derive(Default)]
pub(super) struct Faces {
    /// The photo `found` (or the search in `rx`) is for.
    path: Option<PathBuf>,
    found: Option<Arc<Found>>,
    rx: Option<mpsc::Receiver<Result<Found, String>>>,
    /// `G`: every face over the photo.
    pub(super) grid_open: bool,
    /// A face to zoom to (0..1 box), once the photo's frame is known.
    pub(super) zoom_to: Option<[f32; 4]>,
}

/// What is known about the current photo's faces right now.
pub(super) enum State {
    Loading,
    Unknown,
    None,
    Ready(Arc<Found>),
}

impl State {
    pub(super) fn shown(&self) -> Shown<'_> {
        match self {
            Self::Loading => Shown::Loading,
            Self::Unknown => Shown::Unknown,
            Self::None => Shown::None,
            Self::Ready(found) => Shown::Ready {
                crops: &found.crops,
                small: found.small,
            },
        }
    }
}

impl CernoApp {
    /// The menu's *Faces*: the faces tab of the details panel (`Ctrl+Tab` steps there too).
    pub(super) fn open_faces(&mut self) {
        if self.details == DetailsMode::Off {
            self.set_details(DetailsMode::On);
            self.save_panels();
        }
        self.set_details_tab(DetailsTab::Faces);
    }

    /// `G`: every face of the photo large over it, or the photo again.
    pub(super) fn toggle_face_grid(&mut self) {
        self.faces.grid_open = !self.faces.grid_open;
    }

    /// The current photo's faces: from what the analysis knows, the cut-out pictures once
    /// they are ready – a search starts the first time they are asked for.
    pub(super) fn faces_of_current(&mut self, ctx: &egui::Context) -> State {
        let Some(path) = self.view.get(self.current).cloned() else {
            return State::Unknown;
        };
        if library::format_of(&path) == Some(library::Format::Video) {
            return State::Unknown;
        }
        let known = self.board.get(&path);
        match known.and_then(|k| k.scores.faces) {
            None => return State::Unknown,
            Some(0) => return State::None,
            Some(_) => {}
        }
        if self.faces.path.as_ref() != Some(&path) {
            self.faces.path = Some(path.clone());
            self.faces.found = None;
            self.faces.rx = Some(search(
                ctx.clone(),
                Arc::clone(&self.db),
                Arc::clone(&self.files),
                path,
                known.and_then(|k| k.fingerprint),
            ));
        }
        match &self.faces.found {
            Some(found) => State::Ready(Arc::clone(found)),
            None if self.faces.rx.is_some() => State::Loading,
            None => State::None,
        }
    }

    /// A finished search, if its photo is still the current one.
    pub(super) fn poll_faces(&mut self) {
        let Some(rx) = &self.faces.rx else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.faces.rx = None;
                match result {
                    Ok(found) => self.faces.found = Some(Arc::new(found)),
                    Err(err) => log::warn!("faces: {err}"),
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.faces.rx = None,
        }
    }

    /// A face was clicked in the tab or the grid: zoom to it (the grid closes, the single
    /// photo shows).
    pub(super) fn zoom_to_face(&mut self, index: usize) {
        let Some(found) = &self.faces.found else {
            return;
        };
        let Some(face) = found.boxes.get(index).copied() else {
            return;
        };
        self.faces.grid_open = false;
        if self.grid {
            self.set_grid(false);
        }
        self.faces.zoom_to = Some(face);
    }

    /// Applies a face zoom once the photo's frame is known: the face's box takes about half
    /// the area's height, centred – at least the whole photo, at most 200 %.
    pub(super) fn apply_face_zoom(&mut self, frames: &[viewer::Frame]) {
        let Some(frame) = frames.last() else {
            return;
        };
        let Some([x, y, w, h]) = self.faces.zoom_to.take() else {
            return;
        };
        let ih = frame.image_size[1] as f32;
        let area_h = frame.area.height() * frame.pixels_per_point;
        let wanted = area_h * ZOOM_SHARE / (h * ih).max(1.0);
        let fit = frame.fit_scale();
        self.zoom.scale = Some(wanted.clamp(fit, ZOOM_MAX.max(fit)));
        self.zoom.center = vec2(x + w / 2.0, y + h / 2.0);
    }

    /// The photo was changed (an edit, another program's save): its faces are looked for
    /// again the next time they show.
    pub(super) fn forget_faces(&mut self, path: &Path) {
        if self.faces.path.as_deref() == Some(path) {
            self.faces = Faces {
                grid_open: self.faces.grid_open,
                ..Faces::default()
            };
        }
    }
}

/// Looks for the faces of `path` on a thread and cuts them out.
fn search(
    ctx: egui::Context,
    db: Arc<Db>,
    files: Arc<FileLocks>,
    path: PathBuf,
    fingerprint: Option<u64>,
) -> mpsc::Receiver<Result<Found, String>> {
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("cerno-faces".into())
        .spawn(move || {
            let found =
                cut_out(&ctx, &db, &files, &path, fingerprint).map_err(|e| format!("{e:#}"));
            let _ = tx.send(found);
            ctx.request_repaint();
        });
    if let Err(err) = spawned {
        log::warn!("faces: {err}");
    }
    rx
}

fn cut_out(
    ctx: &egui::Context,
    db: &Db,
    files: &FileLocks,
    path: &Path,
    fingerprint: Option<u64>,
) -> anyhow::Result<Found> {
    use anyhow::Context as _;
    let format = library::format_of(path).context("unsupported file type")?;
    let bytes = {
        let _held = files.hold(path);
        std::fs::read(path).context("cannot read the photo")?
    };
    let meta = metadata::read_for(path, &bytes);
    let full = decode::catch_panic(|| {
        decode::decode_for_display(&bytes, format, meta.orientation, [u32::MAX, u32::MAX])
    })?;
    drop(bytes);
    let [aw, ah] = decode::fit_within([full.width, full.height], [ANALYSIS_SIZE, ANALYSIS_SIZE]);
    let stored = match fingerprint {
        Some(fp) => db.faces_of(fp)?,
        None => None,
    };
    let rows: Vec<FaceRow> = match stored {
        Some(rows) => rows,
        None => {
            // Analysed before 1.7: looked for once more, at the analysis size, and kept.
            let small = decode::resize_rgb(full.rgb.clone(), full.width, full.height, aw, ah)?;
            let found = detection::FaceDetector::load()?.detect(&small, aw, ah)?;
            let rows = detection::rows(&found, &small, aw, ah);
            if let Some(fp) = fingerprint {
                db.put_face_rows(fp, &rows)?;
            }
            rows
        }
    };
    let mut judged: Vec<&FaceRow> = rows
        .iter()
        .filter(|row| detection::measurable(row.bbox[2], aw))
        .collect();
    judged.sort_by(|a, b| a.bbox[0].total_cmp(&b.bbox[0]));
    let small = rows.len() - judged.len();
    let mut boxes = Vec::new();
    let mut crops = Vec::new();
    for row in judged {
        let picture = crop(&full, row.bbox)?;
        let texture = ctx.load_texture("face", picture, TextureOptions::LINEAR);
        boxes.push(row.bbox);
        crops.push(Crop {
            texture,
            eyes_blurry: row.eyes.is_some_and(|eyes| eyes < view::BLURRY_EYES_MAX),
        });
    }
    Ok(Found {
        boxes,
        crops,
        small,
    })
}

/// A square around the face, `CROP_MARGIN` times its box, inside the photo, at most
/// `CROP_SIDE` pixels.
fn crop(full: &decode::DecodedImage, bbox: [f32; 4]) -> anyhow::Result<ColorImage> {
    let (w, h) = (full.width as f32, full.height as f32);
    let [x, y, bw, bh] = bbox;
    let side = (bw * w).max(bh * h) * CROP_MARGIN;
    let side = side.min(w).min(h).max(1.0);
    let cx = (x + bw / 2.0) * w;
    let cy = (y + bh / 2.0) * h;
    let left = (cx - side / 2.0).clamp(0.0, w - side) as u32;
    let top = (cy - side / 2.0).clamp(0.0, h - side) as u32;
    let side = side as u32;
    let mut rgb = Vec::with_capacity((side * side * 3) as usize);
    for row in top..top + side {
        let start = ((row * full.width + left) * 3) as usize;
        rgb.extend_from_slice(&full.rgb[start..start + (side * 3) as usize]);
    }
    let out = side.min(CROP_SIDE);
    let rgb = decode::resize_rgb(rgb, side, side, out, out)?;
    Ok(ColorImage::from_rgb([out as usize, out as usize], &rgb))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32) -> decode::DecodedImage {
        let rgb = (0..width * height)
            .flat_map(|i| [(i % width) as u8, (i / width) as u8, 0])
            .collect();
        decode::DecodedImage {
            width,
            height,
            rgb,
            original_size: [width, height],
        }
    }

    /// A face's picture is a square of its box plus margin – shifted inside at an edge, cut at
    /// the photo's short side, at most `CROP_SIDE` large.
    #[test]
    fn a_face_is_cut_out_as_a_square_inside_the_photo() {
        let photo = image(200, 100);
        let middle = crop(&photo, [0.45, 0.4, 0.1, 0.2]).expect("crop");
        assert_eq!(middle.size, [36, 36], "20 px box × 1.8");
        // The top left pixel of the picture is where the square starts in the photo.
        let corner = crop(&photo, [0.0, 0.0, 0.05, 0.1]).expect("crop");
        assert_eq!(
            corner.pixels[0],
            egui::Color32::from_rgb(0, 0, 0),
            "pushed inside"
        );
        let huge = crop(&photo, [0.0, 0.0, 1.0, 1.0]).expect("crop");
        assert_eq!(huge.size, [100, 100], "no larger than the short side");
        let big = image(2000, 1500);
        assert_eq!(
            crop(&big, [0.2, 0.2, 0.5, 0.5]).expect("crop").size,
            [CROP_SIDE as usize; 2]
        );
    }
}
