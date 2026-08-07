use std::collections::HashSet;
use std::num::{NonZeroU32, NonZeroUsize};

use chromoxide_image::{
    KMeansPlusPlusLabConfig, PreprocessConfig, ResizeFilter, SaliencyConfig, SamplingConfig,
    SamplingMethod, compute_saliency, prepare_image, select_representatives_with_rng,
};
use image::{DynamicImage, Rgba, RgbaImage};
use rand::SeedableRng;
use rand::rngs::StdRng;

fn kmeans_cfg(count: usize, stride: u32, bias: f64) -> SamplingConfig {
    SamplingConfig {
        method: SamplingMethod::KMeansPlusPlusLab(KMeansPlusPlusLabConfig {
            count: NonZeroUsize::new(count).expect("non-zero"),
            candidate_stride: NonZeroU32::new(stride).expect("non-zero"),
            saliency_bias: bias,
            max_iters: NonZeroUsize::new(30).expect("non-zero"),
            convergence_tol: 1.0e-7,
        }),
    }
}

fn prepare_uniform_rgba(width: u32, height: u32, rgb: [u8; 3]) -> (DynamicImage, usize) {
    let mut rgba = RgbaImage::new(width, height);
    for pixel in rgba.pixels_mut() {
        *pixel = Rgba([rgb[0], rgb[1], rgb[2], 255]);
    }
    (DynamicImage::ImageRgba8(rgba), (width * height) as usize)
}

fn prepared_support(
    img: DynamicImage,
) -> (
    chromoxide_image::PreparedImage,
    chromoxide_image::SaliencyMap,
) {
    let prepared = prepare_image(
        &img,
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
    (prepared, saliency)
}

#[test]
fn kmeans_is_deterministic_with_seeded_rng() {
    let (img, _) = prepare_uniform_rgba(16, 12, [90, 140, 210]);
    let (prepared, saliency) = prepared_support(img);
    let cfg = kmeans_cfg(6, 2, 0.35);

    let mut rng_a = StdRng::seed_from_u64(99);
    let mut rng_b = StdRng::seed_from_u64(99);
    let a = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng_a).unwrap();
    let b = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng_b).unwrap();

    assert_eq!(a.len(), b.len());
    for (ra, rb) in a.iter().zip(b.iter()) {
        assert_eq!(ra.pixel_index, rb.pixel_index);
        assert_eq!(ra.lab, rb.lab);
    }
}

#[test]
fn kmeans_centers_split_two_color_clusters() {
    let mut rgba = RgbaImage::new(20, 10);
    for y in 0..10 {
        for x in 0..20 {
            let color = if x < 10 { [220, 30, 30] } else { [30, 60, 220] };
            rgba.put_pixel(x, y, Rgba([color[0], color[1], color[2], 255]));
        }
    }
    let (prepared, saliency) = prepared_support(DynamicImage::ImageRgba8(rgba));
    let red_lab = prepared.pixels[0].lab;
    let blue_lab = prepared.pixels[10].lab;

    let cfg = kmeans_cfg(2, 1, 0.0);
    let mut rng = StdRng::seed_from_u64(11);
    let reps = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng).unwrap();

    assert_eq!(reps.len(), 2);
    for rep in &reps {
        let d_red = rep.lab.distance2(red_lab);
        let d_blue = rep.lab.distance2(blue_lab);
        assert!(
            d_red < 1.0e-8 || d_blue < 1.0e-8,
            "center {rep:?} is not near either true cluster color"
        );
    }
}

#[test]
fn kmeans_pads_candidate_pool_beyond_stride() {
    let (img, _) = prepare_uniform_rgba(12, 1, [70, 90, 40]);
    let (prepared, saliency) = prepared_support(img);
    // Stride 5 yields 3 candidates initially; count 6 must be padded.
    let cfg = kmeans_cfg(6, 5, 0.0);
    let mut rng = StdRng::seed_from_u64(3);
    let reps = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng).unwrap();
    assert_eq!(reps.len(), 6);
}

#[test]
fn kmeans_anchor_indices_are_unique() {
    let (img, _) = prepare_uniform_rgba(20, 8, [200, 120, 60]);
    let (prepared, saliency) = prepared_support(img);
    let cfg = kmeans_cfg(9, 3, 0.0);
    let mut rng = StdRng::seed_from_u64(5);
    let reps = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng).unwrap();

    let anchors: HashSet<usize> = reps.iter().map(|rep| rep.pixel_index).collect();
    assert_eq!(anchors.len(), reps.len());
}

#[test]
fn kmeans_handles_empty_clusters_without_panic_or_nan() {
    // A uniform image forces several identical seeds, so Lloyd must repair
    // empty clusters without producing NaN centers.
    let (img, _) = prepare_uniform_rgba(16, 16, [120, 160, 40]);
    let (prepared, saliency) = prepared_support(img);
    let cfg = kmeans_cfg(4, 1, 0.0);
    let mut rng = StdRng::seed_from_u64(7);
    let reps = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng).unwrap();

    assert_eq!(reps.len(), 4);
    for rep in &reps {
        assert!(rep.lab.l.is_finite());
        assert!(rep.lab.a.is_finite());
        assert!(rep.lab.b.is_finite());
        assert!(rep.pixel_index < prepared.pixels.len());
    }
}

#[test]
fn kmeans_rejects_negative_saliency_bias() {
    let (img, _) = prepare_uniform_rgba(8, 8, [30, 60, 90]);
    let (prepared, saliency) = prepared_support(img);
    let cfg = kmeans_cfg(2, 1, -0.1);
    let mut rng = StdRng::seed_from_u64(1);
    let err = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng).unwrap_err();
    assert!(err.to_string().contains("saliency_bias"));
}

#[test]
fn kmeans_rejects_non_positive_convergence_tol() {
    let (img, _) = prepare_uniform_rgba(8, 8, [30, 60, 90]);
    let (prepared, saliency) = prepared_support(img);
    let mut cfg = kmeans_cfg(2, 1, 0.0);
    match &mut cfg.method {
        SamplingMethod::KMeansPlusPlusLab(km) => km.convergence_tol = 0.0,
        _ => unreachable!(),
    }
    let mut rng = StdRng::seed_from_u64(1);
    let err = select_representatives_with_rng(&prepared, &saliency, &cfg, &mut rng).unwrap_err();
    assert!(err.to_string().contains("convergence_tol"));
}
