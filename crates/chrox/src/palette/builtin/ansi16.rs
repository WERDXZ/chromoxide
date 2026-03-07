use chromoxide::{
    ChromaTargetTerm, ContrastTerm, CoverTerm, Interval, OrderRelation, PairOrderTerm,
    ScalarTarget, SupportTerm, Term, WeightedTerm,
};

use super::common::{accent_slot, low_chroma_term, neutral_ladder_term, neutral_slot, weighted};
use super::export::DirectExport;
use super::recipe::BuiltinPalette;
use crate::palette::Palette;
use crate::solve_config::PartialSolveConfig;

pub fn ansi_16() -> Box<dyn Palette> {
    Box::new(BuiltinPalette::new(
        "ansi-16",
        "ANSI 16",
        slots(),
        terms(),
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
                min: 0.35,
                max: 0.68,
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
                min: 0.35,
                max: 0.68,
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
                min: 0.55,
                max: 0.84,
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
                min: 0.35,
                max: 0.70,
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
                min: 0.35,
                max: 0.72,
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
                min: 0.40,
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
                min: 0.48,
                max: 0.80,
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
                min: 0.50,
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
                max: 0.92,
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
                min: 0.48,
                max: 0.82,
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
                min: 0.50,
                max: 0.82,
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
                min: 0.54,
                max: 0.84,
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
            &format!("{name}-regular-chroma"),
            3.0,
            Term::ChromaTarget(ChromaTargetTerm {
                slot: regular,
                target: ScalarTarget::Target {
                    value: 1.0,
                    delta: 0.20,
                },
                hinge_delta: Some(0.03),
            }),
        ));
        out.push(weighted(
            &format!("{name}-bright-chroma"),
            3.0,
            Term::ChromaTarget(ChromaTargetTerm {
                slot: bright,
                target: ScalarTarget::Target {
                    value: 1.0,
                    delta: 0.20,
                },
                hinge_delta: Some(0.03),
            }),
        ));
        out.push(weighted(
            &format!("{name}-bright-order"),
            3.0,
            Term::Order(PairOrderTerm {
                a: bright,
                b: regular,
                relation: OrderRelation::BrighterBy { delta: 0.08 },
                hinge_delta: Some(0.04),
            }),
        ));
    }

    out
}
