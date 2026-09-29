//! Clipped highlights and shadows: the share of pixels without any detail left.

/// Stored with every value; bump when the measure changes.
pub const VERSION: i64 = 2;

/// A pixel counts as blown out when all channels reach this value (white without detail – a
/// saturated blue sky with only the blue channel at 255 is not blown) …
pub const HIGHLIGHT: u8 = 250;
/// … and as crushed black when all channels stay at or below this one.
pub const SHADOW: u8 = 2;

/// More clipped highlights than this is worth a warning.
pub const HIGHLIGHTS_WARN: f32 = 0.01;
/// Deep shadows are often intended, so the threshold is higher.
pub const SHADOWS_WARN: f32 = 0.05;

/// `(highlights, shadows)`, each as share of all pixels (0..1).
pub fn measure(rgb: &[u8]) -> (f32, f32) {
    let pixels = rgb.as_chunks::<3>().0;
    if pixels.is_empty() {
        return (0.0, 0.0);
    }
    let (mut high, mut low) = (0usize, 0usize);
    for &[r, g, b] in pixels {
        if r.min(g).min(b) >= HIGHLIGHT {
            high += 1;
        } else if r.max(g).max(b) <= SHADOW {
            low += 1;
        }
    }
    let n = pixels.len() as f32;
    (high as f32 / n, low as f32 / n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_black_and_grey() {
        assert_eq!(measure(&[255; 30]), (1.0, 0.0));
        assert_eq!(measure(&[0; 30]), (0.0, 1.0));
        assert_eq!(measure(&[128; 30]), (0.0, 0.0));
        // A single saturated channel (deep blue sky, red flower) is not blown out.
        assert_eq!(measure(&[40, 90, 255, 251, 252, 255]), (0.5, 0.0));
        assert_eq!(measure(&[]), (0.0, 0.0));
    }
}
