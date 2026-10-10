//! Rating, orientation and camera data, read from the file bytes that were loaded for
//! decoding anyway.

use std::borrow::Cow;
use std::io::Cursor;

use exif::{Exif, In, Tag, Value};
use memchr::memmem;

mod camera;
mod iptc;
mod time;
mod xmp;

use camera::*;
use iptc::*;
use time::*;
use xmp::*;

pub use camera::{maps_url, osm_url};
#[cfg(test)]
pub use iptc::iptc_description;
pub use time::format_millis;

/// Microsoft's EXIF rating tag (IFD0 0x4746), written by Windows Explorer.
const EXIF_RATING: exif::Tag = exif::Tag(exif::Context::Tiff, 0x4746);

/// Bumped when capture time, camera or colour-label reading changes, so the analysis re-reads
/// metadata without decoding the image again. 2: the camera model (series per camera).
pub const VERSION: i64 = 2;

/// Stable id of a camera model for series (FNV-1a, not `DefaultHasher`): case and outer
/// spaces don't count.
pub fn camera_id(model: &str) -> u64 {
    model
        .trim()
        .to_lowercase()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
}

/// A photo's rating field (`xmp:Rating`): the XMP standard defines -1 as "rejected" and 0 (or
/// no value) as "unrated".
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rating {
    #[default]
    Unrated,
    /// Out of the running (`X`), but kept – nothing is deleted.
    Rejected,
    /// 1..=5
    Stars(u8),
}

impl Rating {
    /// From the tag value: -1 rejected, 1..=5 stars, anything else unrated.
    pub fn from_value(value: i64) -> Self {
        match value {
            -1 => Self::Rejected,
            1..=5 => Self::Stars(value as u8),
            _ => Self::Unrated,
        }
    }

    /// What goes into `xmp:Rating` and the index; `None` removes it.
    pub fn value(self) -> Option<i64> {
        match self {
            Self::Unrated => None,
            Self::Rejected => Some(-1),
            Self::Stars(n) => Some(i64::from(n)),
        }
    }

    pub fn stars(self) -> Option<u8> {
        match self {
            Self::Stars(n) => Some(n),
            _ => None,
        }
    }
}

/// Colour label stored in `xmp:Label`. The file holds the English name Lightroom and Bridge
/// use; reading also accepts the names a localised Lightroom writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Label {
    Red,
    Yellow,
    Green,
    Blue,
    Purple,
}

impl Label {
    pub const ALL: [Label; 5] = [
        Self::Red,
        Self::Yellow,
        Self::Green,
        Self::Blue,
        Self::Purple,
    ];

    /// The string written to `xmp:Label`.
    pub fn xmp_name(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Purple => "purple",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.id() == id)
    }

    /// `red` (the settings id) or `Red` (the XMP name).
    pub fn from_stored(value: &str) -> Option<Self> {
        Self::from_id(value).or_else(|| {
            Self::ALL
                .into_iter()
                .find(|label| label.xmp_name().eq_ignore_ascii_case(value))
        })
    }
}

/// What `xmp:Label` held. `Other` is a text Cerno does not use as a colour; it is left alone
/// until the user sets a colour of their own.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LabelInfo {
    #[default]
    None,
    Known(Label),
    Other,
}

impl LabelInfo {
    pub fn known(self) -> Option<Label> {
        match self {
            Self::Known(label) => Some(label),
            _ => None,
        }
    }

    pub fn parse(text: &str) -> Self {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Self::None;
        }
        let folded = trimmed.to_lowercase();
        LABEL_NAMES
            .iter()
            .find(|(name, _)| *name == folded)
            .map(|(_, label)| Self::Known(*label))
            .unwrap_or(Self::Other)
    }
}

/// English names plus the colour names of a German, French, Spanish or Italian Lightroom.
const LABEL_NAMES: &[(&str, Label)] = &[
    ("red", Label::Red),
    ("rot", Label::Red),
    ("rouge", Label::Red),
    ("rojo", Label::Red),
    ("rosso", Label::Red),
    ("yellow", Label::Yellow),
    ("gelb", Label::Yellow),
    ("jaune", Label::Yellow),
    ("amarillo", Label::Yellow),
    ("giallo", Label::Yellow),
    ("green", Label::Green),
    ("grün", Label::Green),
    ("gruen", Label::Green),
    ("vert", Label::Green),
    ("verde", Label::Green),
    ("blue", Label::Blue),
    ("blau", Label::Blue),
    ("bleu", Label::Blue),
    ("azul", Label::Blue),
    ("blu", Label::Blue),
    ("purple", Label::Purple),
    ("lila", Label::Purple),
    ("violet", Label::Purple),
    ("violett", Label::Purple),
    ("morado", Label::Purple),
    ("púrpura", Label::Purple),
    ("purpura", Label::Purple),
    ("viola", Label::Purple),
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RatingInfo {
    pub value: Rating,
    /// Windows Explorer keeps extra copies of the rating. If they exist they are updated too,
    /// so Explorer never shows a stale value.
    pub has_exif_rating: bool,
    pub has_ms_photo_rating: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileMetadata {
    pub rating: RatingInfo,
    pub label: LabelInfo,
    /// EXIF orientation 1..=8 (1 = as stored).
    pub orientation: u16,
    pub camera: CameraInfo,
    pub description: Description,
}

/// A photo's comment and keywords as other programs show them: XMP `dc:description` /
/// `dc:subject`, else IPTC Caption-Abstract / Keywords. Written back through ExifTool's MWG
/// tags, which keep IPTC, XMP and the EXIF description in step.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Description {
    /// Empty: none.
    pub comment: String,
    pub keywords: Vec<String>,
}

impl Description {
    /// Adds a keyword unless it is empty or already there (case-insensitive). Returns whether
    /// it was added.
    pub fn add_keyword(&mut self, keyword: &str) -> bool {
        let keyword = keyword.trim();
        if keyword.is_empty()
            || self
                .keywords
                .iter()
                .any(|k| k.to_lowercase() == keyword.to_lowercase())
        {
            return false;
        }
        self.keywords.push(keyword.to_owned());
        true
    }
}

/// Capture settings for the info bar.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct CameraInfo {
    pub camera: Option<String>,
    /// Tells cameras apart for series: the EXIF model, else the make. The model alone, because
    /// some files of the same camera lack the make ("FC7503" next to "DJI FC7503"). Two bodies
    /// of the same model look alike – phones write no serial number.
    pub model: Option<String>,
    pub lens: Option<String>,
    pub focal_mm: Option<f64>,
    pub focal_35mm: Option<u32>,
    pub f_number: Option<f64>,
    pub exposure_s: Option<f64>,
    pub iso: Option<u32>,
    /// `YYYY-MM-DD HH:MM`
    pub taken: Option<String>,
    /// `DateTimeOriginal` (else `DateTimeDigitized`) as local wall-clock milliseconds, with
    /// sub-seconds when the file has them. Compared only with other photos, never with UTC.
    pub taken_ms: Option<i64>,
    /// Latitude and longitude in degrees (south and west negative).
    pub gps: Option<(f64, f64)>,
    /// EXIF `DigitalZoomRatio`, only when a digital zoom was used (> 1).
    pub digital_zoom: Option<f64>,
}

impl CameraInfo {
    /// `35 mm · f/2.8 · 1/250 s · ISO 400`
    pub fn exposure_line(&self) -> String {
        let mut parts = Vec::new();
        if let Some(focal) = self.focal_mm {
            let mut text = format!("{} mm", trim_decimal(focal, 0));
            if let Some(eq) = self
                .focal_35mm
                .filter(|eq| (f64::from(*eq) - focal).abs() >= 1.0)
            {
                text.push_str(&format!(" ({eq} mm eq.)"));
            }
            parts.push(text);
        }
        if let Some(f) = self.f_number {
            parts.push(format!("f/{}", trim_decimal(f, 1)));
        }
        if let Some(t) = self.exposure_s {
            parts.push(format_exposure(t));
        }
        if let Some(iso) = self.iso {
            parts.push(format!("ISO {iso}"));
        }
        parts.join("  ·  ")
    }

    /// `Sony ILCE-7M4  ·  FE 24-70mm F2.8 GM`
    pub fn gear_line(&self) -> String {
        [self.camera.as_deref(), self.lens.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("  ·  ")
    }
}

/// The metadata of the file at `path`, from its bytes. A RAW whose container the EXIF reader
/// does not know (CR3, RAF, ORF, RW2 …) gives its camera data and orientation through its
/// JPEG preview. Where the marks live in an XMP sidecar, rating and label come from there as
/// soon as one exists – until then the file's own (a rating given in the camera) stay.
pub fn read_for(path: &std::path::Path, bytes: &[u8]) -> FileMetadata {
    let format = crate::library::format_of(path);
    let mut meta = read(bytes);
    if format.is_some_and(crate::library::Format::is_raw)
        && meta.camera.model.is_none()
        && let Some(preview) = crate::raw::preview(bytes)
    {
        let inner = read(preview);
        meta.camera = inner.camera;
        meta.orientation = inner.orientation;
    }
    if crate::sidecar::applies(path) {
        with_sidecar(&mut meta, path);
    }
    meta
}

/// Only what the photo's sidecar says (defaults without one): for a video, which is never read
/// whole, and for the writer once the sidecar exists.
pub fn read_sidecar(path: &std::path::Path) -> FileMetadata {
    let mut meta = read(&[]);
    with_sidecar(&mut meta, path);
    meta
}

/// The sidecar's rating, label, comment and keywords replace the file's. It holds no Windows
/// rating tags.
fn with_sidecar(meta: &mut FileMetadata, path: &std::path::Path) {
    if let Some(bytes) = crate::sidecar::read(path) {
        let side = read(&bytes);
        meta.rating = RatingInfo {
            value: side.rating.value,
            has_exif_rating: false,
            has_ms_photo_rating: false,
        };
        meta.label = side.label;
        meta.description = side.description;
    }
}

pub fn read(bytes: &[u8]) -> FileMetadata {
    let exif = exif::Reader::new()
        .read_from_container(&mut Cursor::new(bytes))
        .ok();
    let exif_uint = |tag| {
        exif.as_ref()
            .and_then(|e| e.get_field(tag, In::PRIMARY))
            .and_then(|f| f.value.get_uint(0))
    };

    let orientation = exif_uint(Tag::Orientation)
        .filter(|o| (1..=8).contains(o))
        .map_or(1, |o| o as u16);
    let exif_rating = exif_uint(EXIF_RATING);
    let camera = exif.as_ref().map(camera_info).unwrap_or_default();

    let xmp = find_xmp_packet(bytes);
    let xmp_rating = xmp.as_deref().and_then(parse_xmp_rating);
    let value = xmp_rating
        .map(i64::from)
        .or(exif_rating.map(i64::from))
        .map_or(Rating::Unrated, Rating::from_value);
    let label = xmp
        .as_deref()
        .and_then(parse_xmp_label)
        .as_deref()
        .map_or(LabelInfo::None, LabelInfo::parse);
    let description = read_description(bytes, xmp.as_deref());

    FileMetadata {
        rating: RatingInfo {
            value,
            has_exif_rating: exif_rating.is_some(),
            has_ms_photo_rating: xmp.is_some_and(|x| x.contains("MicrosoftPhoto:Rating")),
        },
        label,
        orientation,
        camera,
        description,
    }
}

/// XMP first (what current programs write), IPTC IIM for older files. Each part on its own:
/// a file may have XMP keywords but only an IPTC caption.
fn read_description(bytes: &[u8], xmp: Option<&str>) -> Description {
    let iim = iptc_iim(bytes);
    let comment = xmp
        .and_then(|x| xmp_alt_default(x, "dc:description"))
        .or_else(|| iim.as_ref().and_then(|i| i.caption.clone()))
        .unwrap_or_default();
    let keywords = xmp
        .and_then(|x| xmp_bag(x, "dc:subject"))
        .or_else(|| iim.map(|i| i.keywords).filter(|k| !k.is_empty()))
        .unwrap_or_default();
    Description {
        comment: comment.trim().to_owned(),
        keywords,
    }
}

#[cfg(test)]
mod tests;
