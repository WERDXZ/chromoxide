//! Shared numeric utilities.

use std::f64::consts::TAU;

/// Small value used to avoid division by zero and invalid logs.
pub const EPS: f64 = 1.0e-12;

/// Wraps hue in radians to `[0, 2π)`.
pub fn wrap_hue(mut h: f64) -> f64 {
    if !h.is_finite() {
        return 0.0;
    }
    h %= TAU;
    if h < 0.0 {
        h += TAU;
    }
    h
}

/// Positive arc length from `start` to `end` in radians.
pub fn arc_length(start: f64, end: f64) -> f64 {
    wrap_hue(end - start)
}

/// Circular distance between two hue angles in radians.
pub fn circular_hue_distance(h1: f64, h2: f64) -> f64 {
    let d = (wrap_hue(h1) - wrap_hue(h2)).abs();
    d.min(TAU - d)
}

/// Sigmoid function.
pub fn sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        let z = (-x).exp();
        1.0 / (1.0 + z)
    } else {
        let z = x.exp();
        z / (1.0 + z)
    }
}

/// Inverse sigmoid (logit) with clamped input.
pub fn inv_sigmoid(y: f64) -> f64 {
    let y = y.clamp(EPS, 1.0 - EPS);
    (y / (1.0 - y)).ln()
}

/// ReLU.
pub fn relu(x: f64) -> f64 {
    x.max(0.0)
}

/// Pseudo-Huber robust penalty.
pub fn pseudo_huber(z: f64, delta: f64) -> f64 {
    let d = delta.max(EPS);
    let t = z / d;
    d * d * ((1.0 + t * t).sqrt() - 1.0)
}

/// Smoothstep interpolation on `[0, 1]`.
pub fn smoothstep01(x: f64) -> f64 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Numerically stable softmin.
pub fn softmin(values: &[f64], tau: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    if tau <= EPS {
        return values
            .iter()
            .copied()
            .fold(f64::INFINITY, |acc, v| if v < acc { v } else { acc });
    }

    let inv_tau = 1.0 / tau;
    let mut max_scaled = f64::NEG_INFINITY;
    for &v in values {
        let s = -v * inv_tau;
        if s > max_scaled {
            max_scaled = s;
        }
    }

    let mut sum = 0.0;
    for &v in values {
        sum += ((-v * inv_tau) - max_scaled).exp();
    }
    -tau * (sum.ln() + max_scaled)
}

/// L2 norm of a dense vector.
pub fn l2_norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}
