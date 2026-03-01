use std::num::{NonZeroU64, NonZeroUsize};

use chromoxide::{
    CapPolicy, GradientMode, GroupAxis, GroupMember, GroupQuantileTerm, GroupTarget, HueDomain,
    Interval, Monotonicity, PaletteProblem, SlotDomain, SlotSpec, SolveConfig, Term,
    WeightedSample, WeightedTerm, solve_with_rng,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn neutral_ladder_is_approximately_monotonic() {
    let mut samples = Vec::new();
    for i in 0..24 {
        let l = 0.1 + 0.8 * (i as f64 / 23.0);
        samples.push(WeightedSample::new(
            chromoxide::Oklch { l, c: 0.01, h: 0.0 }.to_oklab(),
            1.0,
            0.5,
        ));
    }

    let slots = (0..8)
        .map(|i| SlotSpec {
            name: format!("n{i}"),
            domain: SlotDomain {
                lightness: Interval {
                    min: 0.05,
                    max: 0.95,
                },
                chroma: Interval {
                    min: 0.0,
                    max: 0.03,
                },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::Ignore,
                chroma_epsilon: 0.01,
            },
        })
        .collect::<Vec<_>>();

    let problem = PaletteProblem {
        slots,
        samples,
        image_cap: None,
        terms: vec![WeightedTerm {
            weight: 8.0,
            name: Some("neutral-ladder".into()),
            term: Term::GroupQuantile(GroupQuantileTerm {
                members: (0..8).map(|slot| GroupMember { slot, mass: 1.0 }).collect(),
                axis: GroupAxis::Lightness,
                target: GroupTarget::UniformRange {
                    min: 0.12,
                    max: 0.88,
                },
                monotonic: Some(Monotonicity::Increasing { min_gap: 0.04 }),
                huber_delta: 0.02,
            }),
        }],
        config: SolveConfig {
            seed_count: NonZeroUsize::new(12).expect("non-zero"),
            max_iters: NonZeroU64::new(120).expect("non-zero"),
            gradient_mode: GradientMode::FiniteDifferenceCentral,
            fd_epsilon: 1.0e-4,
            keep_top_k: NonZeroUsize::new(4).expect("non-zero"),
            convergence_ftol: 1.0e-9,
            convergence_gtol: 1.0e-6,
            cap_interpolation: chromoxide::CapInterpolation::Bilinear,
        },
    };

    let mut rng = StdRng::seed_from_u64(11);
    let solution = solve_with_rng(&problem, &mut rng).unwrap();
    let lights = solution.colors_lch.iter().map(|c| c.l).collect::<Vec<_>>();
    for i in 0..lights.len() - 1 {
        assert!(lights[i + 1] + 0.02 >= lights[i]);
    }
}
