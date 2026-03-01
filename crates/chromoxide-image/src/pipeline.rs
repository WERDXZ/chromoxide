//! High-level image pipeline orchestration APIs.

use std::path::Path;

use rand::Rng;

use crate::assignment::export_samples;
use crate::cap::build_image_cap;
use crate::config::ImagePipelineConfig;
use crate::diagnostics::{ImagePipelineDiagnostics, compute_saliency_stats};
use crate::error::ImagePipelineError;
use crate::load::load_image_from_path;
use crate::preprocess::prepare_image;
use crate::saliency::compute_saliency;
use crate::sampling::select_representatives_with_rng;

/// Pipeline output: weighted samples, optional image cap, and diagnostics.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ImageSupport {
    /// Exported weighted support samples.
    pub samples: Vec<chromoxide::WeightedSample>,
    /// Optional built image cap.
    pub image_cap: Option<chromoxide::ImageCap>,
    /// Pipeline diagnostics.
    pub diagnostics: ImagePipelineDiagnostics,
}

/// Runs the full pipeline on an in-memory image.
///
/// This convenience entrypoint uses a thread-local RNG.
pub fn prepare_support_from_image(
    img: &image::DynamicImage,
    cfg: &ImagePipelineConfig,
) -> Result<ImageSupport, ImagePipelineError> {
    let mut rng = rand::rng();
    prepare_support_from_image_with_rng(img, cfg, &mut rng)
}

/// Runs the full pipeline on an in-memory image with explicit RNG.
pub fn prepare_support_from_image_with_rng(
    img: &image::DynamicImage,
    cfg: &ImagePipelineConfig,
    rng: &mut dyn Rng,
) -> Result<ImageSupport, ImagePipelineError> {
    let (original_width, original_height) = image::GenericImageView::dimensions(img);
    let prepared = prepare_image(img, &cfg.preprocess)?;
    let saliency = compute_saliency(&prepared, &cfg.saliency)?;
    let reps = select_representatives_with_rng(&prepared, &saliency, &cfg.sampling, rng)?;
    let samples = export_samples(&prepared, &saliency, &reps, &cfg.export)?;

    let image_cap = match &cfg.cap {
        Some(cap_cfg) => Some(build_image_cap(
            &prepared,
            &saliency,
            Some(&samples),
            cap_cfg,
        )?),
        None => None,
    };

    let diagnostics = ImagePipelineDiagnostics {
        original_width,
        original_height,
        working_width: prepared.width,
        working_height: prepared.height,
        valid_pixel_count: prepared.valid_indices.len(),
        invalid_pixel_count: prepared.pixels.len() - prepared.valid_indices.len(),
        saliency_stats: compute_saliency_stats(&saliency, &prepared.valid_indices),
        representative_count: reps.len(),
        exported_sample_count: samples.len(),
        weight_sum: samples.iter().map(|s| s.weight).sum(),
    };

    Ok(ImageSupport {
        samples,
        image_cap,
        diagnostics,
    })
}

/// Loads an image from path and runs the full pipeline.
///
/// This convenience entrypoint uses a thread-local RNG.
pub fn prepare_support_from_path<P: AsRef<Path>>(
    path: P,
    cfg: &ImagePipelineConfig,
) -> Result<ImageSupport, ImagePipelineError> {
    let mut rng = rand::rng();
    prepare_support_from_path_with_rng(path, cfg, &mut rng)
}

/// Loads an image from path and runs the full pipeline with explicit RNG.
pub fn prepare_support_from_path_with_rng<P: AsRef<Path>>(
    path: P,
    cfg: &ImagePipelineConfig,
    rng: &mut dyn Rng,
) -> Result<ImageSupport, ImagePipelineError> {
    let img = load_image_from_path(path)?;
    prepare_support_from_image_with_rng(&img, cfg, rng)
}
