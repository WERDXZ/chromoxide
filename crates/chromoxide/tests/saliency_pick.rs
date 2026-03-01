use std::num::{NonZeroU64, NonZeroUsize};

use chromoxide::{
    CapPolicy, CoverTerm, GradientMode, HueDomain, Interval, PaletteProblem, SaliencyTarget,
    SaliencyTerm, SlotDomain, SlotSpec, SolveConfig, Term, WeightedSample, WeightedTerm,
    solve_with_rng,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn saliency_term_pulls_one_slot_to_salient_region() {
    let base_peak = chromoxide::Oklch {
        l: 0.55,
        c: 0.06,
        h: 1.2,
    }
    .to_oklab();
    let salient_peak = chromoxide::Oklch {
        l: 0.72,
        c: 0.15,
        h: 5.5,
    }
    .to_oklab();

    let mut samples = Vec::new();
    for i in 0..28 {
        let t = i as f64 / 28.0 - 0.5;
        samples.push(WeightedSample::new(
            chromoxide::Oklab {
                l: base_peak.l + 0.03 * t,
                a: base_peak.a + 0.01 * t,
                b: base_peak.b - 0.01 * t,
            },
            4.0,
            0.2,
        ));
    }
    for i in 0..6 {
        let t = i as f64 / 6.0 - 0.5;
        samples.push(WeightedSample::new(
            chromoxide::Oklab {
                l: salient_peak.l + 0.02 * t,
                a: salient_peak.a - 0.01 * t,
                b: salient_peak.b + 0.01 * t,
            },
            1.0,
            1.0,
        ));
    }

    let slots = vec![
        SlotSpec {
            name: "main".to_string(),
            domain: SlotDomain {
                lightness: Interval {
                    min: 0.25,
                    max: 0.9,
                },
                chroma: Interval {
                    min: 0.0,
                    max: 0.24,
                },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::Ignore,
                chroma_epsilon: 0.02,
            },
        },
        SlotSpec {
            name: "accent".to_string(),
            domain: SlotDomain {
                lightness: Interval {
                    min: 0.25,
                    max: 0.9,
                },
                chroma: Interval {
                    min: 0.0,
                    max: 0.24,
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
        terms: vec![
            WeightedTerm {
                weight: 3.0,
                name: Some("cover".into()),
                term: Term::Cover(CoverTerm {
                    slots: vec![0, 1],
                    tau: 0.02,
                    delta: 0.03,
                }),
            },
            WeightedTerm {
                weight: 2.0,
                name: Some("saliency-accent".into()),
                term: Term::Saliency(SaliencyTerm {
                    slot: 1,
                    sigma: 0.08,
                    target: SaliencyTarget::Min(0.8),
                }),
            },
        ],
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

    let mut rng = StdRng::seed_from_u64(777);
    let solution = solve_with_rng(&problem, &mut rng).unwrap();
    let d0 = solution.colors[0].distance2(salient_peak).sqrt();
    let d1 = solution.colors[1].distance2(salient_peak).sqrt();
    assert!(d0.min(d1) < 0.16);
}
