//! Cover term.

use crate::term::{CoverTerm, EvalContext, TermEvaluation};
use crate::util::{pseudo_huber, softmin};

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
        let d2 = softmin(&dist_buffer, term.tau);
        raw += sample.weight.max(0.0) * pseudo_huber(d2, term.delta);
        mean_soft_dist += d2;
        n += 1.0;
    }

    TermEvaluation {
        raw,
        components: vec![if n > 0.0 { mean_soft_dist / n } else { 0.0 }],
    }
}
