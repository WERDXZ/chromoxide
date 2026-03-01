//! Prepared image data model.

/// Preprocessed per-pixel data used by later pipeline stages.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug)]
pub struct PreparedPixel {
    /// Pixel color in Oklab.
    pub lab: chromoxide::Oklab,
    /// Pixel color in linear sRGB.
    pub lin_rgb: [f64; 3],
    /// Relative luminance (`Y`).
    pub luminance: f64,
    /// Effective alpha mass used by export (`1.0` when alpha weighting is disabled).
    pub alpha: f64,
}

/// Working image representation after preprocessing.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PreparedImage {
    /// Working image width.
    pub width: u32,
    /// Working image height.
    pub height: u32,
    /// Row-major prepared pixels; length is `width * height`.
    pub pixels: Vec<PreparedPixel>,
    /// Indices of valid pixels in `pixels`.
    pub valid_indices: Vec<usize>,
}
