use std::num::{NonZeroU32, NonZeroUsize};

use chromoxide_image::{
    FarthestPointLabConfig, PreprocessConfig, ResizeFilter, SaliencyConfig, SamplingConfig,
    SamplingMethod, compute_saliency, prepare_image, select_representatives_with_rng,
};
use image::{DynamicImage, Rgba, RgbaImage};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn farthest_point_lab_covers_two_color_clusters() {
    let mut rgba = RgbaImage::new(20, 10);
    for y in 0..10 {
        for x in 0..20 {
            if x < 10 {
                rgba.put_pixel(x, y, Rgba([220, 30, 30, 255]));
            } else {
                rgba.put_pixel(x, y, Rgba([30, 60, 220, 255]));
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

    let mut rng = StdRng::seed_from_u64(11);
    let reps = select_representatives_with_rng(
        &prepared,
        &saliency,
        &SamplingConfig {
            method: SamplingMethod::FarthestPointLab(FarthestPointLabConfig {
                count: NonZeroUsize::new(2).expect("non-zero"),
                candidate_stride: NonZeroU32::new(1).expect("non-zero"),
                saliency_bias: 0.0,
            }),
        },
        &mut rng,
    )
    .unwrap();

    assert_eq!(reps.len(), 2);
    let xs: Vec<u32> = reps
        .iter()
        .map(|rep| (rep.pixel_index as u32) % prepared.width)
        .collect();
    assert!(xs.iter().any(|&x| x < 10));
    assert!(xs.iter().any(|&x| x >= 10));
}
