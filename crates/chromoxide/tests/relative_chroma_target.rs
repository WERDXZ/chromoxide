use chromoxide::{
    CapPolicy, HueDomain, Interval, PaletteProblem, RelativeChromaTargetTerm, ScalarTarget,
    SlotDomain, SlotSpec, Term, WeightedSample, WeightedTerm,
};

fn one_slot_problem(term: Term) -> PaletteProblem {
    PaletteProblem {
        slots: vec![SlotSpec {
            name: "a".to_string(),
            domain: SlotDomain {
                lightness: Interval { min: 0.0, max: 1.0 },
                chroma: Interval { min: 0.0, max: 0.3 },
                hue: HueDomain::Any,
                cap_policy: CapPolicy::Ignore,
                chroma_epsilon: 0.02,
            },
        }],
        samples: vec![WeightedSample::new(
            chromoxide::Oklch {
                l: 0.5,
                c: 0.1,
                h: 1.0,
            }
            .to_oklab(),
            1.0,
            0.5,
        )],
        image_cap: None,
        terms: vec![WeightedTerm {
            weight: 1.0,
            name: Some("test".to_string()),
            term,
        }],
        config: Default::default(),
    }
}

fn relative_chroma(target: ScalarTarget) -> Term {
    Term::RelativeChromaTarget(RelativeChromaTargetTerm {
        slot: 0,
        target,
        hinge_delta: None,
    })
}

#[test]
fn relative_chroma_target_must_be_in_unit_interval() {
    let problem = one_slot_problem(relative_chroma(ScalarTarget::Target {
        value: 1.5,
        delta: 0.1,
    }));
    let err = problem.validate().unwrap_err().to_string();
    assert!(
        err.contains("RelativeChromaTargetTerm.target"),
        "unexpected error: {err}"
    );

    let problem = one_slot_problem(relative_chroma(ScalarTarget::Min(-0.1)));
    let err = problem.validate().unwrap_err().to_string();
    assert!(err.contains("RelativeChromaTargetTerm.target"));

    let problem = one_slot_problem(relative_chroma(ScalarTarget::Range { min: 0.2, max: 0.1 }));
    let err = problem.validate().unwrap_err().to_string();
    assert!(err.contains("RelativeChromaTargetTerm.target"));
}

#[test]
fn saliency_support_scale_must_be_positive() {
    let problem = one_slot_problem(Term::Saliency(chromoxide::SaliencyTerm {
        slot: 0,
        sigma: 0.08,
        support_scale: 0.0,
        target: chromoxide::SaliencyTarget::Min(0.5),
        hinge_delta: None,
    }));
    let err = problem.validate().unwrap_err().to_string();
    assert!(err.contains("SaliencyTerm.support_scale"));
}
