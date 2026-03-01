//! Contrast-ratio term.

use crate::convert::contrast_ratio;
use crate::term::{ContrastTerm, EvalContext, TermEvaluation};
use crate::util::{pseudo_huber, relu};

/// Evaluates contrast term.
pub fn evaluate(term: &ContrastTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let cr = contrast_ratio(ctx.luminance[term.fg], ctx.luminance[term.bg]);
    let raw = pseudo_huber(relu(term.min_ratio - cr), 0.25);
    TermEvaluation {
        raw,
        components: vec![cr],
    }
}
