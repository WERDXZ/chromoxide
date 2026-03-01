//! Color-space conversion helpers used by objective terms.

use crate::color::Oklab;

/// Linear (non-gamma encoded) sRGB.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinearRgb {
    /// Red channel.
    pub r: f64,
    /// Green channel.
    pub g: f64,
    /// Blue channel.
    pub b: f64,
}

/// Converts Oklab to linear sRGB.
pub fn oklab_to_linear_srgb(lab: Oklab) -> LinearRgb {
    let l = lab.l + 0.396_337_777_4 * lab.a + 0.215_803_757_3 * lab.b;
    let m = lab.l - 0.105_561_345_8 * lab.a - 0.063_854_172_8 * lab.b;
    let s = lab.l - 0.089_484_177_5 * lab.a - 1.291_485_548 * lab.b;

    let l = l.powi(3);
    let m = m.powi(3);
    let s = s.powi(3);

    LinearRgb {
        r: 4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        g: -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        b: -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    }
}

/// Computes WCAG-style relative luminance from linear sRGB.
pub fn relative_luminance(rgb: LinearRgb) -> f64 {
    let r = rgb.r.max(0.0);
    let g = rgb.g.max(0.0);
    let b = rgb.b.max(0.0);
    (0.2126 * r + 0.7152 * g + 0.0722 * b).max(0.0)
}

/// Contrast ratio from two relative luminance values.
pub fn contrast_ratio(y1: f64, y2: f64) -> f64 {
    let hi = y1.max(y2);
    let lo = y1.min(y2);
    (hi + 0.05) / (lo + 0.05)
}
