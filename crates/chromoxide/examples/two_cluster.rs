use std::num::{NonZeroU64, NonZeroUsize};

use chromoxide::{
    CapPolicy, CoverTerm, GradientMode, HueDomain, Interval, PaletteProblem, SlotDomain, SlotSpec,
    SolveConfig, Term, WeightedSample, WeightedTerm, solve,
};

fn main() {
    let peak_a = chromoxide::Oklch {
        l: 0.38,
        c: 0.16,
        h: 0.4,
    }
    .to_oklab();
    let peak_b = chromoxide::Oklch {
        l: 0.78,
        c: 0.1,
        h: 3.0,
    }
    .to_oklab();

    let mut samples = Vec::new();
    for i in 0..20 {
        let t = i as f64 / 20.0 - 0.5;
        samples.push(WeightedSample::new(
            chromoxide::Oklab {
                l: peak_a.l + 0.05 * t,
                a: peak_a.a + 0.02 * t,
                b: peak_a.b - 0.02 * t,
            },
            2.0,
            0.4,
        ));
    }
    for i in 0..20 {
        let t = i as f64 / 20.0 - 0.5;
        samples.push(WeightedSample::new(
            chromoxide::Oklab {
                l: peak_b.l + 0.05 * t,
                a: peak_b.a - 0.02 * t,
                b: peak_b.b + 0.02 * t,
            },
            2.0,
            0.7,
        ));
    }

    let problem = PaletteProblem {
        slots: vec![
            SlotSpec {
                name: "primary".into(),
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
                name: "secondary".into(),
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
        ],
        samples,
        image_cap: None,
        terms: vec![WeightedTerm {
            weight: 4.0,
            name: Some("cover".into()),
            term: Term::Cover(CoverTerm {
                slots: vec![0, 1],
                tau: 0.015,
                delta: 0.03,
            }),
        }],
        config: SolveConfig {
            seed_count: NonZeroUsize::new(16).expect("non-zero"),
            max_iters: NonZeroU64::new(120).expect("non-zero"),
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
