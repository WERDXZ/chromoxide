//! Problem definitions and preflight validation.

use std::num::{NonZeroU64, NonZeroUsize};

use crate::cap::{CapInterpolation, ImageCap};
use crate::domain::{CapPolicy, SlotDomain};
use crate::error::PaletteError;
use crate::support::WeightedSample;
use crate::term::{GroupAxis, Term, WeightedTerm};
use crate::terms::group_quantile::{compute_mass_quantile_centers, compute_targets};

/// Slot specification.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct SlotSpec {
    /// Human-readable slot name.
    pub name: String,
    /// Slot hard domain.
    pub domain: SlotDomain,
}

/// Gradient mode.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug)]
pub enum GradientMode {
    /// Central finite difference.
    FiniteDifferenceCentral,
}

/// Solver configuration.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct SolveConfig {
    /// Number of multi-start seeds.
    ///
    /// Higher values improve robustness at higher compute cost.
    pub seed_count: NonZeroUsize,
    /// Maximum iterations for each local solve.
    ///
    /// Higher values allow harder cases to converge, but increase runtime.
    pub max_iters: NonZeroU64,
    /// Gradient mode.
    pub gradient_mode: GradientMode,
    /// Base finite-difference epsilon used by central finite differences.
    ///
    /// Effective per-dimension epsilon is scaled as `fd_epsilon * max(1, |u_j|)`.
    pub fd_epsilon: f64,
    /// Number of best seeds to keep in diagnostics.
    pub keep_top_k: NonZeroUsize,
    /// Cost tolerance used by L-BFGS convergence checks.
    pub convergence_ftol: f64,
    /// Gradient tolerance used by L-BFGS convergence checks.
    pub convergence_gtol: f64,
    /// Interpolation mode used when querying `c_cap(L, h)`.
    pub cap_interpolation: CapInterpolation,
}

impl Default for SolveConfig {
    fn default() -> Self {
        Self {
            seed_count: NonZeroUsize::new(16).expect("16 is non-zero"),
            max_iters: NonZeroU64::new(180).expect("180 is non-zero"),
            gradient_mode: GradientMode::FiniteDifferenceCentral,
            fd_epsilon: 1.0e-4,
            keep_top_k: NonZeroUsize::new(8).expect("8 is non-zero"),
            convergence_ftol: 1.0e-8,
            convergence_gtol: 1.0e-6,
            cap_interpolation: CapInterpolation::Bilinear,
        }
    }
}

/// Top-level optimization problem.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PaletteProblem {
    /// Slots to optimize.
    pub slots: Vec<SlotSpec>,
    /// Weighted support samples.
    pub samples: Vec<WeightedSample>,
    /// Optional image cap model.
    pub image_cap: Option<ImageCap>,
    /// Weighted objective terms.
    pub terms: Vec<WeightedTerm>,
    /// Solve configuration.
    pub config: SolveConfig,
}

impl PaletteProblem {
    /// Runs preflight validation.
    ///
    /// This catches shape/range/unit errors before optimization starts, including:
    /// slot validity, sample validity, cap requirements, and per-term consistency.
    pub fn validate(&self) -> Result<(), PaletteError> {
        if self.slots.is_empty() {
            return Err(PaletteError::EmptySlots);
        }
        if self.samples.is_empty() {
            return Err(PaletteError::EmptySamples);
        }
        if !self.config.fd_epsilon.is_finite() || self.config.fd_epsilon <= 0.0 {
            return Err(PaletteError::InvalidProblem(
                "fd_epsilon must be finite and > 0".to_string(),
            ));
        }
        if !self.config.convergence_ftol.is_finite() || self.config.convergence_ftol < 0.0 {
            return Err(PaletteError::InvalidProblem(
                "convergence_ftol must be finite and >= 0".to_string(),
            ));
        }
        if !self.config.convergence_gtol.is_finite() || self.config.convergence_gtol < 0.0 {
            return Err(PaletteError::InvalidProblem(
                "convergence_gtol must be finite and >= 0".to_string(),
            ));
        }
        self.config.cap_interpolation.validate()?;

        for slot in &self.slots {
            slot.domain.validate()?;
        }

        for sample in &self.samples {
            if !sample.weight.is_finite() || sample.weight <= 0.0 {
                return Err(PaletteError::InvalidProblem(
                    "all sample weights must be finite and > 0".to_string(),
                ));
            }
            if !sample.saliency.is_finite() {
                return Err(PaletteError::InvalidProblem(
                    "sample saliency must be finite".to_string(),
                ));
            }
        }

        let any_hard_or_soft = self.slots.iter().any(|s| {
            matches!(
                s.domain.cap_policy,
                CapPolicy::HardIntersect | CapPolicy::SoftPenalty { .. }
            )
        });
        if any_hard_or_soft && self.image_cap.is_none() {
            return Err(PaletteError::InvalidProblem(
                "at least one slot requires image_cap but problem.image_cap is None".to_string(),
            ));
        }
        if self
            .slots
            .iter()
            .any(|s| matches!(s.domain.cap_policy, CapPolicy::HardIntersect))
        {
            let cap = self.image_cap.as_ref().ok_or_else(|| {
                PaletteError::InvalidProblem(
                    "HardIntersect requires image_cap to be present".to_string(),
                )
            })?;
            if cap.max_cap() <= 0.0 {
                return Err(PaletteError::EmptyFeasibleCap);
            }
        }

        let n_slots = self.slots.len();
        for wt in &self.terms {
            if !wt.weight.is_finite() || wt.weight < 0.0 {
                return Err(PaletteError::InvalidProblem(
                    "term weights must be finite and >= 0".to_string(),
                ));
            }
            validate_term(&wt.term, n_slots)?;
        }

        Ok(())
    }
}

/// Ensures a slot index is inside `[0, n_slots)`.
fn validate_slot_index(idx: usize, n_slots: usize, field: &str) -> Result<(), PaletteError> {
    if idx >= n_slots {
        return Err(PaletteError::InvalidProblem(format!(
            "{field} index {idx} out of range 0..{}",
            n_slots.saturating_sub(1)
        )));
    }
    Ok(())
}

/// Validates a single term payload against global problem shape/rules.
fn validate_term(term: &Term, n_slots: usize) -> Result<(), PaletteError> {
    match term {
        Term::Cover(t) => {
            if t.slots.is_empty() {
                return Err(PaletteError::InvalidProblem(
                    "CoverTerm.slots cannot be empty".to_string(),
                ));
            }
            for &s in &t.slots {
                validate_slot_index(s, n_slots, "CoverTerm.slot")?;
            }
        }
        Term::Support(t) => {
            if t.slots.is_empty() {
                return Err(PaletteError::InvalidProblem(
                    "SupportTerm.slots cannot be empty".to_string(),
                ));
            }
            for &s in &t.slots {
                validate_slot_index(s, n_slots, "SupportTerm.slot")?;
            }
        }
        Term::Saliency(t) => {
            validate_slot_index(t.slot, n_slots, "SaliencyTerm.slot")?;
        }
        Term::DeltaL(t) => {
            validate_slot_index(t.a, n_slots, "PairDeltaLTerm.a")?;
            validate_slot_index(t.b, n_slots, "PairDeltaLTerm.b")?;
        }
        Term::DeltaC(t) => {
            validate_slot_index(t.a, n_slots, "PairDeltaCTerm.a")?;
            validate_slot_index(t.b, n_slots, "PairDeltaCTerm.b")?;
        }
        Term::DeltaH(t) => {
            validate_slot_index(t.a, n_slots, "PairDeltaHTerm.a")?;
            validate_slot_index(t.b, n_slots, "PairDeltaHTerm.b")?;
        }
        Term::Order(t) => {
            validate_slot_index(t.a, n_slots, "PairOrderTerm.a")?;
            validate_slot_index(t.b, n_slots, "PairOrderTerm.b")?;
        }
        Term::Contrast(t) => {
            validate_slot_index(t.fg, n_slots, "ContrastTerm.fg")?;
            validate_slot_index(t.bg, n_slots, "ContrastTerm.bg")?;
            if !t.min_ratio.is_finite() || t.min_ratio <= 0.0 {
                return Err(PaletteError::InvalidProblem(
                    "ContrastTerm.min_ratio must be finite and > 0".to_string(),
                ));
            }
        }
        Term::GroupQuantile(t) => {
            if t.members.is_empty() {
                return Err(PaletteError::InvalidGroupTerm(
                    "GroupQuantileTerm.members cannot be empty".to_string(),
                ));
            }
            let mut masses = Vec::with_capacity(t.members.len());
            for member in &t.members {
                validate_slot_index(member.slot, n_slots, "GroupQuantileTerm.member.slot")?;
                if !member.mass.is_finite() || member.mass <= 0.0 {
                    return Err(PaletteError::InvalidGroupTerm(
                        "GroupQuantileTerm.member.mass must be finite and > 0".to_string(),
                    ));
                }
                masses.push(member.mass);
            }
            if let GroupAxis::HueArc { start, end } = t.axis {
                let len = crate::util::arc_length(start, end);
                if len <= 0.0 {
                    return Err(PaletteError::InvalidGroupTerm(
                        "HueArc must have positive arc length".to_string(),
                    ));
                }
            }
            let q = compute_mass_quantile_centers(&masses)?;
            let _ = compute_targets(&q, &t.target, t.members.len())?;
            if let Some(m) = &t.monotonic {
                let gap = match m {
                    crate::term::Monotonicity::Increasing { min_gap } => *min_gap,
                    crate::term::Monotonicity::Decreasing { min_gap } => *min_gap,
                };
                if !gap.is_finite() || gap < 0.0 {
                    return Err(PaletteError::InvalidGroupTerm(
                        "min_gap must be finite and >= 0".to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}
