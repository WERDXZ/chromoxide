//! Pairwise delta terms.

use crate::term::{
    DeltaCTarget, DeltaHTarget, DeltaLTarget, EvalContext, PairDeltaCTerm, PairDeltaHTerm,
    PairDeltaLTerm, TermEvaluation,
};
use crate::util::{circular_hue_distance, pseudo_huber, relu};

/// Evaluates Delta-L pair term.
pub fn evaluate_delta_l(term: &PairDeltaLTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let v = (ctx.slots_lch[term.a].l - ctx.slots_lch[term.b].l).abs();
    let raw = penalty_l(v, &term.target);
    TermEvaluation {
        raw,
        components: vec![v],
    }
}

/// Evaluates Delta-C pair term.
pub fn evaluate_delta_c(term: &PairDeltaCTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let v = (ctx.slots_lch[term.a].c - ctx.slots_lch[term.b].c).abs();
    let raw = penalty_c(v, &term.target);
    TermEvaluation {
        raw,
        components: vec![v],
    }
}

/// Evaluates Delta-h pair term with chroma gate.
pub fn evaluate_delta_h(term: &PairDeltaHTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let v = circular_hue_distance(ctx.slots_lch[term.a].h, ctx.slots_lch[term.b].h);
    let gate = ctx.pair_hue_gate(term.a, term.b);
    let raw = penalty_h(v, &term.target) * gate;
    TermEvaluation {
        raw,
        components: vec![v, gate],
    }
}

fn penalty_l(v: f64, target: &DeltaLTarget) -> f64 {
    match *target {
        DeltaLTarget::Min(min) => pseudo_huber(relu(min - v), 0.03),
        DeltaLTarget::Max(max) => pseudo_huber(relu(v - max), 0.03),
        DeltaLTarget::Range { min, max } => {
            pseudo_huber(relu(min - v), 0.03) + pseudo_huber(relu(v - max), 0.03)
        }
        DeltaLTarget::Target { value, delta } => pseudo_huber(v - value, delta.max(1.0e-6)),
    }
}

fn penalty_c(v: f64, target: &DeltaCTarget) -> f64 {
    match *target {
        DeltaCTarget::Min(min) => pseudo_huber(relu(min - v), 0.03),
        DeltaCTarget::Max(max) => pseudo_huber(relu(v - max), 0.03),
        DeltaCTarget::Range { min, max } => {
            pseudo_huber(relu(min - v), 0.03) + pseudo_huber(relu(v - max), 0.03)
        }
        DeltaCTarget::Target { value, delta } => pseudo_huber(v - value, delta.max(1.0e-6)),
    }
}

fn penalty_h(v: f64, target: &DeltaHTarget) -> f64 {
    match *target {
        DeltaHTarget::Min(min) => pseudo_huber(relu(min - v), 0.08),
        DeltaHTarget::Max(max) => pseudo_huber(relu(v - max), 0.08),
        DeltaHTarget::Range { min, max } => {
            pseudo_huber(relu(min - v), 0.08) + pseudo_huber(relu(v - max), 0.08)
        }
        DeltaHTarget::Target { value, delta } => pseudo_huber(v - value, delta.max(1.0e-6)),
    }
}
