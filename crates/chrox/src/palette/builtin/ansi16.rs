use chromoxide::{
    ContrastTerm, CoverTerm, Interval, LightnessTargetTerm, OrderRelation, PairOrderTerm,
    ScalarTarget, SupportTerm, Term, WeightedTerm,
};

use super::common::{accent_slot, low_chroma_term, neutral_ladder_term, neutral_slot, weighted};
use super::export::DirectExport;
use super::priors;
use super::recipe::BuiltinPalette;
use crate::palette::Palette;
use crate::solve_config::PartialSolveConfig;

pub fn ansi_16() -> Box<dyn Palette> {
    Box::new(BuiltinPalette::new_with_dynamic_terms(
        "ansi-16",
        "ANSI 16",
        slots(),
        terms(),
        Some(priors::ansi16_terms),
        PartialSolveConfig {
            seed_count: Some(28),
            keep_top_k: Some(8),
            ..Default::default()
        },
        Box::new(DirectExport),
    ))
}

fn slots() -> Vec<chromoxide::SlotSpec> {
    vec![
        neutral_slot(
            "black",
            Interval {
                min: 0.02,
                max: 0.20,
            },
            0.04,
        ),
        accent_slot(
            "red",
            340.0,
            45.0,
            Interval {
                min: 0.50,
                max: 0.66,
            },
            Interval {
                min: 0.08,
                max: 0.24,
            },
        ),
        accent_slot(
            "green",
            110.0,
            55.0,
            Interval {
                min: 0.60,
                max: 0.78,
            },
            Interval {
                min: 0.08,
                max: 0.22,
            },
        ),
        accent_slot(
            "yellow",
            70.0,
            40.0,
            Interval {
                min: 0.66,
                max: 0.86,
            },
            Interval {
                min: 0.07,
                max: 0.20,
            },
        ),
        accent_slot(
            "blue",
            225.0,
            60.0,
            Interval {
                min: 0.50,
                max: 0.66,
            },
            Interval {
                min: 0.08,
                max: 0.22,
            },
        ),
        accent_slot(
            "magenta",
            285.0,
            55.0,
            Interval {
                min: 0.52,
                max: 0.68,
            },
            Interval {
                min: 0.08,
                max: 0.22,
            },
        ),
        accent_slot(
            "cyan",
            165.0,
            60.0,
            Interval {
                min: 0.56,
                max: 0.74,
            },
            Interval {
                min: 0.07,
                max: 0.20,
            },
        ),
        neutral_slot(
            "white",
            Interval {
                min: 0.72,
                max: 0.92,
            },
            0.05,
        ),
        neutral_slot(
            "bright_black",
            Interval {
                min: 0.18,
                max: 0.34,
            },
            0.05,
        ),
        accent_slot(
            "bright_red",
            340.0,
            45.0,
            Interval {
                min: 0.52,
                max: 0.68,
            },
            Interval {
                min: 0.10,
                max: 0.26,
            },
        ),
        accent_slot(
            "bright_green",
            110.0,
            55.0,
            Interval {
                min: 0.62,
                max: 0.80,
            },
            Interval {
                min: 0.10,
                max: 0.24,
            },
        ),
        accent_slot(
            "bright_yellow",
            70.0,
            40.0,
            Interval {
                min: 0.68,
                max: 0.88,
            },
            Interval {
                min: 0.08,
                max: 0.22,
            },
        ),
        accent_slot(
            "bright_blue",
            225.0,
            60.0,
            Interval {
                min: 0.52,
                max: 0.68,
            },
            Interval {
                min: 0.10,
                max: 0.24,
            },
        ),
        accent_slot(
            "bright_magenta",
            285.0,
            55.0,
            Interval {
                min: 0.54,
                max: 0.70,
            },
            Interval {
                min: 0.10,
                max: 0.24,
            },
        ),
        accent_slot(
            "bright_cyan",
            165.0,
            60.0,
            Interval {
                min: 0.58,
                max: 0.76,
            },
            Interval {
                min: 0.08,
                max: 0.22,
            },
        ),
        neutral_slot(
            "bright_white",
            Interval {
                min: 0.88,
                max: 0.98,
            },
            0.04,
        ),
    ]
}

fn terms() -> Vec<WeightedTerm> {
    let mut out = vec![
        weighted(
            "dark-cover",
            5.0,
            Term::Cover(CoverTerm {
                slots: vec![0, 8],
                tau: 0.02,
                delta: 0.03,
            }),
        ),
        neutral_ladder_term(
            "neutral-ladder",
            &[0, 8, 7, 15],
            vec![0.10, 0.24, 0.82, 0.94],
            chromoxide::Monotonicity::Increasing { min_gap: 0.05 },
        ),
        weighted(
            "white-on-black",
            6.0,
            Term::Contrast(ContrastTerm {
                fg: 7,
                bg: 0,
                min_ratio: 5.0,
                hinge_delta: Some(0.3),
            }),
        ),
        weighted(
            "bright-white-on-black",
            6.0,
            Term::Contrast(ContrastTerm {
                fg: 15,
                bg: 0,
                min_ratio: 7.0,
                hinge_delta: Some(0.3),
            }),
        ),
        low_chroma_term("black-low-chroma", 0, 0.04),
        low_chroma_term("bright-black-low-chroma", 8, 0.05),
        low_chroma_term("white-low-chroma", 7, 0.05),
        low_chroma_term("bright-white-low-chroma", 15, 0.04),
    ];

    for &(regular, bright, name) in &[
        (1, 9, "red"),
        (2, 10, "green"),
        (3, 11, "yellow"),
        (4, 12, "blue"),
        (5, 13, "magenta"),
        (6, 14, "cyan"),
    ] {
        out.push(weighted(
            &format!("{name}-support"),
            2.0,
            Term::Support(SupportTerm {
                slots: vec![regular, bright],
                tau: 0.03,
                beta: 0.10,
                epsilon: 1.0e-4,
            }),
        ));
        out.push(weighted(
            &format!("{name}-regular-lightness"),
            3.0,
            Term::LightnessTarget(LightnessTargetTerm {
                slot: regular,
                target: ScalarTarget::Target {
                    value: accent_lightness_target(name, false),
                    delta: 0.035,
                },
                hinge_delta: None,
            }),
        ));
        out.push(weighted(
            &format!("{name}-bright-lightness"),
            1.8,
            Term::LightnessTarget(LightnessTargetTerm {
                slot: bright,
                target: ScalarTarget::Target {
                    value: accent_lightness_target(name, false) + 0.015,
                    delta: 0.035,
                },
                hinge_delta: None,
            }),
        ));
        out.push(weighted(
            &format!("{name}-bright-order"),
            1.2,
            Term::Order(PairOrderTerm {
                a: bright,
                b: regular,
                relation: OrderRelation::BrighterBy { delta: 0.015 },
                hinge_delta: Some(0.02),
            }),
        ));
    }

    out
}

fn accent_lightness_target(name: &str, light: bool) -> f64 {
    match (light, name) {
        (false, "red") => 0.59,
        (false, "green") => 0.68,
        (false, "yellow") => 0.72,
        (false, "blue") => 0.59,
        (false, "magenta") => 0.60,
        (false, "cyan") => 0.65,
        (true, "red") => 0.57,
        (true, "green") => 0.62,
        (true, "yellow") => 0.69,
        (true, "blue") => 0.58,
        (true, "magenta") => 0.59,
        (true, "cyan") => 0.63,
        _ => 0.60,
    }
}
