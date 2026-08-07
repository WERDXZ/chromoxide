use std::num::{NonZeroU64, NonZeroUsize};

use chromoxide::{
    CapPolicy, GradientMode, HueDomain, ImageCapBuilder, Interval, PaletteProblem, ScalarTarget,
    SlotDomain, SlotSpec, SolveConfig, StatisticalCapConfig, Term, WeightedSample, WeightedTerm,
    solve_with_rng,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn solver_output_respects_hard_cap() {
    let low_cap_hue = 1.4;
    let mut samples = Vec::new();
    for i in 0..16 {
        let t = i as f64 / 15.0;
        samples.push(WeightedSample::new(
            chromoxide::Oklch {
                l: 0.4 + 0.2 * t,
                c: 0.03,
                h: low_cap_hue,
            }
            .to_oklab(),
            1.0,
            0.5,
        ));
    }

    let cap = ImageCapBuilder {
        n_l: 12,
        n_h: 48,
        smooth_l_radius: 1,
        smooth_h_radius: 1,
        relax: 1.0,
    }
    .build(&samples)
    .unwrap();

    let problem = PaletteProblem {
        slots: vec![SlotSpec {
            name: "hard".to_string(),
            domain: SlotDomain {
                lightness: Interval { min: 0.2, max: 0.8 },
                chroma: Interval { min: 0.0, max: 0.2 },
                hue: HueDomain::Arc {
                    start: low_cap_hue - 0.2,
                    len: 0.4,
                },
                cap_policy: CapPolicy::HardIntersect,
                chroma_epsilon: 0.02,
            },
        }],
        samples,
        image_cap: Some(cap.clone()),
        terms: vec![],
        config: SolveConfig {
            seed_count: NonZeroUsize::new(6).expect("non-zero"),
            max_iters: NonZeroU64::new(40).expect("non-zero"),
            gradient_mode: GradientMode::FiniteDifferenceCentral,
            fd_epsilon: 1.0e-4,
            keep_top_k: NonZeroUsize::new(3).expect("non-zero"),
            convergence_ftol: 1.0e-9,
            convergence_gtol: 1.0e-6,
            cap_interpolation: chromoxide::CapInterpolation::Bilinear,
        },
    };

    let mut rng = StdRng::seed_from_u64(99);
    let solution = solve_with_rng(&problem, &mut rng).unwrap();
    let lch = solution.colors_lch[0];
    let cap_value = cap.query(lch.l, lch.h);
    assert!(lch.c <= cap_value + 1.0e-9);
}

#[test]
fn statistical_cap_builder_downweights_single_outliers() {
    let mut samples = Vec::new();
    for _ in 0..39 {
        samples.push(WeightedSample::new(
            chromoxide::Oklch {
                l: 0.55,
                c: 0.04,
                h: 1.1,
            }
            .to_oklab(),
            1.0,
            0.5,
        ));
    }
    samples.push(WeightedSample::new(
        chromoxide::Oklch {
            l: 0.55,
            c: 0.16,
            h: 1.1,
        }
        .to_oklab(),
        1.0,
        0.5,
    ));

    let builder = ImageCapBuilder {
        n_l: 8,
        n_h: 24,
        smooth_l_radius: 0,
        smooth_h_radius: 0,
        relax: 1.0,
    };
    let max_cap = builder.build(&samples).unwrap();
    let statistical_cap = builder
        .build_statistical(
            &samples,
            StatisticalCapConfig {
                percentile: 0.95,
                tolerance_factor: 0.12,
                smoothing: 0.0,
                use_conditional_hue: false,
            },
        )
        .unwrap();

    let max_value = max_cap.query(0.55, 1.1);
    let statistical_value = statistical_cap.query(0.55, 1.1);
    assert!(max_value > 0.15);
    assert!((statistical_value - 0.0448).abs() < 1.0e-3);
    assert!(statistical_value < max_value);
}

#[test]
fn solver_accepts_statistical_cap_without_prebuilt_image_cap() {
    let samples = vec![
        WeightedSample::new(
            chromoxide::Oklch {
                l: 0.45,
                c: 0.05,
                h: 1.2,
            }
            .to_oklab(),
            1.0,
            0.5,
        ),
        WeightedSample::new(
            chromoxide::Oklch {
                l: 0.5,
                c: 0.055,
                h: 1.2,
            }
            .to_oklab(),
            1.0,
            0.5,
        ),
    ];

    let problem = PaletteProblem {
        slots: vec![SlotSpec {
            name: "stat".to_string(),
            domain: SlotDomain {
                lightness: Interval { min: 0.3, max: 0.7 },
                chroma: Interval { min: 0.0, max: 0.2 },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::Statistical {
                    percentile: 0.95,
                    tolerance_factor: 0.12,
                    smoothing: 1.0,
                    use_conditional_hue: true,
                    penalty_weight: 4.0,
                },
                chroma_epsilon: 0.02,
            },
        }],
        samples,
        image_cap: None,
        terms: vec![WeightedTerm {
            weight: 1.0,
            name: Some("prefer-bright-chroma".into()),
            term: Term::ChromaTarget(chromoxide::ChromaTargetTerm {
                slot: 0,
                target: ScalarTarget::Target {
                    value: 0.12,
                    delta: 0.02,
                },
                hinge_delta: None,
            }),
        }],
        config: SolveConfig::default(),
    };

    let mut rng = StdRng::seed_from_u64(7);
    let solution = solve_with_rng(&problem, &mut rng).unwrap();
    assert!(solution.objective.is_finite());
    assert!(solution.colors_lch[0].c >= 0.0);
}
