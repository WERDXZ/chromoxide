//! Support prior term.

use crate::term::{EvalContext, SupportTerm, TermEvaluation};
use crate::util::softmin;

/// Evaluates support prior term.
pub fn evaluate(term: &SupportTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    if term.slots.is_empty() || ctx.samples.is_empty() {
        return TermEvaluation::default();
    }

    let mut raw = 0.0;
    let mut mean_score = 0.0;
    let mut count = 0.0;

    let mut buffer = vec![0.0; ctx.samples.len()];
    for &slot_idx in &term.slots {
        for (k, sample) in ctx.samples.iter().enumerate() {
            let dist2 = ctx.slots_lab[slot_idx].distance2(sample.lab);
            let prior = term.beta * (sample.weight.max(0.0) + term.epsilon).ln();
            buffer[k] = dist2 - prior;
        }
        let score = softmin(&buffer, term.tau);
        raw += score;
        mean_score += score;
        count += 1.0;
    }

    TermEvaluation {
        raw,
        components: vec![if count > 0.0 { mean_score / count } else { 0.0 }],
    }
}
