//! Pairwise Oklab distance term.

use crate::term::{EvalContext, PairDistanceTerm, TermEvaluation};
use crate::util::eval_scalar_target;

const DEFAULT_DISTANCE_HINGE_DELTA: f64 = 0.03;
const DEFAULT_DISTANCE2_HINGE_DELTA: f64 = 0.002;

/// Evaluates a pairwise Oklab distance target.
pub fn evaluate(term: &PairDistanceTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let d2 = ctx.slots_lab[term.a].distance2(ctx.slots_lab[term.b]);
    let v = if term.squared { d2 } else { d2.sqrt() };
    let default_hinge_delta = if term.squared {
        DEFAULT_DISTANCE2_HINGE_DELTA
    } else {
        DEFAULT_DISTANCE_HINGE_DELTA
    };
    let raw = eval_scalar_target(
        v,
        &term.target,
        term.hinge_delta.unwrap_or(default_hinge_delta),
    );
    TermEvaluation {
        raw,
        components: vec![v, d2],
    }
}
