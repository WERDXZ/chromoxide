//! Saliency map computation.

use crate::config::{GlobalContrastConfig, LocalContrastConfig, SaliencyConfig, SaliencyMethod};
use crate::error::ImagePipelineError;
use crate::prepared::PreparedImage;
use crate::util::{EPSILON, checked_len, clamp01, lab_distance2, percentile};

/// Saliency map aligned with a prepared image.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct SaliencyMap {
    /// Map width.
    pub width: u32,
    /// Map height.
    pub height: u32,
    /// Row-major saliency values in `[0, 1]`.
    pub values: Vec<f64>,
}

/// Computes saliency map for a prepared image.
pub fn compute_saliency(
    prepared: &PreparedImage,
    cfg: &SaliencyConfig,
) -> Result<SaliencyMap, ImagePipelineError> {
    let len = checked_len(prepared.width, prepared.height)?;
    if prepared.pixels.len() != len {
        return Err(ImagePipelineError::Numeric(
            "prepared.pixels length does not match image dimensions".to_string(),
        ));
    }
    if prepared.valid_indices.is_empty() {
        return Err(ImagePipelineError::NoValidPixels);
    }

    let mut values = match cfg.method {
        SaliencyMethod::None => vec![1.0; len],
        SaliencyMethod::GlobalContrast(global_cfg) => {
            compute_global_contrast(prepared, &global_cfg)?
        }
        SaliencyMethod::LocalContrast(local_cfg) => compute_local_contrast(prepared, &local_cfg)?,
    };

    for value in &mut values {
        *value = clamp01(*value);
    }

    Ok(SaliencyMap {
        width: prepared.width,
        height: prepared.height,
        values,
    })
}

/// Computes global-contrast saliency as distance to global mean Oklab.
fn compute_global_contrast(
    prepared: &PreparedImage,
    cfg: &GlobalContrastConfig,
) -> Result<Vec<f64>, ImagePipelineError> {
    let len = prepared.pixels.len();
    let mean_lab = global_mean_lab(prepared);

    let mut raw = vec![0.0; len];
    for &idx in &prepared.valid_indices {
        raw[idx] = lab_distance2(prepared.pixels[idx].lab, mean_lab).sqrt();
    }

    normalize_valid_values(&raw, &prepared.valid_indices, cfg.robust_normalize)
}

/// Computes local-contrast saliency from blurred local baselines.
///
/// The final score mixes color contrast, luminance contrast, and optional
/// global contrast according to `cfg`.
fn compute_local_contrast(
    prepared: &PreparedImage,
    cfg: &LocalContrastConfig,
) -> Result<Vec<f64>, ImagePipelineError> {
    if !cfg.color_weight.is_finite() || cfg.color_weight < 0.0 {
        return Err(ImagePipelineError::InvalidConfig(
            "saliency.local.color_weight must be finite and >= 0".to_string(),
        ));
    }
    if !cfg.luminance_weight.is_finite() || cfg.luminance_weight < 0.0 {
        return Err(ImagePipelineError::InvalidConfig(
            "saliency.local.luminance_weight must be finite and >= 0".to_string(),
        ));
    }
    if !cfg.global_mix.is_finite() || !(0.0..=1.0).contains(&cfg.global_mix) {
        return Err(ImagePipelineError::InvalidConfig(
            "saliency.local.global_mix must be finite and in [0, 1]".to_string(),
        ));
    }

    let width = usize::try_from(prepared.width)
        .map_err(|_| ImagePipelineError::Numeric("width does not fit usize".to_string()))?;
    let height = usize::try_from(prepared.height)
        .map_err(|_| ImagePipelineError::Numeric("height does not fit usize".to_string()))?;
    let radius = usize::try_from(cfg.blur_radius)
        .map_err(|_| ImagePipelineError::Numeric("blur_radius too large".to_string()))?;

    let mut valid_mask = vec![false; prepared.pixels.len()];
    for &idx in &prepared.valid_indices {
        valid_mask[idx] = true;
    }

    let mut l_values = Vec::with_capacity(prepared.pixels.len());
    let mut a_values = Vec::with_capacity(prepared.pixels.len());
    let mut b_values = Vec::with_capacity(prepared.pixels.len());
    let mut y_values = Vec::with_capacity(prepared.pixels.len());
    for px in &prepared.pixels {
        l_values.push(px.lab.l);
        a_values.push(px.lab.a);
        b_values.push(px.lab.b);
        y_values.push(px.luminance);
    }

    let local_l = box_blur_masked(&l_values, &valid_mask, width, height, radius);
    let local_a = box_blur_masked(&a_values, &valid_mask, width, height, radius);
    let local_b = box_blur_masked(&b_values, &valid_mask, width, height, radius);
    let local_y = box_blur_masked(&y_values, &valid_mask, width, height, radius);

    let mean_lab = global_mean_lab(prepared);
    let mut raw = vec![0.0; prepared.pixels.len()];
    for &idx in &prepared.valid_indices {
        let px = prepared.pixels[idx];
        let dl = px.lab.l - local_l[idx];
        let da = px.lab.a - local_a[idx];
        let db = px.lab.b - local_b[idx];
        let color_term = (dl * dl + da * da + db * db).sqrt();
        let y_term = (px.luminance - local_y[idx]).abs();

        let mut s = cfg.color_weight * color_term + cfg.luminance_weight * y_term;
        if cfg.global_mix > 0.0 {
            let global_term = lab_distance2(px.lab, mean_lab).sqrt();
            s = (1.0 - cfg.global_mix) * s + cfg.global_mix * global_term;
        }
        raw[idx] = s.max(0.0);
    }

    normalize_valid_values(&raw, &prepared.valid_indices, cfg.robust_normalize)
}

/// Computes mean Oklab over valid pixels.
fn global_mean_lab(prepared: &PreparedImage) -> chromoxide::Oklab {
    let mut sum_l = 0.0;
    let mut sum_a = 0.0;
    let mut sum_b = 0.0;
    for &idx in &prepared.valid_indices {
        let lab = prepared.pixels[idx].lab;
        sum_l += lab.l;
        sum_a += lab.a;
        sum_b += lab.b;
    }
    let inv_n = 1.0 / prepared.valid_indices.len() as f64;
    chromoxide::Oklab {
        l: sum_l * inv_n,
        a: sum_a * inv_n,
        b: sum_b * inv_n,
    }
}

/// Normalizes raw saliency over valid pixels into `[0, 1]`.
///
/// When `robust` is enabled, p1/p99 are used instead of min/max.
fn normalize_valid_values(
    raw: &[f64],
    valid_indices: &[usize],
    robust: bool,
) -> Result<Vec<f64>, ImagePipelineError> {
    if valid_indices.is_empty() {
        return Err(ImagePipelineError::NoValidPixels);
    }

    let mut valid_values = Vec::with_capacity(valid_indices.len());
    for &idx in valid_indices {
        let value = raw[idx];
        if !value.is_finite() {
            return Err(ImagePipelineError::Numeric(
                "non-finite saliency encountered during normalization".to_string(),
            ));
        }
        valid_values.push(value);
    }

    let (lo, hi) = if robust {
        valid_values.sort_by(f64::total_cmp);
        (
            percentile(&valid_values, 0.01),
            percentile(&valid_values, 0.99),
        )
    } else {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for &v in &valid_values {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        (lo, hi)
    };

    let mut out = vec![0.0; raw.len()];
    if (hi - lo).abs() <= EPSILON {
        for &idx in valid_indices {
            out[idx] = 1.0;
        }
        return Ok(out);
    }

    for &idx in valid_indices {
        out[idx] = clamp01((raw[idx] - lo) / (hi - lo));
    }
    Ok(out)
}

/// Applies separable masked box blur with clamp-at-edge border handling.
///
/// Invalid pixels do not contribute to neighborhood sums.
fn box_blur_masked(
    values: &[f64],
    valid: &[bool],
    width: usize,
    height: usize,
    radius: usize,
) -> Vec<f64> {
    if values.is_empty() || radius == 0 {
        return values.to_vec();
    }

    let mut h_sum = vec![0.0; values.len()];
    let mut h_count = vec![0_u32; values.len()];

    for y in 0..height {
        for x in 0..width {
            let mut sum = 0.0;
            let mut count = 0_u32;
            for dx in 0..=(2 * radius) {
                let offset = dx as isize - radius as isize;
                let xx = (x as isize + offset).clamp(0, width as isize - 1) as usize;
                let idx = y * width + xx;
                if valid[idx] {
                    sum += values[idx];
                    count += 1;
                }
            }
            let idx = y * width + x;
            h_sum[idx] = sum;
            h_count[idx] = count;
        }
    }

    let mut out = vec![0.0; values.len()];
    for y in 0..height {
        for x in 0..width {
            let mut sum = 0.0;
            let mut count = 0_u32;
            for dy in 0..=(2 * radius) {
                let offset = dy as isize - radius as isize;
                let yy = (y as isize + offset).clamp(0, height as isize - 1) as usize;
                let idx = yy * width + x;
                sum += h_sum[idx];
                count += h_count[idx];
            }
            let idx = y * width + x;
            out[idx] = if count > 0 {
                sum / f64::from(count)
            } else {
                values[idx]
            };
        }
    }

    out
}
