use std::num::NonZeroU32;

use chromoxide_image::{
    PreprocessConfig, ResizeFilter, SaliencyConfig, SamplingConfig, SamplingMethod,
    StratifiedConfig, compute_saliency, prepare_image, select_representatives_with_rng,
};
use image::{DynamicImage, Rgba, RgbaImage};
use rand::SeedableRng;
use rand::rngs::StdRng;

fn rep_indices(reps: &[chromoxide_image::Representative]) -> Vec<usize> {
    reps.iter().map(|rep| rep.pixel_index).collect()
}

#[test]
fn stratified_is_deterministic_with_fixed_seed() {
    let mut rgba = RgbaImage::new(20, 12);
    for pixel in rgba.pixels_mut() {
        *pixel = Rgba([60, 100, 180, 255]);
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
        method: SamplingMethod::Stratified(StratifiedConfig {
            tiles_x: NonZeroU32::new(4).expect("non-zero"),
            tiles_y: NonZeroU32::new(3).expect("non-zero"),
            per_tile: NonZeroU32::new(2).expect("non-zero"),
        }),
    };

    let mut rng_a = StdRng::seed_from_u64(42);
    let mut rng_b = StdRng::seed_from_u64(42);
    let a = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng_a).unwrap();
    let b = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng_b).unwrap();
    assert_eq!(rep_indices(&a), rep_indices(&b));
}
