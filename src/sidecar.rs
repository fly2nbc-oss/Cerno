//! XMP sidecars: for formats whose marks are not written into the file (`Format::
//! marks_in_sidecar` – proprietary RAW, BMP, video), stars, colour label and keywords go into
//! `IMG_1.xmp` beside `IMG_1.CR2`, the name Lightroom uses. In a RAW + JPEG pair the sidecar
//! belongs to the RAW; the JPEG keeps its marks inside.
//!
//! Two such files of one name (`IMG_1.CR3` and `IMG_1.MOV`) would share `IMG_1.xmp`, and a
//! star on the video would land on the RAW. The RAW keeps the short name; a video or BMP takes
//! `IMG_1.MOV.xmp` whenever another file of its folder would want the short one (case aside),
//! and keeps a long name it has, also once that neighbour is gone. The folder is asked each
//! time, so a copy's or move's destination and `.originals` follow the same rule.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

use crate::library::{self, Format};

/// Where `photo`'s marks live: `IMG_1.CR2` → `IMG_1.xmp`; `IMG_1.MOV` → `IMG_1.xmp`, or
/// `IMG_1.MOV.xmp` beside `IMG_1.CR2` (see the module).
pub fn path_of(photo: &Path) -> PathBuf {
    let short = photo.with_extension("xmp");
    if !matches!(library::format_of(photo), Some(Format::Video | Format::Bmp)) {
        return short;
    }
    let (Some(dir), Some(name)) = (photo.parent(), photo.file_name()) else {
        return short;
    };
    long_in(dir, &name.to_string_lossy()).unwrap_or(short)
}

/// `IMG_1.MOV` → `IMG_1.MOV.xmp`.
fn long_of(photo: &Path) -> PathBuf {
    let mut name = photo.file_name().map(OsString::from).unwrap_or_default();
    name.push(".xmp");
    photo.with_file_name(name)
}

/// The long sidecar of `name` in `dir`, when it is there (whatever its case) or another file of
/// the same stem keeps its marks in a sidecar too. One listing; a folder that can't be listed
/// gets the short name.
fn long_in(dir: &Path, name: &str) -> Option<PathBuf> {
    let long = format!("{name}.xmp").to_lowercase();
    let stem = stem_of(name);
    let mut wanted = false;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let other = entry.file_name();
        let other = other.to_string_lossy();
        if other.to_lowercase() == long {
            return Some(dir.join(&*other));
        }
        wanted |= other != name
            && stem_of(&other) == stem
            && library::format_of(Path::new(&*other)).is_some_and(Format::marks_in_sidecar)
            && entry.file_type().is_ok_and(|kind| !kind.is_dir());
    }
    wanted.then(|| dir.join(format!("{name}.xmp")))
}

fn stem_of(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Where `sidecar` (`path_of(photo)`) goes when `photo` moves to `to` (delete, put back, copy,
/// move). A long name stays long – the neighbour it was taken for may come along too – a short
/// one asks `to`'s folder.
pub fn moved(photo: &Path, sidecar: &Path, to: &Path) -> PathBuf {
    let long = match (photo.file_name(), sidecar.file_name()) {
        (Some(photo), Some(sidecar)) => {
            sidecar.to_string_lossy().to_lowercase()
                == format!("{}.xmp", photo.to_string_lossy()).to_lowercase()
        }
        _ => false,
    };
    if long { long_of(to) } else { path_of(to) }
}

/// Whether `photo`'s marks live in a sidecar.
pub fn applies(photo: &Path) -> bool {
    library::format_of(photo).is_some_and(Format::marks_in_sidecar)
}

/// The sidecar's bytes, when there is one.
pub fn read(photo: &Path) -> Option<Vec<u8>> {
    std::fs::read(path_of(photo)).ok()
}

/// An empty XMP packet ExifTool can write into; it does not create a missing `.xmp` itself.
const EMPTY: &str = "<?xpacket begin='\u{feff}' id='W5M0MpCehiHzreSzNTczkc9d'?>\n\
<x:xmpmeta xmlns:x='adobe:ns:meta/' x:xmptk='Cerno'>\n\
<rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>\n\
</rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end='w'?>\n";

/// The sidecar at `path` (from `path_of`, so the write and the file agree), created empty when
/// missing – never overwritten.
pub fn ensure(path: &Path) -> Result<()> {
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(EMPTY.as_bytes())
                .context("cannot write the XMP sidecar")?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(err).context("cannot create the XMP sidecar"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str, files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerno-sidecar-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for file in files {
            std::fs::write(dir.join(file), file.as_bytes()).unwrap();
        }
        dir
    }

    #[test]
    fn a_sidecar_is_created_once_and_read_back() {
        let dir = folder("once", &["IMG_1.CR2"]);
        let raw = dir.join("IMG_1.CR2");
        assert_eq!(path_of(&raw), dir.join("IMG_1.xmp"));
        assert!(applies(&raw) && !applies(&dir.join("IMG_1.JPG")));
        assert!(read(&raw).is_none());

        let path = path_of(&raw);
        ensure(&path).unwrap();
        let empty = std::fs::read(&path).unwrap();
        assert!(String::from_utf8_lossy(&empty).contains("<x:xmpmeta"));
        std::fs::write(&path, b"<x:xmpmeta xmp:Rating='3'/>").unwrap();
        ensure(&path).unwrap();
        assert_eq!(read(&raw).unwrap(), b"<x:xmpmeta xmp:Rating='3'/>", "kept");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A video alone keeps the name of 1.5–1.9 (and Adobe's); beside a RAW of its name it moves
    /// aside, whatever the case of either name, and the RAW keeps `IMG_1.xmp`.
    #[test]
    fn a_video_beside_a_raw_of_its_name_takes_the_long_name() {
        let alone = folder(
            "alone",
            &["IMG_1.MOV", "IMG_1.JPG", "IMG_2.CR3", "IMG_4.bmp"],
        );
        assert_eq!(path_of(&alone.join("IMG_1.MOV")), alone.join("IMG_1.xmp"));
        assert_eq!(path_of(&alone.join("IMG_4.bmp")), alone.join("IMG_4.xmp"));

        let shared = folder("shared", &["IMG_1.MOV", "img_1.cr3", "IMG_1.xmp"]);
        assert_eq!(
            path_of(&shared.join("IMG_1.MOV")),
            shared.join("IMG_1.MOV.xmp")
        );
        assert_eq!(path_of(&shared.join("img_1.cr3")), shared.join("img_1.xmp"));

        // No RAW: a video and a BMP of one name both move aside – no "who came first".
        let pair = folder("video-bmp", &["IMG_3.MOV", "IMG_3.bmp"]);
        assert_eq!(path_of(&pair.join("IMG_3.MOV")), pair.join("IMG_3.MOV.xmp"));
        assert_eq!(path_of(&pair.join("IMG_3.bmp")), pair.join("IMG_3.bmp.xmp"));
        for dir in [alone, shared, pair] {
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    /// A long name, once there, stays – also when the RAW it was taken for is deleted, and
    /// whatever its case. A RAW never takes one, not even a stray `IMG_1.CR3.xmp`.
    #[test]
    fn a_long_name_that_is_there_stays() {
        let dir = folder("stays", &["IMG_1.MOV", "IMG_1.mov.XMP", "IMG_1.xmp"]);
        assert_eq!(path_of(&dir.join("IMG_1.MOV")), dir.join("IMG_1.mov.XMP"));

        let raw = folder("raw", &["IMG_1.CR3", "IMG_1.CR3.xmp"]);
        assert_eq!(path_of(&raw.join("IMG_1.CR3")), raw.join("IMG_1.xmp"));
        for dir in [dir, raw] {
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn a_moving_sidecar_keeps_a_long_name_and_asks_the_destination_for_a_short_one() {
        let dest = folder("dest", &["IMG_1.CR3", "IMG_1.xmp"]);
        let x = Path::new("x");
        // Short at the source, but the destination has a RAW of that name.
        assert_eq!(
            moved(
                &x.join("IMG_1.MOV"),
                &x.join("IMG_1.xmp"),
                &dest.join("IMG_1.MOV")
            ),
            dest.join("IMG_1.MOV.xmp")
        );
        // Long at the source stays long where nothing would ask for it.
        assert_eq!(
            moved(
                &x.join("IMG_2.MOV"),
                &x.join("IMG_2.mov.xmp"),
                &dest.join("IMG_2 (2).MOV")
            ),
            dest.join("IMG_2 (2).MOV.xmp")
        );
        assert_eq!(
            moved(
                &x.join("IMG_2.MOV"),
                &x.join("IMG_2.xmp"),
                &dest.join("IMG_2.MOV")
            ),
            dest.join("IMG_2.xmp")
        );
        // A RAW whose stem looks like a video's name still has a short sidecar.
        assert_eq!(
            moved(
                &x.join("a.MOV.CR3"),
                &x.join("a.MOV.xmp"),
                &dest.join("a.MOV.CR3")
            ),
            dest.join("a.MOV.xmp")
        );
        std::fs::remove_dir_all(dest).unwrap();
    }
}
