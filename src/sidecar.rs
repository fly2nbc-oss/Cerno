//! XMP sidecars: for formats whose marks are not written into the file (`Format::
//! marks_in_sidecar` – proprietary RAW, BMP, video), stars, colour label and keywords go into
//! `IMG_1.xmp` beside `IMG_1.CR2`, the name Lightroom uses. In a RAW + JPEG pair the sidecar
//! belongs to the RAW; the JPEG keeps its marks inside.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

use crate::library::{self, Format};

/// `IMG_1.CR2` → `IMG_1.xmp`.
pub fn path_of(photo: &Path) -> PathBuf {
    photo.with_extension("xmp")
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

/// The sidecar, created empty when missing (never overwritten). Returns its path.
pub fn ensure(photo: &Path) -> Result<PathBuf> {
    let path = path_of(photo);
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(EMPTY.as_bytes())
                .context("cannot write the XMP sidecar")?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(err).context("cannot create the XMP sidecar"),
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sidecar_is_created_once_and_read_back() {
        let dir = std::env::temp_dir().join(format!("cerno-sidecar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let raw = dir.join("IMG_1.CR2");
        assert_eq!(path_of(&raw), dir.join("IMG_1.xmp"));
        assert!(applies(&raw) && !applies(&dir.join("IMG_1.JPG")));
        assert!(read(&raw).is_none());

        let path = ensure(&raw).unwrap();
        let empty = std::fs::read(&path).unwrap();
        assert!(String::from_utf8_lossy(&empty).contains("<x:xmpmeta"));
        std::fs::write(&path, b"<x:xmpmeta xmp:Rating='3'/>").unwrap();
        ensure(&raw).unwrap();
        assert_eq!(read(&raw).unwrap(), b"<x:xmpmeta xmp:Rating='3'/>", "kept");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
