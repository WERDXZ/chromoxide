//! Relative chroma target term.

use crate::term::{EvalContext, RelativeChromaTargetTerm, TermEvaluation};
use crate::util::{EPS, eval_scalar_target};

const DEFAULT_HINGE_DELTA: f64 = 0.02;

/// Evaluates a relative chroma target.
///
/// The relative ratio is `(C - lo) / (hi - lo)` clamped to `[0, 1]`. When the
/// effective interval is pinned (`hi - lo <= EPS`) the ratio is defined as `1`.
pub fn evaluate(term: &RelativeChromaTargetTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    let c = ctx.slots_lch[term.slot].c;
    let lo = ctx.chroma_lower_bounds[term.slot];
    let hi = ctx.chroma_upper_bounds[term.slot];

    let span = hi - lo;
    let ratio = if span <= EPS {
        1.0
    } else {
        ((c - lo) / span).clamp(0.0, 1.0)
    };

    let raw = eval_scalar_target(
        ratio,
        &term.target,
        term.hinge_delta.unwrap_or(DEFAULT_HINGE_DELTA),
    );
    TermEvaluation {
        raw,
        components: vec![ratio, c, lo, hi],
    }
}

#[cfg(test)]
mod tests {
    use crate::color::Oklch;
    use crate::term::{EvalContext, RelativeChromaTargetTerm, ScalarTarget};
    use crate::terms::relative_chroma_target::evaluate;

    fn ctx_with_bounds(lch: Oklch, lo: f64, hi: f64) -> EvalContext<'static> {
        let lch_slice = Box::leak(vec![lch].into_boxed_slice());
        let lab_slice = Box::leak(vec![lch.to_oklab()].into_boxed_slice());
        let luminance = Box::leak(vec![0.5].into_boxed_slice());
        let gates = Box::leak(vec![1.0].into_boxed_slice());
        let lower = Box::leak(vec![lo].into_boxed_slice());
        let upper = Box::leak(vec![hi].into_boxed_slice());
        EvalContext {
            slots_lab: lab_slice,
            slots_lch: lch_slice,
            luminance,
            hue_gates: gates,
            chroma_lower_bounds: lower,
            chroma_upper_bounds: upper,
            samples: &[],
        }
    }

    fn target(value: f64) -> RelativeChromaTargetTerm {
        RelativeChromaTargetTerm {
            slot: 0,
            target: ScalarTarget::Target { value, delta: 0.1 },
            hinge_delta: None,
        }
    }

    #[test]
    fn ratio_midpoint_zero_two() {
        let term = target(0.5);
        let ctx = ctx_with_bounds(
            Oklch {
                l: 0.6,
                c: 0.1,
                h: 1.0,
            },
            0.0,
            0.2,
        );
        let eval = evaluate(&term, &ctx);
        assert!((eval.components[0] - 0.5).abs() < 1.0e-12);
        assert!(eval.raw.abs() < 1.0e-10);
    }

    #[test]
    fn ratio_midpoint_one_three() {
        let term = target(0.5);
        let ctx = ctx_with_bounds(
            Oklch {
                l: 0.6,
                c: 0.2,
                h: 1.0,
            },
            0.1,
            0.3,
        );
        let eval = evaluate(&term, &ctx);
        assert!((eval.components[0] - 0.5).abs() < 1.0e-12);
    }

    #[test]
    fn pinned_interval_ratio_is_one_and_finite() {
        let term = target(1.0);
        let ctx = ctx_with_bounds(
            Oklch {
                l: 0.6,
                c: 0.12,
                h: 1.0,
            },
            0.12,
            0.12,
        );
        let eval = evaluate(&term, &ctx);
        assert!((eval.components[0] - 1.0).abs() < 1.0e-12);
        assert!(eval.raw.is_finite());
    }

    #[test]
    fn components_are_ratio_chroma_lo_hi() {
        let term = target(0.5);
        let ctx = ctx_with_bounds(
            Oklch {
                l: 0.6,
                c: 0.05,
                h: 1.0,
            },
            0.0,
            0.1,
        );
        let eval = evaluate(&term, &ctx);
        assert_eq!(eval.components.len(), 4);
        assert!((eval.components[1] - 0.05).abs() < 1.0e-12);
        assert!((eval.components[2] - 0.0).abs() < 1.0e-12);
        assert!((eval.components[3] - 0.1).abs() < 1.0e-12);
    }
}
