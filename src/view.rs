//! Which photos are shown and in which order: sorting and filtering of the folder list.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use std::sync::Arc;

use crate::analysis::sharpness;
use crate::db::Scores;
use crate::i18n;
use crate::metadata::{Label, Rating};

/// Photos taken at most this far apart belong to one series.
pub const SERIES_GAP_MS: i64 = 2_000;

/// Sharpness percentile (within the folder) below which a photo counts as probably blurry.
pub const BLURRY_PERCENTILE: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    /// Capture time, oldest first. Photos without a time stay at the end.
    Taken,
    Rating,
    Aesthetics,
    AestheticsV25,
    Personal,
    Sharpness,
}

impl SortKey {
    pub const ALL: [SortKey; 7] = [
        Self::Name,
        Self::Taken,
        Self::Rating,
        Self::Aesthetics,
        Self::AestheticsV25,
        Self::Personal,
        Self::Sharpness,
    ];

    pub fn label(self) -> &'static str {
        let t = i18n::t();
        match self {
            Self::Name => t.sort_name,
            Self::Taken => t.sort_taken,
            Self::Rating => t.sort_rating,
            Self::Aesthetics => t.sort_laion,
            Self::AestheticsV25 => t.sort_v25,
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
            Self::AestheticsV25 => "aesthetics25",
            Self::Personal => "personal",
            Self::Sharpness => "sharpness",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
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
}

impl FilterKind {
    pub const ALL: [FilterKind; 14] = [
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
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.token() == token)
    }
}

/// Which photos stay visible. Nothing ticked means every photo. Otherwise a photo stays when
/// it matches any ticked category.
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
}

impl PhotoFilter {
    pub fn is_all(self) -> bool {
        !self.stars.iter().any(|on| *on)
            && !self.unrated
            && !self.rejected
            && !self.blurry
            && !self.duplicate
            && !self.colours.iter().any(|on| *on)
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

    /// A photo matches when its rating or colour is ticked, or when it is blurry or a copy and
    /// that box is ticked.
    pub fn accepts(
        self,
        rating: Rating,
        is_blurry: bool,
        is_duplicate: bool,
        colour: Option<Label>,
    ) -> bool {
        if self.is_all() {
            return true;
        }
        let by_rating = match rating {
            Rating::Stars(n) if (1..=5).contains(&n) => self.stars[n as usize - 1],
            Rating::Unrated => self.unrated,
            Rating::Rejected => self.rejected,
            Rating::Stars(_) => false,
        };
        let by_colour = colour.is_some_and(|label| self.colours[colour_index(label)]);
        by_rating || by_colour || (self.blurry && is_blurry) || (self.duplicate && is_duplicate)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewOptions {
    pub sort: SortKey,
    pub filter: PhotoFilter,
    /// One photo per series: the sharpest that is not rejected.
    pub best_of_series: bool,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            sort: SortKey::Name,
            filter: PhotoFilter::default(),
            best_of_series: false,
        }
    }
}

impl ViewOptions {
    /// Whether new analysis results can change the view (so a refresh makes sense).
    pub fn depends_on_scores(&self) -> bool {
        *self != Self::default()
    }
}

/// What sorting and filtering look at for one photo.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Facts {
    pub rating: Rating,
    pub label: Option<Label>,
    /// Capture time in local wall-clock milliseconds.
    pub taken_ms: Option<i64>,
    /// Pixel fingerprint; `None` until the photo has been indexed.
    pub fingerprint: Option<u64>,
    pub scores: Scores,
    /// Personal taste model, 0..=5.
    pub personal: Option<f32>,
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
}

struct Entry<'a> {
    path: &'a PathBuf,
    facts: Option<Facts>,
    rating: Rating,
    label: Option<Label>,
    taken: Option<i64>,
    /// Subject sharpness percentile, when the photo has been measured.
    sharp: Option<f32>,
}

/// Filters and sorts `all` (which is in name order). Session ratings and labels win over those
/// read from the files. Photos without scores yet are kept and sorted last, so nothing
/// disappears just because the analysis hasn't reached it. `hidden` drops photos that are
/// waiting to be deleted.
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
                rating,
                label,
                facts: known,
                sharp: None,
            }
        })
        .collect();
    let percentiles = Percentiles::from_scores(
        entries
            .iter()
            .filter_map(|e| e.facts.as_ref().map(|f| &f.scores)),
    );
    let fingerprints: HashMap<&Path, u64> = entries
        .iter()
        .filter_map(|entry| {
            entry
                .facts
                .and_then(|facts| facts.fingerprint)
                .map(|fp| (entry.path.as_path(), fp))
        })
        .collect();
    let copies = duplicate_originals(all, |path| fingerprints.get(path).copied());

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
            let blurry = entry.sharp.is_some_and(|p| p < BLURRY_PERCENTILE);
            let is_duplicate = copies.contains_key(entry.path.as_path());
            options
                .filter
                .accepts(entry.rating, blurry, is_duplicate, entry.label)
        })
        .collect();

    let places = series_places(&shown);
    let mut order: Vec<usize> = (0..shown.len()).collect();
    if options.best_of_series {
        order.retain(|&i| places[i].is_none_or(|place| place.index == 1));
    }
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
                    SortKey::Aesthetics => entry.facts.and_then(|f| f.scores.aesthetic),
                    SortKey::AestheticsV25 => entry.facts.and_then(|f| f.scores.aesthetic25),
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

/// First path in folder order is the original; every later photo with the same fingerprint
/// points at it.
fn duplicate_originals(
    all: &[PathBuf],
    fingerprint: impl Fn(&Path) -> Option<u64>,
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
        let original = paths[0];
        for dup in paths.into_iter().skip(1) {
            copies.insert(dup.clone(), original.clone());
        }
    }
    copies
}

/// Series of photos that follow each other by at most [`SERIES_GAP_MS`]. A single photo is
/// not a series. Within a series the sharpest non-rejected photo is index 1; rejected and
/// not-yet-measured photos come last.
fn series_places(shown: &[Entry<'_>]) -> Vec<Option<SeriesPlace>> {
    let mut timed: Vec<usize> = shown
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.taken.is_some())
        .map(|(index, _)| index)
        .collect();
    timed.sort_by(|&a, &b| shown[a].taken.cmp(&shown[b].taken).then(a.cmp(&b)));

    let mut groups: Vec<Vec<usize>> = Vec::new();
    for index in timed {
        let taken = shown[index].taken.unwrap_or(0);
        if let Some(group) = groups.last_mut()
            && let Some(&prev) = group.last()
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
    let key = |index: usize| -> Option<(i64, u32)> {
        let taken = shown[index].taken?;
        let (start, rank) = places[index]
            .map(|place| (place.start_ms, place.index))
            .unwrap_or((taken, 0));
        Some((start, rank))
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
                aesthetic25: aesthetic.map(|a| 10.0 - a),
                ..Scores::default()
            },
            personal,
            ..Facts::default()
        }
    }

    fn fixture() -> (Vec<PathBuf>, HashMap<PathBuf, Facts>) {
        let all: Vec<PathBuf> = ["a", "b", "c", "d", "e"].map(PathBuf::from).to_vec();
        let known = HashMap::from([
            (
                all[0].clone(),
                facts(Rating::Stars(2), Some(4.0), Some(10.0), Some(1.0)),
            ),
            (
                all[1].clone(),
                facts(Rating::Unrated, Some(6.5), Some(500.0), None),
            ),
            (
                all[2].clone(),
                facts(Rating::Stars(5), Some(5.0), Some(300.0), Some(4.5)),
            ),
            (
                all[3].clone(),
                facts(Rating::Stars(3), None, Some(200.0), Some(3.0)),
            ),
            // "e" not analysed yet
        ]);
        (all, known)
    }

    fn names(view: &[PathBuf]) -> String {
        view.iter().map(|p| p.to_string_lossy()).collect()
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
        assert_eq!(sorted(SortKey::Aesthetics), "bcade");
        assert_eq!(sorted(SortKey::AestheticsV25), "acbde");
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

    #[test]
    fn blurry_is_one_more_category() {
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
        // "a" is the least sharp; "e" is not measured, so it is not blurry.
        assert_eq!(names_of(&[FilterKind::Blurry]), "a");
        assert_eq!(names_of(&[FilterKind::Stars(5), FilterKind::Blurry]), "ac");
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

        let best = ViewOptions {
            sort: SortKey::Taken,
            best_of_series: true,
            ..ViewOptions::default()
        };
        let view = build(&all, best, lookup, &HashMap::new(), &HashMap::new(), |_| {
            false
        });
        assert_eq!(names(&view.paths), "bef");
        assert_eq!(view.series[0].unwrap().len, 3);
        assert_eq!(view.series[1].unwrap().index, 1);
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

    #[test]
    fn ids_round_trip() {
        for key in SortKey::ALL {
            assert_eq!(SortKey::from_id(key.id()), Some(key));
        }
        let mut filter = PhotoFilter::default();
        for kind in FilterKind::ALL {
            filter.set(kind, true);
        }
        assert_eq!(PhotoFilter::from_stored(&filter.id()), filter);
    }
}
