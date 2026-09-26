//! Aesthetics score: CLIP ViT-L/14 image embedding → LAION "improved aesthetic predictor".
//!
//! The CLIP vision model runs in ONNX Runtime (DirectML on Windows, so any DX12 GPU incl. AMD;
//! CPU elsewhere, WebGPU behind the `webgpu` feature). The LAION MLP has no activations, so it
//! collapses into one linear layer (`tools/make_aesthetic_head.py`), embedded below. Scores are
//! on the LAION scale of roughly 1–10; ordinary photos land around 4–6.

use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context as _, Result, anyhow};
use ort::ep::ExecutionProviderDispatch;
use ort::session::Session;
use ort::value::Tensor;

use crate::decode;

/// File name in the models directory.
pub const MODEL_FILE: &str = "clip-vit-large-patch14-vision.onnx";
pub const MODEL_URL: &str =
    "https://huggingface.co/Xenova/clip-vit-large-patch14/resolve/main/onnx/vision_model.onnx";
pub const MODEL_BYTES: u64 = 1_216_438_437;
/// Stored with every score; change it when the model, head or preprocessing change.
pub const MODEL_ID: &str = "clip-vit-l14+laion-sac-logos-ava1-linear/1";

const SIZE: u32 = 224;
const MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const STD: [f32; 3] = [0.268_629_5, 0.261_302_6, 0.275_777_1];

/// 768 weights followed by the bias, little-endian f32.
static HEAD: LazyLock<Vec<f32>> = LazyLock::new(|| {
    include_bytes!("aesthetic_head.bin")
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
});

pub struct AestheticModel {
    session: Session,
    input: String,
    output: String,
    pub backend: &'static str,
}

impl AestheticModel {
    /// GPU first (reported, not silently skipped), CPU as fallback.
    pub fn load(path: &Path) -> Result<Self> {
        #[cfg(windows)]
        match Self::build(
            path,
            Some(ort::ep::DirectML::default().build().error_on_failure()),
            "DirectML",
        ) {
            Ok(model) => return Ok(model),
            Err(err) => log::warn!("DirectML unavailable, using the CPU: {err:#}"),
        }
        #[cfg(all(not(windows), feature = "webgpu"))]
        match Self::build(
            path,
            Some(ort::ep::WebGPU::default().build().error_on_failure()),
            "WebGPU",
        ) {
            Ok(model) => return Ok(model),
            Err(err) => log::warn!("WebGPU unavailable, using the CPU: {err:#}"),
        }
        Self::build(path, None, "CPU")
    }

    fn build(
        path: &Path,
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
        // `image_embeds` is [batch, 768]; `last_hidden_state` has the same leading dims but 1024.
        let output = session
            .outputs()
            .iter()
            .find(|o| {
                o.dtype()
                    .tensor_shape()
                    .is_some_and(|s| s.len() == 2 && s[1] == 768)
            })
            .context("model has no 768-wide embedding output")?
            .name()
            .to_owned();
        log::info!("aesthetics model on {backend}: input `{input}`, output `{output}`");
        Ok(Self {
            session,
            input,
            output,
            backend,
        })
    }

    /// Scores an RGB8 image of any size. Also returns the CLIP embedding.
    pub fn score(&mut self, rgb: &[u8], width: u32, height: u32) -> Result<(f32, Vec<f32>)> {
        let pixels = preprocess(rgb, width, height)?;
        let tensor = Tensor::from_array(([1usize, 3, SIZE as usize, SIZE as usize], pixels))
            .map_err(|e| anyhow!("{e}"))?;
        let outputs = self
            .session
            .run(ort::inputs![self.input.as_str() => tensor])
            .map_err(|e| anyhow!("{e}"))?;
        let (_, embedding) = outputs[self.output.as_str()]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow!("{e}"))?;
        Ok((head(embedding), embedding.to_vec()))
    }
}

/// CLIP preprocessing: shorter side to 224, centre crop 224×224, normalise, planar RGB.
fn preprocess(rgb: &[u8], width: u32, height: u32) -> Result<Vec<f32>> {
    let scale = f64::from(SIZE) / f64::from(width.min(height));
    let rw = ((f64::from(width) * scale).round() as u32).max(SIZE);
    let rh = ((f64::from(height) * scale).round() as u32).max(SIZE);
    let resized = decode::resize_rgb(rgb.to_vec(), width, height, rw, rh)?;
    let (x0, y0) = ((rw - SIZE) / 2, (rh - SIZE) / 2);
    let plane = (SIZE * SIZE) as usize;
    let mut out = vec![0.0f32; 3 * plane];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let s = (((y0 + y) * rw + x0 + x) * 3) as usize;
            let d = (y * SIZE + x) as usize;
            for c in 0..3 {
                out[c * plane + d] = (f32::from(resized[s + c]) / 255.0 - MEAN[c]) / STD[c];
            }
        }
    }
    Ok(out)
}

/// The LAION predictor on the L2-normalised embedding.
fn head(embedding: &[f32]) -> f32 {
    let norm = embedding
        .iter()
        .map(|v| v * v)
        .sum::<f32>()
        .sqrt()
        .max(f32::EPSILON);
    let dot: f32 = HEAD[..768]
        .iter()
        .zip(embedding)
        .map(|(w, x)| w * x / norm)
        .sum();
    dot + HEAD[768]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_weights_are_embedded() {
        assert_eq!(HEAD.len(), 769);
        // A zero embedding scores exactly the bias.
        assert!((head(&[0.0; 768]) - HEAD[768]).abs() < 1e-6);
        assert!((4.0..7.0).contains(&HEAD[768]), "bias {}", HEAD[768]);
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
        let out = preprocess(&rgb, 448, 224).unwrap();
        assert_eq!(out.len(), 3 * 224 * 224);
        let black = (0.0 - MEAN[0]) / STD[0];
        let white = (1.0 - MEAN[0]) / STD[0];
        assert!((out[224 * 100 + 5] - black).abs() < 0.05);
        assert!((out[224 * 100 + 218] - white).abs() < 0.05);
    }
}
