use std::num::NonZeroU32;

use chromoxide_image::{PreprocessConfig, ResizeFilter, prepare_image};
use image::{DynamicImage, Rgba, RgbaImage};

#[test]
fn preprocess_resize_preserves_aspect_ratio() {
    let mut rgba = RgbaImage::new(400, 200);
    for pixel in rgba.pixels_mut() {
        *pixel = Rgba([64, 128, 192, 255]);
    }

    let cfg = PreprocessConfig {
        max_working_dim: Some(NonZeroU32::new(100).expect("non-zero")),
        resize_filter: ResizeFilter::Triangle,
        background_rgb8: [255, 255, 255],
        min_alpha: 0.0,
        alpha_into_weight: false,
    };

    let prepared = prepare_image(&DynamicImage::ImageRgba8(rgba), &cfg).unwrap();
    assert_eq!(prepared.width, 100);
    assert_eq!(prepared.height, 50);
    assert_eq!(prepared.pixels.len(), 5_000);
    assert_eq!(prepared.valid_indices.len(), 5_000);
}
