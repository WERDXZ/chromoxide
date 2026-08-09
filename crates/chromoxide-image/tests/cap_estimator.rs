use std::f64::consts::TAU;

use chromoxide::{ImageCapBuilder, Oklab, Oklch, StatisticalCapConfig};
use chromoxide_image::{
    CapConfig, CapEstimator, CapSource, PreparedImage, PreparedPixel, SaliencyMap, build_image_cap,
};

fn prepared_from_pixels(pixels: Vec<(Oklab, f64)>) -> (PreparedImage, SaliencyMap) {
    let prepared = PreparedImage {
        width: pixels.len() as u32,
        height: 1,
        pixels: pixels
            .iter()
            .map(|&(lab, alpha)| PreparedPixel {
                lab,
                lin_rgb: [0.0, 0.0, 0.0],
                luminance: 0.5,
                alpha,
            })
            .collect(),
        valid_indices: (0..pixels.len()).collect(),
    };
    let saliency = SaliencyMap {
        width: prepared.width,
        height: prepared.height,
        values: vec![1.0; prepared.pixels.len()],
    };
    (prepared, saliency)
}

#[test]
fn default_cap_config_is_statistical_prepared_pixels() {
    let cfg = CapConfig::default();
    assert_eq!(cfg.source, CapSource::PreparedPixels);
    assert!(matches!(&cfg.estimator, CapEstimator::Statistical(_)));
}

#[test]
fn statistical_cap_uses_all_prepared_pixels_not_exported_outliers() {
    let low = Oklch {
        l: 0.5,
        c: 0.04,
        h: 0.0,
    }
    .to_oklab();
    let high = Oklch {
        l: 0.5,
        c: 0.16,
        h: 0.0,
    }
    .to_oklab();

    let mut pixels = vec![(low, 1.0); 99];
    pixels.push((high, 1.0));
    let (prepared, saliency) = prepared_from_pixels(pixels);

    let cap = build_image_cap(
        &prepared,
        &saliency,
        None,
        &CapConfig {
            source: CapSource::PreparedPixels,
            estimator: CapEstimator::Statistical(StatisticalCapConfig {
                percentile: 0.95,
                global_chroma_percentile: 0.90,
                tolerance_factor: 0.0,
                smoothing: 0.0,
                use_conditional_hue: false,
            }),
            builder: ImageCapBuilder {
                n_l: 2,
                n_h: 4,
                smooth_l_radius: 0,
                smooth_h_radius: 0,
                relax: 1.0,
            },
        },
    )
    .unwrap();

    let queried = cap.query(0.5, 0.0);
    assert!(
        (queried - 0.04).abs() < 1.0e-6,
        "statistical cap from prepared pixels should track the low-chroma mass, got {queried}"
    );
    assert!(queried < 0.1);
}

#[test]
fn conditional_hue_does_not_borrow_neighbor_cap() {
    let hue_a = 0.0;
    let hue_b = TAU / 16.0;
    let mut pixels = Vec::new();
    for l in [0.49, 0.51] {
        for _ in 0..50 {
            pixels.push((
                Oklch {
                    l,
                    c: 0.2,
                    h: hue_a,
                }
                .to_oklab(),
                1.0,
            ));
        }
    }
    let (prepared, saliency) = prepared_from_pixels(pixels);

    let cap = build_image_cap(
        &prepared,
        &saliency,
        None,
        &CapConfig {
            source: CapSource::PreparedPixels,
            estimator: CapEstimator::Statistical(StatisticalCapConfig {
                percentile: 1.0,
                global_chroma_percentile: 0.90,
                tolerance_factor: 0.0,
                smoothing: 1.0,
                use_conditional_hue: true,
            }),
            builder: ImageCapBuilder {
                n_l: 2,
                n_h: 16,
                smooth_l_radius: 1,
                smooth_h_radius: 2,
                relax: 1.0,
            },
        },
    )
    .unwrap();

    let cap_a = cap.query(0.5, hue_a);
    let cap_b = cap.query(0.5, hue_b);
    assert!(
        cap_a > 0.1,
        "expected strong cap at supported hue, got {cap_a}"
    );
    assert!(
        cap_b <= 0.25 * cap_a,
        "conditional hue borrowed neighbor cap: A={cap_a}, B={cap_b}"
    );
    assert!(cap_b < 1.0e-6);
}
