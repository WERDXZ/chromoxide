use std::num::{NonZeroU64, NonZeroUsize};

use chromoxide::{
    CapPolicy, GradientMode, GroupAxis, GroupMember, GroupQuantileTerm, GroupTarget, HueDomain,
    Interval, Monotonicity, PaletteProblem, SlotDomain, SlotSpec, SolveConfig, Term,
    WeightedSample, WeightedTerm, solve,
};

fn main() {
    let samples = (0..40)
        .map(|i| {
            let l = 0.08 + 0.84 * (i as f64 / 39.0);
            WeightedSample::new(
                chromoxide::Oklch { l, c: 0.01, h: 0.0 }.to_oklab(),
                1.0,
                0.5,
            )
        })
        .collect::<Vec<_>>();

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
            weight: 10.0,
            name: Some("lightness_ladder".into()),
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
            seed_count: NonZeroUsize::new(14).expect("non-zero"),
            max_iters: NonZeroU64::new(120).expect("non-zero"),
            gradient_mode: GradientMode::FiniteDifferenceCentral,
            fd_epsilon: 1.0e-4,
            keep_top_k: NonZeroUsize::new(6).expect("non-zero"),
            convergence_ftol: 1.0e-9,
            convergence_gtol: 1.0e-6,
            cap_interpolation: chromoxide::CapInterpolation::Bilinear,
        },
    };

    let solution = solve(&problem).expect("solve failed");
    println!("objective: {:.6}", solution.objective);
    for (i, c) in solution.colors_lch.iter().enumerate() {
        println!("n{i}: L={:.4} C={:.4}", c.l, c.c);
    }
    println!("term breakdown:");
    for t in &solution.term_breakdown {
        println!("  {}: raw={:.6}, weighted={:.6}", t.name, t.raw, t.weighted);
    }
}
