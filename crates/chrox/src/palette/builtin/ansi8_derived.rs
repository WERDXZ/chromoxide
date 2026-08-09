use std::collections::HashMap;

use chromoxide::{
    ContrastTerm, CoverTerm, Interval, LightnessTargetTerm, Oklch, ScalarTarget, SupportTerm, Term,
    WeightedTerm,
};

use super::common::{accent_slot, low_chroma_term, neutral_slot, weighted};
use super::export::BuiltinExport;
use super::priors;
use super::recipe::BuiltinPalette;
use crate::palette::Palette;
use crate::solve_config::PartialSolveConfig;

pub fn ansi_8_derived() -> Box<dyn Palette> {
    Box::new(BuiltinPalette::new_with_dynamic_terms(
        "ansi-8-derived",
        "ANSI 8 Derived",
        slots(),
        terms(),
        Some(priors::ansi8_terms),
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
            &format!("{name}-lightness"),
            3.0,
            Term::LightnessTarget(LightnessTargetTerm {
                slot,
                target: ScalarTarget::Target {
                    value: accent_lightness_target(name, false),
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

pub(crate) struct DeriveAnsiBrightExport {
    pub(crate) light: bool,
}

impl BuiltinExport for DeriveAnsiBrightExport {
    fn members(&self, slots: &[chromoxide::SlotSpec]) -> Vec<String> {
        let mut out = Vec::with_capacity(slots.len() * 2);
        for slot in slots {
            out.push(slot.name.clone());
            out.push(format!("bright_{}", slot.name));
        }
        out
    }

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
    let (delta_l, scale_c) = if light { (0.012, 1.02) } else { (0.015, 1.02) };
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

pub(crate) fn accent_lightness_target(name: &str, light: bool) -> f64 {
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

#[cfg(test)]
mod tests {
    use chromoxide::{ImageCapBuilder, Oklch, StatisticalCapConfig, WeightedSample};

    use super::{DeriveAnsiBrightExport, derive_bright, slots};
    use crate::palette::builtin::export::BuiltinExport;
    use crate::solve_config::PartialSolveConfig;

    const ACCENT_NAMES: [&str; 6] = ["red", "green", "yellow", "blue", "magenta", "cyan"];

    fn synthetic_support(chromas: &[f64; 8]) -> Vec<WeightedSample> {
        let lightnesses = [0.20, 0.42, 0.50, 0.58, 0.64, 0.70, 0.78, 0.90];
        lightnesses
            .into_iter()
            .zip(chromas.iter().copied())
            .enumerate()
            .map(|(index, (l, c))| {
                WeightedSample::new(
                    Oklch { l, c, h: 0.45 }.to_oklab(),
                    1.0 + (index % 3) as f64 * 0.25,
                    0.20 + index as f64 * 0.08,
                )
            })
            .collect()
    }

    fn mean_accent_chroma(colors: &std::collections::HashMap<String, Oklch>) -> f64 {
        ACCENT_NAMES.iter().map(|name| colors[*name].c).sum::<f64>() / ACCENT_NAMES.len() as f64
    }

    fn solve_synthetic(
        palette: &dyn crate::palette::Palette,
        chromas: &[f64; 8],
        seed: chromoxide::SolveSeed,
    ) -> std::collections::HashMap<String, Oklch> {
        let samples = synthetic_support(chromas);
        let cap = ImageCapBuilder {
            n_l: 8,
            n_h: 36,
            smooth_l_radius: 1,
            smooth_h_radius: 1,
            relax: 1.0,
        }
        .build_statistical(&samples, StatisticalCapConfig::default())
        .expect("synthetic image cap should build");
        palette
            .solve_with_seed(
                samples,
                Some(cap),
                &PartialSolveConfig {
                    max_iters: Some(80),
                    ..Default::default()
                },
                seed,
            )
            .expect("synthetic ANSI palette should solve")
    }

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

    #[test]
    fn palette_reports_derived_members() {
        let palette = super::ansi_8_derived();
        let members = palette.members();
        assert!(members.contains(&"red".to_string()));
        assert!(members.contains(&"bright_red".to_string()));
        assert!(members.contains(&"black".to_string()));
        assert!(members.contains(&"bright_black".to_string()));
    }

    #[test]
    fn vivid_support_produces_more_chromatic_accents_and_repeats_exactly() {
        let muted = [0.040, 0.050, 0.055, 0.060, 0.052, 0.047, 0.058, 0.045];
        let vivid = [0.140, 0.150, 0.160, 0.180, 0.170, 0.145, 0.175, 0.155];
        let seed = [0x5a; 32];
        let palette = super::ansi_8_derived();

        let muted_colors = solve_synthetic(palette.as_ref(), &muted, seed);
        let vivid_colors = solve_synthetic(palette.as_ref(), &vivid, seed);
        let vivid_repeat = solve_synthetic(palette.as_ref(), &vivid, seed);

        assert_eq!(vivid_colors, vivid_repeat);
        let muted_mean = mean_accent_chroma(&muted_colors);
        let vivid_mean = mean_accent_chroma(&vivid_colors);
        assert!(
            vivid_mean > muted_mean + 0.03,
            "vivid mean {vivid_mean} must exceed muted mean {muted_mean} by more than 0.03"
        );
    }
}
