use approx::assert_relative_eq;
use std::num::{NonZeroU32, NonZeroUsize};

use chromoxide::ImageCapBuilder;
use chromoxide_image::{
    CapConfig, CapSource, CenterMode, ExportConfig, FarthestPointLabConfig, ImagePipelineConfig,
    LocalContrastConfig, PreprocessConfig, ResizeFilter, SaliencyConfig, SaliencyMethod,
    SamplingConfig, SamplingMethod, prepare_support_from_image_with_rng,
};
use image::{DynamicImage, Rgba, RgbaImage};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn pipeline_end_to_end_returns_samples_cap_and_diagnostics() {
    let mut rgba = RgbaImage::new(64, 48);
    for y in 0..48 {
        for x in 0..64 {
            let r = (x * 4).min(255) as u8;
            let g = (y * 5).min(255) as u8;
            let b = 70u8;
            rgba.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }
    for y in 18..28 {
        for x in 26..38 {
            rgba.put_pixel(x, y, Rgba([245, 40, 40, 255]));
        }
    }

    let cfg = ImagePipelineConfig {
        preprocess: PreprocessConfig {
            max_working_dim: Some(NonZeroU32::new(32).expect("non-zero")),
            resize_filter: ResizeFilter::Triangle,
            background_rgb8: [255, 255, 255],
            min_alpha: 0.0,
            alpha_into_weight: false,
        },
        saliency: SaliencyConfig {
            method: SaliencyMethod::LocalContrast(LocalContrastConfig {
                blur_radius: 3,
                color_weight: 1.0,
                luminance_weight: 0.7,
                global_mix: 0.3,
                robust_normalize: true,
            }),
        },
        sampling: SamplingConfig {
            method: SamplingMethod::FarthestPointLab(FarthestPointLabConfig {
                count: NonZeroUsize::new(14).expect("non-zero"),
                candidate_stride: NonZeroU32::new(2).expect("non-zero"),
                saliency_bias: 0.5,
            }),
        },
        export: ExportConfig {
            center_mode: CenterMode::Centroid,
            normalize_weights: true,
            saliency_to_weight_mix: 0.0,
            saliency_weight_gamma: 1.0,
            frequency_gamma: 1.0,
            min_cluster_weight: 0.0,
        },
        cap: Some(CapConfig {
            source: CapSource::PreparedPixels,
            builder: ImageCapBuilder::default(),
        }),
    };

    let mut rng = StdRng::seed_from_u64(123);
    let support =
        prepare_support_from_image_with_rng(&DynamicImage::ImageRgba8(rgba), &cfg, &mut rng)
            .unwrap();
    assert!(!support.samples.is_empty());
    assert!(support.image_cap.is_some());

    let diag = &support.diagnostics;
    assert_eq!(diag.original_width, 64);
    assert_eq!(diag.original_height, 48);
    assert!(diag.working_width <= 32);
    assert!(diag.working_height <= 32);
    assert_eq!(
        diag.valid_pixel_count + diag.invalid_pixel_count,
        (diag.working_width * diag.working_height) as usize
    );
    assert!(diag.representative_count > 0);
    assert_eq!(diag.exported_sample_count, support.samples.len());
    assert_relative_eq!(diag.weight_sum, 1.0, epsilon = 1e-9);
    assert!(diag.saliency_stats.max >= diag.saliency_stats.min);

    for sample in &support.samples {
        assert!(sample.weight >= 0.0);
        assert!((0.0..=1.0).contains(&sample.saliency));
    }
}
