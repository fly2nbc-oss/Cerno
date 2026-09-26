//! Which photos are shown and in which order: sorting and filtering of the folder list.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::analysis::{Known, sharpness};

/// Sharpness percentile (within the folder) below which a photo counts as probably blurry.
pub const BLURRY_PERCENTILE: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Rating,
    Aesthetics,
    Sharpness,
}

impl SortKey {
    pub const ALL: [SortKey; 4] = [Self::Name, Self::Rating, Self::Aesthetics, Self::Sharpness];

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Rating => "Rating",
            Self::Aesthetics => "Aesthetics",
            Self::Sharpness => "Sharpness",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Rating => "rating",
            Self::Aesthetics => "aesthetics",
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
}

impl RatingFilter {
    pub const ALL: [RatingFilter; 7] = [
        Self::All,
        Self::AtLeast(1),
        Self::AtLeast(2),
        Self::AtLeast(3),
        Self::AtLeast(4),
        Self::AtLeast(5),
        Self::Unrated,
    ];

    pub fn label(self) -> String {
        match self {
            Self::All => "All".to_owned(),
            Self::AtLeast(5) => "5 stars".to_owned(),
            Self::AtLeast(n) => format!("{n}+ stars"),
            Self::Unrated => "Unrated".to_owned(),
        }
    }

    pub fn id(self) -> String {
        match self {
            Self::All => "all".to_owned(),
            Self::AtLeast(n) => n.to_string(),
            Self::Unrated => "unrated".to_owned(),
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }

    fn accepts(self, rating: Option<u8>) -> bool {
        match self {
            Self::All => true,
            Self::AtLeast(n) => rating.is_some_and(|r| r >= n),
            Self::Unrated => rating.is_none(),
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

/// Filters and sorts `all` (which is in name order). Session ratings win over those read from
/// the files. Photos without scores yet are kept and sorted last, so nothing disappears just
/// because the analysis hasn't reached it.
pub fn build(
    all: &[PathBuf],
    options: ViewOptions,
    known: impl Fn(&Path) -> Option<Known>,
    session_ratings: &HashMap<PathBuf, Option<u8>>,
) -> Vec<PathBuf> {
    let entries: Vec<(&PathBuf, Option<Known>, Option<u8>)> = all
        .iter()
        .map(|path| {
            let known = known(path);
            let rating = match session_ratings.get(path) {
                Some(stars) => *stars,
                None => known.and_then(|k| k.rating),
            };
            (path, known, rating)
        })
        .collect();
    let mut sharpness_sorted: Vec<f32> = entries
        .iter()
        .filter_map(|(_, k, _)| k.and_then(|k| k.scores.sharpness))
        .collect();
    sharpness_sorted.sort_by(f32::total_cmp);

    let mut shown: Vec<_> = entries
        .into_iter()
        .filter(|(_, known, rating)| {
            let blurry = options.hide_blurry
                && known.and_then(|k| k.scores.sharpness).is_some_and(|s| {
                    sharpness::percentile(s, &sharpness_sorted) < BLURRY_PERCENTILE
                });
            options.filter.accepts(*rating) && !blurry
        })
        .collect();

    // Stable sort, descending, missing values last; ties keep the name order.
    let key = |(_, known, rating): &(&PathBuf, Option<Known>, Option<u8>)| -> Option<f32> {
        match options.sort {
            SortKey::Name => None,
            SortKey::Rating => rating.map(f32::from),
            SortKey::Aesthetics => known.and_then(|k| k.scores.aesthetic),
            SortKey::Sharpness => known.and_then(|k| k.scores.sharpness),
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
    shown.into_iter().map(|(path, _, _)| path.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Scores;

    fn known(rating: Option<u8>, aesthetic: Option<f32>, sharpness: Option<f32>) -> Known {
        Known {
            rating,
            scores: Scores {
                sharpness,
                aesthetic,
            },
        }
    }

    fn fixture() -> (Vec<PathBuf>, HashMap<PathBuf, Known>) {
        let all: Vec<PathBuf> = ["a", "b", "c", "d", "e"].map(PathBuf::from).to_vec();
        let known = HashMap::from([
            (all[0].clone(), known(Some(2), Some(4.0), Some(10.0))),
            (all[1].clone(), known(None, Some(6.5), Some(500.0))),
            (all[2].clone(), known(Some(5), Some(5.0), Some(300.0))),
            (all[3].clone(), known(Some(3), None, Some(200.0))),
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
        assert_eq!(sorted(SortKey::Sharpness), "bcdae");
        assert_eq!(sorted(SortKey::Rating), "cdabe");
    }

    #[test]
    fn filters_by_rating_with_session_override() {
        let (all, known) = fixture();
        let lookup = |p: &Path| known.get(p).copied();
        let session = HashMap::from([(PathBuf::from("b"), Some(4)), (PathBuf::from("c"), None)]);
        let filtered = |filter| {
            let options = ViewOptions {
                filter,
                ..ViewOptions::default()
            };
            names(&build(&all, options, lookup, &session))
        };
        assert_eq!(filtered(RatingFilter::AtLeast(3)), "bd");
        assert_eq!(filtered(RatingFilter::Unrated), "ce");
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
    fn ids_round_trip() {
        for key in SortKey::ALL {
            assert_eq!(SortKey::from_id(key.id()), Some(key));
        }
        for filter in RatingFilter::ALL {
            assert_eq!(RatingFilter::from_id(&filter.id()), Some(filter));
        }
    }
}
