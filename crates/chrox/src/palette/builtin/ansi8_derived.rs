use std::collections::HashMap;

use chromoxide::{
    ChromaTargetTerm, ContrastTerm, CoverTerm, Interval, Oklch, ScalarTarget, SupportTerm, Term,
    WeightedTerm,
};

use super::common::{accent_slot, low_chroma_term, neutral_slot, weighted};
use super::export::BuiltinExport;
use super::recipe::BuiltinPalette;
use crate::palette::Palette;
use crate::solve_config::PartialSolveConfig;

pub fn ansi_8_derived() -> Box<dyn Palette> {
    Box::new(BuiltinPalette::new(
        "ansi-8-derived",
        "ANSI 8 Derived",
        slots(),
        terms(),
        PartialSolveConfig {
            seed_count: Some(24),
            keep_top_k: Some(8),
            ..Default::default()
        },
        Box::new(DeriveAnsiBrightExport { light: false }),
    ))
}

fn slots() -> Vec<chromoxide::SlotSpec> {
    vec![
        neutral_slot(
            "black",
            Interval {
                min: 0.02,
                max: 0.22,
            },
            0.05,
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
                max: 0.94,
            },
            0.05,
        ),
    ]
}

fn terms() -> Vec<WeightedTerm> {
    let mut out = vec![
        weighted(
            "dark-cover",
            5.0,
            Term::Cover(CoverTerm {
                slots: vec![0],
                tau: 0.02,
                delta: 0.03,
            }),
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
        low_chroma_term("black-low-chroma", 0, 0.05),
        low_chroma_term("white-low-chroma", 7, 0.05),
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

pub(crate) struct DeriveAnsiBrightExport {
    pub(crate) light: bool,
}

impl BuiltinExport for DeriveAnsiBrightExport {
    fn export(&self, slots: &[chromoxide::SlotSpec], colors: &[Oklch]) -> HashMap<String, Oklch> {
        let mut out = HashMap::with_capacity(16);
        for (slot, color) in slots.iter().zip(colors.iter().copied()) {
            out.insert(slot.name.clone(), color);
            let bright_name = format!("bright_{}", slot.name);
            out.insert(
                bright_name,
                derive_bright(slot.name.as_str(), color, self.light),
            );
        }
        out
    }
}

fn derive_bright(name: &str, color: Oklch, light: bool) -> Oklch {
    let (delta_l, scale_c) = if light { (0.05, 1.12) } else { (0.09, 1.12) };
    match name {
        "black" => Oklch {
            l: (color.l + if light { 0.10 } else { 0.14 }).clamp(0.0, 1.0),
            c: (color.c * 0.8).clamp(0.0, 1.0),
            h: color.h,
        },
        "white" => Oklch {
            l: (color.l + 0.04).clamp(0.0, 1.0),
            c: (color.c * 0.8).clamp(0.0, 1.0),
            h: color.h,
        },
        _ => Oklch {
            l: (color.l + delta_l).clamp(0.0, 1.0),
            c: (color.c * scale_c).clamp(0.0, 1.0),
            h: color.h,
        },
    }
}

#[cfg(test)]
mod tests {
    use chromoxide::Oklch;

    use super::{derive_bright, slots, DeriveAnsiBrightExport};
    use crate::palette::builtin::export::BuiltinExport;

    #[test]
    fn export_derives_bright_variants() {
        let export = DeriveAnsiBrightExport { light: false };
        let slots = slots();
        let colors = slots
            .iter()
            .enumerate()
            .map(|(i, _)| Oklch {
                l: 0.2 + i as f64 * 0.05,
                c: 0.04 + i as f64 * 0.01,
                h: i as f64,
            })
            .collect::<Vec<_>>();
        let out = export.export(&slots, &colors);
        assert!(out.contains_key("bright_red"));
        assert!(out["bright_red"].l > out["red"].l);
    }

    #[test]
    fn black_brightening_stays_neutralish() {
        let black = Oklch {
            l: 0.10,
            c: 0.03,
            h: 0.0,
        };
        let bright = derive_bright("black", black, false);
        assert!(bright.l > black.l);
        assert!(bright.c <= black.c);
    }
}
