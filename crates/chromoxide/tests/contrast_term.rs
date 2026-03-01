use chromoxide::color::Oklch;
use chromoxide::convert::{oklab_to_linear_srgb, relative_luminance};
use chromoxide::term::{ContrastTerm, DeltaHTarget, EvalContext, PairDeltaHTerm};
use chromoxide::terms::contrast::evaluate as eval_contrast;
use chromoxide::terms::pair_delta::evaluate_delta_h;

#[test]
fn contrast_term_behaves_as_expected() {
    let bw_lch = [
        Oklch {
            l: 0.0,
            c: 0.0,
            h: 0.0,
        },
        Oklch {
            l: 1.0,
            c: 0.0,
            h: 0.0,
        },
    ];
    let bw_lab = bw_lch.iter().map(|v| v.to_oklab()).collect::<Vec<_>>();
    let bw_y = bw_lab
        .iter()
        .map(|&lab| relative_luminance(oklab_to_linear_srgb(lab)))
        .collect::<Vec<_>>();
    let bw_ctx = EvalContext {
        slots_lab: &bw_lab,
        slots_lch: &bw_lch,
        luminance: &bw_y,
        hue_gates: &[1.0, 1.0],
        samples: &[],
    };

    let term = ContrastTerm {
        fg: 1,
        bg: 0,
        min_ratio: 4.5,
    };
    let bw_loss = eval_contrast(&term, &bw_ctx).raw;
    assert!(bw_loss < 1.0e-6);

    let gray_lch = [
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
    ];
    let gray_lab = gray_lch.iter().map(|v| v.to_oklab()).collect::<Vec<_>>();
    let gray_y = gray_lab
        .iter()
        .map(|&lab| relative_luminance(oklab_to_linear_srgb(lab)))
        .collect::<Vec<_>>();
    let gray_ctx = EvalContext {
        slots_lab: &gray_lab,
        slots_lch: &gray_lch,
        luminance: &gray_y,
        hue_gates: &[1.0, 1.0],
        samples: &[],
    };
    let gray_loss = eval_contrast(&term, &gray_ctx).raw;
    assert!(gray_loss > 0.0);
}

#[test]
fn hue_gate_suppresses_delta_h_at_low_chroma() {
    let lchs = [
        Oklch {
            l: 0.6,
            c: 0.1,
            h: 0.0,
        },
        Oklch {
            l: 0.6,
            c: 0.1,
            h: std::f64::consts::PI,
        },
    ];
    let labs = lchs.iter().map(|v| v.to_oklab()).collect::<Vec<_>>();
    let ys = vec![0.2, 0.2];

    let term = PairDeltaHTerm {
        a: 0,
        b: 1,
        target: DeltaHTarget::Target {
            value: std::f64::consts::FRAC_PI_2,
            delta: 0.1,
        },
    };

    let full_ctx = EvalContext {
        slots_lab: &labs,
        slots_lch: &lchs,
        luminance: &ys,
        hue_gates: &[1.0, 1.0],
        samples: &[],
    };
    let gated_ctx = EvalContext {
        slots_lab: &labs,
        slots_lch: &lchs,
        luminance: &ys,
        hue_gates: &[0.05, 0.05],
        samples: &[],
    };

    let full = evaluate_delta_h(&term, &full_ctx).raw;
    let gated = evaluate_delta_h(&term, &gated_ctx).raw;
    assert!(gated < full * 0.02);
}
