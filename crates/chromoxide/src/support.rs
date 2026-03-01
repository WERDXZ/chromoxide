//! Support sample abstractions.

use crate::color::Oklab;

/// Weighted support sample in Oklab with optional saliency metadata.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct WeightedSample {
    /// Sample color.
    pub lab: Oklab,
    /// Relative sample weight (frequency, mass, or prior support).
    pub weight: f64,
    /// Normalized saliency in `[0, 1]`.
    pub saliency: f64,
}

impl WeightedSample {
    /// Creates a weighted sample, clamping saliency to `[0, 1]`.
    ///
    /// Full semantic validation still happens in `PaletteProblem::validate`.
    pub fn new(lab: Oklab, weight: f64, saliency: f64) -> Self {
        debug_assert!(lab.l.is_finite() && lab.a.is_finite() && lab.b.is_finite());
        debug_assert!(weight.is_finite());
        Self {
            lab,
            weight,
            saliency: saliency.clamp(0.0, 1.0),
        }
    }
}
