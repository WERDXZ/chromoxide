use std::num::NonZeroUsize;

use chromoxide_image::{
    PreprocessConfig, RandomUniformConfig, ResizeFilter, SaliencyConfig, SamplingConfig,
    SamplingMethod, compute_saliency, prepare_image, select_representatives_with_rng,
};
use image::{DynamicImage, Rgba, RgbaImage};
use rand::SeedableRng;
use rand::rngs::StdRng;

fn rep_indices(reps: &[chromoxide_image::Representative]) -> Vec<usize> {
    reps.iter().map(|rep| rep.pixel_index).collect()
}

#[test]
fn random_uniform_is_deterministic_and_bounded() {
    let mut rgba = RgbaImage::new(12, 10);
    for pixel in rgba.pixels_mut() {
        *pixel = Rgba([90, 200, 40, 255]);
    }

    let prepared = prepare_image(
        &DynamicImage::ImageRgba8(rgba),
        &PreprocessConfig {
            max_working_dim: None,
            resize_filter: ResizeFilter::Nearest,
            background_rgb8: [255, 255, 255],
            min_alpha: 0.0,
            alpha_into_weight: false,
        },
    )
    .unwrap();
    let saliency = compute_saliency(&prepared, &SaliencyConfig::default()).unwrap();

    let cfg = SamplingConfig {
        method: SamplingMethod::RandomUniform(RandomUniformConfig {
            count: NonZeroUsize::new(20).expect("non-zero"),
        }),
    };

    let mut rng_a = StdRng::seed_from_u64(7);
    let mut rng_b = StdRng::seed_from_u64(7);
    let a = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng_a).unwrap();
    let b = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng_b).unwrap();

    assert_eq!(rep_indices(&a), rep_indices(&b));
    assert_eq!(a.len(), 20);
    assert!(a.len() <= prepared.valid_indices.len());
}
