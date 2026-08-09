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

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU32, NonZeroUsize};

    use image::{DynamicImage, Rgb, RgbImage};
    use rand::{SeedableRng, rngs::ChaCha8Rng};

    use super::{ImageSupport, prepare_support_from_image_with_rng};
    use crate::{
        CapConfig, ImagePipelineConfig, KMeansPlusPlusLabConfig, LocalContrastConfig,
        SaliencyConfig, SaliencyMethod, SamplingConfig, SamplingMethod,
    };

    fn test_image() -> DynamicImage {
        let mut image = RgbImage::new(8, 8);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = Rgb([
                (x * 29 + y * 11) as u8,
                (x * 7 + y * 31) as u8,
                (x * 19 + y * 23) as u8,
            ]);
        }
        DynamicImage::ImageRgb8(image)
    }

    fn test_config() -> ImagePipelineConfig {
        ImagePipelineConfig {
            saliency: SaliencyConfig {
                method: SaliencyMethod::LocalContrast(LocalContrastConfig::default()),
            },
            sampling: SamplingConfig {
                method: SamplingMethod::KMeansPlusPlusLab(KMeansPlusPlusLabConfig {
                    count: NonZeroUsize::new(6).expect("6 is non-zero"),
                    candidate_stride: NonZeroU32::new(1).expect("1 is non-zero"),
                    saliency_bias: 0.35,
                    max_iters: NonZeroUsize::new(8).expect("8 is non-zero"),
                    convergence_tol: 1.0e-8,
                }),
            },
            cap: Some(CapConfig::default()),
            ..Default::default()
        }
    }

    fn assert_exact_support_eq(first: &ImageSupport, second: &ImageSupport) {
        assert_eq!(first.samples.len(), second.samples.len());
        for (a, b) in first.samples.iter().zip(&second.samples) {
            assert_eq!(a.lab.l, b.lab.l);
            assert_eq!(a.lab.a, b.lab.a);
            assert_eq!(a.lab.b, b.lab.b);
            assert_eq!(a.weight, b.weight);
            assert_eq!(a.saliency, b.saliency);
        }

        let first_cap = first.image_cap.as_ref().expect("cap should be built");
        let second_cap = second.image_cap.as_ref().expect("cap should be built");
        assert_eq!(first_cap.n_l, second_cap.n_l);
        assert_eq!(first_cap.n_h, second_cap.n_h);
        assert_eq!(first_cap.l_min, second_cap.l_min);
        assert_eq!(first_cap.l_max, second_cap.l_max);
        assert_eq!(first_cap.grid, second_cap.grid);
        assert_eq!(
            first_cap.global_chroma_by_lightness,
            second_cap.global_chroma_by_lightness
        );
        assert_eq!(first_cap.support_confidence, second_cap.support_confidence);
        assert_eq!(first_cap.confidence, second_cap.confidence);

        assert_eq!(
            first.diagnostics.original_width,
            second.diagnostics.original_width
        );
        assert_eq!(
            first.diagnostics.original_height,
            second.diagnostics.original_height
        );
        assert_eq!(
            first.diagnostics.working_width,
            second.diagnostics.working_width
        );
        assert_eq!(
            first.diagnostics.working_height,
            second.diagnostics.working_height
        );
        assert_eq!(
            first.diagnostics.valid_pixel_count,
            second.diagnostics.valid_pixel_count
        );
        assert_eq!(
            first.diagnostics.invalid_pixel_count,
            second.diagnostics.invalid_pixel_count
        );
        assert_eq!(
            first.diagnostics.saliency_stats.min,
            second.diagnostics.saliency_stats.min
        );
        assert_eq!(
            first.diagnostics.saliency_stats.max,
            second.diagnostics.saliency_stats.max
        );
        assert_eq!(
            first.diagnostics.saliency_stats.mean,
            second.diagnostics.saliency_stats.mean
        );
        assert_eq!(
            first.diagnostics.representative_count,
            second.diagnostics.representative_count
        );
        assert_eq!(
            first.diagnostics.exported_sample_count,
            second.diagnostics.exported_sample_count
        );
        assert_eq!(first.diagnostics.weight_sum, second.diagnostics.weight_sum);
    }

    #[test]
    fn same_seed_produces_identical_image_support() {
        let image = test_image();
        let config = test_config();
        let image_seed = [0x3c; 32];
        let mut first_rng = ChaCha8Rng::from_seed(image_seed);
        let mut second_rng = ChaCha8Rng::from_seed(image_seed);

        let first = prepare_support_from_image_with_rng(&image, &config, &mut first_rng)
            .expect("first pipeline should succeed");
        let second = prepare_support_from_image_with_rng(&image, &config, &mut second_rng)
            .expect("second pipeline should succeed");

        assert_exact_support_eq(&first, &second);
    }
}
