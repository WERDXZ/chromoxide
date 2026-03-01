use approx::assert_relative_eq;
use chromoxide_image::{
    CenterMode, ExportConfig, PreprocessConfig, Representative, ResizeFilter, SaliencyConfig,
    compute_saliency, export_samples, prepare_image,
};
use image::{DynamicImage, Rgba, RgbaImage};

#[test]
fn export_samples_normalized_weight_sum_is_one() {
    let mut rgba = RgbaImage::new(8, 8);
    for y in 0..8 {
        for x in 0..8 {
            if x < 4 {
                rgba.put_pixel(x, y, Rgba([220, 40, 40, 255]));
            } else {
                rgba.put_pixel(x, y, Rgba([30, 70, 220, 255]));
            }
        }
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

    let reps = vec![
        Representative { pixel_index: 1 },
        Representative { pixel_index: 6 },
    ];

    let samples = export_samples(
        &prepared,
        &saliency,
        &reps,
        &ExportConfig {
            center_mode: CenterMode::Centroid,
            normalize_weights: true,
            saliency_to_weight_mix: 0.0,
            saliency_weight_gamma: 1.0,
            frequency_gamma: 1.0,
            min_cluster_weight: 0.0,
        },
    )
    .unwrap();

    assert!(!samples.is_empty());
    let weight_sum: f64 = samples.iter().map(|sample| sample.weight).sum();
    assert_relative_eq!(weight_sum, 1.0, epsilon = 1e-9);
    for sample in &samples {
        assert!((0.0..=1.0).contains(&sample.saliency));
    }
}
