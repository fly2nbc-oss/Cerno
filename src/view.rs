//! Which photos are shown and in which order: sorting and filtering of the folder list.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use std::sync::Arc;

use crate::analysis::{aesthetic, sharpness};
use crate::db::Scores;
use crate::i18n;
use crate::library::{self, Format};
use crate::metadata::{Label, Rating};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    /// Capture time, oldest first. Photos without a time stay at the end.
    Taken,
    Rating,
    /// The one aesthetics score: the mean of LAION and V2.5 (`aesthetic::combined`).
    Aesthetics,
    Personal,
    Sharpness,
}

impl SortKey {
    pub const ALL: [SortKey; 6] = [
        Self::Name,
        Self::Taken,
        Self::Rating,
        Self::Aesthetics,
        Self::Personal,
        Self::Sharpness,
    ];

    pub fn label(self) -> &'static str {
        let t = i18n::t();
        match self {
            Self::Name => t.sort_name,
            Self::Taken => t.sort_taken,
            Self::Rating => t.sort_rating,
            Self::Aesthetics => t.sort_aesthetics,
            Self::Personal => t.sort_personal,
            Self::Sharpness => t.sort_sharpness,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Taken => "taken",
            Self::Rating => "rating",
            Self::Aesthetics => "aesthetics",
            Self::Personal => "personal",
            Self::Sharpness => "sharpness",
        }
    }

    /// A stored sort. Up to 1.2 LAION and V2.5 were sorted apart; a saved V2.5 sort
    /// (`"aesthetics25"`) now means the combined score, like `"aesthetics"`.
    pub fn from_id(id: &str) -> Option<Self> {
        if id == "aesthetics25" {
            return Some(Self::Aesthetics);
        }
        Self::ALL.into_iter().find(|k| k.id() == id)
    }
}

/// One checkbox in the filter bar. Several may be on at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    /// 1..=5.
    Stars(u8),
    Unrated,
    Rejected,
    Blurry,
    /// A later copy of an earlier photo. The first path in folder order stays out.
    Duplicate,
    /// A colour label. Nothing ticked means every colour.
    Colour(Label),
    /// The face detection found a face – people from behind or very small don't count.
    People,
    /// The face detection ran and found none.
    NoPeople,
    /// Deleted in Cerno, lying in `.originals`; shown only while this box is ticked.
    Deleted,
}

impl FilterKind {
    pub const ALL: [FilterKind; 17] = [
        Self::Stars(1),
        Self::Stars(2),
        Self::Stars(3),
        Self::Stars(4),
        Self::Stars(5),
        Self::Unrated,
        Self::Rejected,
        Self::Blurry,
        Self::Duplicate,
        Self::Colour(Label::Red),
        Self::Colour(Label::Yellow),
        Self::Colour(Label::Green),
        Self::Colour(Label::Blue),
        Self::Colour(Label::Purple),
        Self::People,
        Self::NoPeople,
        Self::Deleted,
    ];

    pub fn label(self) -> String {
        let t = i18n::t();
        match self {
            Self::Stars(n) => (t.filter_stars)(n),
            Self::Unrated => t.filter_unrated.to_owned(),
            Self::Rejected => t.filter_rejected.to_owned(),
            Self::Blurry => t.filter_blurry.to_owned(),
            Self::Duplicate => t.filter_duplicate.to_owned(),
            Self::Colour(label) => i18n::label_name(label).to_owned(),
            Self::People => t.filter_people.to_owned(),
            Self::NoPeople => t.filter_no_people.to_owned(),
            Self::Deleted => t.filter_deleted.to_owned(),
        }
    }

    fn token(self) -> &'static str {
        match self {
            Self::Stars(1) => "1",
            Self::Stars(2) => "2",
            Self::Stars(3) => "3",
            Self::Stars(4) => "4",
            Self::Stars(5) => "5",
            Self::Stars(_) => "",
            Self::Unrated => "unrated",
            Self::Rejected => "rejected",
            Self::Blurry => "blurry",
            Self::Duplicate => "duplicate",
            Self::Colour(label) => label.id(),
            Self::People => "people",
            Self::NoPeople => "nopeople",
            Self::Deleted => "deleted",
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.token() == token)
    }

    /// The filter bar's groups, in its order (the menu follows it). Within a group a photo
    /// needs any ticked box, across groups every group with a ticked box. `ALL` keeps the
    /// order of the stored ids.
    pub const GROUPS: [&'static [FilterKind]; 4] = [
        &[
            Self::Rejected,
            Self::Deleted,
            Self::Unrated,
            Self::Stars(1),
            Self::Stars(2),
            Self::Stars(3),
            Self::Stars(4),
            Self::Stars(5),
        ],
        &[
            Self::Colour(Label::Red),
            Self::Colour(Label::Yellow),
            Self::Colour(Label::Green),
            Self::Colour(Label::Blue),
            Self::Colour(Label::Purple),
        ],
        &[Self::Blurry, Self::Duplicate],
        &[Self::People, Self::NoPeople],
    ];
}

/// Which photos stay visible. Nothing ticked means every photo. Otherwise a photo stays when,
/// in every group with a ticked box (rating, colour, blurry / duplicate, people –
/// `FilterKind::GROUPS`), it matches one of them: 4★ + blurry are the blurry 4-star photos,
/// 4★ + 5★ both ratings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhotoFilter {
    /// Index 0 is 1 star.
    stars: [bool; 5],
    unrated: bool,
    rejected: bool,
    blurry: bool,
    duplicate: bool,
    /// Same order as `Label::ALL`.
    colours: [bool; 5],
    people: bool,
    no_people: bool,
    deleted: bool,
}

impl PhotoFilter {
    pub fn is_all(self) -> bool {
        !self.stars.iter().any(|on| *on)
            && !self.unrated
            && !self.rejected
            && !self.blurry
            && !self.duplicate
            && !self.colours.iter().any(|on| *on)
            && !self.people
            && !self.no_people
            && !self.deleted
    }

    /// Any colour box is ticked, so changing a label can hide the current photo.
    pub fn has_colour(self) -> bool {
        self.colours.iter().any(|on| *on)
    }

    pub fn contains(self, kind: FilterKind) -> bool {
        match kind {
            FilterKind::Stars(n) if (1..=5).contains(&n) => self.stars[n as usize - 1],
            FilterKind::Unrated => self.unrated,
            FilterKind::Rejected => self.rejected,
            FilterKind::Blurry => self.blurry,
            FilterKind::Duplicate => self.duplicate,
            FilterKind::Colour(label) => self.colours[colour_index(label)],
            FilterKind::People => self.people,
            FilterKind::NoPeople => self.no_people,
            FilterKind::Deleted => self.deleted,
            FilterKind::Stars(_) => false,
        }
    }

    pub fn set(&mut self, kind: FilterKind, on: bool) {
        match kind {
            FilterKind::Stars(n) if (1..=5).contains(&n) => self.stars[n as usize - 1] = on,
            FilterKind::Unrated => self.unrated = on,
            FilterKind::Rejected => self.rejected = on,
            FilterKind::Blurry => self.blurry = on,
            FilterKind::Duplicate => self.duplicate = on,
            FilterKind::Colour(label) => self.colours[colour_index(label)] = on,
            FilterKind::People => self.people = on,
            FilterKind::NoPeople => self.no_people = on,
            FilterKind::Deleted => self.deleted = on,
            FilterKind::Stars(_) => {}
        }
    }

    pub fn toggle(&mut self, kind: FilterKind) {
        let on = !self.contains(kind);
        self.set(kind, on);
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// A photo matches every group with a ticked box: its rating is ticked, its colour is
    /// ticked, it is blurry or a copy when one of those boxes is ticked, and it has faces or
    /// none (`faces`: `None` until the face detection ran – then neither box takes it). A
    /// deleted photo shows only through the 🗑 box, which stands for its rating: 🗑 + ✕ are the
    /// deleted and the rejected photos, 🗑 + red the deleted ones with a red label.
    pub fn accepts(
        self,
        rating: Rating,
        is_blurry: bool,
        is_duplicate: bool,
        colour: Option<Label>,
        faces: Option<u8>,
        deleted: bool,
    ) -> bool {
        if deleted && !self.deleted {
            return false;
        }
        let rating_ticked =
            self.stars.iter().any(|on| *on) || self.unrated || self.rejected || self.deleted;
        let by_rating = !rating_ticked
            || deleted
            || match rating {
                Rating::Stars(n) if (1..=5).contains(&n) => self.stars[n as usize - 1],
                Rating::Unrated => self.unrated,
                Rating::Rejected => self.rejected,
                Rating::Stars(_) => false,
            };
        let by_colour =
            !self.has_colour() || colour.is_some_and(|label| self.colours[colour_index(label)]);
        let by_quality = !(self.blurry || self.duplicate)
            || (self.blurry && is_blurry)
            || (self.duplicate && is_duplicate);
        let by_people = !(self.people || self.no_people)
            || (self.people && faces.is_some_and(|n| n > 0))
            || (self.no_people && faces == Some(0));
        by_rating && by_colour && by_quality && by_people
    }

    /// Stored setting. A leading `*` marks the exact set, so an old `"3"` (at least 3 stars)
    /// still reads as 3, 4 and 5. Empty means every photo.
    pub fn id(self) -> String {
        if self.is_all() {
            return String::new();
        }
        let tokens: Vec<&str> = FilterKind::ALL
            .into_iter()
            .filter(|kind| self.contains(*kind))
            .map(FilterKind::token)
            .filter(|token| !token.is_empty())
            .collect();
        format!("*{}", tokens.join(","))
    }

    pub fn from_stored(id: &str) -> Self {
        let mut filter = Self::default();
        if id.is_empty() || id == "all" {
            return filter;
        }
        if let Some(rest) = id.strip_prefix('*') {
            for token in rest.split(',').filter(|token| !token.is_empty()) {
                if let Some(kind) = FilterKind::from_token(token) {
                    filter.set(kind, true);
                }
            }
            return filter;
        }
        // Saved before checkboxes: one choice, and a digit meant "at least".
        if let Some(kind) = FilterKind::from_token(id)
            && !matches!(kind, FilterKind::Stars(_))
        {
            filter.set(kind, true);
            return filter;
        }
        if let Ok(n) = id.parse::<u8>()
            && (1..=5).contains(&n)
        {
            for star in n..=5 {
                filter.set(FilterKind::Stars(star), true);
            }
        }
        filter
    }
}

fn colour_index(label: Label) -> usize {
    Label::ALL
        .iter()
        .position(|item| *item == label)
        .unwrap_or(0)
}

/// Photos at least this similar to the chosen one pass the "similar photos" filter (cosine of
/// their CLIP embeddings). Measured on the author's index (2026-09-29, 3077 photos): 90 % of
/// the pairs within a two-second series reach it, 0.4 % of photos taken more than an hour
/// apart, 0.03 % of photos from different folders.
pub const SIMILAR_MIN: f32 = 0.85;

/// Photos, videos or both – together with the boxes, like "similar photos".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Media {
    #[default]
    All,
    Photos,
    Videos,
}

impl Media {
    pub const ALL: [Media; 3] = [Self::All, Self::Photos, Self::Videos];

    pub fn label(self) -> &'static str {
        let t = i18n::t();
        match self {
            Self::All => t.media_all,
            Self::Photos => t.media_photos,
            Self::Videos => t.media_videos,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Photos => "photos",
            Self::Videos => "videos",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|media| media.id() == id)
    }

    /// Whether a file of this kind stays visible.
    pub fn accepts(self, video: bool) -> bool {
        match self {
            Self::All => true,
            Self::Photos => !video,
            Self::Videos => video,
        }
    }
}

/// The sizes "Top N" offers: highlights, a preview, a slideshow, a photo book, a gallery.
pub const TOP_LEVELS: [u16; 5] = [10, 25, 50, 100, 250];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewOptions {
    pub sort: SortKey,
    pub filter: PhotoFilter,
    /// Only photos like the chosen one (`M`, `Facts::similarity`), together with the boxes.
    /// Never saved: the photo it is about belongs to this folder.
    pub similar: bool,
    /// Photos, videos or both; saved like the boxes.
    pub media: Media,
    /// Only the best N photos of what the other filters leave (`pick_top`, `Facts::top`); it
    /// takes the place of `media` – the best photos are photos. Never saved.
    pub top: Option<u16>,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            sort: SortKey::Name,
            filter: PhotoFilter::default(),
            similar: false,
            media: Media::All,
            top: None,
        }
    }
}

impl ViewOptions {
    /// Whether new analysis results can change the view (so a refresh makes sense). Photos or
    /// videos only is decided by the file name, not by a score.
    pub fn depends_on_scores(&self) -> bool {
        Self {
            media: Media::All,
            ..*self
        } != Self::default()
    }

    /// Whether anything hides photos: a box, "similar photos", photos / videos only or Top N.
    pub fn is_filtered(&self) -> bool {
        !self.filter.is_all() || self.similar || self.media != Media::All || self.top.is_some()
    }

    /// "Show all": every filter off, the sort stays.
    pub fn clear_filters(&mut self) {
        self.filter.clear();
        self.similar = false;
        self.media = Media::All;
        self.top = None;
    }

    /// What a Top N pick depends on: every option but the sort – another sort shows the same
    /// photos in another order. `None` while Top N is off.
    pub fn top_key(&self) -> Option<ViewOptions> {
        self.top.map(|_| ViewOptions {
            sort: SortKey::Name,
            ..*self
        })
    }
}

/// The filter bar's first box: photos, videos or both – or the best N photos. One choice, so
/// "videos only" and "the best 50" can't contradict each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Media(Media),
    Top(u16),
}

impl Scope {
    pub fn all() -> impl Iterator<Item = Scope> {
        Media::ALL
            .into_iter()
            .map(Self::Media)
            .chain(TOP_LEVELS.into_iter().map(Self::Top))
    }

    pub fn of(options: &ViewOptions) -> Self {
        options.top.map_or(Self::Media(options.media), Self::Top)
    }

    /// Choosing photos, videos or both ends Top N; choosing Top N keeps the saved media
    /// choice for later.
    pub fn apply(self, options: &mut ViewOptions) {
        match self {
            Self::Media(media) => {
                options.media = media;
                options.top = None;
            }
            Self::Top(n) => options.top = Some(n),
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Media(media) => media.label().to_owned(),
            Self::Top(n) => (i18n::t().top_photos)(n),
        }
    }

    /// What a level is for ("Slideshow"), shown beside it in the list.
    pub fn purpose(self) -> Option<&'static str> {
        let Self::Top(n) = self else {
            return None;
        };
        let index = TOP_LEVELS.iter().position(|level| *level == n)?;
        i18n::t().top_purposes.get(index).copied()
    }
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
            let blurry = entry
                .facts
                .as_ref()
                .is_some_and(|f| percentiles.is_blurry(&f.scores));
            let is_duplicate = copies.contains_key(entry.path.as_path());
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
                && options.filter.accepts(
                    entry.rating,
                    blurry,
                    is_duplicate,
                    entry.label,
                    faces,
                    entry.deleted,
                )
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

/// The best `n` photos of what the other filters leave, for "Top N". A photo's value is the
/// mean of what is known about it, each 0..=1: its own stars (else the For-you prediction),
/// the aesthetics (`aesthetic::as_percent`, the info bar's scale) and the subject sharpness
/// within the folder. Rejected and probably blurry photos, copies, videos and photos without
/// any value never count. Round one takes the best photo of every series and every photo
/// outside one, best first; round two the second best of each series, and so on – a burst
/// can't fill the list with look-alikes. The caller keeps the result: picking again after
/// every mark would slip the next photo into a rejected one's place unnoticed.
pub fn pick_top(
    all: &[PathBuf],
    options: ViewOptions,
    facts: impl Fn(&Path) -> Option<Facts>,
    session_ratings: &HashMap<PathBuf, Rating>,
    session_labels: &HashMap<PathBuf, Option<Label>>,
    hidden: impl Fn(&Path) -> bool,
    n: usize,
) -> HashSet<PathBuf> {
    let candidates = build(
        all,
        ViewOptions {
            top: None,
            media: Media::Photos,
            ..options
        },
        &facts,
        session_ratings,
        session_labels,
        &hidden,
    );
    let scores: Vec<Scores> = all
        .iter()
        .filter_map(|p| facts(p))
        .map(|f| f.scores)
        .collect();
    let percentiles = Percentiles::from_scores(scores.iter());
    // Each series is one group, each photo outside a series a group of its own.
    let mut groups: Vec<Vec<(f32, &PathBuf)>> = Vec::new();
    let mut of_series: HashMap<u32, usize> = HashMap::new();
    for (i, path) in candidates.paths.iter().enumerate() {
        if candidates.duplicate_of[i].is_some() {
            continue;
        }
        let Some(known) = facts(path) else {
            continue;
        };
        let rating = session_ratings.get(path).copied().unwrap_or(known.rating);
        if rating == Rating::Rejected || known.deleted || percentiles.is_blurry(&known.scores) {
            continue;
        }
        let Some(value) = top_value(rating, &known, &percentiles) else {
            continue;
        };
        let group = match candidates.series[i] {
            Some(place) => *of_series.entry(place.id).or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            }),
            None => {
                groups.push(Vec::new());
                groups.len() - 1
            }
        };
        groups[group].push((value, path));
    }
    for group in &mut groups {
        group.sort_by(|a, b| b.0.total_cmp(&a.0));
    }
    let mut picked = HashSet::new();
    for round in 0.. {
        let mut best: Vec<(f32, &PathBuf)> = groups
            .iter()
            .filter_map(|group| group.get(round).copied())
            .collect();
        if best.is_empty() {
            break;
        }
        best.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, path) in best {
            if picked.len() == n {
                return picked;
            }
            picked.insert(path.clone());
        }
    }
    picked
}

/// One photo's value for Top N, 0..=1: the mean of what is known – none of it, no value.
fn top_value(rating: Rating, facts: &Facts, percentiles: &Percentiles) -> Option<f32> {
    let stars = match rating {
        Rating::Stars(n) => Some(f32::from(n) / 5.0),
        Rating::Unrated => facts.personal.map(|p| p / 5.0),
        Rating::Rejected => None,
    };
    let aesthetics = aesthetic::combined(facts.scores.aesthetic, facts.scores.aesthetic25)
        .map(aesthetic::as_percent);
    let sharpness = percentiles.subject(&facts.scores).map(|(p, _)| p);
    let known: Vec<f32> = [stars, aesthetics, sharpness]
        .into_iter()
        .flatten()
        .collect();
    (!known.is_empty()).then(|| known.iter().sum::<f32>() / known.len() as f32)
}

/// Photos with the same fingerprint: one is the original (`pick_original`), every other one
/// points at it.
fn duplicate_originals(
    all: &[PathBuf],
    fingerprint: impl Fn(&Path) -> Option<u64>,
    marked: impl Fn(&Path) -> bool,
) -> HashMap<PathBuf, PathBuf> {
    let mut groups: HashMap<u64, Vec<&PathBuf>> = HashMap::new();
    for path in all {
        if let Some(fp) = fingerprint(path) {
            groups.entry(fp).or_default().push(path);
        }
    }
    let mut copies = HashMap::new();
    for paths in groups.into_values() {
        if paths.len() < 2 {
            continue;
        }
        let original = pick_original(&paths, &marked);
        for dup in paths.into_iter().filter(|path| *path != original) {
            copies.insert(dup.clone(), original.clone());
        }
    }
    copies
}

/// Which of several identical files is the original:
/// 1. the one whose name the others only extend – `IMG_1.jpg` for `IMG_1 - Kopie.jpg` or
///    `IMG_1 (1).jpg`, which sort *before* it (a space comes before the dot);
/// 2. otherwise the only one with stars, a rejection or a colour label;
/// 3. otherwise the first in folder order.
fn pick_original<'a>(paths: &[&'a PathBuf], marked: &impl Fn(&Path) -> bool) -> &'a PathBuf {
    let stem = |path: &Path| -> Vec<char> {
        path.file_stem()
            .map(|s| {
                s.to_string_lossy()
                    .chars()
                    .map(crate::library::sort_key)
                    .collect()
            })
            .unwrap_or_default()
    };
    let stems: Vec<Vec<char>> = paths.iter().map(|path| stem(path)).collect();
    let shortest: Vec<usize> = (0..paths.len())
        .filter(|&i| {
            (0..paths.len()).all(|j| {
                j == i || (stems[j].len() > stems[i].len() && stems[j].starts_with(&stems[i]))
            })
        })
        .collect();
    if let [only] = shortest[..] {
        return paths[only];
    }
    let with_marks: Vec<usize> = (0..paths.len()).filter(|&i| marked(paths[i])).collect();
    if let [only] = with_marks[..] {
        return paths[only];
    }
    paths[0]
}

/// Series of photos from one camera that follow each other by at most [`SERIES_GAP_MS`] – two
/// cameras firing at the same moment make two series. Photos without a camera model only form
/// series with each other. A single photo is not a series. Within a series the sharpest
/// non-rejected photo is index 1; rejected and not-yet-measured photos come last.
fn series_places(shown: &[Entry<'_>]) -> Vec<Option<SeriesPlace>> {
    let mut timed: Vec<usize> = shown
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.taken.is_some())
        .map(|(index, _)| index)
        .collect();
    timed.sort_by(|&a, &b| {
        (shown[a].camera, shown[a].taken, a).cmp(&(shown[b].camera, shown[b].taken, b))
    });

    let mut groups: Vec<Vec<usize>> = Vec::new();
    for index in timed {
        let taken = shown[index].taken.unwrap_or(0);
        if let Some(group) = groups.last_mut()
            && let Some(&prev) = group.last()
            && shown[prev].camera == shown[index].camera
            && taken - shown[prev].taken.unwrap_or(0) <= SERIES_GAP_MS
        {
            group.push(index);
            continue;
        }
        groups.push(vec![index]);
    }

    let mut places = vec![None; shown.len()];
    let mut id = 0u32;
    for mut group in groups {
        if group.len() < 2 {
            continue;
        }
        let start_ms = group
            .iter()
            .filter_map(|&index| shown[index].taken)
            .min()
            .unwrap_or(0);
        group.sort_by(|&a, &b| series_rank(&shown[a], &shown[b]).then(a.cmp(&b)));
        let len = group.len() as u32;
        id += 1;
        for (rank, index) in group.into_iter().enumerate() {
            places[index] = Some(SeriesPlace {
                id,
                index: rank as u32 + 1,
                len,
                start_ms,
            });
        }
    }
    places
}

/// Rejected last, then unmeasured, then the sharpest first.
fn series_rank(a: &Entry<'_>, b: &Entry<'_>) -> std::cmp::Ordering {
    let key = |entry: &Entry<'_>| (entry.rating == Rating::Rejected, entry.sharp.is_none());
    key(a).cmp(&key(b)).then_with(|| match (a.sharp, b.sharp) {
        (Some(x), Some(y)) => y.total_cmp(&x),
        _ => std::cmp::Ordering::Equal,
    })
}

fn taken_order(
    shown: &[Entry<'_>],
    places: &[Option<SeriesPlace>],
    a: usize,
    b: usize,
) -> std::cmp::Ordering {
    // The series id keeps two cameras' series apart when they start in the same millisecond
    // (files with whole seconds only).
    let key = |index: usize| -> Option<(i64, u32, u32)> {
        let taken = shown[index].taken?;
        Some(
            places[index]
                .map(|place| (place.start_ms, place.id, place.index))
                .unwrap_or((taken, 0, 0)),
        )
    };
    match (key(a), key(b)) {
        (Some(x), Some(y)) => x.cmp(&y).then(a.cmp(&b)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.cmp(&b),
    }
}

/// `index`, or – if that is the pinned photo (compare mode) – its neighbour in `direction`,
/// else the other one. `None` if nothing but the pinned photo is left.
pub fn skip_pinned(
    len: usize,
    index: usize,
    pinned: Option<usize>,
    direction: isize,
) -> Option<usize> {
    if index >= len {
        return None;
    }
    if Some(index) != pinned {
        return Some(index);
    }
    let valid = |i: Option<usize>| i.filter(|&i| i < len);
    valid(index.checked_add_signed(direction)).or(valid(index.checked_add_signed(-direction)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_steps_over_the_pinned_photo() {
        assert_eq!(skip_pinned(5, 2, None, 1), Some(2));
        assert_eq!(skip_pinned(5, 2, Some(2), 1), Some(3));
        assert_eq!(skip_pinned(5, 2, Some(2), -1), Some(1));
        // At the end there is only the way back.
        assert_eq!(skip_pinned(5, 4, Some(4), 1), Some(3));
        assert_eq!(skip_pinned(5, 0, Some(0), -1), Some(1));
        assert_eq!(skip_pinned(1, 0, Some(0), 1), None);
        assert_eq!(skip_pinned(0, 0, None, 1), None);
    }

    fn facts(
        rating: Rating,
        aesthetic: Option<f32>,
        sharpness: Option<f32>,
        personal: Option<f32>,
    ) -> Facts {
        Facts {
            rating,
            scores: Scores {
                sharpness,
                aesthetic,
                ..Scores::default()
            },
            personal,
            ..Facts::default()
        }
    }

    /// The same facts with a V2.5 score as well.
    fn with_v25(mut facts: Facts, v25: f32) -> Facts {
        facts.scores.aesthetic25 = Some(v25);
        facts
    }

    fn fixture() -> (Vec<PathBuf>, HashMap<PathBuf, Facts>) {
        let all: Vec<PathBuf> = ["a", "b", "c", "d", "e"].map(PathBuf::from).to_vec();
        let known = HashMap::from([
            // Aesthetics: a (4.0 + 8.0) / 2 = 6.0, b LAION alone 6.5, c (5.0 + 4.0) / 2 = 4.5,
            // d V2.5 alone 5.5.
            (
                all[0].clone(),
                with_v25(
                    facts(Rating::Stars(2), Some(4.0), Some(10.0), Some(1.0)),
                    8.0,
                ),
            ),
            (
                all[1].clone(),
                facts(Rating::Unrated, Some(6.5), Some(500.0), None),
            ),
            (
                all[2].clone(),
                with_v25(
                    facts(Rating::Stars(5), Some(5.0), Some(300.0), Some(4.5)),
                    4.0,
                ),
            ),
            (
                all[3].clone(),
                with_v25(facts(Rating::Stars(3), None, Some(200.0), Some(3.0)), 5.5),
            ),
            // "e" not analysed yet
        ]);
        (all, known)
    }

    fn names(view: &[PathBuf]) -> String {
        view.iter().map(|p| p.to_string_lossy()).collect()
    }

    /// Only photos close enough stay, together with the boxes; one without an embedding can't
    /// be judged and stays out.
    #[test]
    fn the_similar_filter_keeps_close_photos() {
        let (all, mut known) = fixture();
        for (name, similarity) in [("a", 1.0), ("b", 0.91), ("c", 0.6), ("d", SIMILAR_MIN)] {
            if let Some(facts) = known.get_mut(Path::new(name)) {
                facts.similarity = Some(similarity);
            }
        }
        let shown = |options: ViewOptions| {
            let view = build(
                &all,
                options,
                |p: &Path| known.get(p).copied(),
                &HashMap::new(),
                &HashMap::new(),
                |_| false,
            );
            names(&view.paths)
        };
        let mut options = ViewOptions {
            similar: true,
            ..ViewOptions::default()
        };
        assert_eq!(shown(options), "abd");
        options.filter.set(FilterKind::Stars(2), true);
        assert_eq!(shown(options), "a", "and the boxes still apply");
        assert_eq!(shown(ViewOptions::default()), "abcde");
        assert!(
            ViewOptions {
                similar: true,
                ..ViewOptions::default()
            }
            .depends_on_scores()
        );
    }

    #[test]
    fn sorts_descending_with_unknown_last() {
        let (all, known) = fixture();
        let lookup = |p: &Path| known.get(p).copied();
        let sorted = |sort| {
            let options = ViewOptions {
                sort,
                ..ViewOptions::default()
            };
            names(
                &build(
                    &all,
                    options,
                    lookup,
                    &HashMap::new(),
                    &HashMap::new(),
                    |_| false,
                )
                .paths,
            )
        };
        assert_eq!(sorted(SortKey::Name), "abcde");
        assert_eq!(sorted(SortKey::Aesthetics), "badce");
        assert_eq!(sorted(SortKey::Personal), "cdabe");
        assert_eq!(sorted(SortKey::Sharpness), "bcdae");
        assert_eq!(sorted(SortKey::Rating), "cdabe");
    }

    #[test]
    fn filters_by_rating_with_session_override() {
        let (all, known) = fixture();
        let lookup = |p: &Path| known.get(p).copied();
        let session = HashMap::from([
            (PathBuf::from("b"), Rating::Stars(4)),
            (PathBuf::from("c"), Rating::Unrated),
            (PathBuf::from("a"), Rating::Rejected),
        ]);
        let filtered = |kinds: &[FilterKind]| {
            let mut filter = PhotoFilter::default();
            for kind in kinds {
                filter.set(*kind, true);
            }
            let options = ViewOptions {
                filter,
                ..ViewOptions::default()
            };
            names(&build(&all, options, lookup, &session, &HashMap::new(), |_| false).paths)
        };
        // Session: a rejected, b 4 stars, c unrated, d stays 3 stars, e not analysed (unrated).
        assert_eq!(
            filtered(&[FilterKind::Stars(3), FilterKind::Stars(4)]),
            "bd"
        );
        assert_eq!(filtered(&[FilterKind::Stars(1), FilterKind::Stars(2)]), "");
        assert_eq!(filtered(&[FilterKind::Unrated]), "ce");
        assert_eq!(filtered(&[FilterKind::Rejected]), "a");
        assert_eq!(filtered(&[]), "abcde");
    }

    /// Within the rating group any ticked box, across groups all of them: blurry 2-star photos,
    /// not every 2-star photo plus every blurry one.
    #[test]
    fn groups_go_together_boxes_in_a_group_either_way() {
        let (all, known) = fixture();
        let lookup = |p: &Path| known.get(p).copied();
        let names_of = |kinds: &[FilterKind]| {
            let mut filter = PhotoFilter::default();
            for kind in kinds {
                filter.set(*kind, true);
            }
            names(
                &build(
                    &all,
                    ViewOptions {
                        filter,
                        ..ViewOptions::default()
                    },
                    lookup,
                    &HashMap::new(),
                    &HashMap::new(),
                    |_| false,
                )
                .paths,
            )
        };
        // "a" (2 stars) is the least sharp; "e" is not measured, so it is not blurry.
        assert_eq!(names_of(&[FilterKind::Blurry]), "a");
        assert_eq!(names_of(&[FilterKind::Stars(2), FilterKind::Blurry]), "a");
        assert_eq!(names_of(&[FilterKind::Stars(5), FilterKind::Blurry]), "");
        assert_eq!(
            names_of(&[FilterKind::Stars(5), FilterKind::Stars(2)]),
            "ac"
        );
    }

    /// People: a face found, or none; a photo the face detection hasn't seen is in neither box.
    #[test]
    fn people_boxes_need_the_face_detection() {
        let filter_of = |kinds: &[FilterKind]| {
            let mut filter = PhotoFilter::default();
            for kind in kinds {
                filter.set(*kind, true);
            }
            filter
        };
        let accepts = |kinds: &[FilterKind], faces: Option<u8>| {
            filter_of(kinds).accepts(Rating::Unrated, false, false, None, faces, false)
        };
        assert!(accepts(&[FilterKind::People], Some(2)));
        assert!(!accepts(&[FilterKind::People], Some(0)));
        assert!(!accepts(&[FilterKind::People], None));
        assert!(accepts(&[FilterKind::NoPeople], Some(0)));
        assert!(!accepts(&[FilterKind::NoPeople], None));
        assert!(accepts(
            &[FilterKind::People, FilterKind::NoPeople],
            Some(0)
        ));
        assert!(
            !accepts(&[FilterKind::People, FilterKind::Stars(5)], Some(1)),
            "the rating group still counts"
        );
        assert!(accepts(&[], None));
        let stored = PhotoFilter::from_stored(&filter_of(&[FilterKind::NoPeople]).id());
        assert!(stored.contains(FilterKind::NoPeople) && !stored.contains(FilterKind::People));
    }

    /// Deleted photos show only through the 🗑 box, which stands for their rating – 🗑 + ✕ are
    /// everything sorted out – and other groups still apply. They don't rank the folder's
    /// sharpness or make a photo still there a duplicate.
    #[test]
    fn deleted_photos_show_only_through_their_box() {
        let (mut all, mut known) = fixture();
        // "x" is a deleted copy of "b" (same pixels) with 5 stars and a red label.
        all.push(PathBuf::from("x"));
        known.get_mut(Path::new("b")).unwrap().fingerprint = Some(7);
        known.insert(
            PathBuf::from("x"),
            Facts {
                rating: Rating::Stars(5),
                label: Some(Label::Red),
                fingerprint: Some(7),
                deleted: true,
                ..facts(Rating::Stars(5), None, Some(1.0), None)
            },
        );
        let lookup = |p: &Path| known.get(p).copied();
        let session = HashMap::from([(PathBuf::from("a"), Rating::Rejected)]);
        let view_of = |kinds: &[FilterKind]| {
            let mut filter = PhotoFilter::default();
            for kind in kinds {
                filter.set(*kind, true);
            }
            let options = ViewOptions {
                filter,
                ..ViewOptions::default()
            };
            build(&all, options, lookup, &session, &HashMap::new(), |_| false)
        };
        assert_eq!(
            names(&view_of(&[]).paths),
            "abcde",
            "hidden without the box"
        );
        assert_eq!(names(&view_of(&[FilterKind::Stars(5)]).paths), "c");
        assert_eq!(names(&view_of(&[FilterKind::Deleted]).paths), "x");
        assert_eq!(
            names(&view_of(&[FilterKind::Rejected, FilterKind::Deleted]).paths),
            "ax"
        );
        assert_eq!(
            names(&view_of(&[FilterKind::Deleted, FilterKind::Colour(Label::Red)]).paths),
            "x"
        );
        assert_eq!(
            names(&view_of(&[FilterKind::Deleted, FilterKind::Colour(Label::Blue)]).paths),
            ""
        );
        assert_eq!(
            names(&view_of(&[FilterKind::Blurry]).paths),
            "a",
            "x, blurrier still, would lift a out of the blurriest fifth"
        );
        let all_shown = view_of(&[FilterKind::Deleted, FilterKind::Unrated]);
        assert_eq!(names(&all_shown.paths), "bex");
        assert!(
            all_shown.duplicate_of.iter().all(Option::is_none),
            "b is no copy of a deleted photo"
        );
        let picked = pick_top(
            &all,
            ViewOptions::default(),
            lookup,
            &HashMap::new(),
            &HashMap::new(),
            |_| false,
            10,
        );
        assert!(!picked.contains(Path::new("x")), "never among the best");
        assert_eq!(
            PhotoFilter::from_stored("*rejected,deleted").id(),
            "*rejected,deleted"
        );
    }

    #[test]
    fn stored_filter_keeps_old_at_least_values() {
        assert!(PhotoFilter::from_stored("").is_all());
        assert!(PhotoFilter::from_stored("all").is_all());
        let legacy = PhotoFilter::from_stored("3");
        assert!(legacy.contains(FilterKind::Stars(3)));
        assert!(legacy.contains(FilterKind::Stars(5)));
        assert!(!legacy.contains(FilterKind::Stars(2)));
        let exact = PhotoFilter::from_stored("*3");
        assert!(exact.contains(FilterKind::Stars(3)));
        assert!(!exact.contains(FilterKind::Stars(4)));
        let mixed = PhotoFilter::from_stored("*1,2,unrated,blurry,duplicate");
        assert_eq!(mixed.id(), "*1,2,unrated,blurry,duplicate");
        assert_eq!(PhotoFilter::from_stored("rejected").id(), "*rejected");
        assert_eq!(PhotoFilter::default().id(), "");
        let red = PhotoFilter::from_stored("*red");
        assert!(red.contains(FilterKind::Colour(Label::Red)));
        assert!(!red.contains(FilterKind::Colour(Label::Blue)));
        assert_eq!(red.id(), "*red");
    }

    #[test]
    fn colour_is_one_more_category() {
        let (all, known) = fixture();
        let lookup = |p: &Path| known.get(p).copied();
        let labels = HashMap::from([
            (PathBuf::from("a"), Some(Label::Red)),
            (PathBuf::from("c"), Some(Label::Green)),
        ]);
        let mut filter = PhotoFilter::default();
        filter.set(FilterKind::Colour(Label::Red), true);
        let options = ViewOptions {
            filter,
            ..ViewOptions::default()
        };
        assert_eq!(
            names(&build(&all, options, lookup, &HashMap::new(), &labels, |_| false,).paths),
            "a"
        );
        // Red and 5 stars: "a" is red but has 2 stars, "c" 5 stars but is green.
        let mut both = options;
        both.filter.set(FilterKind::Stars(5), true);
        assert_eq!(
            names(&build(&all, both, lookup, &HashMap::new(), &labels, |_| false).paths),
            ""
        );
        both.filter.set(FilterKind::Colour(Label::Green), true);
        assert_eq!(
            names(&build(&all, both, lookup, &HashMap::new(), &labels, |_| false).paths),
            "c"
        );
    }

    fn top_of(
        all: &[PathBuf],
        known: &HashMap<PathBuf, Facts>,
        session: &HashMap<PathBuf, Rating>,
        n: usize,
    ) -> String {
        let picked = pick_top(
            all,
            ViewOptions::default(),
            |p: &Path| known.get(p).copied(),
            session,
            &HashMap::new(),
            |_| false,
            n,
        );
        let mut names: Vec<String> = picked
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        names.sort();
        names.concat()
    }

    /// Blurry and rejected photos and those without any value never count; own stars, the
    /// aesthetics and the sharpness rank the rest.
    #[test]
    fn top_takes_the_best_and_leaves_out_what_never_counts() {
        let (all, known) = fixture();
        // a is the blurriest (and clearly soft), e has no values yet.
        assert_eq!(top_of(&all, &known, &HashMap::new(), 10), "bcd");
        // b: aesthetics 75 %, sharpest; c: 5 stars, 42 %, 67 %; d: 3 stars, 58 %, 33 %.
        assert_eq!(top_of(&all, &known, &HashMap::new(), 2), "bc");
        assert_eq!(top_of(&all, &known, &HashMap::new(), 0), "");
        let rejected = HashMap::from([(PathBuf::from("b"), Rating::Rejected)]);
        assert_eq!(top_of(&all, &known, &rejected, 2), "cd");
    }

    /// A photo's own stars stand in for the For-you prediction.
    #[test]
    fn own_stars_replace_the_prediction() {
        let same = |rating, personal| Facts {
            rating,
            personal: Some(personal),
            scores: Scores {
                sharpness: Some(500.0),
                aesthetic: Some(6.0),
                ..Scores::default()
            },
            ..Facts::default()
        };
        let all = vec![PathBuf::from("x"), PathBuf::from("y")];
        let known = HashMap::from([
            (all[0].clone(), same(Rating::Stars(5), 1.0)),
            (all[1].clone(), same(Rating::Unrated, 4.0)),
        ]);
        assert_eq!(top_of(&all, &known, &HashMap::new(), 1), "x");
        let unrated = HashMap::from([(all[0].clone(), Rating::Unrated)]);
        assert_eq!(top_of(&all, &known, &unrated, 1), "y");
    }

    /// Round one takes the best of each series and every single photo; only then the second
    /// best of the series – a burst can't fill the list.
    #[test]
    fn top_takes_one_photo_per_series_first() {
        let photo = |taken: i64, aesthetic: f32, sharpness: f32| Facts {
            taken_ms: Some(taken),
            scores: Scores {
                sharpness: Some(sharpness),
                aesthetic: Some(aesthetic),
                ..Scores::default()
            },
            ..Facts::default()
        };
        let rows = [
            ("s1", photo(0, 8.0, 900.0)),
            ("s2", photo(1_000, 8.0, 800.0)),
            ("s3", photo(1_800, 8.0, 700.0)),
            ("p", photo(60_000, 6.0, 600.0)),
            ("q", photo(120_000, 5.0, 500.0)),
        ];
        let all: Vec<PathBuf> = rows.iter().map(|(name, _)| PathBuf::from(name)).collect();
        let known: HashMap<PathBuf, Facts> = rows
            .into_iter()
            .map(|(name, facts)| (PathBuf::from(name), facts))
            .collect();
        assert_eq!(top_of(&all, &known, &HashMap::new(), 1), "s1");
        assert_eq!(top_of(&all, &known, &HashMap::new(), 2), "ps1");
        assert_eq!(top_of(&all, &known, &HashMap::new(), 3), "pqs1");
        assert_eq!(top_of(&all, &known, &HashMap::new(), 4), "pqs1s2");
    }

    /// Videos and later copies of a photo never count.
    #[test]
    fn top_leaves_out_videos_and_copies() {
        let good = |fingerprint| Facts {
            fingerprint: Some(fingerprint),
            scores: Scores {
                sharpness: Some(900.0),
                aesthetic: Some(8.0),
                ..Scores::default()
            },
            ..Facts::default()
        };
        let all: Vec<PathBuf> = ["v.mp4", "z (1).jpg", "z.jpg", "w.jpg"]
            .map(PathBuf::from)
            .to_vec();
        let known: HashMap<PathBuf, Facts> = HashMap::from([
            (all[0].clone(), good(1)),
            (all[1].clone(), good(2)),
            (all[2].clone(), good(2)),
            (all[3].clone(), good(3)),
        ]);
        assert_eq!(top_of(&all, &known, &HashMap::new(), 10), "w.jpgz.jpg");
    }

    /// With Top N on, the view shows the picked photos – together with the boxes – and the
    /// photos / videos choice waits for later.
    #[test]
    fn the_view_shows_the_picked_photos_with_the_boxes() {
        let (all, mut known) = fixture();
        for name in ["b", "c"] {
            if let Some(facts) = known.get_mut(Path::new(name)) {
                facts.top = true;
            }
        }
        let shown = |options: ViewOptions| {
            names(
                &build(
                    &all,
                    options,
                    |p: &Path| known.get(p).copied(),
                    &HashMap::new(),
                    &HashMap::new(),
                    |_| false,
                )
                .paths,
            )
        };
        let mut options = ViewOptions {
            top: Some(2),
            media: Media::Videos,
            ..ViewOptions::default()
        };
        assert_eq!(shown(options), "bc");
        options.filter.set(FilterKind::Stars(5), true);
        assert_eq!(shown(options), "c");
        assert!(options.is_filtered() && options.depends_on_scores());
        let mut cleared = options;
        cleared.clear_filters();
        assert_eq!(cleared.top, None);
        // Another sort keeps the pick, another filter makes a new one.
        let sorted = ViewOptions {
            sort: SortKey::Taken,
            ..options
        };
        assert_eq!(sorted.top_key(), options.top_key());
        assert_ne!(cleared.top_key(), options.top_key());
        let mut boxes = options;
        boxes.filter.set(FilterKind::Stars(4), true);
        assert_ne!(boxes.top_key(), options.top_key());
        assert_eq!(ViewOptions::default().top_key(), None);
        let mut chosen = ViewOptions::default();
        Scope::Top(50).apply(&mut chosen);
        assert_eq!(Scope::of(&chosen), Scope::Top(50));
        Scope::Media(Media::Photos).apply(&mut chosen);
        assert_eq!((chosen.top, chosen.media), (None, Media::Photos));
    }

    #[test]
    fn a_folder_of_sharp_photos_has_no_blurry_ones() {
        let scores = |s| Scores {
            sharpness: Some(s),
            ..Scores::default()
        };
        let sharp = [900.0, 1500.0, 3000.0, 4000.0, 6000.0].map(scores);
        let p = Percentiles::from_scores(sharp.iter());
        assert!(
            sharp.iter().all(|s| !p.is_blurry(s)),
            "the least sharp photo is still sharp"
        );
        let mixed = [90.0, 1500.0, 3000.0, 4000.0, 6000.0].map(scores);
        let p = Percentiles::from_scores(mixed.iter());
        assert!(p.is_blurry(&mixed[0]));
        assert!(!p.is_blurry(&mixed[1]));
        // The eye region has its own, lower ceiling.
        assert!(is_blurry(0.1, 50.0, true));
        assert!(!is_blurry(0.1, 70.0, true));
        assert!(!is_blurry(0.5, 10.0, false), "not among the blurriest 20 %");
    }

    #[test]
    fn eyes_decide_for_portraits() {
        let scores = |sharpness, eyes| Scores {
            sharpness: Some(sharpness),
            eyes,
            ..Scores::default()
        };
        // A portrait with a soft frame (skin, bokeh) but the sharpest eyes of the series.
        let all = [
            scores(900.0, None),
            scores(100.0, Some(80.0)),
            scores(120.0, Some(20.0)),
        ];
        let p = Percentiles::from_scores(all.iter());
        assert_eq!(p.subject(&all[1]), Some((1.0, true)));
        assert_eq!(p.subject(&all[2]), Some((0.0, true)));
        assert_eq!(p.subject(&all[0]), Some((1.0, false)));
    }

    fn timed(
        name: &str,
        taken: Option<i64>,
        sharp: Option<f32>,
        rating: Rating,
    ) -> (PathBuf, Facts) {
        (
            PathBuf::from(name),
            Facts {
                rating,
                taken_ms: taken,
                scores: Scores {
                    sharpness: sharp,
                    ..Scores::default()
                },
                ..Facts::default()
            },
        )
    }

    #[test]
    fn series_follow_capture_time_and_put_the_sharpest_first() {
        let rows = [
            timed("a", Some(0), Some(10.0), Rating::Unrated),
            timed("b", Some(1_000), Some(90.0), Rating::Unrated),
            timed("c", Some(1_800), Some(40.0), Rating::Rejected),
            timed("d", Some(5_000), Some(5.0), Rating::Unrated),
            timed("e", Some(6_000), Some(70.0), Rating::Unrated),
            timed("f", None, Some(100.0), Rating::Unrated),
        ];
        let all: Vec<_> = rows.iter().map(|(p, _)| p.clone()).collect();
        let known: HashMap<_, _> = rows.into_iter().collect();
        let lookup = |p: &Path| known.get(p).copied();
        let options = ViewOptions {
            sort: SortKey::Taken,
            ..ViewOptions::default()
        };
        let view = build(
            &all,
            options,
            lookup,
            &HashMap::new(),
            &HashMap::new(),
            |_| false,
        );
        // b is the sharpest of the first burst, c is rejected so last; e beats d; f has no time.
        assert_eq!(names(&view.paths), "bacedf");
        assert_eq!(view.series[0].unwrap().index, 1);
        assert_eq!(view.series[0].unwrap().len, 3);
        assert_eq!(view.series[2].unwrap().index, 3);
        assert_eq!(view.series[3].unwrap().len, 2);
        assert!(view.series[5].is_none());
        assert!(view.grouped);
    }

    #[test]
    fn series_never_mix_cameras() {
        let shot = |name: &str, camera: Option<&str>, taken: i64, sharp: f32| {
            let (path, mut facts) = timed(name, Some(taken), Some(sharp), Rating::Unrated);
            facts.camera = camera.map(crate::metadata::camera_id);
            (path, facts)
        };
        // Two phones fire together; a screenshot without EXIF camera sits in between.
        let rows = [
            shot("a", Some("Pixel 7a"), 0, 10.0),
            shot("b", Some("moto g42"), 400, 20.0),
            shot("c", Some("Pixel 7a"), 1_000, 30.0),
            shot("d", Some("moto g42"), 1_300, 40.0),
            shot("e", None, 1_500, 50.0),
            shot("f", None, 2_500, 60.0),
            // Same second as a and c, other camera: whole seconds only.
            shot("g", Some("XQ-ES54"), 0, 70.0),
            shot("h", Some("XQ-ES54"), 1_000, 80.0),
        ];
        let all: Vec<_> = rows.iter().map(|(p, _)| p.clone()).collect();
        let known: HashMap<_, _> = rows.into_iter().collect();
        let view = build(
            &all,
            ViewOptions {
                sort: SortKey::Taken,
                ..ViewOptions::default()
            },
            |p: &Path| known.get(p).copied(),
            &HashMap::new(),
            &HashMap::new(),
            |_| false,
        );
        let series_of = |name: &str| {
            let index = view.iter().position(|p| p == Path::new(name)).unwrap();
            view.series[index].map(|place| (place.id, place.len))
        };
        assert_eq!(series_of("a").map(|s| s.1), Some(2));
        assert_eq!(series_of("a"), series_of("c"));
        assert_eq!(series_of("b"), series_of("d"));
        assert_eq!(
            series_of("e"),
            series_of("f"),
            "no camera: among themselves"
        );
        assert_eq!(series_of("g"), series_of("h"));
        let ids: HashSet<u32> = ["a", "b", "e", "g"]
            .iter()
            .map(|n| series_of(n).unwrap().0)
            .collect();
        assert_eq!(ids.len(), 4, "four series");
        // Each series stays together, even two that start in the same millisecond.
        let order = names(&view.paths);
        for pair in ["ca", "db", "fe", "hg"] {
            assert!(order.contains(pair), "{pair} in {order}");
        }
    }

    /// Windows names copies "IMG - Kopie.jpg"; they sort before "IMG.jpg" but are not the
    /// original. Without a name hint, the only marked copy wins, then folder order.
    #[test]
    fn duplicates_pick_the_original_by_name_then_marks() {
        let refs = |names: &[&str]| -> Vec<PathBuf> { names.iter().map(PathBuf::from).collect() };
        let pick = |paths: &[PathBuf], marked: &[&str]| {
            let list: Vec<&PathBuf> = paths.iter().collect();
            let marked = |p: &Path| marked.iter().any(|m| Path::new(m) == p);
            pick_original(&list, &marked).to_string_lossy().into_owned()
        };
        let copies = refs(&["DSC_0211 Kopie.jpg", "DSC_0211.jpg"]);
        assert_eq!(pick(&copies, &[]), "DSC_0211.jpg");
        assert_eq!(
            pick(&copies, &["DSC_0211 Kopie.jpg"]),
            "DSC_0211.jpg",
            "name first"
        );
        let numbered = refs(&["IMG_1 (1).JPG", "img_1.jpg", "IMG_1 - Copy.JPG"]);
        assert_eq!(pick(&numbered, &[]), "img_1.jpg");
        let folders = refs(&["a/IMG.jpg", "b/IMG.jpg"]);
        assert_eq!(pick(&folders, &[]), "a/IMG.jpg");
        assert_eq!(pick(&folders, &["b/IMG.jpg"]), "b/IMG.jpg");
        assert_eq!(pick(&folders, &["a/IMG.jpg", "b/IMG.jpg"]), "a/IMG.jpg");
    }

    #[test]
    fn duplicates_keep_the_first_path_as_original() {
        let all: Vec<PathBuf> = ["a", "b", "c"].map(PathBuf::from).to_vec();
        let known = HashMap::from([
            (
                all[0].clone(),
                Facts {
                    fingerprint: Some(1),
                    ..Facts::default()
                },
            ),
            (
                all[1].clone(),
                Facts {
                    fingerprint: Some(2),
                    ..Facts::default()
                },
            ),
            (
                all[2].clone(),
                Facts {
                    fingerprint: Some(1),
                    ..Facts::default()
                },
            ),
        ]);
        let lookup = |p: &Path| known.get(p).copied();
        let view = build(
            &all,
            ViewOptions::default(),
            lookup,
            &HashMap::new(),
            &HashMap::new(),
            |_| false,
        );
        assert_eq!(view.duplicate_of[0], None);
        assert_eq!(view.duplicate_of[1], None);
        assert_eq!(view.duplicate_of[2].as_deref(), Some(all[0].as_path()));

        let mut only = PhotoFilter::default();
        only.set(FilterKind::Duplicate, true);
        let view = build(
            &all,
            ViewOptions {
                filter: only,
                ..ViewOptions::default()
            },
            lookup,
            &HashMap::new(),
            &HashMap::new(),
            |_| false,
        );
        assert_eq!(names(&view.paths), "c");
    }

    /// Photos or videos only goes together with the boxes (AND), decided by the file name.
    #[test]
    fn media_filter_goes_with_the_boxes() {
        let all: Vec<PathBuf> = ["a.jpg", "b.mp4", "c.jpg", "d.mov"]
            .map(PathBuf::from)
            .to_vec();
        let rated = |p: &Path| {
            let name = p.to_string_lossy();
            Some(Facts {
                rating: if name.starts_with('a') || name.starts_with('b') {
                    Rating::Stars(3)
                } else {
                    Rating::Unrated
                },
                ..Facts::default()
            })
        };
        let shown = |options: ViewOptions| {
            names(
                &build(
                    &all,
                    options,
                    rated,
                    &HashMap::new(),
                    &HashMap::new(),
                    |_| false,
                )
                .paths,
            )
        };
        let mut options = ViewOptions::default();
        assert_eq!(shown(options), "a.jpgb.mp4c.jpgd.mov");
        options.media = Media::Photos;
        assert_eq!(shown(options), "a.jpgc.jpg");
        options.media = Media::Videos;
        assert_eq!(shown(options), "b.mp4d.mov");
        options.filter.set(FilterKind::Stars(3), true);
        assert_eq!(shown(options), "b.mp4");
        assert!(options.is_filtered());
        options.clear_filters();
        assert_eq!(options, ViewOptions::default());
    }

    /// Photos or videos only is no reason to refresh the order when scores arrive.
    #[test]
    fn media_does_not_depend_on_scores() {
        let options = ViewOptions {
            media: Media::Videos,
            ..ViewOptions::default()
        };
        assert!(!options.depends_on_scores());
        assert!(options.is_filtered());
        for media in Media::ALL {
            assert_eq!(Media::from_id(media.id()), Some(media));
        }
        assert_eq!(Media::from_id("something"), None);
    }

    #[test]
    fn ids_round_trip() {
        for key in SortKey::ALL {
            assert_eq!(SortKey::from_id(key.id()), Some(key));
        }
        // The V2.5 sort of 1.2 and earlier is the combined score now.
        assert_eq!(SortKey::from_id("aesthetics25"), Some(SortKey::Aesthetics));
        assert_eq!(SortKey::from_id("laion"), None);
        let mut filter = PhotoFilter::default();
        for kind in FilterKind::ALL {
            filter.set(kind, true);
        }
        assert_eq!(PhotoFilter::from_stored(&filter.id()), filter);
    }
}
