//! Pairwise order relation term.

use crate::term::{EvalContext, OrderRelation, PairOrderTerm, TermEvaluation};
use crate::util::{pseudo_huber, relu};

/// Evaluates pairwise order term.
pub fn evaluate(term: &PairOrderTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let la = ctx.slots_lch[term.a].l;
    let lb = ctx.slots_lch[term.b].l;
    let hinge_delta = term.hinge_delta.unwrap_or(0.03);

    let raw = match term.relation {
        OrderRelation::BrighterBy { delta } => pseudo_huber(relu(delta - (la - lb)), hinge_delta),
        OrderRelation::DarkerBy { delta } => pseudo_huber(relu(delta - (lb - la)), hinge_delta),
    };

    TermEvaluation {
        raw,
        components: vec![la - lb],
    }
}
