//! Internal numeric and conversion utilities.

use image::imageops::FilterType;

use crate::config::ResizeFilter;
use crate::error::ImagePipelineError;

/// Small epsilon used for numeric tie-breaking and stability checks.
pub(crate) const EPSILON: f64 = 1.0e-12;

/// Clamps a scalar to `[0, 1]`.
pub(crate) fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Checked conversion from `(width, height)` to linear buffer length.
pub(crate) fn checked_len(width: u32, height: u32) -> Result<usize, ImagePipelineError> {
    let w = usize::try_from(width).map_err(|_| {
        ImagePipelineError::Numeric("image width does not fit into usize".to_string())
    })?;
    let h = usize::try_from(height).map_err(|_| {
        ImagePipelineError::Numeric("image height does not fit into usize".to_string())
    })?;
    w.checked_mul(h)
        .ok_or_else(|| ImagePipelineError::Numeric("image area overflow".to_string()))
}

/// Converts an sRGB-encoded channel in `[0, 1]` to linear sRGB.
pub(crate) fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts an 8-bit sRGB channel to linear sRGB.
pub(crate) fn srgb_u8_to_linear(v: u8) -> f64 {
    srgb_to_linear(f64::from(v) / 255.0)
}

/// Converts linear sRGB to Oklab.
pub(crate) fn linear_rgb_to_oklab(rgb: [f64; 3]) -> chromoxide::Oklab {
    let l = 0.412_221_470_8 * rgb[0] + 0.536_332_536_3 * rgb[1] + 0.051_445_992_9 * rgb[2];
    let m = 0.211_903_498_2 * rgb[0] + 0.680_699_545_1 * rgb[1] + 0.107_396_956_6 * rgb[2];
    let s = 0.088_302_461_9 * rgb[0] + 0.281_718_837_6 * rgb[1] + 0.629_978_700_5 * rgb[2];

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    chromoxide::Oklab {
        l: 0.210_454_255_3 * l_ + 0.793_617_785 * m_ - 0.004_072_046_8 * s_,
        a: 1.977_998_495_1 * l_ - 2.428_592_205 * m_ + 0.450_593_709_9 * s_,
        b: 0.025_904_037_1 * l_ + 0.782_771_766_2 * m_ - 0.808_675_766 * s_,
    }
}

/// Computes relative luminance from linear sRGB.
pub(crate) fn relative_luminance(rgb: [f64; 3]) -> f64 {
    let r = rgb[0].max(0.0);
    let g = rgb[1].max(0.0);
    let b = rgb[2].max(0.0);
    (0.2126 * r + 0.7152 * g + 0.0722 * b).max(0.0)
}

/// Squared Euclidean distance in Oklab.
pub(crate) fn lab_distance2(a: chromoxide::Oklab, b: chromoxide::Oklab) -> f64 {
    let dl = a.l - b.l;
    let da = a.a - b.a;
    let db = a.b - b.b;
    dl * dl + da * da + db * db
}

/// Computes percentile from a sorted array.
pub(crate) fn percentile(sorted: &[f64], p: f64) -> f64 {
    debug_assert!(!sorted.is_empty());
    let p = p.clamp(0.0, 1.0);
    if sorted.len() == 1 {
        return sorted[0];
    }

    let pos = p * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let t = pos - lo as f64;
        sorted[lo] * (1.0 - t) + sorted[hi] * t
    }
}

/// Converts public resize filter enum to `image` filter type.
pub(crate) fn resize_filter_to_image(filter: ResizeFilter) -> FilterType {
    match filter {
        ResizeFilter::Nearest => FilterType::Nearest,
        ResizeFilter::Triangle => FilterType::Triangle,
        ResizeFilter::CatmullRom => FilterType::CatmullRom,
        ResizeFilter::Gaussian => FilterType::Gaussian,
        ResizeFilter::Lanczos3 => FilterType::Lanczos3,
    }
}
