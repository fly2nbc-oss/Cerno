//! The ExifTool arguments for a mark: only what differs from the file.

use super::*;

pub(super) fn label_needs_write(on_disk: LabelInfo, wanted: Option<Label>) -> bool {
    match (on_disk, wanted) {
        (LabelInfo::Known(have), Some(want)) => have != want,
        (LabelInfo::None, None) => false,
        _ => true,
    }
}

/// Only the parts that changed. ExifTool's MWG tags write IPTC and XMP together (and the EXIF
/// description): `MWG:Keywords` = IPTC Keywords + XMP `dc:subject`, `MWG:Description` = IPTC
/// Caption-Abstract + XMP `dc:description` + EXIF ImageDescription. IPTC is marked and written
/// as UTF-8, or umlauts would come out as Latin-1. Repeating `-MWG:Keywords=` builds the new
/// list; one empty value removes them all.
pub(super) fn description_args(wanted: &Description, on_disk: &Description) -> Vec<String> {
    let mut args = Vec::new();
    if wanted.keywords != on_disk.keywords {
        if wanted.keywords.is_empty() {
            args.push("-MWG:Keywords=".to_owned());
        }
        for keyword in &wanted.keywords {
            // One line each: keywords never hold line breaks.
            let keyword = keyword.replace(['\r', '\n'], " ");
            args.push(format!("-MWG:Keywords={}", keyword.trim()));
        }
    }
    if wanted.comment.trim() != on_disk.comment {
        args.push(format!("-MWG:Description={}", wanted.comment.trim()));
    }
    if !args.is_empty() {
        let options = [
            "-use",
            "MWG",
            "-charset",
            "iptc=UTF8",
            "-IPTC:CodedCharacterSet=UTF8",
        ];
        args.splice(0..0, options.map(str::to_owned));
    }
    args
}

/// Lightroom's keyword paths after a write (CER-67): every path whose last part is a keyword
/// the file had and `wanted` no longer has goes – case aside. A path that has the keyword only
/// as a parent (`Orte|Bodensee|Strand` without `Bodensee`) stays: taking the parent out would
/// move the child to another place, so Lightroom may bring the parent back. New keywords get
/// no path (Cerno knows no hierarchy).
pub(super) fn kept_hierarchy(paths: &[String], had: &[String], wanted: &[String]) -> Vec<String> {
    let fold = |text: &str| text.trim().to_lowercase();
    let wanted: Vec<String> = wanted.iter().map(|k| fold(k)).collect();
    let removed: Vec<String> = had
        .iter()
        .map(|k| fold(k))
        .filter(|k| !wanted.contains(k))
        .collect();
    if removed.is_empty() {
        return paths.to_vec();
    }
    paths
        .iter()
        .filter(|path| {
            let leaf = path.rsplit('|').next().unwrap_or(path);
            !removed.contains(&fold(leaf))
        })
        .cloned()
        .collect()
}

/// The new list of Lightroom's keyword paths, when it differs from the file's. Repeating the
/// tag builds the list; one empty value removes it, like the keywords.
pub(super) fn hierarchy_args(wanted: &[String], on_disk: &[String]) -> Vec<String> {
    if wanted == on_disk {
        return Vec::new();
    }
    if wanted.is_empty() {
        return vec!["-XMP-lr:HierarchicalSubject=".to_owned()];
    }
    wanted
        .iter()
        .map(|path| format!("-XMP-lr:HierarchicalSubject={path}"))
        .collect()
}

pub(super) fn label_arg(label: Option<Label>) -> String {
    match label {
        Some(label) => format!("-XMP-xmp:Label={}", label.xmp_name()),
        None => "-XMP-xmp:Label=".to_owned(),
    }
}

/// An empty value makes ExifTool delete the tag. Windows' own tags know no "rejected"; they are
/// cleared then, so Explorer shows no stars.
pub(super) fn rating_args(rating: Rating, on_disk: &RatingInfo) -> Vec<String> {
    let value = rating.value().map(|v| v.to_string()).unwrap_or_default();
    let stars = rating.stars();
    let windows = stars.map(|s| s.to_string()).unwrap_or_default();
    let percent = stars.map(|s| percent(s).to_string()).unwrap_or_default();
    let mut args = vec![format!("-XMP-xmp:Rating={value}")];
    if on_disk.has_exif_rating {
        args.push(format!("-EXIF:Rating={windows}"));
        args.push(format!("-EXIF:RatingPercent={percent}"));
    }
    if on_disk.has_ms_photo_rating {
        args.push(format!("-XMP-microsoft:RatingPercent={percent}"));
    }
    args
}

/// Windows' mapping of stars to its percentage rating.
pub(super) fn percent(stars: u8) -> u8 {
    match stars {
        1 => 1,
        2 => 25,
        3 => 50,
        4 => 75,
        _ => 99,
    }
}
