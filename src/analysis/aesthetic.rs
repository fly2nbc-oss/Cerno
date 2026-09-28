//! Aesthetics scores from image embeddings.
//!
//! * **LAION**: CLIP ViT-L/14 → LAION "improved aesthetic predictor". The CLIP model is
//!   downloaded on request; the LAION head (Apache-2.0) is embedded.
//! * **V2.5**: SigLIP so400m → Aesthetic Predictor V2.5. Both files live in the models
//!   directory; the head is AGPL-3.0 and therefore never embedded (see THIRD_PARTY.md).
//!
//! Both heads are linear-only MLPs, collapsed into one layer by `tools/make_aesthetic_head.py`,
//! applied to the L2-normalised embedding. The vision models run in ONNX Runtime: DirectML on
//! Windows (any DX12 GPU incl. AMD), CPU elsewhere, WebGPU behind the `webgpu` feature. Scores
//! are roughly 1–10; ordinary photos land around 4–6.

use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context as _, Result, anyhow, bail};
use ort::ep::ExecutionProviderDispatch;
use ort::session::Session;
use ort::value::Tensor;

use crate::decode;

/// A score on the star scale, so it reads like the user's own stars: real photos score
/// between 2 and 8, that range becomes 0–5 (the bars always used it).
pub fn as_stars(score: f32) -> f32 {
    ((score - 2.0) * 5.0 / 6.0).clamp(0.0, 5.0)
}

/// CLIP vision model, file name in the models directory.
pub const MODEL_FILE: &str = "clip-vit-large-patch14-vision.onnx";
/// Pinned to a commit, not `main`: a change in that repository must not reach Cerno, and
/// the scores stay comparable with the ones already stored.
pub const MODEL_URL: &str = "https://huggingface.co/Xenova/clip-vit-large-patch14/resolve/\
     c307790166907339eed5a9a53a249af534102536/onnx/vision_model.onnx";
pub const MODEL_BYTES: u64 = 1_216_438_437;
/// Checked after the download (the LFS object id Hugging Face lists for the file).
pub const MODEL_SHA256: &str = "ff49f8aa57c7abfd26e382eb083e4dbf988505223a9bd3767dbfd4e729206709";
/// Stored with every LAION score; change it when the model, head or preprocessing change.
pub const MODEL_ID: &str = "clip-vit-l14+laion-sac-logos-ava1-linear/1";

/// SigLIP image tower (`tools/extract_siglip_vision.py`) and the collapsed V2.5 head.
pub const SIGLIP_FILE: &str = "siglip-so400m-patch14-384-vision.onnx";
pub const V25_HEAD_FILE: &str = "aesthetic-predictor-v2.5-head.bin";
/// Stored with every V2.5 score.
pub const V25_MODEL_ID: &str = "siglip-so400m-384+v2.5-linear/1";

const CLIP_DIM: usize = 768;
const SIGLIP_DIM: usize = 1152;
const CLIP_SIZE: u32 = 224;
const SIGLIP_SIZE: u32 = 384;
const CLIP_MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const CLIP_STD: [f32; 3] = [0.268_629_5, 0.261_302_6, 0.275_777_1];

/// LAION head: 768 weights followed by the bias, little-endian f32.
static LAION_HEAD: LazyLock<Vec<f32>> =
    LazyLock::new(|| f32_le(include_bytes!("aesthetic_head.bin")));

fn f32_le(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
}

/// An ONNX image encoder with the embedding output picked by its width.
struct Encoder {
    session: Session,
    input: String,
    output: String,
    backend: &'static str,
}

impl Encoder {
    /// GPU first (reported, not silently skipped), CPU as fallback.
    fn load(path: &Path, dim: usize) -> Result<Self> {
        #[cfg(windows)]
        match Self::build(
            path,
            dim,
            Some(ort::ep::DirectML::default().build().error_on_failure()),
            "DirectML",
        ) {
            Ok(encoder) => return Ok(encoder),
            Err(err) => log::warn!("DirectML unavailable, using the CPU: {err:#}"),
        }
        #[cfg(all(not(windows), feature = "webgpu"))]
        match Self::build(
            path,
            dim,
            Some(ort::ep::WebGPU::default().build().error_on_failure()),
            "WebGPU",
        ) {
            Ok(encoder) => return Ok(encoder),
            Err(err) => log::warn!("WebGPU unavailable, using the CPU: {err:#}"),
        }
        Self::build(path, dim, None, "CPU")
    }

    fn build(
        path: &Path,
        dim: usize,
        provider: Option<ExecutionProviderDispatch>,
        backend: &'static str,
    ) -> Result<Self> {
        let mut builder = Session::builder().map_err(|e| anyhow!("{e}"))?;
        if let Some(provider) = provider {
            // DirectML can't use ORT's memory pattern optimisation.
            builder = builder
                .with_execution_providers([provider])
                .map_err(|e| anyhow!("{e}"))?
                .with_memory_pattern(false)
                .map_err(|e| anyhow!("{e}"))?;
        }
        let session = builder
            .commit_from_file(path)
            .map_err(|e| anyhow!("{e}"))
            .with_context(|| format!("cannot load {}", path.display()))?;
        let input = session
            .inputs()
            .first()
            .context("model has no input")?
            .name()
            .to_owned();
        // E.g. CLIP's `image_embeds` is [batch, 768]; `last_hidden_state` has other widths.
        let output = session
            .outputs()
            .iter()
            .find(|o| {
                o.dtype()
                    .tensor_shape()
                    .is_some_and(|s| s.len() == 2 && s[1] == dim as i64)
            })
            .with_context(|| format!("model has no {dim}-wide embedding output"))?
            .name()
            .to_owned();
        log::info!(
            "{} on {backend}: input `{input}`, output `{output}`",
            path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned()
            )
        );
        Ok(Self {
            session,
            input,
            output,
            backend,
        })
    }

    fn embed(&mut self, pixels: Vec<f32>, size: u32) -> Result<Vec<f32>> {
        let tensor = Tensor::from_array(([1usize, 3, size as usize, size as usize], pixels))
            .map_err(|e| anyhow!("{e}"))?;
        let outputs = self
            .session
            .run(ort::inputs![self.input.as_str() => tensor])
            .map_err(|e| anyhow!("{e}"))?;
        let (_, embedding) = outputs[self.output.as_str()]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow!("{e}"))?;
        Ok(embedding.to_vec())
    }
}

/// CLIP ViT-L/14 with the LAION head.
pub struct AestheticModel {
    encoder: Encoder,
    pub backend: &'static str,
}

impl AestheticModel {
    pub fn load(path: &Path) -> Result<Self> {
        let encoder = Encoder::load(path, CLIP_DIM)?;
        Ok(Self {
            backend: encoder.backend,
            encoder,
        })
    }

    /// LAION score of an RGB8 image of any size, plus the CLIP embedding.
    pub fn score(&mut self, rgb: &[u8], width: u32, height: u32) -> Result<(f32, Vec<f32>)> {
        let embedding = self
            .encoder
            .embed(preprocess_clip(rgb, width, height)?, CLIP_SIZE)?;
        Ok((linear_head(&LAION_HEAD, &embedding), embedding))
    }
}

/// SigLIP so400m with the Aesthetic Predictor V2.5 head.
pub struct V25Model {
    encoder: Encoder,
    head: Vec<f32>,
    pub backend: &'static str,
}

impl V25Model {
    pub fn installed(models_dir: &Path) -> bool {
        models_dir.join(SIGLIP_FILE).is_file() && models_dir.join(V25_HEAD_FILE).is_file()
    }

    pub fn load(models_dir: &Path) -> Result<Self> {
        let head = f32_le(
            &std::fs::read(models_dir.join(V25_HEAD_FILE)).context("cannot read the V2.5 head")?,
        );
        if head.len() != SIGLIP_DIM + 1 {
            bail!(
                "V2.5 head has {} values, expected {}",
                head.len(),
                SIGLIP_DIM + 1
            );
        }
        let encoder = Encoder::load(&models_dir.join(SIGLIP_FILE), SIGLIP_DIM)?;
        Ok(Self {
            backend: encoder.backend,
            encoder,
            head,
        })
    }

    pub fn score(&mut self, rgb: &[u8], width: u32, height: u32) -> Result<f32> {
        let embedding = self
            .encoder
            .embed(preprocess_siglip(rgb, width, height)?, SIGLIP_SIZE)?;
        Ok(linear_head(&self.head, &embedding))
    }
}

/// CLIP preprocessing: shorter side to 224, centre crop 224×224, normalise, planar RGB.
fn preprocess_clip(rgb: &[u8], width: u32, height: u32) -> Result<Vec<f32>> {
    let scale = f64::from(CLIP_SIZE) / f64::from(width.min(height));
    let rw = ((f64::from(width) * scale).round() as u32).max(CLIP_SIZE);
    let rh = ((f64::from(height) * scale).round() as u32).max(CLIP_SIZE);
    let resized = decode::resize_rgb(rgb.to_vec(), width, height, rw, rh)?;
    let (x0, y0) = ((rw - CLIP_SIZE) / 2, (rh - CLIP_SIZE) / 2);
    Ok(planar(&resized, rw, x0, y0, CLIP_SIZE, |c, v| {
        (v / 255.0 - CLIP_MEAN[c]) / CLIP_STD[c]
    }))
}

/// SigLIP preprocessing: squash to 384×384 (no crop), scale to -1..1, planar RGB.
fn preprocess_siglip(rgb: &[u8], width: u32, height: u32) -> Result<Vec<f32>> {
    let resized = decode::resize_rgb(rgb.to_vec(), width, height, SIGLIP_SIZE, SIGLIP_SIZE)?;
    Ok(planar(&resized, SIGLIP_SIZE, 0, 0, SIGLIP_SIZE, |_, v| {
        v / 127.5 - 1.0
    }))
}

/// Cuts a `size`² window at (x0, y0) out of an RGB8 image `row_width` wide into planar floats.
fn planar(
    rgb: &[u8],
    row_width: u32,
    x0: u32,
    y0: u32,
    size: u32,
    normalise: impl Fn(usize, f32) -> f32,
) -> Vec<f32> {
    let plane = (size * size) as usize;
    let mut out = vec![0.0f32; 3 * plane];
    for y in 0..size {
        for x in 0..size {
            let s = (((y0 + y) * row_width + x0 + x) * 3) as usize;
            let d = (y * size + x) as usize;
            for c in 0..3 {
                out[c * plane + d] = normalise(c, f32::from(rgb[s + c]));
            }
        }
    }
    out
}

/// A collapsed predictor head (`weights…, bias`) on the L2-normalised embedding.
fn linear_head(head: &[f32], embedding: &[f32]) -> f32 {
    let norm = embedding
        .iter()
        .map(|v| v * v)
        .sum::<f32>()
        .sqrt()
        .max(f32::EPSILON);
    let (weights, bias) = head.split_at(head.len() - 1);
    let dot: f32 = weights
        .iter()
        .zip(embedding)
        .map(|(w, x)| w * x / norm)
        .sum();
    dot + bias[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_map_onto_the_star_scale() {
        assert_eq!(as_stars(2.0), 0.0);
        assert_eq!(as_stars(5.0), 2.5);
        assert_eq!(as_stars(8.0), 5.0);
        assert_eq!(as_stars(1.0), 0.0);
        assert_eq!(as_stars(9.5), 5.0);
    }

    #[test]
    fn head_weights_are_embedded() {
        assert_eq!(LAION_HEAD.len(), CLIP_DIM + 1);
        // A zero embedding scores exactly the bias.
        assert!((linear_head(&LAION_HEAD, &[0.0; CLIP_DIM]) - LAION_HEAD[CLIP_DIM]).abs() < 1e-6);
        assert!(
            (4.0..7.0).contains(&LAION_HEAD[CLIP_DIM]),
            "bias {}",
            LAION_HEAD[CLIP_DIM]
        );
    }

    #[test]
    fn preprocess_crops_the_centre() {
        // 448×224: left half black, right half white → the centre crop is half black, half white.
        let rgb: Vec<u8> = (0..448 * 224)
            .flat_map(|i| {
                if i % 448 < 224 {
                    [0, 0, 0]
                } else {
                    [255, 255, 255]
                }
            })
            .collect();
        let out = preprocess_clip(&rgb, 448, 224).unwrap();
        assert_eq!(out.len(), 3 * 224 * 224);
        let black = (0.0 - CLIP_MEAN[0]) / CLIP_STD[0];
        let white = (1.0 - CLIP_MEAN[0]) / CLIP_STD[0];
        assert!((out[224 * 100 + 5] - black).abs() < 0.05);
        assert!((out[224 * 100 + 218] - white).abs() < 0.05);
    }

    #[test]
    fn siglip_input_is_squashed_and_centred_on_zero() {
        let rgb = vec![255u8; 800 * 200 * 3];
        let out = preprocess_siglip(&rgb, 800, 200).unwrap();
        assert_eq!(out.len(), 3 * 384 * 384);
        assert!(out.iter().all(|v| (v - 1.0).abs() < 1e-3));
    }
}
