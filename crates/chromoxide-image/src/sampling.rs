//! Representative point sampling methods.

use std::collections::HashSet;

use rand::seq::SliceRandom;
use rand::{Rng, RngExt};

use crate::config::{
    FarthestPointLabConfig, KMeansPlusPlusLabConfig, RandomUniformConfig, SamplingConfig,
    SamplingMethod, StratifiedConfig, UniformGridConfig,
};
use crate::error::ImagePipelineError;
use crate::prepared::PreparedImage;
use crate::saliency::SaliencyMap;
use crate::util::{EPSILON, checked_len, lab_distance2};

/// Representative cluster in a `PreparedImage`.
///
/// `pixel_index` anchors the representative to a real image pixel; `lab` is the
/// actual assignment center (which may be a Lloyd centroid rather than the
/// anchor pixel's color).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Representative {
    /// Pixel index in `PreparedImage::pixels`.
    pub pixel_index: usize,
    /// Assignment center in Oklab.
    pub lab: chromoxide::Oklab,
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

    let reps = match cfg.method {
        SamplingMethod::UniformGrid(grid_cfg) => indices_to_reps(
            prepared,
            &valid_mask,
            uniform_grid(prepared, &valid_mask, grid_cfg)?,
        )?,
        SamplingMethod::Stratified(strat_cfg) => indices_to_reps(
            prepared,
            &valid_mask,
            stratified(prepared, &valid_mask, rng, strat_cfg)?,
        )?,
        SamplingMethod::RandomUniform(random_cfg) => indices_to_reps(
            prepared,
            &valid_mask,
            random_uniform(prepared, rng, random_cfg)?,
        )?,
        SamplingMethod::FarthestPointLab(fps_cfg) => indices_to_reps(
            prepared,
            &valid_mask,
            farthest_point_lab(prepared, saliency, rng, fps_cfg)?,
        )?,
        SamplingMethod::KMeansPlusPlusLab(km_cfg) => {
            k_means_plus_plus_lab(prepared, saliency, rng, km_cfg)?
        }
    };

    for rep in &reps {
        if rep.pixel_index >= prepared.pixels.len() || !valid_mask[rep.pixel_index] {
            return Err(ImagePipelineError::InvalidConfig(
                "sampling returned an invalid representative index".to_string(),
            ));
        }
        if !rep.lab.l.is_finite() || !rep.lab.a.is_finite() || !rep.lab.b.is_finite() {
            return Err(ImagePipelineError::Numeric(
                "sampling returned a non-finite representative lab".to_string(),
            ));
        }
    }
    if reps.is_empty() {
        return Err(ImagePipelineError::InvalidConfig(
            "sampling produced no representatives".to_string(),
        ));
    }

    Ok(reps)
}

fn indices_to_reps(
    prepared: &PreparedImage,
    valid_mask: &[bool],
    indices: Vec<usize>,
) -> Result<Vec<Representative>, ImagePipelineError> {
    let mut seen = HashSet::new();
    let mut reps = Vec::with_capacity(indices.len());
    for idx in indices {
        if !seen.insert(idx) {
            continue;
        }
        if idx >= prepared.pixels.len() || !valid_mask[idx] {
            return Err(ImagePipelineError::InvalidConfig(
                "sampling returned an invalid representative index".to_string(),
            ));
        }
        reps.push(Representative {
            pixel_index: idx,
            lab: prepared.pixels[idx].lab,
        });
    }
    if reps.is_empty() {
        return Err(ImagePipelineError::InvalidConfig(
            "sampling produced no representatives".to_string(),
        ));
    }
    Ok(reps)
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

/// Saliency-weighted k-means++ seeding followed by Lloyd refinement.
///
/// Seeding draws from a strided candidate pool (padded to the target count when
/// needed); Lloyd assignment and centroid updates use every valid pixel.
fn k_means_plus_plus_lab(
    prepared: &PreparedImage,
    saliency: &SaliencyMap,
    rng: &mut dyn Rng,
    cfg: KMeansPlusPlusLabConfig,
) -> Result<Vec<Representative>, ImagePipelineError> {
    if !cfg.saliency_bias.is_finite() || cfg.saliency_bias < 0.0 {
        return Err(ImagePipelineError::InvalidConfig(
            "sampling.k_means_plus_plus_lab.saliency_bias must be finite and >= 0".to_string(),
        ));
    }
    if !cfg.convergence_tol.is_finite() || cfg.convergence_tol <= 0.0 {
        return Err(ImagePipelineError::InvalidConfig(
            "sampling.k_means_plus_plus_lab.convergence_tol must be finite and > 0".to_string(),
        ));
    }
    for &idx in &prepared.valid_indices {
        let lab = prepared.pixels[idx].lab;
        if !lab.l.is_finite() || !lab.a.is_finite() || !lab.b.is_finite() {
            return Err(ImagePipelineError::Numeric(
                "non-finite pixel lab encountered during k-means sampling".to_string(),
            ));
        }
    }

    let valid_count = prepared.valid_indices.len();
    let target_count = cfg.count.get().min(valid_count);
    let stride = usize::try_from(cfg.candidate_stride.get()).map_err(|_| {
        ImagePipelineError::Numeric(
            "sampling.k_means_plus_plus_lab.candidate_stride too large".into(),
        )
    })?;

    let mut candidates: Vec<usize> = prepared
        .valid_indices
        .iter()
        .copied()
        .step_by(stride)
        .collect();
    let mut seen: HashSet<usize> = candidates.iter().copied().collect();
    for &idx in &prepared.valid_indices {
        if candidates.len() >= target_count {
            break;
        }
        if seen.insert(idx) {
            candidates.push(idx);
        }
    }
    if candidates.is_empty() {
        return Err(ImagePipelineError::NoValidPixels);
    }

    let base_weights: Vec<f64> = candidates
        .iter()
        .map(|&idx| {
            let alpha = prepared.pixels[idx].alpha.max(0.0);
            let raw = saliency.values[idx];
            let sal = if raw.is_finite() {
                raw.clamp(0.0, 1.0)
            } else {
                0.0
            };
            alpha * (1.0 + cfg.saliency_bias * sal)
        })
        .collect();

    let mut selected_mask = vec![false; candidates.len()];
    let mut centers: Vec<chromoxide::Oklab> = Vec::with_capacity(target_count);

    let first_pos = weighted_select_candidate(&candidates, &base_weights, &selected_mask, rng)
        .ok_or_else(|| {
            ImagePipelineError::InvalidConfig("k-means++ candidate pool is empty".to_string())
        })?;
    selected_mask[first_pos] = true;
    centers.push(prepared.pixels[candidates[first_pos]].lab);

    while centers.len() < target_count {
        let mut weights = Vec::with_capacity(candidates.len());
        for (pos, &idx) in candidates.iter().enumerate() {
            if selected_mask[pos] {
                weights.push(0.0);
                continue;
            }
            let mut min_d2 = f64::INFINITY;
            for center in &centers {
                min_d2 = min_d2.min(lab_distance2(prepared.pixels[idx].lab, *center));
            }
            weights.push(base_weights[pos] * min_d2);
        }
        let next_pos = weighted_select_candidate(&candidates, &weights, &selected_mask, rng)
            .ok_or_else(|| {
                ImagePipelineError::InvalidConfig(
                    "k-means++ failed to draw a subsequent center".to_string(),
                )
            })?;
        selected_mask[next_pos] = true;
        centers.push(prepared.pixels[candidates[next_pos]].lab);
    }

    let centers = lloyd_refine(prepared, centers, cfg.max_iters.get(), cfg.convergence_tol)?;

    let mut used_anchors = HashSet::new();
    let mut reps = Vec::with_capacity(centers.len());
    for center in centers {
        let anchor = find_nearest_unused_anchor(prepared, center, &used_anchors);
        used_anchors.insert(anchor);
        reps.push(Representative {
            pixel_index: anchor,
            lab: center,
        });
    }
    Ok(reps)
}

/// Stable cumulative weighted selection without replacement.
///
/// Returns the candidate position. Falls back to the smallest unselected
/// pixel index when the weight sum is non-positive or floating-point drift
/// leaves the draw unspent.
fn weighted_select_candidate(
    candidates: &[usize],
    weights: &[f64],
    selected_mask: &[bool],
    rng: &mut dyn Rng,
) -> Option<usize> {
    let mut total = 0.0;
    for (i, &weight) in weights.iter().enumerate() {
        if !selected_mask[i] {
            total += weight.max(0.0);
        }
    }
    if total <= EPSILON {
        return min_unselected_candidate_pos(candidates, selected_mask);
    }

    let mut draw = rng.random_range(0.0..total);
    for (i, &weight) in weights.iter().enumerate() {
        if selected_mask[i] {
            continue;
        }
        draw -= weight.max(0.0);
        if draw < 0.0 {
            return Some(i);
        }
    }
    min_unselected_candidate_pos(candidates, selected_mask)
}

fn min_unselected_candidate_pos(candidates: &[usize], selected_mask: &[bool]) -> Option<usize> {
    let mut best_pos = None;
    let mut best_idx = usize::MAX;
    for (pos, &idx) in candidates.iter().enumerate() {
        if selected_mask[pos] {
            continue;
        }
        if idx < best_idx {
            best_idx = idx;
            best_pos = Some(pos);
        }
    }
    best_pos
}

/// Runs Lloyd iterations over all valid pixels with alpha-weighted centroids.
fn lloyd_refine(
    prepared: &PreparedImage,
    initial_centers: Vec<chromoxide::Oklab>,
    max_iters: usize,
    convergence_tol: f64,
) -> Result<Vec<chromoxide::Oklab>, ImagePipelineError> {
    let mut centers = initial_centers;
    let tol2 = convergence_tol * convergence_tol;

    for _ in 0..max_iters {
        let mut cluster_pixels = vec![Vec::new(); centers.len()];
        for &pixel_idx in &prepared.valid_indices {
            let px = prepared.pixels[pixel_idx];
            let mut best_cluster = 0usize;
            let mut best_d2 = f64::INFINITY;
            for (ci, center) in centers.iter().enumerate() {
                let d2 = lab_distance2(px.lab, *center);
                if d2 < best_d2 {
                    best_d2 = d2;
                    best_cluster = ci;
                }
            }
            cluster_pixels[best_cluster].push(pixel_idx);
        }

        let mut new_centers = vec![
            chromoxide::Oklab {
                l: 0.0,
                a: 0.0,
                b: 0.0,
            };
            centers.len()
        ];
        let mut non_empty = vec![false; centers.len()];
        for (ci, members) in cluster_pixels.iter().enumerate() {
            let mut mass = 0.0;
            let mut sum_l = 0.0;
            let mut sum_a = 0.0;
            let mut sum_b = 0.0;
            for &idx in members {
                let px = prepared.pixels[idx];
                let w = px.alpha.max(0.0);
                mass += w;
                sum_l += w * px.lab.l;
                sum_a += w * px.lab.a;
                sum_b += w * px.lab.b;
            }
            if mass > EPSILON {
                new_centers[ci] = chromoxide::Oklab {
                    l: sum_l / mass,
                    a: sum_a / mass,
                    b: sum_b / mass,
                };
                non_empty[ci] = true;
            }
        }

        let mut used_in_round = HashSet::new();
        for ci in 0..centers.len() {
            if non_empty[ci] {
                continue;
            }
            let pick = choose_empty_replacement(prepared, &new_centers, &non_empty, &used_in_round);
            new_centers[ci] = prepared.pixels[pick].lab;
            non_empty[ci] = true;
            used_in_round.insert(pick);
        }

        let mut max_movement2: f64 = 0.0;
        for (old, new) in centers.iter().zip(new_centers.iter()) {
            max_movement2 = max_movement2.max(lab_distance2(*old, *new));
        }
        centers = new_centers;
        if max_movement2 <= tol2 {
            break;
        }
    }

    Ok(centers)
}

/// Chooses a replacement pixel for an empty cluster.
///
/// Maximizes `alpha * min_distance2` to non-empty centers; ties and all-zero
/// scores fall back to the smallest unused valid pixel index.
fn choose_empty_replacement(
    prepared: &PreparedImage,
    centers: &[chromoxide::Oklab],
    non_empty: &[bool],
    used: &HashSet<usize>,
) -> usize {
    let mut best_idx = usize::MAX;
    let mut best_score = f64::NEG_INFINITY;
    for &idx in &prepared.valid_indices {
        if used.contains(&idx) {
            continue;
        }
        let mut min_d2 = f64::INFINITY;
        let mut has_center = false;
        for (ci, &is_non_empty) in non_empty.iter().enumerate() {
            if is_non_empty {
                has_center = true;
                min_d2 = min_d2.min(lab_distance2(prepared.pixels[idx].lab, centers[ci]));
            }
        }
        let min_d2 = if has_center { min_d2 } else { 0.0 };
        let score = prepared.pixels[idx].alpha.max(0.0) * min_d2;
        if score > best_score + EPSILON || ((score - best_score).abs() <= EPSILON && idx < best_idx)
        {
            best_score = score;
            best_idx = idx;
        }
    }

    if best_idx != usize::MAX {
        return best_idx;
    }
    match prepared
        .valid_indices
        .iter()
        .copied()
        .find(|idx| !used.contains(idx))
    {
        Some(idx) => idx,
        None => prepared.valid_indices[0],
    }
}

/// Finds the valid pixel closest to `center` that is not already an anchor.
fn find_nearest_unused_anchor(
    prepared: &PreparedImage,
    center: chromoxide::Oklab,
    used: &HashSet<usize>,
) -> usize {
    let mut best_idx = usize::MAX;
    let mut best_d2 = f64::INFINITY;
    for &idx in &prepared.valid_indices {
        if used.contains(&idx) {
            continue;
        }
        let d2 = lab_distance2(prepared.pixels[idx].lab, center);
        if d2 < best_d2 - EPSILON || ((d2 - best_d2).abs() <= EPSILON && idx < best_idx) {
            best_d2 = d2;
            best_idx = idx;
        }
    }
    match best_idx {
        usize::MAX => prepared.valid_indices[0],
        idx => idx,
    }
}
