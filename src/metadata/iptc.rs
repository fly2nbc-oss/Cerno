//! IPTC IIM (APP13): the comment and keywords of files without XMP.

use super::*;

/// Only what the IPTC IIM block says – for tests that check both copies were written.
#[cfg(test)]
pub fn iptc_description(bytes: &[u8]) -> Description {
    iptc_iim(bytes)
        .map(|iim| Description {
            comment: iim.caption.unwrap_or_default(),
            keywords: iim.keywords,
        })
        .unwrap_or_default()
}

/// IPTC IIM from a JPEG's Photoshop block (APP13, resource 0x0404).
#[derive(Debug, Default, PartialEq)]
pub(super) struct Iim {
    pub(super) caption: Option<String>,
    pub(super) keywords: Vec<String>,
}

pub(super) fn iptc_iim(bytes: &[u8]) -> Option<Iim> {
    const PHOTOSHOP: &[u8] = b"Photoshop 3.0\0";
    let search = &bytes[..bytes.len().min(512 * 1024)];
    let mut pos = memmem::find(search, PHOTOSHOP)? + PHOTOSHOP.len();
    let be16 = |at: usize| Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    let be32 = |at: usize| Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    while bytes.get(pos..pos + 4) == Some(b"8BIM") {
        let id = be16(pos + 4)?;
        // Pascal name, padded to an even length together with its length byte.
        let name_len = usize::from(*bytes.get(pos + 6)?);
        let size_at = pos + 6 + (name_len + 2) / 2 * 2;
        let size = be32(size_at)? as usize;
        let data = bytes.get(size_at + 4..size_at + 4 + size)?;
        if id == 0x0404 {
            return Some(parse_iim(data));
        }
        pos = size_at + 4 + size + size % 2;
    }
    None
}

pub(super) fn parse_iim(data: &[u8]) -> Iim {
    let mut utf8 = false;
    let mut caption = None;
    let mut keywords = Vec::new();
    let mut i = 0;
    while i + 5 <= data.len() && data[i] == 0x1C {
        let (record, dataset) = (data[i + 1], data[i + 2]);
        let size = usize::from(u16::from_be_bytes([data[i + 3], data[i + 4]]));
        // Extended sizes (high bit) only occur for huge binary data; stop there.
        if size & 0x8000 != 0 {
            break;
        }
        let Some(value) = data.get(i + 5..i + 5 + size) else {
            break;
        };
        match (record, dataset) {
            (1, 90) => utf8 = value == [0x1B, 0x25, 0x47],
            (2, 25) => keywords.push(value.to_vec()),
            (2, 120) => caption = Some(value.to_vec()),
            _ => {}
        }
        i += 5 + size;
    }
    // Without the UTF-8 marker, text that is valid UTF-8 still is (many programs leave the
    // marker out); anything else is taken as Latin-1.
    let text = |raw: &[u8]| -> String {
        let decoded = if utf8 {
            String::from_utf8_lossy(raw).into_owned()
        } else {
            std::str::from_utf8(raw)
                .map(str::to_owned)
                .unwrap_or_else(|_| raw.iter().map(|&b| char::from(b)).collect())
        };
        decoded.trim_matches(['\0', ' ']).trim().to_owned()
    };
    Iim {
        caption: caption.map(|raw| text(&raw)).filter(|c| !c.is_empty()),
        keywords: keywords
            .iter()
            .map(|raw| text(raw))
            .filter(|k| !k.is_empty())
            .collect(),
    }
}
