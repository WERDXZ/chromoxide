use std::cmp::Ordering;
use std::collections::HashMap;

use chromoxide::{
    ChromaTargetTerm, CoverTerm, DeltaHTarget, Oklch, PairDeltaHTerm, PairDistanceTerm,
    SaliencyTarget, SaliencyTerm, ScalarTarget, SlotSpec, Term, WeightedTerm,
};

use super::common::unconstrained_slot;
use super::export::BuiltinExport;
use super::recipe::BuiltinPalette;
use crate::palette::Palette;
use crate::solve_config::PartialSolveConfig;

pub fn cover_salient() -> Box<dyn Palette> {
    Box::new(BuiltinPalette::new(
        "cover-salient",
        "Cover + 2 Salients",
        slots(),
        terms(),
        PartialSolveConfig {
            seed_count: Some(32),
            keep_top_k: Some(6),
            ..Default::default()
        },
        Box::new(CoverSalientExport),
    ))
}

fn slots() -> Vec<SlotSpec> {
    vec![
        unconstrained_slot("cover"),
        unconstrained_slot("salient-a"),
        unconstrained_slot("salient-b"),
    ]
}

fn terms() -> Vec<WeightedTerm> {
    vec![
        WeightedTerm {
            weight: 5.0,
            name: Some("cover-fit".into()),
            term: Term::Cover(CoverTerm {
                slots: vec![0],
                tau: 0.02,
                delta: 0.03,
            }),
        },
        salient_saliency_term(1, "salient-a-saliency"),
        salient_saliency_term(2, "salient-b-saliency"),
        salient_chroma_term(1, "salient-a-max-chroma"),
        salient_chroma_term(2, "salient-b-max-chroma"),
        WeightedTerm {
            weight: 3.0,
            name: Some("cover-salient-a-separation".into()),
            term: Term::Distance(PairDistanceTerm {
                a: 0,
                b: 1,
                target: ScalarTarget::Min(0.14),
                squared: false,
                hinge_delta: Some(0.03),
            }),
        },
        WeightedTerm {
            weight: 3.0,
            name: Some("cover-salient-b-separation".into()),
            term: Term::Distance(PairDistanceTerm {
                a: 0,
                b: 2,
                target: ScalarTarget::Min(0.14),
                squared: false,
                hinge_delta: Some(0.03),
            }),
        },
        WeightedTerm {
            weight: 8.0,
            name: Some("salient-pair-separation".into()),
            term: Term::Distance(PairDistanceTerm {
                a: 1,
                b: 2,
                target: ScalarTarget::Min(0.18),
                squared: false,
                hinge_delta: Some(0.03),
            }),
        },
        WeightedTerm {
            weight: 5.0,
            name: Some("salient-pair-delta-h".into()),
            term: Term::DeltaH(PairDeltaHTerm {
                a: 1,
                b: 2,
                target: DeltaHTarget::Min(std::f64::consts::FRAC_PI_4),
                hinge_delta: Some(0.12),
            }),
        },
    ]
}

fn salient_saliency_term(slot: usize, name: &str) -> WeightedTerm {
    WeightedTerm {
        weight: 8.0,
        name: Some(name.into()),
        term: Term::Saliency(SaliencyTerm {
            slot,
            sigma: 0.10,
            target: SaliencyTarget::Target {
                value: 1.0,
                delta: 0.05,
            },
            hinge_delta: Some(0.05),
        }),
    }
}

fn salient_chroma_term(slot: usize, name: &str) -> WeightedTerm {
    WeightedTerm {
        weight: 5.0,
        name: Some(name.into()),
        term: Term::ChromaTarget(ChromaTargetTerm {
            slot,
            target: ScalarTarget::Target {
                value: 1.0,
                delta: 0.20,
            },
            hinge_delta: Some(0.03),
        }),
    }
}

struct CoverSalientExport;

impl BuiltinExport for CoverSalientExport {
    fn export(&self, slots: &[SlotSpec], colors: &[Oklch]) -> HashMap<String, Oklch> {
        let mut out = HashMap::with_capacity(3);
        let mut salients = Vec::with_capacity(2);

        for (slot, color) in slots.iter().zip(colors.iter().copied()) {
            match slot.name.as_str() {
                "cover" => {
                    out.insert("cover".to_string(), color);
                }
                _ => salients.push((slot.name.as_str(), color)),
            }
        }

        salients.sort_by(|(_, a), (_, b)| {
            let hue_cmp = a.h.total_cmp(&b.h);
            if hue_cmp == Ordering::Equal {
                b.c.total_cmp(&a.c)
            } else {
                hue_cmp
            }
        });

        for (idx, (_, color)) in salients.into_iter().enumerate() {
            out.insert(format!("salient-{}", idx + 1), color);
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use chromoxide::{ImageCapBuilder, Oklch};

    use super::{cover_salient, slots, CoverSalientExport};
    use crate::palette::builtin::export::BuiltinExport;
    use crate::solve_config::PartialSolveConfig;

    fn clustered_samples() -> Vec<chromoxide::WeightedSample> {
        vec![
            chromoxide::WeightedSample::new(
                Oklch {
                    l: 0.42,
                    c: 0.03,
                    h: 0.2,
                }
                .to_oklab(),
                4.0,
                0.15,
            ),
            chromoxide::WeightedSample::new(
                Oklch {
                    l: 0.40,
                    c: 0.04,
                    h: 0.3,
                }
                .to_oklab(),
                3.5,
                0.20,
            ),
            chromoxide::WeightedSample::new(
                Oklch {
                    l: 0.66,
                    c: 0.18,
                    h: 0.9,
                }
                .to_oklab(),
                1.0,
                0.95,
            ),
        ]
    }

    #[test]
    fn export_reorders_salients_by_hue() {
        let slots = slots();
        let export = CoverSalientExport;
        let colors = vec![
            Oklch {
                l: 0.4,
                c: 0.02,
                h: 1.0,
            },
            Oklch {
                l: 0.6,
                c: 0.1,
                h: 5.0,
            },
            Oklch {
                l: 0.6,
                c: 0.1,
                h: 1.5,
            },
        ];

        let out = export.export(&slots, &colors);
        assert_eq!(out["salient-1"].h, 1.5);
        assert_eq!(out["salient-2"].h, 5.0);
    }

    #[test]
    fn cover_salient_solves_and_exports_named_members() {
        let palette = cover_salient();
        let samples = clustered_samples();
        let image_cap = ImageCapBuilder::default()
            .build(&samples)
            .expect("image cap should build");
        let colors = palette
            .solve(samples, Some(image_cap), &PartialSolveConfig::default())
            .expect("builtin palette should solve");

        assert_eq!(palette.id(), "cover-salient");
        assert!(colors.contains_key("cover"));
        assert!(colors.contains_key("salient-1"));
        assert!(colors.contains_key("salient-2"));
        assert!(colors["salient-1"].h <= colors["salient-2"].h);
    }
}
