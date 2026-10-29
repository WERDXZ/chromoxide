use chromoxide::{CapInterpolation, ImageCapBuilder, Oklch, StatisticalCapConfig, WeightedSample};

fn samples(red_area: f64) -> Vec<WeightedSample> {
    vec![
        WeightedSample::new(
            Oklch {
                l: 0.60,
                c: 0.01,
                h: std::f64::consts::PI,
            }
            .to_oklab(),
            1.0 - red_area,
            0.0,
        ),
        WeightedSample::new(
            Oklch {
                l: 0.56,
                c: 0.20,
                h: 0.0,
            }
            .to_oklab(),
            red_area,
            1.0,
        ),
    ]
}

#[test]
fn sparse_chromatic_row_cannot_lend_its_cap_to_unsupported_hues() {
    for red_area in [1.0 / 5120.0, 0.0125] {
        for n_l in [8, 24, 48] {
            for smoothing in [0.0, 1.0] {
                let cap = ImageCapBuilder {
                    n_l,
                    ..Default::default()
                }
                .build_statistical(
                    &samples(red_area),
                    StatisticalCapConfig {
                        smoothing,
                        ..Default::default()
                    },
                )
                .unwrap();
                for step in 0..=32 {
                    let l = 0.56 + 0.04 * f64::from(step) / 32.0;
                    let query = cap.query_adaptive_with(
                        l,
                        std::f64::consts::FRAC_PI_2,
                        CapInterpolation::Bilinear,
                    );
                    assert_eq!(query.support_confidence, 0.0);
                    assert!(
                        query.chroma <= 0.0112 + 1e-12,
                        "area={red_area} rows={n_l} smoothing={smoothing} L={l}: {query:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn directly_supported_minority_keeps_its_chroma_even_when_fallback_is_low() {
    let cap = ImageCapBuilder::default()
        .build_statistical(&samples(0.0125), StatisticalCapConfig::default())
        .unwrap();
    let query = cap.query_adaptive_with(0.56, 0.0, CapInterpolation::Nearest);
    assert_eq!(query.support_confidence, 1.0);
    assert!(query.chroma > 0.20);
    assert!(query.global_chroma <= 0.0112 + 1e-12);
}

#[test]
fn abundant_chroma_and_monochromatic_vivid_images_keep_their_fallback() {
    let mut images = vec![samples(0.2)];
    images.push(vec![samples(0.2).remove(1)]);
    for samples in images {
        let cap = ImageCapBuilder::default()
            .build_statistical(&samples, StatisticalCapConfig::default())
            .unwrap();
        assert!(cap.query_global_chroma(0.56) > 0.20);
    }
}

#[test]
fn fallback_uses_mass_not_sample_count_or_absolute_weight_scale() {
    let evidence = samples(0.0125);
    let builder = ImageCapBuilder::default();
    let expected = builder
        .build_statistical(&evidence, StatisticalCapConfig::default())
        .unwrap();
    for scale in [1.0, 5120.0] {
        let mut split = vec![
            WeightedSample {
                weight: scale * evidence[1].weight / 17.0,
                ..evidence[1]
            };
            17
        ];
        split.push(WeightedSample {
            weight: scale * evidence[0].weight,
            ..evidence[0]
        });
        let actual = builder
            .build_statistical(&split, StatisticalCapConfig::default())
            .unwrap();
        for (a, b) in actual
            .global_chroma_by_lightness
            .iter()
            .zip(&expected.global_chroma_by_lightness)
        {
            assert!((a - b).abs() < 1e-12);
        }
    }
}
