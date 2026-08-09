//! Cover term.

use crate::term::{CoverTerm, EvalContext, TermEvaluation};
use crate::util::{pseudo_huber, soft_assignment_expected_value};

/// Evaluates cover term.
pub fn evaluate(term: &CoverTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    if term.slots.is_empty() || ctx.samples.is_empty() {
        return TermEvaluation::default();
    }

    let mut raw = 0.0;
    let mut mean_soft_dist = 0.0;
    let mut n = 0.0;

    let mut dist_buffer = vec![0.0; term.slots.len()];
    for sample in ctx.samples {
        for (j, &slot_idx) in term.slots.iter().enumerate() {
            dist_buffer[j] = ctx.slots_lab[slot_idx].distance2(sample.lab);
        }
        let d2 = soft_assignment_expected_value(&dist_buffer, term.tau);
        raw += sample.weight.max(0.0) * pseudo_huber(d2, term.delta);
        mean_soft_dist += d2;
        n += 1.0;
    }

    TermEvaluation {
        raw,
        components: vec![if n > 0.0 { mean_soft_dist / n } else { 0.0 }],
    }
}

#[cfg(test)]
mod tests {
    use crate::color::{Oklab, Oklch};
    use crate::support::WeightedSample;
    use crate::term::{CoverTerm, EvalContext, TermEvaluation};
    use crate::terms::cover::evaluate;

    fn ctx_for_slots(labs: &[Oklab]) -> EvalContext<'static> {
        let lchs: Vec<Oklch> = labs.iter().copied().map(Oklch::from_oklab).collect();
        let labs = Box::leak(labs.to_vec().into_boxed_slice());
        let lchs = Box::leak(lchs.into_boxed_slice());
        let luminance = Box::leak(vec![0.5; labs.len()].into_boxed_slice());
        let gates = Box::leak(vec![1.0; labs.len()].into_boxed_slice());
        let lower = Box::leak(vec![0.0; labs.len()].into_boxed_slice());
        let upper = Box::leak(vec![0.2; labs.len()].into_boxed_slice());
        let user_lower = Box::leak(vec![0.0; labs.len()].into_boxed_slice());
        let user_upper = Box::leak(vec![0.2; labs.len()].into_boxed_slice());
        let cap_bounds = Box::leak(vec![None; labs.len()].into_boxed_slice());
        EvalContext {
            slots_lab: labs,
            slots_lch: lchs,
            luminance,
            hue_gates: gates,
            chroma_lower_bounds: lower,
            chroma_upper_bounds: upper,
            user_chroma_lower_bounds: user_lower,
            user_chroma_upper_bounds: user_upper,
            effective_chroma_lower_bounds: lower,
            effective_chroma_upper_bounds: upper,
            image_cap_chroma_upper_bounds: cap_bounds,
            adaptive_image_cap_chroma_upper_bounds: cap_bounds,
            samples: &[],
        }
    }

    #[test]
    fn cover_raw_loss_is_zero_when_sample_matches_both_slots() {
        let sample_lab = Oklab {
            l: 0.5,
            a: 0.1,
            b: 0.05,
        };
        let samples = vec![WeightedSample::new(sample_lab, 2.0, 0.8)];
        let labs = vec![sample_lab, sample_lab];
        let ctx = ctx_for_slots(&labs);
        let ctx = EvalContext {
            samples: &samples,
            ..ctx
        };
        let term = CoverTerm {
            slots: vec![0, 1],
            tau: 0.02,
            delta: 0.03,
        };
        let eval: TermEvaluation = evaluate(&term, &ctx);
        assert!(eval.raw.abs() < 1.0e-10);
    }
}
