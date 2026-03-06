use std::f64::consts::{PI, TAU};

use chromoxide::color::Oklch;
use chromoxide::convert::{oklab_to_linear_srgb, relative_luminance};
use chromoxide::term::{
    ChromaTargetTerm, ContrastTerm, EvalContext, HueTargetTerm, HueUnaryTarget,
    LightnessTargetTerm, PairDistanceTerm, ScalarTarget,
};
use chromoxide::terms::{chroma_target, contrast, hue_target, lightness_target, pair_distance};

fn make_ctx(slots_lch: &[Oklch], hue_gates: &[f64]) -> EvalContext<'static> {
    let labs = slots_lch
        .iter()
        .copied()
        .map(Oklch::to_oklab)
        .collect::<Vec<_>>();
    let luminance = labs
        .iter()
        .copied()
        .map(oklab_to_linear_srgb)
        .map(relative_luminance)
        .collect::<Vec<_>>();
    let labs = Box::leak(labs.into_boxed_slice());
    let luminance = Box::leak(luminance.into_boxed_slice());
    let lch = Box::leak(slots_lch.to_vec().into_boxed_slice());
    let gates = Box::leak(hue_gates.to_vec().into_boxed_slice());
    EvalContext {
        slots_lab: labs,
        slots_lch: lch,
        luminance,
        hue_gates: gates,
        samples: &[],
    }
}

#[test]
fn lightness_target_term_penalizes_expected_regions() {
    let inside_ctx = make_ctx(
        &[Oklch {
            l: 0.55,
            c: 0.05,
            h: 0.2,
        }],
        &[1.0],
    );
    let low_ctx = make_ctx(
        &[Oklch {
            l: 0.20,
            c: 0.05,
            h: 0.2,
        }],
        &[1.0],
    );
    let high_ctx = make_ctx(
        &[Oklch {
            l: 0.85,
            c: 0.05,
            h: 0.2,
        }],
        &[1.0],
    );

    let target = LightnessTargetTerm {
        slot: 0,
        target: ScalarTarget::Target {
            value: 0.55,
            delta: 0.03,
        },
        hinge_delta: None,
    };
    let min_term = LightnessTargetTerm {
        slot: 0,
        target: ScalarTarget::Min(0.4),
        hinge_delta: None,
    };
    let max_term = LightnessTargetTerm {
        slot: 0,
        target: ScalarTarget::Max(0.7),
        hinge_delta: None,
    };
    let range_term = LightnessTargetTerm {
        slot: 0,
        target: ScalarTarget::Range { min: 0.4, max: 0.7 },
        hinge_delta: None,
    };

    assert!(lightness_target::evaluate(&target, &inside_ctx).raw < 1.0e-9);
    assert!(lightness_target::evaluate(&min_term, &low_ctx).raw > 0.0);
    assert!(lightness_target::evaluate(&max_term, &high_ctx).raw > 0.0);
    assert!(lightness_target::evaluate(&range_term, &inside_ctx).raw < 1.0e-9);
    assert!(lightness_target::evaluate(&range_term, &low_ctx).raw > 0.0);
    assert!(lightness_target::evaluate(&range_term, &high_ctx).raw > 0.0);
}

#[test]
fn chroma_target_term_supports_neutral_and_accent_preferences() {
    let neutral_ctx = make_ctx(
        &[Oklch {
            l: 0.65,
            c: 0.08,
            h: 1.0,
        }],
        &[1.0],
    );
    let accent_ctx = make_ctx(
        &[Oklch {
            l: 0.65,
            c: 0.04,
            h: 1.0,
        }],
        &[1.0],
    );

    let neutral_term = ChromaTargetTerm {
        slot: 0,
        target: ScalarTarget::Max(0.03),
        hinge_delta: None,
    };
    let accent_term = ChromaTargetTerm {
        slot: 0,
        target: ScalarTarget::Min(0.08),
        hinge_delta: None,
    };

    assert!(chroma_target::evaluate(&neutral_term, &neutral_ctx).raw > 0.0);
    assert!(chroma_target::evaluate(&accent_term, &accent_ctx).raw > 0.0);
}

#[test]
fn hue_target_term_handles_target_wrap_arc_and_gate() {
    let near_wrap_ctx = make_ctx(
        &[Oklch {
            l: 0.6,
            c: 0.12,
            h: TAU - (1.0_f64).to_radians(),
        }],
        &[1.0],
    );
    let far_ctx = make_ctx(
        &[Oklch {
            l: 0.6,
            c: 0.12,
            h: PI,
        }],
        &[1.0],
    );
    let arc_inside_ctx = make_ctx(
        &[Oklch {
            l: 0.6,
            c: 0.12,
            h: (355.0_f64).to_radians(),
        }],
        &[1.0],
    );
    let arc_outside_ctx = make_ctx(
        &[Oklch {
            l: 0.6,
            c: 0.12,
            h: (180.0_f64).to_radians(),
        }],
        &[1.0],
    );
    let gated_ctx = make_ctx(
        &[Oklch {
            l: 0.6,
            c: 0.12,
            h: PI,
        }],
        &[0.01],
    );

    let target_term = HueTargetTerm {
        slot: 0,
        target: HueUnaryTarget::Target {
            center: (1.0_f64).to_radians(),
            delta: 0.05,
        },
        use_hue_gate: false,
    };
    let arc_term = HueTargetTerm {
        slot: 0,
        target: HueUnaryTarget::ArcPreference {
            start: (350.0_f64).to_radians(),
            end: (20.0_f64).to_radians(),
            delta: 0.05,
        },
        use_hue_gate: false,
    };
    let gated_term = HueTargetTerm {
        use_hue_gate: true,
        ..target_term.clone()
    };

    let near_loss = hue_target::evaluate(&target_term, &near_wrap_ctx).raw;
    let far_loss = hue_target::evaluate(&target_term, &far_ctx).raw;
    let arc_inside = hue_target::evaluate(&arc_term, &arc_inside_ctx).raw;
    let arc_outside = hue_target::evaluate(&arc_term, &arc_outside_ctx).raw;
    let ungated = hue_target::evaluate(&gated_term, &far_ctx).raw;
    let gated = hue_target::evaluate(&gated_term, &gated_ctx).raw;

    assert!(near_loss < far_loss);
    assert!(arc_inside < 1.0e-9);
    assert!(arc_outside > 0.0);
    assert!(gated < ungated * 0.02);
}

#[test]
fn pair_distance_term_supports_min_target_and_squared_modes() {
    let ctx = make_ctx(
        &[
            Oklch {
                l: 0.5,
                c: 0.0,
                h: 0.0,
            },
            Oklch {
                l: 0.52,
                c: 0.0,
                h: 0.0,
            },
            Oklch {
                l: 0.7,
                c: 0.0,
                h: 0.0,
            },
        ],
        &[1.0, 1.0, 1.0],
    );

    let min_term = PairDistanceTerm {
        a: 0,
        b: 1,
        target: ScalarTarget::Min(0.05),
        squared: false,
        hinge_delta: None,
    };
    let target_near = PairDistanceTerm {
        a: 0,
        b: 2,
        target: ScalarTarget::Target {
            value: 0.2,
            delta: 0.03,
        },
        squared: false,
        hinge_delta: None,
    };
    let target_far = PairDistanceTerm {
        a: 0,
        b: 1,
        target: ScalarTarget::Target {
            value: 0.2,
            delta: 0.03,
        },
        squared: false,
        hinge_delta: None,
    };
    let squared_term = PairDistanceTerm {
        a: 0,
        b: 2,
        target: ScalarTarget::Min(0.03),
        squared: true,
        hinge_delta: None,
    };

    assert!(pair_distance::evaluate(&min_term, &ctx).raw > 0.0);
    assert!(
        pair_distance::evaluate(&target_near, &ctx).raw
            < pair_distance::evaluate(&target_far, &ctx).raw
    );
    let squared_eval = pair_distance::evaluate(&squared_term, &ctx);
    assert!(squared_eval.raw.abs() < 1.0e-9);
    assert!(squared_eval.components[1] > 0.0);
}

#[test]
fn hinge_delta_changes_curve_and_none_keeps_default_behavior() {
    let ctx = make_ctx(
        &[
            Oklch {
                l: 0.55,
                c: 0.0,
                h: 0.0,
            },
            Oklch {
                l: 0.58,
                c: 0.0,
                h: 0.0,
            },
        ],
        &[1.0, 1.0],
    );

    let default_term = ContrastTerm {
        fg: 1,
        bg: 0,
        min_ratio: 4.5,
        hinge_delta: None,
    };
    let explicit_default = ContrastTerm {
        hinge_delta: Some(0.25),
        ..default_term.clone()
    };
    let softer = ContrastTerm {
        hinge_delta: Some(1.0),
        ..default_term.clone()
    };

    let default_loss = contrast::evaluate(&default_term, &ctx).raw;
    let explicit_default_loss = contrast::evaluate(&explicit_default, &ctx).raw;
    let softer_loss = contrast::evaluate(&softer, &ctx).raw;

    assert!((default_loss - explicit_default_loss).abs() < 1.0e-12);
    assert!((default_loss - softer_loss).abs() > 1.0e-6);
}
