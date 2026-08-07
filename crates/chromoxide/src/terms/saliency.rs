//! Saliency-field term.

use crate::color::Oklab;
use crate::support::WeightedSample;
use crate::term::DEFAULT_SALIENCY_SUPPORT_SCALE;
use crate::term::{EvalContext, SaliencyTarget, SaliencyTerm, TermEvaluation};
use crate::util::{EPS, pseudo_huber, relu};

/// Component breakdown of a saliency estimate.
#[derive(Clone, Copy, Debug, Default)]
pub struct SaliencyEstimate {
    /// Conditional saliency (weighted RBF regression), clamped to `[0, 1]`.
    pub conditional: f64,
    /// Normalized support density at the query color, clamped to `[0, 1]`.
    pub normalized_density: f64,
    /// Support-density gate in `[0, 1]`.
    pub gate: f64,
    /// `conditional * gate`, clamped to `[0, 1]`.
    pub effective: f64,
}

/// Estimates saliency components at a color using mass-weighted RBF regression
/// with a support-density gate.
pub fn estimate_saliency_components_at(
    lab: Oklab,
    samples: &[WeightedSample],
    sigma: f64,
    support_scale: f64,
) -> SaliencyEstimate {
    let sigma2 = (sigma * sigma).max(EPS);
    let inv_2sigma2 = 0.5 / sigma2;

    let total_mass: f64 = samples
        .iter()
        .map(|s| s.weight.max(0.0))
        .fold(0.0, |acc, w| acc + w);

    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for s in samples {
        let d2 = lab.distance2(s.lab);
        let kernel = (-d2 * inv_2sigma2).exp();
        let weighted_kernel = s.weight.max(0.0) * kernel;
        let saliency = if s.saliency.is_finite() {
            s.saliency.clamp(0.0, 1.0)
        } else {
            0.0
        };
        numerator += weighted_kernel * saliency;
        denominator += weighted_kernel;
    }

    let conditional = if denominator <= EPS {
        0.0
    } else {
        (numerator / denominator).clamp(0.0, 1.0)
    };
    let normalized_density = if total_mass <= EPS {
        0.0
    } else {
        (denominator / total_mass).clamp(0.0, 1.0)
    };
    let scale = support_scale.max(EPS);
    let gate = (1.0 - (-normalized_density / scale).exp()).clamp(0.0, 1.0);
    let effective = (conditional * gate).clamp(0.0, 1.0);

    SaliencyEstimate {
        conditional,
        normalized_density,
        gate,
        effective,
    }
}

/// Estimates effective saliency at a color.
///
/// This wrapper preserves the original API and returns the effective
/// (density-gated) saliency using the default support scale.
pub fn estimate_saliency_at(lab: Oklab, samples: &[WeightedSample], sigma: f64) -> f64 {
    estimate_saliency_components_at(lab, samples, sigma, DEFAULT_SALIENCY_SUPPORT_SCALE).effective
}

/// Evaluates saliency term.
pub fn evaluate(term: &SaliencyTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let estimate = estimate_saliency_components_at(
        ctx.slots_lab[term.slot],
        ctx.samples,
        term.sigma.max(1.0e-6),
        term.support_scale,
    );
    let saliency = estimate.effective;
    let hinge_delta = term.hinge_delta.unwrap_or(0.05);
    let raw = match term.target {
        SaliencyTarget::Min(v) => pseudo_huber(relu(v - saliency), hinge_delta),
        SaliencyTarget::Max(v) => pseudo_huber(relu(saliency - v), hinge_delta),
        SaliencyTarget::Range { min, max } => {
            pseudo_huber(relu(min - saliency), hinge_delta)
                + pseudo_huber(relu(saliency - max), hinge_delta)
        }
        SaliencyTarget::Target { value, delta } => {
            pseudo_huber(saliency - value, delta.max(1.0e-4))
        }
    };

    TermEvaluation {
        raw,
        components: vec![
            estimate.effective,
            estimate.conditional,
            estimate.normalized_density,
            estimate.gate,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::{SaliencyEstimate, estimate_saliency_components_at};
    use crate::color::Oklab;
    use crate::support::WeightedSample;

    fn sample_at(lab: Oklab, weight: f64, saliency: f64) -> WeightedSample {
        WeightedSample::new(lab, weight, saliency)
    }

    #[test]
    fn single_sample_at_own_color_is_effectively_one() {
        let lab = Oklab {
            l: 0.6,
            a: 0.1,
            b: 0.05,
        };
        let samples = vec![sample_at(lab, 1.0, 1.0)];
        let est = estimate_saliency_components_at(lab, &samples, 0.08, 0.05);
        assert!(est.effective > 0.99);
        assert!(est.conditional > 0.99);
        assert!(est.normalized_density > 0.99);
    }

    #[test]
    fn far_query_is_suppressed_below_threshold() {
        let lab = Oklab {
            l: 0.6,
            a: 0.1,
            b: 0.05,
        };
        let samples = vec![sample_at(lab, 1.0, 1.0)];
        let far = Oklab {
            l: lab.l + 0.5,
            a: lab.a,
            b: lab.b,
        };
        let est = estimate_saliency_components_at(far, &samples, 0.08, 0.05);
        assert!(est.effective < 1.0e-3);
    }

    #[test]
    fn conditional_is_mass_weighted_not_count_weighted() {
        let lab = Oklab {
            l: 0.6,
            a: 0.1,
            b: 0.05,
        };
        let samples = vec![sample_at(lab, 9.0, 0.0), sample_at(lab, 1.0, 1.0)];
        let est = estimate_saliency_components_at(lab, &samples, 0.08, 0.05);
        assert!((est.conditional - 0.1).abs() < 1.0e-12);
    }

    #[test]
    fn normalized_density_is_bounded() {
        let base = Oklab {
            l: 0.6,
            a: 0.1,
            b: 0.05,
        };
        let samples = vec![
            sample_at(base, 2.0, 0.3),
            sample_at(
                Oklab {
                    l: 0.7,
                    a: 0.0,
                    b: 0.0,
                },
                3.0,
                0.9,
            ),
        ];
        for offset in [0.0, 0.05, 0.2, 0.8] {
            let query = Oklab {
                l: base.l + offset,
                a: base.a,
                b: base.b,
            };
            let est = estimate_saliency_components_at(query, &samples, 0.08, 0.05);
            assert!((0.0..=1.0).contains(&est.normalized_density));
        }
    }

    #[test]
    fn empty_samples_produce_zero_components() {
        let est = estimate_saliency_components_at(
            Oklab {
                l: 0.5,
                a: 0.0,
                b: 0.0,
            },
            &[],
            0.08,
            0.05,
        );
        let zero: SaliencyEstimate = SaliencyEstimate::default();
        assert_eq!(est.conditional, zero.conditional);
        assert_eq!(est.normalized_density, zero.normalized_density);
        assert_eq!(est.gate, zero.gate);
        assert_eq!(est.effective, zero.effective);
    }
}
