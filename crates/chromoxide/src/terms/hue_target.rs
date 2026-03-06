//! Unary hue target term.

use crate::term::{EvalContext, HueTargetTerm, HueUnaryTarget, TermEvaluation};
use crate::util::{circular_hue_distance, hue_distance_to_arc, pseudo_huber};

/// Evaluates a unary hue target.
pub fn evaluate(term: &HueTargetTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let h = ctx.slots_lch[term.slot].h;
    let distance = match term.target {
        HueUnaryTarget::Target { center, .. } => circular_hue_distance(h, center),
        HueUnaryTarget::ArcPreference { start, end, .. } => hue_distance_to_arc(h, start, end),
    };
    let base = match term.target {
        HueUnaryTarget::Target { delta, .. } => pseudo_huber(distance, delta.max(1.0e-6)),
        HueUnaryTarget::ArcPreference { delta, .. } => pseudo_huber(distance, delta.max(1.0e-6)),
    };
    let gate = if term.use_hue_gate {
        ctx.hue_gates[term.slot]
    } else {
        1.0
    };

    TermEvaluation {
        raw: base * gate,
        components: vec![h, distance, gate],
    }
}
