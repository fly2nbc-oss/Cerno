//! Every model file Cerno downloads, with where it comes from, its size and SHA-256 – the one
//! place that names them. The values belong to this Cerno version: a file is used only when
//! its bytes match, so a host can't slip anything else in. New bytes need a new release tag
//! (never the same tag again) and, when the scores change, a new model id.

use super::aesthetic;

pub struct ModelFile {
    /// File name in the models directory.
    pub name: &'static str,
    /// Pinned: a Hugging Face commit or a GitHub release tag that is never re-used.
    pub url: &'static str,
    /// Tried once when `url` fails; it must serve the same bytes.
    pub mirror: Option<&'static str>,
    pub bytes: u64,
    /// Lower-case hex.
    pub sha256: &'static str,
}

/// A model that is downloaded as a whole: usable once all its files are there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pack {
    /// CLIP ViT-L/14: LAION aesthetics, the embeddings for For you and similar photos.
    Clip,
    /// SigLIP so400m + Aesthetic Predictor V2.5.
    V25,
}

impl Pack {
    pub const ALL: [Self; 2] = [Self::Clip, Self::V25];

    pub fn files(self) -> &'static [ModelFile] {
        match self {
            Self::Clip => CLIP,
            Self::V25 => V25,
        }
    }

    pub fn bytes(self) -> u64 {
        self.files().iter().map(|file| file.bytes).sum()
    }
}

/// Pinned to a commit, not `main`: a change in that repository must not reach Cerno, and the
/// scores stay comparable with the ones already stored. The hash is the LFS object id Hugging
/// Face lists for the file.
const CLIP: &[ModelFile] = &[ModelFile {
    name: aesthetic::MODEL_FILE,
    url: "https://huggingface.co/Xenova/clip-vit-large-patch14/resolve/\
          c307790166907339eed5a9a53a249af534102536/onnx/vision_model.onnx",
    mirror: None,
    bytes: 1_216_438_437,
    sha256: "ff49f8aa57c7abfd26e382eb083e4dbf988505223a9bd3767dbfd4e729206709",
}];

/// Cerno's own release `models-1` (fly2nbc-oss/Cerno): the SigLIP vision tower cut out of
/// onnx-community/siglip-so400m-patch14-384-ONNX at commit 1f4ddadf
/// (`tools/extract_siglip_vision.py`, Apache-2.0) and the V2.5 head collapsed from
/// discus0434/aesthetic-predictor-v2-5 at commit 3125a9e1 (`tools/make_aesthetic_head.py`,
/// AGPL-3.0 – downloaded, never part of Cerno). The small head comes first: a missing release
/// fails before 1.7 GB are fetched.
const V25: &[ModelFile] = &[
    ModelFile {
        name: aesthetic::V25_HEAD_FILE,
        url: "https://github.com/fly2nbc-oss/Cerno/releases/download/models-1/\
              aesthetic-predictor-v2.5-head.bin",
        mirror: None,
        bytes: 4_612,
        sha256: "25ec57c5b06d3df3b6f7248c3be8e10555c0a89a7ba8a914c65bd70bf8ea0dc2",
    },
    ModelFile {
        name: aesthetic::SIGLIP_FILE,
        url: "https://github.com/fly2nbc-oss/Cerno/releases/download/models-1/\
              siglip-so400m-patch14-384-vision.onnx",
        mirror: None,
        bytes: 1_713_457_844,
        sha256: "617206ce0befd05f89090cea9bd15780c59b7539d42f87e08378d707d6928e63",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every URL names a fixed commit or the fixed release tag – never `main` or `latest` –
    /// and every hash is 64 hex digits.
    #[test]
    fn every_file_is_pinned() {
        for pack in Pack::ALL {
            for file in pack.files() {
                for url in std::iter::once(file.url).chain(file.mirror) {
                    assert!(!url.contains(char::is_whitespace), "{url}");
                    assert!(url.starts_with("https://"), "{url}");
                    assert!(url.ends_with(file.name) || pack == Pack::Clip, "{url}");
                    let pinned = url.contains("/resolve/c307790166907339eed5a9a53a249af534102536/")
                        || url.contains("/releases/download/models-1/");
                    assert!(pinned, "{url}");
                    assert!(
                        !url.contains("/main/") && !url.contains("/latest/"),
                        "{url}"
                    );
                }
                assert_eq!(file.sha256.len(), 64, "{}", file.name);
                assert!(
                    file.sha256
                        .chars()
                        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
                    "{}",
                    file.name
                );
            }
        }
    }

    #[test]
    fn the_packs_name_the_files_the_models_load() {
        let names: Vec<&str> = Pack::ALL
            .iter()
            .flat_map(|pack| pack.files().iter().map(|file| file.name))
            .collect();
        assert_eq!(
            names,
            [
                aesthetic::MODEL_FILE,
                aesthetic::V25_HEAD_FILE,
                aesthetic::SIGLIP_FILE
            ]
        );
        // The head is SIGLIP_DIM weights plus the bias, little-endian f32.
        assert_eq!(V25[0].bytes, 4 * 1153);
        assert_eq!(Pack::V25.bytes(), 1_713_462_456);
    }
}
