use std::num::{NonZeroU64, NonZeroUsize};

use chromoxide::{
    solve, CapPolicy, ContrastTerm, CoverTerm, GradientMode, GroupAxis, GroupMember,
    GroupQuantileTerm, GroupTarget, HueDomain, Interval, Monotonicity, OrderRelation,
    PairOrderTerm, PaletteProblem, SlotDomain, SlotSpec, SolveConfig, Term, WeightedSample,
    WeightedTerm,
};

fn main() {
    let mut samples = Vec::new();
    for i in 0..64 {
        let t = i as f64 / 63.0;
        let l = 0.18 + 0.7 * t;
        let c = 0.02 + 0.12 * (1.0 - (2.0 * t - 1.0).abs());
        let h = 0.3 + 3.8 * t;
        samples.push(WeightedSample::new(
            chromoxide::Oklch { l, c, h }.to_oklab(),
            1.0,
            t,
        ));
    }

    let slots = vec![
        SlotSpec {
            name: "bg".into(),
            domain: SlotDomain {
                lightness: Interval {
                    min: 0.08,
                    max: 0.45,
                },
                chroma: Interval {
                    min: 0.0,
                    max: 0.08,
                },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::Ignore,
                chroma_epsilon: 0.02,
            },
        },
        SlotSpec {
            name: "surface".into(),
            domain: SlotDomain {
                lightness: Interval { min: 0.2, max: 0.7 },
                chroma: Interval {
                    min: 0.0,
                    max: 0.12,
                },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::Ignore,
                chroma_epsilon: 0.02,
            },
        },
        SlotSpec {
            name: "text".into(),
            domain: SlotDomain {
                lightness: Interval {
                    min: 0.55,
                    max: 0.98,
                },
                chroma: Interval {
                    min: 0.0,
                    max: 0.08,
                },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::Ignore,
                chroma_epsilon: 0.02,
            },
        },
        SlotSpec {
            name: "accent".into(),
            domain: SlotDomain {
                lightness: Interval {
                    min: 0.3,
                    max: 0.85,
                },
                chroma: Interval {
                    min: 0.06,
                    max: 0.22,
                },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::Ignore,
                chroma_epsilon: 0.02,
            },
        },
    ];

    let terms = vec![
        WeightedTerm {
            weight: 3.0,
            name: Some("cover".into()),
            term: Term::Cover(CoverTerm {
                slots: vec![0, 1, 2, 3],
                tau: 0.02,
                delta: 0.03,
            }),
        },
        WeightedTerm {
            weight: 4.0,
            name: Some("lightness-spread".into()),
            term: Term::GroupQuantile(GroupQuantileTerm {
                members: vec![
                    GroupMember { slot: 0, mass: 2.0 },
                    GroupMember { slot: 1, mass: 2.0 },
                    GroupMember { slot: 2, mass: 1.5 },
                    GroupMember { slot: 3, mass: 1.0 },
                ],
                axis: GroupAxis::Lightness,
                target: GroupTarget::UniformRange {
                    min: 0.15,
                    max: 0.9,
                },
                monotonic: Some(Monotonicity::Increasing { min_gap: 0.07 }),
                huber_delta: 0.03,
            }),
        },
        WeightedTerm {
            weight: 4.0,
            name: Some("text-bg-contrast".into()),
            term: Term::Contrast(ContrastTerm {
                fg: 2,
                bg: 0,
                min_ratio: 4.5,
                hinge_delta: None,
            }),
        },
        WeightedTerm {
            weight: 2.0,
            name: Some("text-order".into()),
            term: Term::Order(PairOrderTerm {
                a: 2,
                b: 0,
                relation: OrderRelation::BrighterBy { delta: 0.25 },
                hinge_delta: None,
            }),
        },
    ];

    let problem = PaletteProblem {
        slots,
        samples,
        image_cap: None,
        terms,
        config: SolveConfig {
            seed_count: NonZeroUsize::new(20).expect("non-zero"),
            max_iters: NonZeroU64::new(140).expect("non-zero"),
            gradient_mode: GradientMode::FiniteDifferenceCentral,
            fd_epsilon: 1.0e-4,
            keep_top_k: NonZeroUsize::new(8).expect("non-zero"),
            convergence_ftol: 1.0e-9,
            convergence_gtol: 1.0e-6,
            cap_interpolation: chromoxide::CapInterpolation::Bilinear,
        },
    };

    let solution = solve(&problem).expect("solve failed");
    println!("objective: {:.6}", solution.objective);
    for (i, c) in solution.colors_lch.iter().enumerate() {
        println!("slot {i}: L={:.4} C={:.4} h={:.4}", c.l, c.c, c.h);
    }
    println!("term breakdown:");
    for t in &solution.term_breakdown {
        println!("  {}: raw={:.6}, weighted={:.6}", t.name, t.raw, t.weighted);
    }
}
