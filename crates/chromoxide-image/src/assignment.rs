//! Assignment of pixels to representatives and export to weighted samples.

use crate::config::{CenterMode, ExportConfig};
use crate::error::ImagePipelineError;
use crate::prepared::PreparedImage;
use crate::saliency::SaliencyMap;
use crate::sampling::Representative;
use crate::util::{EPSILON, checked_len, lab_distance2};

/// Exports clustered support as `chromoxide::WeightedSample` values.
pub fn export_samples(
    prepared: &PreparedImage,
    saliency: &SaliencyMap,
    reps: &[Representative],
    cfg: &ExportConfig,
) -> Result<Vec<chromoxide::WeightedSample>, ImagePipelineError> {
    if reps.is_empty() {
        return Err(ImagePipelineError::InvalidConfig(
            "representatives cannot be empty".to_string(),
        ));
    }
    validate_export_config(cfg)?;

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

    let mut valid_mask = vec![false; prepared.pixels.len()];
    for &idx in &prepared.valid_indices {
        valid_mask[idx] = true;
    }

    let mut rep_labs = Vec::with_capacity(reps.len());
    for rep in reps {
        if rep.pixel_index >= prepared.pixels.len() || !valid_mask[rep.pixel_index] {
            return Err(ImagePipelineError::InvalidConfig(
                "representatives must reference valid prepared pixels".to_string(),
            ));
        }
        rep_labs.push(prepared.pixels[rep.pixel_index].lab);
    }

    let mut clusters = vec![ClusterAccum::default(); reps.len()];

    for &pixel_idx in &prepared.valid_indices {
        let px = prepared.pixels[pixel_idx];

        let mut best_rep = 0usize;
        let mut best_d2 = f64::INFINITY;
        for (ri, &rep_lab) in rep_labs.iter().enumerate() {
            let d2 = lab_distance2(px.lab, rep_lab);
            if d2 < best_d2 - EPSILON {
                best_d2 = d2;
                best_rep = ri;
            }
        }

        let sal = saliency.values[pixel_idx].clamp(0.0, 1.0);
        let mass = px.alpha;
        if !mass.is_finite() || mass < 0.0 {
            return Err(ImagePipelineError::Numeric(
                "non-finite or negative pixel mass encountered".to_string(),
            ));
        }

        let cluster = &mut clusters[best_rep];
        cluster.mass += mass;
        cluster.saliency_sum += sal;
        cluster.pixel_count += 1;
        cluster.lab_sum_l += mass * px.lab.l;
        cluster.lab_sum_a += mass * px.lab.a;
        cluster.lab_sum_b += mass * px.lab.b;
        cluster.members.push(pixel_idx);
    }

    let mut tmp = Vec::new();
    for cluster in &clusters {
        if cluster.pixel_count == 0 || cluster.mass <= 0.0 {
            continue;
        }

        let centroid = chromoxide::Oklab {
            l: cluster.lab_sum_l / cluster.mass,
            a: cluster.lab_sum_a / cluster.mass,
            b: cluster.lab_sum_b / cluster.mass,
        };

        let lab = match cfg.center_mode {
            CenterMode::Centroid => centroid,
            CenterMode::Medoid => medoid_lab(prepared, &cluster.members, centroid),
        };

        let mean_saliency = (cluster.saliency_sum / cluster.pixel_count as f64).clamp(0.0, 1.0);
        let mut weight = cluster.mass.powf(cfg.frequency_gamma);
        if cfg.saliency_to_weight_mix > 0.0 {
            let saliency_term = mean_saliency.powf(cfg.saliency_weight_gamma);
            let mixed =
                (1.0 - cfg.saliency_to_weight_mix) + cfg.saliency_to_weight_mix * saliency_term;
            weight *= mixed;
        }

        if !weight.is_finite() {
            return Err(ImagePipelineError::Numeric(
                "non-finite cluster weight encountered".to_string(),
            ));
        }

        tmp.push((lab, weight, mean_saliency));
    }

    if tmp.is_empty() {
        return Err(ImagePipelineError::InvalidConfig(
            "export produced no non-empty clusters".to_string(),
        ));
    }

    if cfg.normalize_weights {
        let sum_w: f64 = tmp.iter().map(|entry| entry.1).sum();
        if !sum_w.is_finite() || sum_w <= 0.0 {
            return Err(ImagePipelineError::Numeric(
                "cannot normalize cluster weights: non-positive sum".to_string(),
            ));
        }
        for (_, weight, _) in &mut tmp {
            *weight /= sum_w;
        }
    }

    let out: Vec<chromoxide::WeightedSample> = tmp
        .into_iter()
        .filter(|(_, weight, _)| *weight >= cfg.min_cluster_weight)
        .map(|(lab, weight, saliency)| chromoxide::WeightedSample::new(lab, weight, saliency))
        .collect();

    if out.is_empty() {
        return Err(ImagePipelineError::InvalidConfig(
            "all clusters filtered out by min_cluster_weight".to_string(),
        ));
    }

    Ok(out)
}

/// Validates numeric/range constraints for export configuration.
fn validate_export_config(cfg: &ExportConfig) -> Result<(), ImagePipelineError> {
    if !cfg.frequency_gamma.is_finite() || cfg.frequency_gamma < 0.0 {
        return Err(ImagePipelineError::InvalidConfig(
            "export.frequency_gamma must be finite and >= 0".to_string(),
        ));
    }
    if !cfg.saliency_to_weight_mix.is_finite() || !(0.0..=1.0).contains(&cfg.saliency_to_weight_mix)
    {
        return Err(ImagePipelineError::InvalidConfig(
            "export.saliency_to_weight_mix must be finite and in [0, 1]".to_string(),
        ));
    }
    if !cfg.saliency_weight_gamma.is_finite() || cfg.saliency_weight_gamma < 0.0 {
        return Err(ImagePipelineError::InvalidConfig(
            "export.saliency_weight_gamma must be finite and >= 0".to_string(),
        ));
    }
    if !cfg.min_cluster_weight.is_finite() || cfg.min_cluster_weight < 0.0 {
        return Err(ImagePipelineError::InvalidConfig(
            "export.min_cluster_weight must be finite and >= 0".to_string(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
struct ClusterAccum {
    mass: f64,
    saliency_sum: f64,
    pixel_count: usize,
    lab_sum_l: f64,
    lab_sum_a: f64,
    lab_sum_b: f64,
    members: Vec<usize>,
}

/// Returns the member pixel color closest to the provided centroid.
fn medoid_lab(
    prepared: &PreparedImage,
    members: &[usize],
    centroid: chromoxide::Oklab,
) -> chromoxide::Oklab {
    let mut best_idx = members[0];
    let mut best_d2 = lab_distance2(prepared.pixels[best_idx].lab, centroid);
    for &idx in members.iter().skip(1) {
        let d2 = lab_distance2(prepared.pixels[idx].lab, centroid);
        if d2 < best_d2 {
            best_d2 = d2;
            best_idx = idx;
        }
    }
    prepared.pixels[best_idx].lab
}
