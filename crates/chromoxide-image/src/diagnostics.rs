//! Pipeline diagnostics data structures and debug helpers.

use image::{GrayImage, Luma};

use crate::saliency::SaliencyMap;

/// Basic saliency summary statistics.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default)]
pub struct SaliencyStats {
    /// Minimum saliency over valid pixels.
    pub min: f64,
    /// Maximum saliency over valid pixels.
    pub max: f64,
    /// Mean saliency over valid pixels.
    pub mean: f64,
}

/// End-to-end image pipeline diagnostics.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct ImagePipelineDiagnostics {
    /// Input image width.
    pub original_width: u32,
    /// Input image height.
    pub original_height: u32,
    /// Working image width after optional resize.
    pub working_width: u32,
    /// Working image height after optional resize.
    pub working_height: u32,
    /// Number of valid pixels in working image.
    pub valid_pixel_count: usize,
    /// Number of invalid pixels in working image.
    pub invalid_pixel_count: usize,
    /// Saliency summary.
    pub saliency_stats: SaliencyStats,
    /// Number of selected representatives.
    pub representative_count: usize,
    /// Number of exported weighted samples.
    pub exported_sample_count: usize,
    /// Sum of exported sample weights.
    pub weight_sum: f64,
}

/// Converts a saliency map into a grayscale image for debugging.
pub fn saliency_to_luma_image(map: &SaliencyMap) -> GrayImage {
    let mut out = GrayImage::new(map.width, map.height);
    for (idx, value) in map.values.iter().enumerate() {
        let x = u32::try_from(idx % map.width as usize).unwrap_or(0);
        let y = u32::try_from(idx / map.width as usize).unwrap_or(0);
        let v = ((*value).clamp(0.0, 1.0) * 255.0).round() as u8;
        out.put_pixel(x, y, Luma([v]));
    }
    out
}

/// Computes min/max/mean saliency over valid indices.
pub(crate) fn compute_saliency_stats(map: &SaliencyMap, valid_indices: &[usize]) -> SaliencyStats {
    if valid_indices.is_empty() {
        return SaliencyStats::default();
    }

    let mut min_v = f64::INFINITY;
    let mut max_v = f64::NEG_INFINITY;
    let mut sum = 0.0;
    for &idx in valid_indices {
        let v = map.values[idx];
        min_v = min_v.min(v);
        max_v = max_v.max(v);
        sum += v;
    }

    SaliencyStats {
        min: min_v,
        max: max_v,
        mean: sum / valid_indices.len() as f64,
    }
}
