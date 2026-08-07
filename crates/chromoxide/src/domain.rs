//! Slot domain definitions.

use crate::error::PaletteError;
use crate::util::{EPS, sigmoid, wrap_hue};

/// Closed scalar interval.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    /// Minimum bound.
    pub min: f64,
    /// Maximum bound.
    pub max: f64,
}

impl Interval {
    /// Returns interval size.
    pub fn span(self) -> f64 {
        self.max - self.min
    }

    /// Returns midpoint.
    pub fn midpoint(self) -> f64 {
        0.5 * (self.min + self.max)
    }

    /// Checks if value is in interval.
    pub fn contains(self, v: f64) -> bool {
        v >= self.min && v <= self.max
    }

    /// Maps unconstrained latent to interval using sigmoid.
    pub fn decode(self, latent: f64) -> f64 {
        self.min + sigmoid(latent) * self.span()
    }

    /// Validates interval.
    ///
    /// Zero-span intervals (`min == max`) are allowed and represent pinned values.
    pub fn validate(self, name: &str) -> Result<(), PaletteError> {
        if !self.min.is_finite() || !self.max.is_finite() {
            return Err(PaletteError::InvalidDomain(format!(
                "{name} interval has non-finite bounds"
            )));
        }
        if self.max < self.min {
            return Err(PaletteError::InvalidDomain(format!(
                "{name} interval has min > max"
            )));
        }
        Ok(())
    }
}

/// Hue admissible range.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HueDomain {
    /// Entire hue circle.
    Any,
    /// Counter-clockwise circular arc from `start` with explicit length `len` (radians).
    ///
    /// This representation avoids `start/end` ambiguity when crossing `0`.
    Arc {
        /// Arc start angle in radians.
        start: f64,
        /// Arc length in radians, in `(0, 2π]`.
        len: f64,
    },
}

impl HueDomain {
    /// Checks if hue is inside the domain.
    ///
    /// For `Arc { start, len }`, containment is measured along the counter-clockwise
    /// direction from `start` over an angular extent `len`.
    pub fn contains(self, hue: f64) -> bool {
        match self {
            HueDomain::Any => true,
            HueDomain::Arc { start, len } => {
                let start = wrap_hue(start);
                let h = wrap_hue(hue);
                if len <= EPS {
                    return false;
                }
                let d = wrap_hue(h - start);
                d <= len + 1.0e-10
            }
        }
    }

    /// Returns arc length when arc-constrained.
    pub fn arc_len(self) -> Option<f64> {
        match self {
            HueDomain::Any => None,
            HueDomain::Arc { len, .. } => Some(len),
        }
    }

    /// Decodes unconstrained hue latent to domain hue.
    ///
    /// - `Any`: wraps latent directly into `[0, 2π)`
    /// - `Arc`: maps through sigmoid to `[0, len]` and offsets by `start`
    pub fn decode(self, latent_h: f64) -> f64 {
        match self {
            HueDomain::Any => wrap_hue(latent_h),
            HueDomain::Arc { start, len } => {
                let s = wrap_hue(start);
                wrap_hue(s + sigmoid(latent_h) * len)
            }
        }
    }
}

/// Chroma cap policy for slot-domain coupling with image cap.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CapPolicy {
    /// Ignore image cap.
    Ignore,
    /// Hard intersection with cap in decode.
    HardIntersect,
    /// Build a statistical cap from image samples and penalize overflow beyond it.
    ///
    /// This replaces the old soft-penalty mode with a more image-faithful cap surface.
    Statistical {
        /// Weighted percentile used per `(L, h)` cell.
        ///
        /// Typical values are in `(0, 1]`, with `0.95` being a good default.
        percentile: f64,
        /// Headroom multiplier applied after percentile estimation.
        ///
        /// `0.0` keeps the raw statistical cap; `0.12` allows 12% more chroma.
        tolerance_factor: f64,
        /// Smoothing blend in `[0, 1]` between unsmoothed and smoothed cap grids.
        smoothing: f64,
        /// Restrict low-mass hue bins at a given lightness before hole filling.
        use_conditional_hue: bool,
        /// Penalty weight applied when solved chroma exceeds the tolerant cap.
        ///
        /// This preserves the old soft-cap ability to trade off against other terms.
        penalty_weight: f64,
    },
}

impl CapPolicy {
    /// Default statistical cap settings for image-faithful but non-brittle solves.
    pub fn statistical_default() -> Self {
        Self::Statistical {
            percentile: 0.95,
            tolerance_factor: 0.12,
            smoothing: 1.0,
            use_conditional_hue: true,
            penalty_weight: 1.0,
        }
    }
}

/// Slot-level hard domain constraints.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotDomain {
    /// Lightness interval.
    pub lightness: Interval,
    /// Chroma interval.
    pub chroma: Interval,
    /// Hue domain.
    pub hue: HueDomain,
    /// Cap policy.
    pub cap_policy: CapPolicy,
    /// Threshold below which hue terms are attenuated.
    ///
    /// Larger values suppress hue-sensitive terms over a wider low-chroma region.
    pub chroma_epsilon: f64,
}

impl SlotDomain {
    /// Validates domain settings.
    ///
    /// Validation checks finite bounds, non-empty intervals, legal hue arcs,
    /// and cap-policy parameters.
    pub fn validate(self) -> Result<(), PaletteError> {
        self.lightness.validate("lightness")?;
        self.chroma.validate("chroma")?;
        if self.chroma.min < 0.0 {
            return Err(PaletteError::InvalidDomain(
                "chroma min must be >= 0".to_string(),
            ));
        }
        if self.chroma_epsilon < 0.0 || !self.chroma_epsilon.is_finite() {
            return Err(PaletteError::InvalidDomain(
                "chroma_epsilon must be finite and >= 0".to_string(),
            ));
        }

        if let HueDomain::Arc { start, len } = self.hue {
            if !start.is_finite() || !len.is_finite() {
                return Err(PaletteError::InvalidDomain(
                    "hue arc must be finite".to_string(),
                ));
            }
            if len <= EPS {
                return Err(PaletteError::InvalidDomain(
                    "hue arc must have positive length".to_string(),
                ));
            }
            if len > std::f64::consts::TAU + 1.0e-10 {
                return Err(PaletteError::InvalidDomain(
                    "hue arc length must be <= 2π".to_string(),
                ));
            }
        }

        if let CapPolicy::Statistical {
            percentile,
            tolerance_factor,
            smoothing,
            use_conditional_hue: _,
            penalty_weight,
        } = self.cap_policy
        {
            if !percentile.is_finite() || !(0.0..=1.0).contains(&percentile) || percentile <= 0.0 {
                return Err(PaletteError::InvalidDomain(
                    "statistical cap percentile must be finite and in (0, 1]".to_string(),
                ));
            }
            if !tolerance_factor.is_finite() || tolerance_factor < 0.0 {
                return Err(PaletteError::InvalidDomain(
                    "statistical cap tolerance_factor must be finite and >= 0".to_string(),
                ));
            }
            if !smoothing.is_finite() || !(0.0..=1.0).contains(&smoothing) {
                return Err(PaletteError::InvalidDomain(
                    "statistical cap smoothing must be finite and in [0, 1]".to_string(),
                ));
            }
            if !penalty_weight.is_finite() || penalty_weight < 0.0 {
                return Err(PaletteError::InvalidDomain(
                    "statistical cap penalty_weight must be finite and >= 0".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// True when slot is effectively near-neutral by domain.
    pub fn is_neutralish(self) -> bool {
        self.chroma.max <= self.chroma.min + 0.06
    }
}
