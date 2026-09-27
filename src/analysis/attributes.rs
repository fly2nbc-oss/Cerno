//! Zero-shot photo attributes from the CLIP embedding (CLIP-IQA style).
//!
//! Each attribute compares the image embedding with a positive and a negative text prompt;
//! the text embeddings are precomputed by `tools/make_clip_prompts.py` and embedded. Costs a
//! dozen dot products per photo – no extra model run.

use std::sync::LazyLock;

/// Attributes, in the order of `PROMPTS` in `tools/make_clip_prompts.py` (overall quality,
/// sharp, lighting, composition, noise, colourfulness); their names are in `i18n::Texts`.
pub const COUNT: usize = 6;

/// CLIP's logit scale.
const SCALE: f32 = 100.0;
const DIM: usize = 768;

/// 12 × 768 little-endian f32: (positive, negative) per attribute, L2-normalised.
static PROMPTS: LazyLock<Vec<f32>> = LazyLock::new(|| {
    include_bytes!("clip_prompts.bin")
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
});

/// Probability-like score per attribute, 0..1 (0.5 = undecided).
pub fn scores(embedding: &[f32]) -> Option<[f32; COUNT]> {
    if embedding.len() != DIM {
        return None;
    }
    let norm = embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm <= f32::EPSILON {
        return None;
    }
    let cosine = |prompt: usize| -> f32 {
        let text = &PROMPTS[prompt * DIM..(prompt + 1) * DIM];
        text.iter().zip(embedding).map(|(t, x)| t * x).sum::<f32>() / norm
    };
    // Softmax over two classes is the logistic of the difference.
    Some(std::array::from_fn(|i| {
        let difference = cosine(2 * i) - cosine(2 * i + 1);
        1.0 / (1.0 + (-SCALE * difference).exp())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_are_embedded() {
        assert_eq!(PROMPTS.len(), COUNT * 2 * DIM);
        let first_norm: f32 = PROMPTS[..DIM].iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((first_norm - 1.0).abs() < 1e-3);
    }

    #[test]
    fn an_embedding_like_the_positive_prompt_scores_high() {
        for i in 0..COUNT {
            let positive = PROMPTS[2 * i * DIM..(2 * i + 1) * DIM].to_vec();
            let negative = PROMPTS[(2 * i + 1) * DIM..(2 * i + 2) * DIM].to_vec();
            assert!(scores(&positive).unwrap()[i] > 0.5, "attribute {i}");
            assert!(scores(&negative).unwrap()[i] < 0.5, "attribute {i}");
        }
        assert!(scores(&[0.0; DIM]).is_none());
        assert!(scores(&[1.0; 3]).is_none());
    }
}
