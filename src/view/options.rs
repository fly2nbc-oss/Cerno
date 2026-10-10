//! What the view shows and in which order: sort, media, Top N, similar photos.

use super::*;

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

    /// A stored sort.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }
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
    /// "without ✕": the rejected photos of the folder are hidden – the other way round from
    /// the ✕ box, which shows only them; the two exclude each other (`toggle_filter`). Saved.
    pub hide_rejected: bool,
    /// Only the photos of a pasted file-name list (`Facts::listed`, `name_list`). Never saved:
    /// the list is about this folder.
    pub name_list: bool,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            sort: SortKey::Name,
            filter: PhotoFilter::default(),
            similar: false,
            media: Media::All,
            top: None,
            hide_rejected: false,
            name_list: false,
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

    /// Whether anything hides photos: a box, "without ✕", "similar photos", photos / videos
    /// only or Top N.
    pub fn is_filtered(&self) -> bool {
        !self.filter.is_all()
            || self.hide_rejected
            || self.name_list
            || self.similar
            || self.media != Media::All
            || self.top.is_some()
    }

    /// "Show all": every filter off, the sort stays.
    pub fn clear_filters(&mut self) {
        self.filter.clear();
        self.hide_rejected = false;
        self.name_list = false;
        self.similar = false;
        self.media = Media::All;
        self.top = None;
    }

    /// A box ticked or unticked. Ticking ✕ ends "without ✕" – both at once would hide every
    /// photo.
    pub fn toggle_filter(&mut self, kind: FilterKind) {
        self.filter.toggle(kind);
        if kind == FilterKind::Rejected && self.filter.contains(kind) {
            self.hide_rejected = false;
        }
    }

    /// "without ✕" on or off; on unticks ✕.
    pub fn toggle_hide_rejected(&mut self) {
        self.hide_rejected = !self.hide_rejected;
        if self.hide_rejected {
            self.filter.set(FilterKind::Rejected, false);
        }
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
