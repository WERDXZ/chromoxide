//! Solution diagnostics.

use crate::color::{Oklab, Oklch};

/// Breakdown for a single term contribution.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TermBreakdown {
    /// Name of term.
    pub name: String,
    /// Unweighted value.
    pub raw: f64,
    /// Weighted contribution to objective.
    pub weighted: f64,
    /// Optional sub-components.
    pub components: Vec<f64>,
}

/// Per-slot diagnostics.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct SlotDiagnostics {
    /// Slot name.
    pub name: String,
    /// Final Oklab color.
    pub final_lab: Oklab,
    /// Final OkLCh color.
    pub final_lch: Oklch,
    /// Relative luminance.
    pub relative_luminance: f64,
    /// Estimated saliency at final color.
    pub estimated_saliency: f64,
    /// Whether chroma is close to cap.
    pub near_chroma_cap: bool,
    /// Cap margin (`cap - C`) if cap available.
    pub cap_margin: Option<f64>,
}

/// Per-seed optimization diagnostics.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct SeedDiagnostics {
    /// Seed index.
    pub seed_index: usize,
    /// Final objective.
    pub objective: f64,
    /// Solver converged state.
    pub converged: bool,
    /// Iteration count.
    pub iterations: u64,
}

/// Global solver diagnostics.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct SolverDiagnostics {
    /// Number of seeds executed.
    pub seed_count: usize,
    /// Index of best seed.
    pub best_seed_index: usize,
    /// Whether best run converged.
    pub converged: bool,
    /// Iteration count of best run.
    pub iterations: u64,
    /// L2 norm of best final gradient if available.
    pub final_gradient_norm: Option<f64>,
    /// Per-seed diagnostics.
    pub seed_runs: Vec<SeedDiagnostics>,
}

/// Final palette solution.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PaletteSolution {
    /// Final colors in Oklab.
    pub colors: Vec<Oklab>,
    /// Final colors in OkLCh.
    pub colors_lch: Vec<Oklch>,
    /// Objective value.
    pub objective: f64,
    /// Best seed index.
    pub seed_index: usize,
    /// Best run convergence status.
    pub converged: bool,
    /// Term-wise loss breakdown.
    pub term_breakdown: Vec<TermBreakdown>,
    /// Per-slot diagnostics.
    pub slot_diagnostics: Vec<SlotDiagnostics>,
    /// Solver diagnostics.
    pub solver_diagnostics: SolverDiagnostics,
}
