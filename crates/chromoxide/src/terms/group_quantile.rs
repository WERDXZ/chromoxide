//! Group mass-quantile distribution term.

use std::f64::consts::TAU;

use crate::error::PaletteError;
use crate::term::{
    EvalContext, GroupAxis, GroupQuantileTerm, GroupTarget, Monotonicity, QuantileKnot,
    TermEvaluation,
};
use crate::util::{EPS, arc_length, pseudo_huber, relu, wrap_hue};

/// Computes mass-quantile centers for ordered masses.
pub fn compute_mass_quantile_centers(masses: &[f64]) -> Result<Vec<f64>, PaletteError> {
    if masses.is_empty() {
        return Err(PaletteError::InvalidGroupTerm(
            "masses must be non-empty".to_string(),
        ));
    }
    let mut total = 0.0;
    for &m in masses {
        if !m.is_finite() || m <= 0.0 {
            return Err(PaletteError::InvalidGroupTerm(
                "all masses must be finite and > 0".to_string(),
            ));
        }
        total += m;
    }
    if total <= EPS {
        return Err(PaletteError::InvalidGroupTerm(
            "sum of masses must be > 0".to_string(),
        ));
    }

    let mut cum = 0.0;
    let mut out = Vec::with_capacity(masses.len());
    for &m in masses {
        cum += m;
        out.push((cum - 0.5 * m) / total);
    }
    Ok(out)
}

/// Computes target values for given quantile centers.
pub fn compute_targets(
    quantile_centers: &[f64],
    target: &GroupTarget,
    expected_len: usize,
) -> Result<Vec<f64>, PaletteError> {
    if quantile_centers.len() != expected_len {
        return Err(PaletteError::InvalidGroupTerm(
            "quantiles length mismatch".to_string(),
        ));
    }

    match target {
        GroupTarget::UniformRange { min, max } => Ok(quantile_centers
            .iter()
            .map(|&q| min + q * (max - min))
            .collect()),
        GroupTarget::ExplicitValues(values) => {
            if values.len() != expected_len {
                return Err(PaletteError::InvalidGroupTerm(
                    "ExplicitValues length must match slots".to_string(),
                ));
            }
            Ok(values.clone())
        }
        GroupTarget::ExplicitQuantiles(knots) => {
            if knots.len() < 2 {
                return Err(PaletteError::InvalidGroupTerm(
                    "ExplicitQuantiles requires at least 2 knots".to_string(),
                ));
            }
            for i in 1..knots.len() {
                if knots[i].quantile < knots[i - 1].quantile {
                    return Err(PaletteError::InvalidGroupTerm(
                        "quantiles must be non-decreasing".to_string(),
                    ));
                }
            }

            let mut out = Vec::with_capacity(expected_len);
            for &q in quantile_centers {
                out.push(interpolate_quantile(q, knots));
            }
            Ok(out)
        }
    }
}

/// Piecewise-linear interpolation for an explicit quantile map.
pub fn interpolate_quantile(q: f64, knots: &[QuantileKnot]) -> f64 {
    if q <= knots[0].quantile {
        return knots[0].value;
    }
    if q >= knots[knots.len() - 1].quantile {
        return knots[knots.len() - 1].value;
    }

    for i in 0..knots.len() - 1 {
        let q0 = knots[i].quantile;
        let q1 = knots[i + 1].quantile;
        if q >= q0 && q <= q1 {
            let t = if (q1 - q0).abs() <= EPS {
                0.0
            } else {
                (q - q0) / (q1 - q0)
            };
            return knots[i].value * (1.0 - t) + knots[i + 1].value * t;
        }
    }
    knots[knots.len() - 1].value
}

/// Evaluates group quantile term.
pub fn evaluate(term: &GroupQuantileTerm, ctx: &EvalContext<'_>) -> TermEvaluation {
    if term.members.is_empty() {
        return TermEvaluation::default();
    }

    let quantiles = match compute_mass_quantile_centers(&member_masses(term)) {
        Ok(v) => v,
        Err(_) => {
            return TermEvaluation {
                raw: 1.0e12,
                components: vec![],
            };
        }
    };
    let targets = match compute_targets(&quantiles, &term.target, term.members.len()) {
        Ok(v) => v,
        Err(_) => {
            return TermEvaluation {
                raw: 1.0e12,
                components: vec![],
            };
        }
    };
    let values = axis_values(term, ctx);

    let delta = term.huber_delta.max(1.0e-6);
    let mut raw = 0.0;
    let mut mean_abs_residual = 0.0;

    for i in 0..term.members.len() {
        let r = values[i] - targets[i];
        raw += pseudo_huber(r, delta);
        mean_abs_residual += r.abs();
    }
    mean_abs_residual /= term.members.len() as f64;

    if let Some(m) = &term.monotonic {
        for i in 0..term.members.len() - 1 {
            let gap = values[i + 1] - values[i];
            let violation = match m {
                Monotonicity::Increasing { min_gap } => relu(*min_gap - gap),
                Monotonicity::Decreasing { min_gap } => relu(*min_gap + gap),
            };
            raw += pseudo_huber(violation, delta);
        }
    }

    TermEvaluation {
        raw,
        components: vec![mean_abs_residual],
    }
}

/// Extracts member masses for quantile-center computation.
fn member_masses(term: &GroupQuantileTerm) -> Vec<f64> {
    term.members.iter().map(|member| member.mass).collect()
}

/// Projects configured members onto the selected axis.
fn axis_values(term: &GroupQuantileTerm, ctx: &EvalContext<'_>) -> Vec<f64> {
    let mut values = Vec::with_capacity(term.members.len());
    for member in &term.members {
        let slot_idx = member.slot;
        let lch = ctx.slots_lch[slot_idx];
        let v = match term.axis {
            GroupAxis::Lightness => lch.l,
            GroupAxis::Chroma => lch.c,
            GroupAxis::HueArc { start, end } => project_hue_to_arc(lch.h, start, end),
        };
        values.push(v);
    }
    values
}

/// Projects hue onto a directed arc, clamping outside values to nearest endpoint.
fn project_hue_to_arc(h: f64, start: f64, end: f64) -> f64 {
    let len = arc_length(start, end);
    if len <= EPS {
        return 0.0;
    }
    let d = wrap_hue(wrap_hue(h) - wrap_hue(start));
    if d <= len {
        d
    } else {
        let to_end = (d - len).abs();
        let to_start = TAU - d;
        if to_end <= to_start { len } else { 0.0 }
    }
}
