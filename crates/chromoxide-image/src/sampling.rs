//! Representative point sampling methods.

use std::collections::HashSet;

use rand::seq::SliceRandom;
use rand::{Rng, RngExt};

use crate::config::{
    FarthestPointLabConfig, RandomUniformConfig, SamplingConfig, SamplingMethod, StratifiedConfig,
    UniformGridConfig,
};
use crate::error::ImagePipelineError;
use crate::prepared::PreparedImage;
use crate::saliency::SaliencyMap;
use crate::util::{EPSILON, checked_len, lab_distance2};

/// Representative pixel reference in a `PreparedImage`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Representative {
    /// Pixel index in `PreparedImage::pixels`.
    pub pixel_index: usize,
}

/// Selects representative points from a prepared image.
pub fn select_representatives(
    prepared: &PreparedImage,
    saliency: &SaliencyMap,
    cfg: &SamplingConfig,
) -> Result<Vec<Representative>, ImagePipelineError> {
    let mut rng = rand::rng();
    select_representatives_with_rng(prepared, saliency, cfg, &mut rng)
}

/// Selects representative points from a prepared image with explicit RNG.
pub fn select_representatives_with_rng(
    prepared: &PreparedImage,
    saliency: &SaliencyMap,
    cfg: &SamplingConfig,
    rng: &mut dyn Rng,
) -> Result<Vec<Representative>, ImagePipelineError> {
    let len = checked_len(prepared.width, prepared.height)?;
    if prepared.pixels.len() != len {
        return Err(ImagePipelineError::Numeric(
            "prepared.pixels length does not match image dimensions".to_string(),
        ));
    }
    if saliency.width != prepared.width
        || saliency.height != prepared.height
        || saliency.values.len() != len
    {
        return Err(ImagePipelineError::InvalidConfig(
            "saliency map dimensions must match prepared image".to_string(),
        ));
    }
    if prepared.valid_indices.is_empty() {
        return Err(ImagePipelineError::NoValidPixels);
    }

    let mut valid_mask = vec![false; prepared.pixels.len()];
    for &idx in &prepared.valid_indices {
        valid_mask[idx] = true;
    }

    let mut indices = match cfg.method {
        SamplingMethod::UniformGrid(grid_cfg) => uniform_grid(prepared, &valid_mask, grid_cfg)?,
        SamplingMethod::Stratified(strat_cfg) => stratified(prepared, &valid_mask, rng, strat_cfg)?,
        SamplingMethod::RandomUniform(random_cfg) => random_uniform(prepared, rng, random_cfg)?,
        SamplingMethod::FarthestPointLab(fps_cfg) => {
            farthest_point_lab(prepared, saliency, rng, fps_cfg)?
        }
    };

    let mut seen = HashSet::new();
    indices.retain(|idx| seen.insert(*idx));

    for &idx in &indices {
        if idx >= prepared.pixels.len() || !valid_mask[idx] {
            return Err(ImagePipelineError::InvalidConfig(
                "sampling returned an invalid representative index".to_string(),
            ));
        }
    }
    if indices.is_empty() {
        return Err(ImagePipelineError::InvalidConfig(
            "sampling produced no representatives".to_string(),
        ));
    }

    Ok(indices
        .into_iter()
        .map(|pixel_index| Representative { pixel_index })
        .collect())
}

/// Picks one representative nearest each grid-cell center.
fn uniform_grid(
    prepared: &PreparedImage,
    valid_mask: &[bool],
    cfg: UniformGridConfig,
) -> Result<Vec<usize>, ImagePipelineError> {
    let width = usize::try_from(prepared.width)
        .map_err(|_| ImagePipelineError::Numeric("width does not fit usize".to_string()))?;
    let height = usize::try_from(prepared.height)
        .map_err(|_| ImagePipelineError::Numeric("height does not fit usize".to_string()))?;
    let step = usize::try_from(cfg.step.get())
        .map_err(|_| ImagePipelineError::Numeric("grid step too large".to_string()))?;

    let mut reps = Vec::new();
    for y0 in (0..height).step_by(step) {
        let y1 = (y0 + step).min(height);
        let cy = 0.5 * (y0 as f64 + (y1 - 1) as f64);
        for x0 in (0..width).step_by(step) {
            let x1 = (x0 + step).min(width);
            let cx = 0.5 * (x0 as f64 + (x1 - 1) as f64);

            let mut best_idx = None;
            let mut best_d2 = f64::INFINITY;
            for y in y0..y1 {
                for x in x0..x1 {
                    let idx = y * width + x;
                    if !valid_mask[idx] {
                        continue;
                    }
                    let dx = x as f64 - cx;
                    let dy = y as f64 - cy;
                    let d2 = dx * dx + dy * dy;
                    let better = match best_idx {
                        None => true,
                        Some(prev_idx) => {
                            d2 < best_d2 - EPSILON
                                || ((d2 - best_d2).abs() <= EPSILON && idx < prev_idx)
                        }
                    };
                    if better {
                        best_idx = Some(idx);
                        best_d2 = d2;
                    }
                }
            }

            if let Some(idx) = best_idx {
                reps.push(idx);
            }
        }
    }

    Ok(reps)
}

/// Samples up to `per_tile` points uniformly inside each image tile.
fn stratified(
    prepared: &PreparedImage,
    valid_mask: &[bool],
    rng: &mut dyn Rng,
    cfg: StratifiedConfig,
) -> Result<Vec<usize>, ImagePipelineError> {
    let width = usize::try_from(prepared.width)
        .map_err(|_| ImagePipelineError::Numeric("width does not fit usize".to_string()))?;
    let height = usize::try_from(prepared.height)
        .map_err(|_| ImagePipelineError::Numeric("height does not fit usize".to_string()))?;
    let tiles_x = usize::try_from(cfg.tiles_x.get())
        .map_err(|_| ImagePipelineError::Numeric("tiles_x too large".to_string()))?;
    let tiles_y = usize::try_from(cfg.tiles_y.get())
        .map_err(|_| ImagePipelineError::Numeric("tiles_y too large".to_string()))?;
    let per_tile = usize::try_from(cfg.per_tile.get())
        .map_err(|_| ImagePipelineError::Numeric("per_tile too large".to_string()))?;

    let mut reps = Vec::new();

    for ty in 0..tiles_y {
        let y0 = ty * height / tiles_y;
        let y1 = (ty + 1) * height / tiles_y;
        for tx in 0..tiles_x {
            let x0 = tx * width / tiles_x;
            let x1 = (tx + 1) * width / tiles_x;

            let mut candidates = Vec::new();
            for y in y0..y1 {
                for x in x0..x1 {
                    let idx = y * width + x;
                    if valid_mask[idx] {
                        candidates.push(idx);
                    }
                }
            }

            if candidates.len() > per_tile {
                candidates.shuffle(rng);
                candidates.truncate(per_tile);
            }
            reps.extend(candidates);
        }
    }

    Ok(reps)
}

/// Uniform random sampling over valid pixels without replacement.
fn random_uniform(
    prepared: &PreparedImage,
    rng: &mut dyn Rng,
    cfg: RandomUniformConfig,
) -> Result<Vec<usize>, ImagePipelineError> {
    let count = cfg.count.get();

    if count >= prepared.valid_indices.len() {
        return Ok(prepared.valid_indices.clone());
    }

    let mut candidates = prepared.valid_indices.clone();
    candidates.shuffle(rng);
    candidates.truncate(count);
    Ok(candidates)
}

/// Greedy farthest-point sampling in Oklab, optionally biased by saliency.
fn farthest_point_lab(
    prepared: &PreparedImage,
    saliency: &SaliencyMap,
    rng: &mut dyn Rng,
    cfg: FarthestPointLabConfig,
) -> Result<Vec<usize>, ImagePipelineError> {
    if !cfg.saliency_bias.is_finite() || cfg.saliency_bias < 0.0 {
        return Err(ImagePipelineError::InvalidConfig(
            "sampling.farthest_point_lab.saliency_bias must be finite and >= 0".to_string(),
        ));
    }

    let stride = usize::try_from(cfg.candidate_stride.get()).map_err(|_| {
        ImagePipelineError::Numeric("sampling.farthest_point_lab.candidate_stride too large".into())
    })?;
    let candidates: Vec<usize> = prepared
        .valid_indices
        .iter()
        .copied()
        .step_by(stride)
        .collect();
    if candidates.is_empty() {
        return Err(ImagePipelineError::NoValidPixels);
    }

    let target_count = cfg.count.get().min(candidates.len());

    let first_pos = if cfg.saliency_bias > 0.0 {
        let mut best_pos = 0;
        let mut best_saliency = f64::NEG_INFINITY;
        let mut best_idx = usize::MAX;
        for (pos, &idx) in candidates.iter().enumerate() {
            let s = saliency.values[idx];
            if s > best_saliency + EPSILON
                || ((s - best_saliency).abs() <= EPSILON && idx < best_idx)
            {
                best_saliency = s;
                best_idx = idx;
                best_pos = pos;
            }
        }
        best_pos
    } else {
        rng.random_range(0..candidates.len())
    };

    let mut selected = Vec::with_capacity(target_count);
    let mut selected_mask = vec![false; candidates.len()];
    selected.push(candidates[first_pos]);
    selected_mask[first_pos] = true;

    let first_lab = prepared.pixels[candidates[first_pos]].lab;
    let mut min_dist2 = vec![f64::INFINITY; candidates.len()];
    for (pos, &idx) in candidates.iter().enumerate() {
        min_dist2[pos] = lab_distance2(prepared.pixels[idx].lab, first_lab);
    }

    while selected.len() < target_count {
        let mut best_pos = None;
        let mut best_score = f64::NEG_INFINITY;
        let mut best_idx = usize::MAX;

        for (pos, &idx) in candidates.iter().enumerate() {
            if selected_mask[pos] {
                continue;
            }
            let d = min_dist2[pos].sqrt();
            let s = saliency.values[idx].clamp(0.0, 1.0);
            let score = d * (1.0 + cfg.saliency_bias * s);
            if score > best_score + EPSILON
                || ((score - best_score).abs() <= EPSILON && idx < best_idx)
            {
                best_score = score;
                best_idx = idx;
                best_pos = Some(pos);
            }
        }

        let Some(next_pos) = best_pos else {
            break;
        };
        selected.push(candidates[next_pos]);
        selected_mask[next_pos] = true;

        let next_lab = prepared.pixels[candidates[next_pos]].lab;
        for (pos, &idx) in candidates.iter().enumerate() {
            if selected_mask[pos] {
                continue;
            }
            let d2 = lab_distance2(prepared.pixels[idx].lab, next_lab);
            if d2 < min_dist2[pos] {
                min_dist2[pos] = d2;
            }
        }
    }

    Ok(selected)
}
