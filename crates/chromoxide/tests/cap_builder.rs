use chromoxide::{CapBiasCurve, CapInterpolation, ImageCapBuilder, Oklch, WeightedSample};

#[test]
fn cap_builder_produces_interpolatable_surface() {
    let samples = vec![
        WeightedSample::new(
            Oklch {
                l: 0.4,
                c: 0.12,
                h: 0.05,
            }
            .to_oklab(),
            2.0,
            0.2,
        ),
        WeightedSample::new(
            Oklch {
                l: 0.6,
                c: 0.18,
                h: 3.1,
            }
            .to_oklab(),
            3.0,
            0.7,
        ),
    ];

    let cap = ImageCapBuilder {
        n_l: 10,
        n_h: 36,
        smooth_l_radius: 1,
        smooth_h_radius: 1,
        relax: 1.0,
    }
    .build(&samples)
    .unwrap();

    let d = cap.diagnostics();
    assert!(d.max_after_smooth > 0.05);
    assert!(d.mean_after_smooth >= 0.0);

    let q1 = cap.query(0.4, 0.01);
    let q2 = cap.query(0.4, std::f64::consts::TAU - 0.01);
    assert!(q1.is_finite());
    assert!(q2.is_finite());
    assert!((q1 - q2).abs() < 0.08);
}

#[test]
fn cap_query_supports_nearest_and_biased_modes() {
    let samples = vec![
        WeightedSample::new(
            Oklch {
                l: 0.2,
                c: 0.10,
                h: 0.10,
            }
            .to_oklab(),
            1.0,
            0.5,
        ),
        WeightedSample::new(
            Oklch {
                l: 0.2,
                c: 0.50,
                h: 3.40,
            }
            .to_oklab(),
            1.0,
            0.5,
        ),
        WeightedSample::new(
            Oklch {
                l: 0.8,
                c: 0.20,
                h: 0.10,
            }
            .to_oklab(),
            1.0,
            0.5,
        ),
        WeightedSample::new(
            Oklch {
                l: 0.8,
                c: 0.80,
                h: 3.40,
            }
            .to_oklab(),
            1.0,
            0.5,
        ),
    ];

    let cap = ImageCapBuilder {
        n_l: 2,
        n_h: 2,
        smooth_l_radius: 0,
        smooth_h_radius: 0,
        relax: 1.0,
    }
    .build(&samples)
    .unwrap();

    let l = 0.5;
    let h = std::f64::consts::FRAC_PI_2;

    let bilinear = cap.query_with(l, h, CapInterpolation::Bilinear);
    let nearest = cap.query_with(l, h, CapInterpolation::Nearest);
    let prefer_high = cap.query_with(
        l,
        h,
        CapInterpolation::BilinearBiased {
            alpha: 1.0,
            curve: CapBiasCurve::Linear,
        },
    );
    let prefer_low = cap.query_with(
        l,
        h,
        CapInterpolation::BilinearBiased {
            alpha: -1.0,
            curve: CapBiasCurve::Smoothstep,
        },
    );
    let bezier_high = cap.query_with(
        l,
        h,
        CapInterpolation::BilinearBiased {
            alpha: 0.7,
            curve: CapBiasCurve::Bezier01 { c1: 0.2, c2: 0.8 },
        },
    );

    assert!((bilinear - 0.40).abs() < 1.0e-6);
    let corner_values = [0.10, 0.20, 0.50, 0.80];
    assert!(corner_values.iter().any(|&v| (nearest - v).abs() < 1.0e-12));
    assert!((nearest - bilinear).abs() > 0.05);
    assert!(prefer_high >= bilinear && prefer_high <= 0.80 + 1.0e-12);
    assert!(prefer_low <= bilinear && prefer_low >= 0.10 - 1.0e-12);
    assert!(bezier_high >= bilinear);
}
