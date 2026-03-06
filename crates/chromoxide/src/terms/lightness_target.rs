//! Unary lightness target term.

use crate::term::{EvalContext, LightnessTargetTerm, TermEvaluation};
use crate::util::eval_scalar_target;

const DEFAULT_HINGE_DELTA: f64 = 0.03;

/// Evaluates a unary lightness target.
pub fn evaluate(term: &LightnessTargetTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let v = ctx.slots_lch[term.slot].l;
    let raw = eval_scalar_target(
        v,
        &term.target,
        term.hinge_delta.unwrap_or(DEFAULT_HINGE_DELTA),
    );
    TermEvaluation {
        raw,
        components: vec![v],
    }
}
