use std::num::{NonZeroU64, NonZeroUsize};

use chromoxide::{
    CapPolicy, CoverTerm, GradientMode, HueDomain, Interval, PaletteProblem, SlotDomain, SlotSpec,
    SolveConfig, Term, WeightedSample, WeightedTerm, solve_with_rng,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn two_peak_samples_pull_two_slots_towards_two_modes() {
    let peak_a = chromoxide::Oklch {
        l: 0.42,
        c: 0.14,
        h: 0.3,
    }
    .to_oklab();
    let peak_b = chromoxide::Oklch {
        l: 0.76,
        c: 0.11,
        h: 2.6,
    }
    .to_oklab();

    let mut samples = Vec::new();
    for i in 0..16 {
        let t = i as f64 / 16.0;
        let a = chromoxide::Oklab {
            l: peak_a.l + (t - 0.5) * 0.04,
            a: peak_a.a + (t - 0.5) * 0.02,
            b: peak_a.b - (t - 0.5) * 0.02,
        };
        samples.push(WeightedSample::new(a, 2.0, 0.4));
    }
    for i in 0..16 {
        let t = i as f64 / 16.0;
        let b = chromoxide::Oklab {
            l: peak_b.l + (t - 0.5) * 0.04,
            a: peak_b.a - (t - 0.5) * 0.02,
            b: peak_b.b + (t - 0.5) * 0.02,
        };
        samples.push(WeightedSample::new(b, 2.0, 0.6));
    }

    let slots = vec![
        SlotSpec {
            name: "slot_a".to_string(),
            domain: SlotDomain {
                lightness: Interval { min: 0.2, max: 0.9 },
                chroma: Interval {
                    min: 0.0,
                    max: 0.25,
                },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::Ignore,
                chroma_epsilon: 0.02,
            },
        },
        SlotSpec {
            name: "slot_b".to_string(),
            domain: SlotDomain {
                lightness: Interval { min: 0.2, max: 0.9 },
                chroma: Interval {
                    min: 0.0,
                    max: 0.25,
                },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::Ignore,
                chroma_epsilon: 0.02,
            },
        },
    ];

    let problem = PaletteProblem {
        slots,
        samples,
        image_cap: None,
        terms: vec![WeightedTerm {
            weight: 4.0,
            name: Some("cover".to_string()),
            term: Term::Cover(CoverTerm {
                slots: vec![0, 1],
                tau: 0.01,
                delta: 0.03,
            }),
        }],
        config: SolveConfig {
            seed_count: NonZeroUsize::new(14).expect("non-zero"),
            max_iters: NonZeroU64::new(100).expect("non-zero"),
            gradient_mode: GradientMode::FiniteDifferenceCentral,
            fd_epsilon: 1.0e-4,
            keep_top_k: NonZeroUsize::new(6).expect("non-zero"),
            convergence_ftol: 1.0e-9,
            convergence_gtol: 1.0e-6,
            cap_interpolation: chromoxide::CapInterpolation::Bilinear,
        },
    };

    let mut rng = StdRng::seed_from_u64(123);
    let solution = solve_with_rng(&problem, &mut rng).unwrap();
    assert!(solution.objective.is_finite());

    let c0 = solution.colors[0];
    let c1 = solution.colors[1];

    let assign1 = c0.distance2(peak_a).sqrt() + c1.distance2(peak_b).sqrt();
    let assign2 = c0.distance2(peak_b).sqrt() + c1.distance2(peak_a).sqrt();
    let best = assign1.min(assign2);

    assert!(best < 0.18);
}
