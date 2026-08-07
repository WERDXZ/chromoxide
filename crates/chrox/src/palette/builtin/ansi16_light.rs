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

pub fn ansi_16_light() -> Box<dyn Palette> {
    Box::new(BuiltinPalette::new_with_dynamic_terms(
        "ansi-16-light",
        "ANSI 16 Light",
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
                min: 0.08,
                max: 0.28,
            },
            0.05,
        ),
        accent_slot(
            "red",
            340.0,
            45.0,
            Interval {
                min: 0.48,
                max: 0.64,
            },
            Interval {
                min: 0.08,
                max: 0.20,
            },
        ),
        accent_slot(
            "green",
            110.0,
            55.0,
            Interval {
                min: 0.54,
                max: 0.70,
            },
            Interval {
                min: 0.08,
                max: 0.20,
            },
        ),
        accent_slot(
            "yellow",
            70.0,
            40.0,
            Interval {
                min: 0.62,
                max: 0.80,
            },
            Interval {
                min: 0.07,
                max: 0.18,
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
                max: 0.20,
            },
        ),
        accent_slot(
            "magenta",
            285.0,
            55.0,
            Interval {
                min: 0.50,
                max: 0.66,
            },
            Interval {
                min: 0.08,
                max: 0.20,
            },
        ),
        accent_slot(
            "cyan",
            165.0,
            60.0,
            Interval {
                min: 0.54,
                max: 0.72,
            },
            Interval {
                min: 0.07,
                max: 0.18,
            },
        ),
        neutral_slot(
            "white",
            Interval {
                min: 0.86,
                max: 0.95,
            },
            0.04,
        ),
        neutral_slot(
            "bright_black",
            Interval {
                min: 0.20,
                max: 0.38,
            },
            0.05,
        ),
        accent_slot(
            "bright_red",
            340.0,
            45.0,
            Interval {
                min: 0.50,
                max: 0.66,
            },
            Interval {
                min: 0.10,
                max: 0.24,
            },
        ),
        accent_slot(
            "bright_green",
            110.0,
            55.0,
            Interval {
                min: 0.56,
                max: 0.72,
            },
            Interval {
                min: 0.10,
                max: 0.22,
            },
        ),
        accent_slot(
            "bright_yellow",
            70.0,
            40.0,
            Interval {
                min: 0.64,
                max: 0.82,
            },
            Interval {
                min: 0.08,
                max: 0.20,
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
                max: 0.22,
            },
        ),
        accent_slot(
            "bright_magenta",
            285.0,
            55.0,
            Interval {
                min: 0.52,
                max: 0.68,
            },
            Interval {
                min: 0.10,
                max: 0.22,
            },
        ),
        accent_slot(
            "bright_cyan",
            165.0,
            60.0,
            Interval {
                min: 0.56,
                max: 0.74,
            },
            Interval {
                min: 0.08,
                max: 0.20,
            },
        ),
        neutral_slot(
            "bright_white",
            Interval {
                min: 0.94,
                max: 0.99,
            },
            0.03,
        ),
    ]
}

fn terms() -> Vec<WeightedTerm> {
    let mut out = vec![
        weighted(
            "light-cover",
            5.0,
            Term::Cover(CoverTerm {
                slots: vec![7, 15],
                tau: 0.02,
                delta: 0.03,
            }),
        ),
        neutral_ladder_term(
            "neutral-ladder",
            &[0, 8, 7, 15],
            vec![0.16, 0.30, 0.90, 0.97],
            chromoxide::Monotonicity::Increasing { min_gap: 0.05 },
        ),
        weighted(
            "black-on-white",
            6.0,
            Term::Contrast(ContrastTerm {
                fg: 0,
                bg: 7,
                min_ratio: 5.0,
                hinge_delta: Some(0.3),
            }),
        ),
        weighted(
            "bright-black-on-white",
            5.0,
            Term::Contrast(ContrastTerm {
                fg: 8,
                bg: 15,
                min_ratio: 4.5,
                hinge_delta: Some(0.3),
            }),
        ),
        low_chroma_term("black-low-chroma", 0, 0.05),
        low_chroma_term("bright-black-low-chroma", 8, 0.05),
        low_chroma_term("white-low-chroma", 7, 0.04),
        low_chroma_term("bright-white-low-chroma", 15, 0.03),
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
                    value: accent_lightness_target(name),
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
                    value: accent_lightness_target(name) + 0.015,
                    delta: 0.035,
                },
                hinge_delta: None,
            }),
        ));
        out.push(weighted(
            &format!("{name}-bright-order"),
            1.0,
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

fn accent_lightness_target(name: &str) -> f64 {
    match name {
        "red" => 0.57,
        "green" => 0.62,
        "yellow" => 0.69,
        "blue" => 0.58,
        "magenta" => 0.59,
        "cyan" => 0.63,
        _ => 0.60,
    }
}
