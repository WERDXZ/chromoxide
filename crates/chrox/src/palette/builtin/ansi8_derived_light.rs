use crate::palette::Palette;

use super::ansi8_derived::DeriveAnsiBrightExport;
use super::common::{accent_slot, low_chroma_term, neutral_slot, weighted};
use super::export::BuiltinExport;
use super::priors;
use super::recipe::BuiltinPalette;
use crate::solve_config::PartialSolveConfig;
use chromoxide::{
    ContrastTerm, CoverTerm, Interval, LightnessTargetTerm, ScalarTarget, SupportTerm, Term,
    WeightedTerm,
};

pub fn ansi_8_derived_light() -> Box<dyn Palette> {
    Box::new(BuiltinPalette::new_with_dynamic_terms(
        "ansi-8-derived-light",
        "ANSI 8 Derived Light",
        slots(),
        terms(),
        Some(priors::ansi8_light_terms),
        PartialSolveConfig {
            seed_count: Some(24),
            keep_top_k: Some(8),
            ..Default::default()
        },
        Box::new(DeriveAnsiBrightExport { light: true }) as Box<dyn BuiltinExport>,
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
                max: 0.97,
            },
            0.04,
        ),
    ]
}

fn terms() -> Vec<WeightedTerm> {
    let mut out = vec![
        weighted(
            "light-cover",
            5.0,
            Term::Cover(CoverTerm {
                slots: vec![7],
                tau: 0.02,
                delta: 0.03,
            }),
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
        low_chroma_term("black-low-chroma", 0, 0.05),
        low_chroma_term("white-low-chroma", 7, 0.04),
    ];
    for &(slot, name) in &[
        (1, "red"),
        (2, "green"),
        (3, "yellow"),
        (4, "blue"),
        (5, "magenta"),
        (6, "cyan"),
    ] {
        out.push(weighted(
            &format!("{name}-lightness"),
            3.0,
            Term::LightnessTarget(LightnessTargetTerm {
                slot,
                target: ScalarTarget::Target {
                    value: super::ansi8_derived::accent_lightness_target(name, true),
                    delta: 0.035,
                },
                hinge_delta: None,
            }),
        ));
        out.push(weighted(
            &format!("{name}-support"),
            2.0,
            Term::Support(SupportTerm {
                slots: vec![slot],
                tau: 0.03,
                beta: 0.10,
                epsilon: 1.0e-4,
            }),
        ));
    }
    out
}
