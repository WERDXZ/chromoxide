use std::num::{NonZeroU64, NonZeroUsize};

use chromoxide::{
    CapPolicy, GradientMode, HueDomain, ImageCapBuilder, Interval, PaletteProblem, SlotDomain,
    SlotSpec, SolveConfig, WeightedSample, solve_with_rng,
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
