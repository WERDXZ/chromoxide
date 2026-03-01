//! Error types.

use thiserror::Error;

/// Error for problem construction, evaluation, and solving.
#[derive(Debug, Error)]
pub enum PaletteError {
    /// Domain configuration is invalid.
    #[error("invalid domain: {0}")]
    InvalidDomain(String),

    /// Group quantile term is invalid.
    #[error("invalid group term: {0}")]
    InvalidGroupTerm(String),

    /// Generic invalid input problem.
    #[error("invalid problem: {0}")]
    InvalidProblem(String),

    /// Empty support sample set.
    #[error("empty samples")]
    EmptySamples,

    /// Empty slot list.
    #[error("empty slots")]
    EmptySlots,

    /// No feasible chroma cap region.
    #[error("empty feasible image cap")]
    EmptyFeasibleCap,

    /// Numerical instability during objective/gradient evaluation.
    #[error("numeric instability: {0}")]
    NumericInstability(String),

    /// Solver-level failure.
    #[error("solver failure: {0}")]
    SolverFailure(String),
}
