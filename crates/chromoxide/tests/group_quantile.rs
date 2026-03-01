use approx::assert_relative_eq;
use chromoxide::color::{Oklab, Oklch};
use chromoxide::term::{
    EvalContext, GroupAxis, GroupMember, GroupQuantileTerm, GroupTarget, Monotonicity,
};
use chromoxide::terms::group_quantile::{compute_mass_quantile_centers, compute_targets, evaluate};

#[test]
fn group_quantile_targets_match_mass_centers() {
    let masses = vec![1.0, 2.0, 1.0];
    let q = compute_mass_quantile_centers(&masses).unwrap();
    assert_relative_eq!(q[0], 0.125, epsilon = 1.0e-12);
    assert_relative_eq!(q[1], 0.5, epsilon = 1.0e-12);
    assert_relative_eq!(q[2], 0.875, epsilon = 1.0e-12);

    let t = compute_targets(
        &q,
        &GroupTarget::UniformRange {
            min: 0.0,
            max: 10.0,
        },
        3,
    )
    .unwrap();
    assert_relative_eq!(t[0], 1.25, epsilon = 1.0e-12);
    assert_relative_eq!(t[1], 5.0, epsilon = 1.0e-12);
    assert_relative_eq!(t[2], 8.75, epsilon = 1.0e-12);
}

#[test]
fn monotonic_penalty_increases_for_crossing_values() {
    let term_base = GroupQuantileTerm {
        members: vec![
            GroupMember { slot: 0, mass: 1.0 },
            GroupMember { slot: 1, mass: 1.0 },
            GroupMember { slot: 2, mass: 1.0 },
        ],
        axis: GroupAxis::Lightness,
        target: GroupTarget::ExplicitValues(vec![0.2, 0.4, 0.6]),
        monotonic: None,
        huber_delta: 0.02,
    };

    let term_mono = GroupQuantileTerm {
        monotonic: Some(Monotonicity::Increasing { min_gap: 0.05 }),
        ..term_base.clone()
    };

    let lchs = vec![
        Oklch {
            l: 0.2,
            c: 0.08,
            h: 0.2,
        },
        Oklch {
            l: 0.45,
            c: 0.08,
            h: 0.2,
        },
        Oklch {
            l: 0.3,
            c: 0.08,
            h: 0.2,
        },
    ];
    let labs = lchs.iter().map(|v| v.to_oklab()).collect::<Vec<Oklab>>();
    let luminance = vec![0.1, 0.2, 0.15];
    let gates = vec![1.0, 1.0, 1.0];

    let ctx = EvalContext {
        slots_lab: &labs,
        slots_lch: &lchs,
        luminance: &luminance,
        hue_gates: &gates,
        samples: &[],
    };

    let base = evaluate(&term_base, &ctx).raw;
    let mono = evaluate(&term_mono, &ctx).raw;
    assert!(mono > base);
}
