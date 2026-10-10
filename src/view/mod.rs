//! Which photos are shown and in which order: sorting and filtering of the folder list.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use std::sync::Arc;

use crate::analysis::{aesthetic, sharpness};
use crate::db::Scores;
use crate::i18n;
use crate::library::{self, Format};
use crate::metadata::{Label, Rating};

mod duplicates;
mod filter;
mod nav;
mod options;
mod series;
mod top;

use duplicates::*;
use series::*;

pub use filter::*;
pub use nav::*;
pub use options::*;
pub use top::*;

/// Photos taken at most this far apart belong to one series.
pub const SERIES_GAP_MS: i64 = 2_000;

/// Sharpness percentile (within the folder) below which a photo counts as probably blurry.
pub const BLURRY_PERCENTILE: f32 = 0.2;
/// Absolute ceilings for "probably blurry", so a folder of sharp photos gets no warnings: the
/// 10th percentile of the author's index on 2026-09-28, rounded (2 777 photos with
/// `sharpness_version = 1`: 260; 720 with `faces_version = 1` and eyes: 60).
pub const BLURRY_FRAME_MAX: f32 = 250.0;
pub const BLURRY_EYES_MAX: f32 = 60.0;

/// Probably out of focus: among the blurriest 20 % of the folder **and** below the absolute
/// ceiling of its measure (`eyes`: the eye region, otherwise the whole frame).
pub fn is_blurry(percentile: f32, raw: f32, eyes: bool) -> bool {
    let ceiling = if eyes {
        BLURRY_EYES_MAX
    } else {
        BLURRY_FRAME_MAX
    };
    percentile < BLURRY_PERCENTILE && raw < ceiling
}

/// What sorting and filtering look at for one photo.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Facts {
    pub rating: Rating,
    pub label: Option<Label>,
    /// Capture time in local wall-clock milliseconds.
    pub taken_ms: Option<i64>,
    /// `metadata::camera_id` of the camera model; series never mix cameras.
    pub camera: Option<u64>,
    /// Pixel fingerprint; `None` until the photo has been indexed.
    pub fingerprint: Option<u64>,
    pub scores: Scores,
    /// Personal taste model, 0..=5.
    pub personal: Option<f32>,
    /// Similarity to the photo the "similar" filter is about (-1..=1); `None` without an
    /// embedding, or while that filter is off.
    pub similarity: Option<f32>,
    /// In the Top N picked when that filter was chosen (`pick_top`).
    pub top: bool,
    /// Deleted in Cerno: it lies in `.originals` and shows only through the 🗑 box.
    pub deleted: bool,
    /// Named by the pasted file-name list, while that filter is on.
    pub listed: bool,
}

/// Where a photo sits in its series (at least two photos). `index` is 1-based, sharpest
/// non-rejected first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeriesPlace {
    pub id: u32,
    pub index: u32,
    pub len: u32,
    /// Capture time of the earliest photo in the series. Orders series against each other.
    pub start_ms: i64,
}

/// The filtered, sorted folder plus what the filmstrip and info bar need beside each path.
/// Derefs to the paths, so navigation can treat it as the list of photos.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub paths: Arc<Vec<PathBuf>>,
    /// Parallel to `paths`.
    pub series: Arc<Vec<Option<SeriesPlace>>>,
    /// Parallel to `paths`: the earlier photo with the same pixels, when this one is a copy.
    pub duplicate_of: Arc<Vec<Option<PathBuf>>>,
    /// Capture-time order keeps each series together, so the filmstrip can separate them.
    pub grouped: bool,
}

impl std::ops::Deref for View {
    type Target = [PathBuf];

    fn deref(&self) -> &[PathBuf] {
        &self.paths
    }
}

/// Sorted values of the folder for percentiles: the whole frame among all photos, the eyes
/// among the photos with a measurable face.
#[derive(Debug, Default, Clone)]
pub struct Percentiles {
    pub sharpness: Vec<f32>,
    pub eyes: Vec<f32>,
}

impl Percentiles {
    pub fn from_scores<'a>(scores: impl Iterator<Item = &'a Scores>) -> Self {
        let (mut sharpness, mut eyes) = (Vec::new(), Vec::new());
        for s in scores {
            sharpness.extend(s.sharpness);
            eyes.extend(s.eyes);
        }
        sharpness.sort_by(f32::total_cmp);
        eyes.sort_by(f32::total_cmp);
        Self { sharpness, eyes }
    }

    /// Whole-frame sharpness percentile, 0..=1.
    pub fn frame(&self, scores: &Scores) -> Option<f32> {
        Some(sharpness::percentile(scores.sharpness?, &self.sharpness))
    }

    /// Eye sharpness percentile among photos with faces, 0..=1.
    pub fn eyes(&self, scores: &Scores) -> Option<f32> {
        Some(sharpness::percentile(scores.eyes?, &self.eyes))
    }

    /// What matters for "is it in focus": the eyes if a face was measured, otherwise the
    /// whole frame. The flag says whether it came from the eyes.
    pub fn subject(&self, scores: &Scores) -> Option<(f32, bool)> {
        self.eyes(scores)
            .map(|p| (p, true))
            .or_else(|| self.frame(scores).map(|p| (p, false)))
    }

    /// The subject (see `subject`) is probably out of focus, see `is_blurry`.
    pub fn is_blurry(&self, scores: &Scores) -> bool {
        match self.subject(scores) {
            Some((p, true)) => scores.eyes.is_some_and(|raw| is_blurry(p, raw, true)),
            Some((p, false)) => scores.sharpness.is_some_and(|raw| is_blurry(p, raw, false)),
            None => false,
        }
    }
}

struct Entry<'a> {
    path: &'a PathBuf,
    facts: Option<Facts>,
    rating: Rating,
    label: Option<Label>,
    taken: Option<i64>,
    camera: Option<u64>,
    /// Subject sharpness percentile, when the photo has been measured.
    sharp: Option<f32>,
    /// In `.originals`: no part of the folder's percentiles or duplicates.
    deleted: bool,
}

/// Filters and sorts `all` (which is in name order). Session ratings and labels win over those
/// read from the files. Photos without scores yet are kept and sorted last, so nothing
/// disappears just because the analysis hasn't reached it. `hidden` drops photos that are
/// waiting to be deleted. Deleted photos (`Facts::deleted`) show only through the 🗑 box and
/// take no part in the sharpness ranks or the duplicate marks of the photos still there.
pub fn build(
    all: &[PathBuf],
    options: ViewOptions,
    facts: impl Fn(&Path) -> Option<Facts>,
    session_ratings: &HashMap<PathBuf, Rating>,
    session_labels: &HashMap<PathBuf, Option<Label>>,
    hidden: impl Fn(&Path) -> bool,
) -> View {
    let entries: Vec<Entry<'_>> = all
        .iter()
        .map(|path| {
            let known = facts(path);
            let rating = match session_ratings.get(path) {
                Some(rating) => *rating,
                None => known.map(|f| f.rating).unwrap_or_default(),
            };
            let label = match session_labels.get(path) {
                Some(label) => *label,
                None => known.and_then(|f| f.label),
            };
            Entry {
                path,
                taken: known.and_then(|f| f.taken_ms),
                camera: known.and_then(|f| f.camera),
                rating,
                label,
                facts: known,
                sharp: None,
                deleted: known.is_some_and(|f| f.deleted),
            }
        })
        .collect();
    let percentiles = Percentiles::from_scores(
        entries
            .iter()
            .filter(|e| !e.deleted)
            .filter_map(|e| e.facts.as_ref().map(|f| &f.scores)),
    );
    let fingerprints: HashMap<&Path, u64> = entries
        .iter()
        .filter(|entry| !entry.deleted)
        .filter_map(|entry| {
            entry
                .facts
                .and_then(|facts| facts.fingerprint)
                .map(|fp| (entry.path.as_path(), fp))
        })
        .collect();
    let marked: HashSet<&Path> = entries
        .iter()
        .filter(|entry| entry.rating != Rating::Unrated || entry.label.is_some())
        .map(|entry| entry.path.as_path())
        .collect();
    let copies = duplicate_originals(
        all,
        |path| fingerprints.get(path).copied(),
        |path| marked.contains(path),
    );

    let shown: Vec<Entry<'_>> = entries
        .into_iter()
        .map(|mut entry| {
            entry.sharp = entry
                .facts
                .as_ref()
                .and_then(|f| percentiles.subject(&f.scores))
                .map(|(p, _)| p);
            entry
        })
        .filter(|entry| {
            if hidden(entry.path) {
                return false;
            }
            // "without ✕" is about the photos still in the folder.
            if options.hide_rejected && !entry.deleted && entry.rating == Rating::Rejected {
                return false;
            }
            if options.name_list && !entry.facts.is_some_and(|f| f.listed) {
                return false;
            }
            let quality = Quality {
                blurry: entry
                    .facts
                    .as_ref()
                    .is_some_and(|f| percentiles.is_blurry(&f.scores)),
                duplicate: copies.contains_key(entry.path.as_path()),
                incomplete: entry
                    .facts
                    .is_some_and(|f| f.scores.truncated == Some(true)),
            };
            // A photo without an embedding can't be judged: it stays out.
            let similar = !options.similar
                || entry
                    .facts
                    .and_then(|f| f.similarity)
                    .is_some_and(|s| s >= SIMILAR_MIN);
            let video = library::format_of(entry.path) == Some(Format::Video);
            let in_scope = match options.top {
                Some(_) => entry.facts.is_some_and(|f| f.top),
                None => options.media.accepts(video),
            };
            let faces = entry.facts.and_then(|f| f.scores.faces);
            similar
                && in_scope
                && options
                    .filter
                    .accepts(entry.rating, quality, entry.label, faces, entry.deleted)
        })
        .collect();

    let places = series_places(&shown);
    let mut order: Vec<usize> = (0..shown.len()).collect();
    match options.sort {
        SortKey::Name => {}
        SortKey::Taken => order.sort_by(|&a, &b| taken_order(&shown, &places, a, b)),
        other => {
            let key = |entry: &Entry<'_>| -> Option<f32> {
                match other {
                    SortKey::Name | SortKey::Taken => None,
                    SortKey::Rating => Some(match entry.rating {
                        Rating::Stars(n) => f32::from(n),
                        Rating::Unrated => 0.0,
                        Rating::Rejected => -1.0,
                    }),
                    SortKey::Aesthetics => entry.facts.and_then(|f| {
                        aesthetic::combined(f.scores.aesthetic, f.scores.aesthetic25)
                    }),
                    SortKey::Personal => entry.facts.and_then(|f| f.personal),
                    SortKey::Sharpness => entry.sharp,
                }
            };
            order.sort_by(|&a, &b| match (key(&shown[a]), key(&shown[b])) {
                (Some(x), Some(y)) => y.total_cmp(&x),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            });
        }
    }

    let mut paths = Vec::with_capacity(order.len());
    let mut series = Vec::with_capacity(order.len());
    let mut duplicate_of = Vec::with_capacity(order.len());
    for index in order {
        paths.push(shown[index].path.clone());
        series.push(places[index]);
        duplicate_of.push(copies.get(shown[index].path).cloned());
    }
    View {
        paths: Arc::new(paths),
        series: Arc::new(series),
        duplicate_of: Arc::new(duplicate_of),
        grouped: options.sort == SortKey::Taken,
    }
}

#[cfg(test)]
mod tests;
