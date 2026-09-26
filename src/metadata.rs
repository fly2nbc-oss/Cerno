//! Rating and orientation, read from the file bytes that were loaded for decoding anyway.

use std::borrow::Cow;
use std::io::Cursor;

use memchr::memmem;

/// Microsoft's EXIF rating tag (IFD0 0x4746), written by Windows Explorer.
const EXIF_RATING: exif::Tag = exif::Tag(exif::Context::Tiff, 0x4746);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RatingInfo {
    /// 1..=5; `None` for unrated, `0` and `-1` ("rejected").
    pub stars: Option<u8>,
    /// Windows Explorer keeps extra copies of the rating. If they exist they are updated too,
    /// so Explorer never shows a stale value.
    pub has_exif_rating: bool,
    pub has_ms_photo_rating: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMetadata {
    pub rating: RatingInfo,
    /// EXIF orientation 1..=8 (1 = as stored).
    pub orientation: u16,
}

pub fn read(bytes: &[u8]) -> FileMetadata {
    let exif = exif::Reader::new()
        .read_from_container(&mut Cursor::new(bytes))
        .ok();
    let exif_uint = |tag| {
        exif.as_ref()
            .and_then(|e| e.get_field(tag, exif::In::PRIMARY))
            .and_then(|f| f.value.get_uint(0))
    };

    let orientation = exif_uint(exif::Tag::Orientation)
        .filter(|o| (1..=8).contains(o))
        .map_or(1, |o| o as u16);
    let exif_rating = exif_uint(EXIF_RATING);

    let xmp = find_xmp_packet(bytes);
    let xmp_rating = xmp.as_deref().and_then(parse_xmp_rating);
    let stars = xmp_rating
        .or(exif_rating.map(|r| r as i32))
        .filter(|r| (1..=5).contains(r))
        .map(|r| r as u8);

    FileMetadata {
        rating: RatingInfo {
            stars,
            has_exif_rating: exif_rating.is_some(),
            has_ms_photo_rating: xmp.is_some_and(|x| x.contains("MicrosoftPhoto:Rating")),
        },
        orientation,
    }
}

/// The first XMP packet in the file. JPEG keeps it in APP1, HEIC in a metadata item – both
/// uncompressed, so a byte search finds it in either container.
fn find_xmp_packet(bytes: &[u8]) -> Option<Cow<'_, str>> {
    for (open, close) in [
        (&b"<x:xmpmeta"[..], &b"</x:xmpmeta>"[..]),
        (&b"<x:xapmeta"[..], &b"</x:xapmeta>"[..]),
    ] {
        if let Some(start) = memmem::find(bytes, open)
            && let Some(len) = memmem::find(&bytes[start..], close)
        {
            return Some(String::from_utf8_lossy(
                &bytes[start..start + len + close.len()],
            ));
        }
    }
    None
}

/// `xmp:Rating` as attribute (`xmp:Rating="4"`) or element (`<xmp:Rating>4</xmp:Rating>`).
/// Old files use the `xap` prefix for the same namespace.
fn parse_xmp_rating(xmp: &str) -> Option<i32> {
    for name in ["xmp:Rating", "xap:Rating"] {
        let mut rest = xmp;
        while let Some(pos) = rest.find(name) {
            rest = &rest[pos + name.len()..];
            let value = if let Some(attr) = rest.trim_start().strip_prefix('=') {
                let attr = attr.trim_start();
                let quote = attr.chars().next().filter(|c| *c == '"' || *c == '\'');
                quote.and_then(|q| attr[1..].split(q).next())
            } else {
                rest.strip_prefix('>').and_then(|el| el.split('<').next())
            };
            if let Some(v) = value.and_then(|v| v.trim().parse::<f64>().ok()) {
                return Some(v.round() as i32);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmp_rating_attribute_and_element() {
        assert_eq!(
            parse_xmp_rating(r#"<rdf:Description xmp:Rating="4" xmp:Label="x"/>"#),
            Some(4)
        );
        assert_eq!(parse_xmp_rating("<a xmp:Rating = '2'/>"), Some(2));
        assert_eq!(
            parse_xmp_rating("<xmp:Rating>5</xmp:Rating><xmp:Label>a</xmp:Label>"),
            Some(5)
        );
        assert_eq!(parse_xmp_rating(r#"<a xap:Rating="3.0"/>"#), Some(3));
        assert_eq!(parse_xmp_rating(r#"<a xmp:Rating="-1"/>"#), Some(-1));
        assert_eq!(parse_xmp_rating(r#"<a MicrosoftPhoto:Rating="75"/>"#), None);
    }

    #[test]
    fn packet_found_in_binary_noise() {
        let mut bytes = vec![0xFF, 0xD8, 0x00, 0x13];
        bytes.extend_from_slice(
            br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:Description xmp:Rating="3" MicrosoftPhoto:Rating="50"/></x:xmpmeta>"#,
        );
        bytes.extend_from_slice(&[0x00, 0xFF, 0xD9]);
        let meta = read(&bytes);
        assert_eq!(meta.rating.stars, Some(3));
        assert!(meta.rating.has_ms_photo_rating);
        assert!(!meta.rating.has_exif_rating);
        assert_eq!(meta.orientation, 1);
    }

    #[test]
    fn rejected_and_unrated_are_no_stars() {
        let meta = read(br#"<x:xmpmeta><a xmp:Rating="-1"/></x:xmpmeta>"#);
        assert_eq!(meta.rating.stars, None);
        let meta = read(br#"<x:xmpmeta><a xmp:Rating="0"/></x:xmpmeta>"#);
        assert_eq!(meta.rating.stars, None);
    }
}
