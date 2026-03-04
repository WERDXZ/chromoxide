//! Partial solver config and resolution.

use std::num::{NonZeroU64, NonZeroUsize};

use chromoxide::{CapInterpolation, GradientMode, SolveConfig};
use serde::{Deserialize, Serialize};

/// Partial solver settings used by config and palette files.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PartialSolveConfig {
    pub seed_count: Option<usize>,
    pub max_iters: Option<u64>,
    pub gradient_mode: Option<GradientMode>,
    pub fd_epsilon: Option<f64>,
    pub keep_top_k: Option<usize>,
    pub convergence_ftol: Option<f64>,
    pub convergence_gtol: Option<f64>,
    pub cap_interpolation: Option<CapInterpolation>,
}

impl PartialSolveConfig {
    /// Resolve into a concrete config.
    ///
    /// Precedence: `self` > `fallback` > `SolveConfig::default()`.
    pub fn resolve_over(&self, fallback: &PartialSolveConfig) -> Result<SolveConfig, Error> {
        let default = SolveConfig::default();

        Ok(SolveConfig {
            seed_count: resolve_nonzero_usize(
                self.seed_count.or(fallback.seed_count),
                default.seed_count,
                "seed_count",
            )?,
            max_iters: resolve_nonzero_u64(
                self.max_iters.or(fallback.max_iters),
                default.max_iters,
                "max_iters",
            )?,
            gradient_mode: self
                .gradient_mode
                .or(fallback.gradient_mode)
                .unwrap_or(default.gradient_mode),
            fd_epsilon: self
                .fd_epsilon
                .or(fallback.fd_epsilon)
                .unwrap_or(default.fd_epsilon),
            keep_top_k: resolve_nonzero_usize(
                self.keep_top_k.or(fallback.keep_top_k),
                default.keep_top_k,
                "keep_top_k",
            )?,
            convergence_ftol: self
                .convergence_ftol
                .or(fallback.convergence_ftol)
                .unwrap_or(default.convergence_ftol),
            convergence_gtol: self
                .convergence_gtol
                .or(fallback.convergence_gtol)
                .unwrap_or(default.convergence_gtol),
            cap_interpolation: self
                .cap_interpolation
                .or(fallback.cap_interpolation)
                .unwrap_or(default.cap_interpolation),
        })
    }
}

fn resolve_nonzero_usize(
    value: Option<usize>,
    default: NonZeroUsize,
    field: &'static str,
) -> Result<NonZeroUsize, Error> {
    match value {
        Some(0) => Err(Error::MustBeNonZero { field }),
        Some(v) => NonZeroUsize::new(v).ok_or(Error::MustBeNonZero { field }),
        None => Ok(default),
    }
}

fn resolve_nonzero_u64(
    value: Option<u64>,
    default: NonZeroU64,
    field: &'static str,
) -> Result<NonZeroU64, Error> {
    match value {
        Some(0) => Err(Error::MustBeNonZero { field }),
        Some(v) => NonZeroU64::new(v).ok_or(Error::MustBeNonZero { field }),
        None => Ok(default),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("`{field}` must be > 0")]
    MustBeNonZero { field: &'static str },
}

#[cfg(test)]
mod tests {
    use super::PartialSolveConfig;

    #[test]
    fn resolves_with_precedence_self_then_fallback_then_default() {
        let palette = PartialSolveConfig {
            seed_count: Some(24),
            max_iters: None,
            gradient_mode: None,
            fd_epsilon: Some(9.0e-5),
            keep_top_k: None,
            convergence_ftol: None,
            convergence_gtol: None,
            cap_interpolation: None,
        };

        let global = PartialSolveConfig {
            seed_count: Some(12),
            max_iters: Some(250),
            gradient_mode: None,
            fd_epsilon: Some(2.0e-4),
            keep_top_k: Some(4),
            convergence_ftol: None,
            convergence_gtol: None,
            cap_interpolation: None,
        };

        let resolved = palette
            .resolve_over(&global)
            .expect("config should resolve");
        assert_eq!(resolved.seed_count.get(), 24);
        assert_eq!(resolved.max_iters.get(), 250);
        assert_eq!(resolved.fd_epsilon, 9.0e-5);
        assert_eq!(resolved.keep_top_k.get(), 4);
    }

    #[test]
    fn rejects_zero_nonzero_fields() {
        let invalid = PartialSolveConfig {
            seed_count: Some(0),
            ..Default::default()
        };
        let err = invalid
            .resolve_over(&PartialSolveConfig::default())
            .expect_err("zero should fail");
        assert_eq!(err.to_string(), "`seed_count` must be > 0");
    }
}
