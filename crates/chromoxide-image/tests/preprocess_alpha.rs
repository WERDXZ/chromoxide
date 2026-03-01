use approx::assert_relative_eq;
use chromoxide_image::{PreprocessConfig, ResizeFilter, prepare_image};
use image::{DynamicImage, Rgba, RgbaImage};

fn srgb_to_linear(v: u8) -> f64 {
    let x = f64::from(v) / 255.0;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn preprocess_alpha_composites_in_linear_srgb() {
    let mut rgba = RgbaImage::new(1, 1);
    rgba.put_pixel(0, 0, Rgba([255, 0, 0, 128]));

    let cfg = PreprocessConfig {
        max_working_dim: None,
        resize_filter: ResizeFilter::Nearest,
        background_rgb8: [0, 0, 255],
        min_alpha: 0.0,
        alpha_into_weight: true,
    };

    let prepared = prepare_image(&DynamicImage::ImageRgba8(rgba), &cfg).unwrap();
    let px = prepared.pixels[0];
    let alpha = 128.0 / 255.0;

    let expected_r = alpha * srgb_to_linear(255) + (1.0 - alpha) * srgb_to_linear(0);
    let expected_g = alpha * srgb_to_linear(0) + (1.0 - alpha) * srgb_to_linear(0);
    let expected_b = alpha * srgb_to_linear(0) + (1.0 - alpha) * srgb_to_linear(255);

    assert_relative_eq!(px.lin_rgb[0], expected_r, epsilon = 1e-12);
    assert_relative_eq!(px.lin_rgb[1], expected_g, epsilon = 1e-12);
    assert_relative_eq!(px.lin_rgb[2], expected_b, epsilon = 1e-12);
    assert_relative_eq!(px.alpha, alpha, epsilon = 1e-12);
    assert_eq!(prepared.valid_indices, vec![0]);
}
