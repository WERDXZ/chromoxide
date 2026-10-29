//! A linear-mass support configuration is possible without changing the API.
use chromoxide::objective::ObjectiveEvaluator;
use chromoxide::*;

fn problem(samples: Vec<WeightedSample>) -> PaletteProblem {
    PaletteProblem {
        slots: vec![SlotSpec {
            name: "probe".into(),
            domain: SlotDomain {
                lightness: Interval { min: 0.0, max: 1.0 },
                chroma: Interval { min: 0.0, max: 0.3 },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::Ignore,
                chroma_epsilon: 0.02,
            },
        }],
        samples,
        image_cap: None,
        terms: vec![WeightedTerm {
            weight: 1.0,
            name: None,
            term: Term::Support(SupportTerm {
                slots: vec![0],
                tau: 0.03,
                beta: 0.03,
                epsilon: 0.0,
            }),
        }],
        config: SolveConfig::default(),
    }
}

fn evidence() -> Vec<WeightedSample> {
    vec![
        WeightedSample::new(
            Oklab {
                l: 0.45,
                a: 0.10,
                b: 0.02,
            },
            0.9,
            0.2,
        ),
        WeightedSample::new(
            Oklab {
                l: 0.72,
                a: -0.08,
                b: 0.01,
            },
            0.1,
            0.8,
        ),
    ]
}

#[test]
fn linear_mass_support_preserves_loss_and_gradient_when_one_color_is_split() {
    let original = problem(evidence());
    original.validate().unwrap();
    let eval = ObjectiveEvaluator::new(&original).unwrap();
    for pieces in [2, 3, 11] {
        let mut samples = evidence();
        let sample = samples.remove(0);
        samples.extend((0..pieces).map(|_| {
            WeightedSample::new(
                sample.lab,
                sample.weight / f64::from(pieces),
                sample.saliency,
            )
        }));
        let split = problem(samples);
        split.validate().unwrap();
        let split_eval = ObjectiveEvaluator::new(&split).unwrap();
        for latent in [[0.0, -1.0, 0.3], [1.0, -0.5, 2.9], [-1.0, 0.5, 5.0]] {
            let a = eval.evaluate_total(&latent).unwrap();
            let b = split_eval.evaluate_total(&latent).unwrap();
            assert!((a - b).abs() < 1.0e-12, "split into {pieces}: {a} != {b}");
            let ga = eval.finite_difference_gradient(&latent, 1.0e-5).unwrap();
            let gb = split_eval
                .finite_difference_gradient(&latent, 1.0e-5)
                .unwrap();
            for (a, b) in ga.iter().zip(gb) {
                assert!((a - b).abs() < 1.0e-9, "gradient changed: {a} != {b}");
            }
        }
    }
}

#[test]
fn linear_mass_support_matches_normalized_weighted_kernel() {
    let problem = problem(evidence());
    let eval = ObjectiveEvaluator::new(&problem).unwrap();
    let latent = [0.4, -0.8, 1.2];
    let lab = eval.decode_palette(&latent).unwrap().slots[0].lab;
    let kernel_sum: f64 = problem
        .samples
        .iter()
        .map(|s| s.weight * (-lab.distance2(s.lab) / 0.03).exp())
        .sum();
    let expected = -0.03 * kernel_sum.ln();
    let actual = eval.evaluate_total(&latent).unwrap();
    assert!(
        (actual - expected).abs() < 1.0e-12,
        "{actual} != {expected}"
    );
}
