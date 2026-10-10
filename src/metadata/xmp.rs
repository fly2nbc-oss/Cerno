//! XMP: the packet, rating, label, comment and keywords – read without an XML parser.

use super::*;

/// The `rdf:li` texts of an array property (`<dc:subject><rdf:Bag><rdf:li>…`).
pub(super) fn xmp_items(xmp: &str, property: &str) -> Option<Vec<(String, String)>> {
    let open = format!("<{property}");
    // `<dc:subjectCode` is another property: look on after it.
    let mut search = xmp;
    let rest = loop {
        let start = search.find(&open)?;
        let rest = &search[start + open.len()..];
        if rest.starts_with(['>', ' ', '\t', '\r', '\n', '/']) {
            break rest;
        }
        search = rest;
    };
    let body = &rest[..rest.find(&format!("</{property}>")).unwrap_or(0)];
    let mut items = Vec::new();
    let mut tail = body;
    while let Some(pos) = tail.find("<rdf:li") {
        tail = &tail[pos + "<rdf:li".len()..];
        let Some(end_of_tag) = tail.find('>') else {
            break;
        };
        let attributes = tail[..end_of_tag].to_owned();
        if attributes.ends_with('/') {
            tail = &tail[end_of_tag + 1..];
            continue;
        }
        tail = &tail[end_of_tag + 1..];
        let Some(close) = tail.find("</rdf:li>") else {
            break;
        };
        items.push((attributes, xml_unescape(&tail[..close])));
        tail = &tail[close..];
    }
    Some(items)
}

/// Keywords: every item of the bag, trimmed, empty ones left out.
pub(super) fn xmp_bag(xmp: &str, property: &str) -> Option<Vec<String>> {
    let items = xmp_items(xmp, property)?;
    Some(
        items
            .into_iter()
            .map(|(_, text)| text.trim().to_owned())
            .filter(|text| !text.is_empty())
            .collect(),
    )
}

/// A language alternative: the `x-default` entry, else the first one.
pub(super) fn xmp_alt_default(xmp: &str, property: &str) -> Option<String> {
    let items = xmp_items(xmp, property)?;
    items
        .iter()
        .find(|(attributes, _)| attributes.contains("x-default"))
        .or(items.first())
        .map(|(_, text)| text.clone())
}

pub(super) fn xml_unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest.find(';').filter(|&s| s <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The first XMP packet in the file. JPEG keeps it in APP1, HEIC in a metadata item – both
/// uncompressed, so a byte search finds it in either container.
pub(super) fn find_xmp_packet(bytes: &[u8]) -> Option<Cow<'_, str>> {
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
pub(super) fn parse_xmp_rating(xmp: &str) -> Option<i32> {
    parse_xmp_text(xmp, &["xmp:Rating", "xap:Rating"])
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| v.round() as i32)
}

pub(super) fn parse_xmp_label(xmp: &str) -> Option<String> {
    parse_xmp_text(xmp, &["xmp:Label", "xap:Label"])
}

/// The text of the first matching XMP attribute or element. A longer tag that only starts
/// with the name (`xmp:RatingPercent`) is skipped.
pub(super) fn parse_xmp_text(xmp: &str, names: &[&str]) -> Option<String> {
    for name in names {
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
            if let Some(value) = value {
                return Some(value.trim().to_owned());
            }
        }
    }
    None
}
