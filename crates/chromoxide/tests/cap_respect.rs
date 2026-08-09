use std::num::{NonZeroU64, NonZeroUsize};

use chromoxide::{
    CapPolicy, GradientMode, HueDomain, ImageCapBuilder, Interval, PaletteProblem, SlotDomain,
    SlotSpec, SolveConfig, StatisticalCapConfig, WeightedSample, decode::decode_slot,
    objective::ObjectiveEvaluator, solve_with_rng,
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
                global_chroma_percentile: 0.90,
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

fn constant_cap(c: f64) -> chromoxide::ImageCap {
    let samples =
        vec![
            WeightedSample::new(chromoxide::Oklch { l: 0.5, c, h: 0.0 }.to_oklab(), 1.0, 0.5,);
            20
        ];
    ImageCapBuilder {
        n_l: 2,
        n_h: 4,
        smooth_l_radius: 0,
        smooth_h_radius: 0,
        relax: 1.0,
    }
    .build(&samples)
    .unwrap()
}

fn soft_penalty_problem(cap: chromoxide::ImageCap) -> PaletteProblem {
    PaletteProblem {
        slots: vec![SlotSpec {
            name: "soft".to_string(),
            domain: SlotDomain {
                lightness: Interval { min: 0.3, max: 0.7 },
                chroma: Interval { min: 0.0, max: 0.2 },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::SoftPenalty {
                    weight: 4.0,
                    huber_delta: 0.02,
                },
                chroma_epsilon: 0.02,
            },
        }],
        samples: vec![WeightedSample::new(
            chromoxide::Oklch {
                l: 0.5,
                c: 0.06,
                h: 0.0,
            }
            .to_oklab(),
            1.0,
            0.5,
        )],
        image_cap: Some(cap),
        terms: vec![],
        config: SolveConfig::default(),
    }
}

fn conditional_style_cap() -> chromoxide::ImageCap {
    let mut samples = Vec::new();
    for l in [0.4, 0.6] {
        for _ in 0..50 {
            samples.push(WeightedSample::new(
                chromoxide::Oklch { l, c: 0.06, h: 0.0 }.to_oklab(),
                1.0,
                0.5,
            ));
            samples.push(WeightedSample::new(
                chromoxide::Oklch {
                    l,
                    c: 0.16,
                    h: std::f64::consts::PI,
                }
                .to_oklab(),
                1.0,
                0.5,
            ));
        }
    }

    ImageCapBuilder {
        n_l: 2,
        n_h: 4,
        smooth_l_radius: 0,
        smooth_h_radius: 0,
        relax: 1.0,
    }
    .build_statistical(
        &samples,
        StatisticalCapConfig {
            percentile: 0.90,
            global_chroma_percentile: 0.90,
            tolerance_factor: 0.0,
            smoothing: 0.0,
            use_conditional_hue: true,
        },
    )
    .expect("conditional cap should build")
}

fn policy_problem(cap: chromoxide::ImageCap, cap_policy: CapPolicy, hue: f64) -> PaletteProblem {
    PaletteProblem {
        slots: vec![SlotSpec {
            name: "accent".to_string(),
            domain: SlotDomain {
                lightness: Interval {
                    min: 0.49,
                    max: 0.51,
                },
                chroma: Interval { min: 0.0, max: 0.2 },
                hue: HueDomain::Arc {
                    start: hue - 0.001,
                    len: 0.002,
                },
                cap_policy,
                chroma_epsilon: 0.02,
            },
        }],
        samples: vec![WeightedSample::new(
            chromoxide::Oklch {
                l: 0.5,
                c: 0.1,
                h: hue,
            }
            .to_oklab(),
            1.0,
            0.5,
        )],
        image_cap: Some(cap),
        terms: vec![],
        config: SolveConfig {
            cap_interpolation: chromoxide::CapInterpolation::Nearest,
            ..SolveConfig::default()
        },
    }
}

fn cap_breakdown(problem: &PaletteProblem, name: &str) -> (chromoxide::TermBreakdown, Option<f64>) {
    problem.validate().expect("problem should validate");
    let evaluator = ObjectiveEvaluator::new(problem).expect("evaluator should build");
    let (_, breakdown, decoded) = evaluator
        .evaluate_breakdown(&[0.0, 2.0, 0.0])
        .expect("evaluation should succeed");
    let entry = breakdown
        .into_iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("missing {name} breakdown"));
    (entry, decoded.slots[0].cap_at_lh)
}

#[test]
fn soft_penalty_remains_strict_conditional() {
    let problem = policy_problem(
        conditional_style_cap(),
        CapPolicy::SoftPenalty {
            weight: 8.0,
            huber_delta: 0.02,
        },
        std::f64::consts::FRAC_PI_2,
    );
    let (entry, decoded_cap) = cap_breakdown(&problem, "soft_cap/accent");

    assert_eq!(entry.components.len(), 2);
    assert!(entry.components[0] > 0.15);
    assert_eq!(entry.components[1], 0.0);
    assert_eq!(decoded_cap, Some(0.0));
}

#[test]
fn adaptive_soft_penalty_uses_global_fallback() {
    let problem = policy_problem(
        conditional_style_cap(),
        CapPolicy::AdaptiveSoftPenalty {
            weight: 8.0,
            huber_delta: 0.02,
        },
        std::f64::consts::FRAC_PI_2,
    );
    let (entry, decoded_cap) = cap_breakdown(&problem, "adaptive_soft_cap/accent");

    assert_eq!(entry.components.len(), 5);
    assert!((entry.components[1] - 0.16).abs() < 1.0e-10);
    assert_eq!(entry.components[2], 0.0);
    assert!((entry.components[3] - 0.16).abs() < 1.0e-10);
    assert_eq!(entry.components[4], 0.0);
    assert!(entry.components[0] > 0.01);
    assert_eq!(decoded_cap, Some(entry.components[1]));
}

#[test]
fn adaptive_soft_penalty_uses_conditional_when_supported() {
    let problem = policy_problem(
        conditional_style_cap(),
        CapPolicy::AdaptiveSoftPenalty {
            weight: 8.0,
            huber_delta: 0.02,
        },
        0.0,
    );
    let (entry, decoded_cap) = cap_breakdown(&problem, "adaptive_soft_cap/accent");

    assert_eq!(entry.components.len(), 5);
    assert!((entry.components[1] - 0.06).abs() < 1.0e-10);
    assert!((entry.components[2] - 0.06).abs() < 1.0e-10);
    assert!((entry.components[3] - 0.16).abs() < 1.0e-10);
    assert!((entry.components[4] - 0.5).abs() < 1.0e-10);
    assert!(entry.components[0] > 0.1);
    assert_eq!(decoded_cap, Some(entry.components[1]));
}

#[test]
fn adaptive_soft_penalty_requires_image_cap() {
    let mut problem = policy_problem(
        conditional_style_cap(),
        CapPolicy::AdaptiveSoftPenalty {
            weight: 8.0,
            huber_delta: 0.02,
        },
        0.0,
    );
    problem.image_cap = None;

    let err = problem.validate().unwrap_err().to_string();
    assert!(
        err.contains("AdaptiveSoftPenalty"),
        "unexpected error: {err}"
    );
    assert!(err.contains("image_cap"), "unexpected error: {err}");
}

#[test]
fn adaptive_soft_penalty_validates_parameters() {
    let mut problem = policy_problem(
        conditional_style_cap(),
        CapPolicy::AdaptiveSoftPenalty {
            weight: f64::NAN,
            huber_delta: 0.02,
        },
        0.0,
    );
    let err = problem.validate().unwrap_err().to_string();
    assert!(err.contains("AdaptiveSoftPenalty weight"));

    problem.slots[0].domain.cap_policy = CapPolicy::AdaptiveSoftPenalty {
        weight: 8.0,
        huber_delta: 0.0,
    };
    let err = problem.validate().unwrap_err().to_string();
    assert!(err.contains("AdaptiveSoftPenalty huber_delta"));
}

#[test]
fn soft_penalty_uses_prebuilt_image_cap_and_reports_components() {
    let cap = constant_cap(0.06);
    let problem = soft_penalty_problem(cap.clone());
    problem.validate().expect("problem should validate");
    let evaluator = ObjectiveEvaluator::new(&problem).expect("evaluator should build");

    let inside_latent = vec![0.0, -10.0, 0.0];
    let (_, inside_breakdown, _) = evaluator
        .evaluate_breakdown(&inside_latent)
        .expect("evaluation should succeed");
    let inside = inside_breakdown
        .iter()
        .find(|entry| entry.name == "soft_cap/soft")
        .expect("soft cap breakdown missing");
    assert_eq!(inside.raw, 0.0);
    assert_eq!(inside.components.len(), 2);
    assert_eq!(inside.components[0], 0.0);
    assert!((inside.components[1] - 0.06).abs() < 1.0e-9);

    let outside_latent = vec![0.0, 10.0, 0.0];
    let (_, outside_breakdown, decoded) = evaluator
        .evaluate_breakdown(&outside_latent)
        .expect("evaluation should succeed");
    let outside = outside_breakdown
        .iter()
        .find(|entry| entry.name == "soft_cap/soft")
        .expect("soft cap breakdown missing");
    assert!(outside.raw > 0.0);
    assert!(outside.components[0] > 0.0);
    assert!((outside.components[1] - 0.06).abs() < 1.0e-9);
    let expected_overflow = (decoded.slots[0].lch.c - 0.06).max(0.0);
    assert!((outside.components[0] - expected_overflow).abs() < 1.0e-12);
}

#[test]
fn hard_intersect_cannot_lower_user_chroma_min() {
    let cap = constant_cap(0.03);
    let domain = SlotDomain {
        lightness: Interval { min: 0.4, max: 0.6 },
        chroma: Interval {
            min: 0.08,
            max: 0.2,
        },
        hue: HueDomain::Any,
        cap_policy: CapPolicy::HardIntersect,
        chroma_epsilon: 0.02,
    };
    let problem = PaletteProblem {
        slots: vec![SlotSpec {
            name: "hard-min".to_string(),
            domain,
        }],
        samples: vec![WeightedSample::new(
            chromoxide::Oklch {
                l: 0.5,
                c: 0.03,
                h: 0.0,
            }
            .to_oklab(),
            1.0,
            0.5,
        )],
        image_cap: Some(cap.clone()),
        terms: vec![],
        config: SolveConfig::default(),
    };
    let err = problem
        .validate()
        .expect_err("must reject infeasible hard cap");
    let message = err.to_string();
    assert!(message.contains("hard-min"), "unexpected error: {message}");
    assert!(message.contains("0.08"));

    let decode_err = decode_slot(&domain, 0.0, 0.0, 0.0, Some(&cap))
        .expect_err("single-slot decode must reject cap below user min");
    assert!(decode_err.to_string().contains("HardIntersect"));
}

#[test]
fn hard_intersect_preserves_user_min_when_feasible() {
    let cap = constant_cap(0.04);
    let domain = SlotDomain {
        lightness: Interval { min: 0.4, max: 0.6 },
        chroma: Interval {
            min: 0.01,
            max: 0.2,
        },
        hue: HueDomain::Any,
        cap_policy: CapPolicy::HardIntersect,
        chroma_epsilon: 0.02,
    };
    let decoded = decode_slot(&domain, 0.0, 8.0, 0.0, Some(&cap)).unwrap();
    assert!((decoded.effective_chroma_min - 0.01).abs() < 1.0e-12);
    assert!(decoded.effective_chroma_max <= 0.04 + 1.0e-9);
    assert!(decoded.effective_chroma_max >= 0.01 - 1.0e-9);
}
