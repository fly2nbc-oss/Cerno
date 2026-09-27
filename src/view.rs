//! Which photos are shown and in which order: sorting and filtering of the folder list.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::analysis::sharpness;
use crate::db::Scores;
use crate::i18n;
use crate::metadata::Rating;

/// Sharpness percentile (within the folder) below which a photo counts as probably blurry.
pub const BLURRY_PERCENTILE: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Rating,
    Aesthetics,
    AestheticsV25,
    Personal,
    Sharpness,
}

impl SortKey {
    pub const ALL: [SortKey; 6] = [
        Self::Name,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatingFilter {
    All,
    AtLeast(u8),
    Unrated,
    Rejected,
}

impl RatingFilter {
    pub const ALL: [RatingFilter; 8] = [
        Self::All,
        Self::AtLeast(1),
        Self::AtLeast(2),
        Self::AtLeast(3),
        Self::AtLeast(4),
        Self::AtLeast(5),
        Self::Unrated,
        Self::Rejected,
    ];

    pub fn label(self) -> String {
        let t = i18n::t();
        match self {
            Self::All => t.filter_all.to_owned(),
            Self::AtLeast(5) => t.filter_five.to_owned(),
            Self::AtLeast(n) => (t.filter_at_least)(n),
            Self::Unrated => t.filter_unrated.to_owned(),
            Self::Rejected => t.filter_rejected.to_owned(),
        }
    }

    pub fn id(self) -> String {
        match self {
            Self::All => "all".to_owned(),
            Self::AtLeast(n) => n.to_string(),
            Self::Unrated => "unrated".to_owned(),
            Self::Rejected => "rejected".to_owned(),
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }

    fn accepts(self, rating: Rating) -> bool {
        match self {
            Self::All => true,
            Self::AtLeast(n) => rating.stars().is_some_and(|r| r >= n),
            Self::Unrated => rating == Rating::Unrated,
            Self::Rejected => rating == Rating::Rejected,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewOptions {
    pub sort: SortKey,
    pub filter: RatingFilter,
    pub hide_blurry: bool,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            sort: SortKey::Name,
            filter: RatingFilter::All,
            hide_blurry: false,
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
    pub scores: Scores,
    /// Personal taste model, 0..=5.
    pub personal: Option<f32>,
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

/// Filters and sorts `all` (which is in name order). Session ratings win over those read from
/// the files. Photos without scores yet are kept and sorted last, so nothing disappears just
/// because the analysis hasn't reached it.
pub fn build(
    all: &[PathBuf],
    options: ViewOptions,
    facts: impl Fn(&Path) -> Option<Facts>,
    session_ratings: &HashMap<PathBuf, Rating>,
) -> Vec<PathBuf> {
    let entries: Vec<(&PathBuf, Option<Facts>, Rating)> = all
        .iter()
        .map(|path| {
            let facts = facts(path);
            let rating = match session_ratings.get(path) {
                Some(rating) => *rating,
                None => facts.map(|f| f.rating).unwrap_or_default(),
            };
            (path, facts, rating)
        })
        .collect();
    let percentiles = Percentiles::from_scores(
        entries
            .iter()
            .filter_map(|(_, f, _)| f.as_ref().map(|f| &f.scores)),
    );
    let subject = |facts: &Option<Facts>| {
        facts
            .as_ref()
            .and_then(|f| percentiles.subject(&f.scores))
            .map(|(p, _)| p)
    };

    let mut shown: Vec<_> = entries
        .iter()
        .filter(|(_, facts, rating)| {
            let blurry =
                options.hide_blurry && subject(facts).is_some_and(|p| p < BLURRY_PERCENTILE);
            options.filter.accepts(*rating) && !blurry
        })
        .collect();

    // Stable sort, descending, missing values last; ties keep the name order.
    let key = |(_, facts, rating): &&(&PathBuf, Option<Facts>, Rating)| -> Option<f32> {
        match options.sort {
            SortKey::Name => None,
            // Stars, then unrated, rejected last.
            SortKey::Rating => Some(match rating {
                Rating::Stars(n) => f32::from(*n),
                Rating::Unrated => 0.0,
                Rating::Rejected => -1.0,
            }),
            SortKey::Aesthetics => facts.and_then(|f| f.scores.aesthetic),
            SortKey::AestheticsV25 => facts.and_then(|f| f.scores.aesthetic25),
            SortKey::Personal => facts.and_then(|f| f.personal),
            SortKey::Sharpness => subject(facts),
        }
    };
    if options.sort != SortKey::Name {
        shown.sort_by(|a, b| match (key(a), key(b)) {
            (Some(x), Some(y)) => y.total_cmp(&x),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
    }
    shown
        .into_iter()
        .map(|(path, _, _)| (*path).clone())
        .collect()
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
            names(&build(&all, options, lookup, &HashMap::new()))
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
        let filtered = |filter| {
            let options = ViewOptions {
                filter,
                ..ViewOptions::default()
            };
            names(&build(&all, options, lookup, &session))
        };
        assert_eq!(filtered(RatingFilter::AtLeast(3)), "bd");
        assert_eq!(filtered(RatingFilter::AtLeast(1)), "bd");
        assert_eq!(filtered(RatingFilter::Unrated), "ce");
        assert_eq!(filtered(RatingFilter::Rejected), "a");
        assert_eq!(filtered(RatingFilter::All), "abcde");
    }

    #[test]
    fn hides_the_blurriest_but_keeps_unanalysed() {
        let (all, known) = fixture();
        let lookup = |p: &Path| known.get(p).copied();
        let options = ViewOptions {
            hide_blurry: true,
            ..ViewOptions::default()
        };
        assert_eq!(
            names(&build(&all, options, lookup, &HashMap::new())),
            "bcde"
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

    #[test]
    fn ids_round_trip() {
        for key in SortKey::ALL {
            assert_eq!(SortKey::from_id(key.id()), Some(key));
        }
        for filter in RatingFilter::ALL {
            assert_eq!(RatingFilter::from_id(&filter.id()), Some(filter));
        }
    }
}
