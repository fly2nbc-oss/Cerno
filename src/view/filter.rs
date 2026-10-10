//! The filter boxes: kinds, groups (AND) and boxes within a group (OR).

use super::*;

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
    /// A JPEG that ends inside its image data (`jpeg_info::is_complete`).
    Incomplete,
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
    pub const ALL: [FilterKind; 18] = [
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
        Self::Incomplete,
    ];

    pub fn label(self) -> String {
        let t = i18n::t();
        match self {
            Self::Stars(n) => (t.filter_stars)(n),
            Self::Unrated => t.filter_unrated.to_owned(),
            Self::Rejected => t.filter_rejected.to_owned(),
            Self::Blurry => t.filter_blurry.to_owned(),
            Self::Duplicate => t.filter_duplicate.to_owned(),
            Self::Incomplete => t.filter_incomplete.to_owned(),
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
            Self::Incomplete => "incomplete",
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
        &[Self::Blurry, Self::Duplicate, Self::Incomplete],
        &[Self::People, Self::NoPeople],
    ];
}

/// What the group of blurry, duplicates and incomplete knows about a photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Quality {
    /// Probably out of focus (`Percentiles::is_blurry`).
    pub blurry: bool,
    /// A later copy of an earlier photo.
    pub duplicate: bool,
    /// A JPEG that ends inside its image data.
    pub incomplete: bool,
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
    incomplete: bool,
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
            && !self.incomplete
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
            FilterKind::Incomplete => self.incomplete,
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
            FilterKind::Incomplete => self.incomplete = on,
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
    /// ticked, it is blurry, a copy or incomplete when one of those boxes is ticked, and it has
    /// faces or none (`faces`: `None` until the face detection ran – then neither box takes
    /// it). A deleted photo shows only through the 🗑 box, which stands for its rating: 🗑 + ✕
    /// are the deleted and the rejected photos, 🗑 + red the deleted ones with a red label.
    pub fn accepts(
        self,
        rating: Rating,
        quality: Quality,
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
        let by_quality = !(self.blurry || self.duplicate || self.incomplete)
            || (self.blurry && quality.blurry)
            || (self.duplicate && quality.duplicate)
            || (self.incomplete && quality.incomplete);
        let by_people = !(self.people || self.no_people)
            || (self.people && faces.is_some_and(|n| n > 0))
            || (self.no_people && faces == Some(0));
        by_rating && by_colour && by_quality && by_people
    }

    /// Stored setting: `*` and the ticked boxes' tokens. Empty means every photo; so does
    /// anything without the `*` (the settings of releases before 0.8).
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
        if let Some(rest) = id.strip_prefix('*') {
            for token in rest.split(',').filter(|token| !token.is_empty()) {
                if let Some(kind) = FilterKind::from_token(token) {
                    filter.set(kind, true);
                }
            }
        }
        filter
    }
}

pub(super) fn colour_index(label: Label) -> usize {
    Label::ALL
        .iter()
        .position(|item| *item == label)
        .unwrap_or(0)
}
