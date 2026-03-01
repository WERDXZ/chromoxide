use std::collections::HashSet;
use std::num::NonZeroU32;

use chromoxide_image::{
    PreprocessConfig, ResizeFilter, SaliencyConfig, SamplingConfig, SamplingMethod,
    UniformGridConfig, compute_saliency, prepare_image, select_representatives,
};
use image::{DynamicImage, Rgba, RgbaImage};

#[test]
fn uniform_grid_returns_unique_valid_indices() {
    let mut rgba = RgbaImage::new(16, 16);
    for pixel in rgba.pixels_mut() {
        *pixel = Rgba([120, 160, 40, 255]);
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
    let reps = select_representatives(
        &prepared,
        &saliency,
        &SamplingConfig {
            method: SamplingMethod::UniformGrid(UniformGridConfig {
                step: NonZeroU32::new(4).expect("non-zero"),
            }),
        },
    )
    .unwrap();

    assert!(!reps.is_empty());
    assert!(reps.len() <= 16);

    let unique: HashSet<usize> = reps.iter().map(|rep| rep.pixel_index).collect();
    assert_eq!(unique.len(), reps.len());
    assert!(
        reps.iter()
            .all(|rep| rep.pixel_index < prepared.pixels.len())
    );
}
