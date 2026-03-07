use chromoxide::{
    ChromaTargetTerm, CoverTerm, Interval, ScalarTarget, SupportTerm, Term, WeightedTerm,
};

use super::common::{accent_slot, low_chroma_term, neutral_ladder_term, neutral_slot, weighted};
use super::export::DirectExport;
use super::recipe::BuiltinPalette;
use crate::palette::Palette;
use crate::solve_config::PartialSolveConfig;

pub fn base16() -> Box<dyn Palette> {
    Box::new(BuiltinPalette::new(
        "base16",
        "Base16",
        slots(),
        terms(),
        PartialSolveConfig {
            seed_count: Some(32),
            keep_top_k: Some(8),
            ..Default::default()
        },
        Box::new(DirectExport),
    ))
}

fn slots() -> Vec<chromoxide::SlotSpec> {
    vec![
        neutral_slot(
            "base00",
            Interval {
                min: 0.06,
                max: 0.18,
            },
            0.03,
        ),
        neutral_slot(
            "base01",
            Interval {
                min: 0.10,
                max: 0.22,
            },
            0.04,
        ),
        neutral_slot(
            "base02",
            Interval {
                min: 0.16,
                max: 0.30,
            },
            0.05,
        ),
        neutral_slot(
            "base03",
            Interval {
                min: 0.28,
                max: 0.46,
            },
            0.06,
        ),
        neutral_slot(
            "base04",
            Interval {
                min: 0.46,
                max: 0.64,
            },
            0.06,
        ),
        neutral_slot(
            "base05",
            Interval {
                min: 0.62,
                max: 0.80,
            },
            0.05,
        ),
        neutral_slot(
            "base06",
            Interval {
                min: 0.76,
                max: 0.90,
            },
            0.04,
        ),
        neutral_slot(
            "base07",
            Interval {
                min: 0.86,
                max: 0.97,
            },
            0.03,
        ),
        accent_slot(
            "base08",
            340.0,
            45.0,
            Interval {
                min: 0.40,
                max: 0.72,
            },
            Interval {
                min: 0.08,
                max: 0.24,
            },
        ),
        accent_slot(
            "base09",
            20.0,
            50.0,
            Interval {
                min: 0.46,
                max: 0.80,
            },
            Interval {
                min: 0.08,
                max: 0.20,
            },
        ),
        accent_slot(
            "base0A",
            65.0,
            45.0,
            Interval {
                min: 0.56,
                max: 0.86,
            },
            Interval {
                min: 0.06,
                max: 0.18,
            },
        ),
        accent_slot(
            "base0B",
            110.0,
            55.0,
            Interval {
                min: 0.40,
                max: 0.72,
            },
            Interval {
                min: 0.08,
                max: 0.22,
            },
        ),
        accent_slot(
            "base0C",
            165.0,
            60.0,
            Interval {
                min: 0.42,
                max: 0.74,
            },
            Interval {
                min: 0.07,
                max: 0.20,
            },
        ),
        accent_slot(
            "base0D",
            225.0,
            60.0,
            Interval {
                min: 0.40,
                max: 0.74,
            },
            Interval {
                min: 0.08,
                max: 0.22,
            },
        ),
        accent_slot(
            "base0E",
            285.0,
            55.0,
            Interval {
                min: 0.40,
                max: 0.74,
            },
            Interval {
                min: 0.08,
                max: 0.22,
            },
        ),
        accent_slot(
            "base0F",
            20.0,
            40.0,
            Interval {
                min: 0.32,
                max: 0.64,
            },
            Interval {
                min: 0.05,
                max: 0.16,
            },
        ),
    ]
}

fn terms() -> Vec<WeightedTerm> {
    let mut out = vec![
        weighted(
            "base-cover",
            5.0,
            Term::Cover(CoverTerm {
                slots: vec![0, 1, 2],
                tau: 0.02,
                delta: 0.03,
            }),
        ),
        neutral_ladder_term(
            "base-ladder",
            &[0, 1, 2, 3, 4, 5, 6, 7],
            vec![0.12, 0.16, 0.22, 0.38, 0.58, 0.72, 0.82, 0.90],
            chromoxide::Monotonicity::Increasing { min_gap: 0.04 },
        ),
    ];
    for idx in 0..8 {
        out.push(low_chroma_term(
            &format!("base0{idx:x}-low-chroma"),
            idx,
            0.06,
        ));
    }
    for &(slot, name) in &[
        (8, "base08"),
        (9, "base09"),
        (10, "base0A"),
        (11, "base0B"),
        (12, "base0C"),
        (13, "base0D"),
        (14, "base0E"),
        (15, "base0F"),
    ] {
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
        out.push(weighted(
            &format!("{name}-chroma"),
            3.0,
            Term::ChromaTarget(ChromaTargetTerm {
                slot,
                target: ScalarTarget::Target {
                    value: 1.0,
                    delta: 0.20,
                },
                hinge_delta: Some(0.03),
            }),
        ));
    }
    out
}
