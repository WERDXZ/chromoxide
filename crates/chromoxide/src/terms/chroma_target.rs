//! Unary chroma target term.

use crate::term::{ChromaTargetTerm, EvalContext, TermEvaluation};
use crate::util::eval_scalar_target;

const DEFAULT_HINGE_DELTA: f64 = 0.02;

/// Evaluates a unary chroma target.
pub fn evaluate(term: &ChromaTargetTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let v = ctx.slots_lch[term.slot].c;
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
