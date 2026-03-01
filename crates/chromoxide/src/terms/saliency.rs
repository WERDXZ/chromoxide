//! Saliency-field term.

use crate::color::Oklab;
use crate::term::{EvalContext, SaliencyTarget, SaliencyTerm, TermEvaluation};
use crate::util::{EPS, pseudo_huber, relu};

/// Estimates saliency at a color by RBF kernel regression.
pub fn estimate_saliency_at(
    lab: Oklab,
    samples: &[crate::support::WeightedSample],
    sigma: f64,
) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sigma2 = (sigma * sigma).max(EPS);
    let inv_2sigma2 = 0.5 / sigma2;

    let mut num = 0.0;
    let mut den = 0.0;
    for s in samples {
        let d2 = lab.distance2(s.lab);
        let k = (-d2 * inv_2sigma2).exp();
        num += s.saliency.clamp(0.0, 1.0) * k;
        den += k;
    }
    if den <= EPS {
        0.0
    } else {
        (num / den).clamp(0.0, 1.0)
    }
}

/// Evaluates saliency term.
pub fn evaluate(term: &SaliencyTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let saliency = estimate_saliency_at(
        ctx.slots_lab[term.slot],
        ctx.samples,
        term.sigma.max(1.0e-6),
    );
    let raw = match term.target {
        SaliencyTarget::Min(v) => pseudo_huber(relu(v - saliency), 0.05),
        SaliencyTarget::Max(v) => pseudo_huber(relu(saliency - v), 0.05),
        SaliencyTarget::Range { min, max } => {
            pseudo_huber(relu(min - saliency), 0.05) + pseudo_huber(relu(saliency - max), 0.05)
        }
        SaliencyTarget::Target { value, delta } => {
            pseudo_huber(saliency - value, delta.max(1.0e-4))
        }
    };

    TermEvaluation {
        raw,
        components: vec![saliency],
    }
}
