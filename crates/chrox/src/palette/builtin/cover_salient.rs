use std::cmp::Ordering;
use std::collections::HashMap;

use chromoxide::{
    CoverTerm, DeltaHTarget, Oklch, PairDeltaHTerm, PairDistanceTerm, RelativeChromaReference,
    RelativeChromaTargetTerm, SaliencyTarget, SaliencyTerm, ScalarTarget, SlotSpec, Term,
    WeightedTerm,
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
        salient_relative_chroma_term(1, "salient-a-relative-chroma"),
        salient_relative_chroma_term(2, "salient-b-relative-chroma"),
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
            support_scale: 0.05,
            target: SaliencyTarget::Target {
                value: 1.0,
                delta: 0.05,
            },
            hinge_delta: Some(0.05),
        }),
    }
}

fn salient_relative_chroma_term(slot: usize, name: &str) -> WeightedTerm {
    WeightedTerm {
        weight: 5.0,
        name: Some(name.into()),
        term: Term::RelativeChromaTarget(RelativeChromaTargetTerm {
            slot,
            target: ScalarTarget::Target {
                value: 0.90,
                delta: 0.10,
            },
            hinge_delta: None,
            reference: RelativeChromaReference::ImageCap,
        }),
    }
}

struct CoverSalientExport;

impl BuiltinExport for CoverSalientExport {
    fn members(&self, _slots: &[SlotSpec]) -> Vec<String> {
        vec![
            "cover".to_string(),
            "salient-1".to_string(),
            "salient-2".to_string(),
        ]
    }

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
    use chromoxide::{ImageCapBuilder, Oklch, ScalarTarget, Term};

    use super::{CoverSalientExport, cover_salient, slots, terms};
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
        for color in colors.values() {
            assert!(color.l.is_finite());
            assert!(color.c.is_finite());
            assert!(color.h.is_finite());
        }
    }

    #[test]
    fn cover_salient_reports_exported_members() {
        let palette = cover_salient();
        assert_eq!(
            palette.members(),
            vec![
                "cover".to_string(),
                "salient-1".to_string(),
                "salient-2".to_string(),
            ]
        );
    }

    #[test]
    fn accent_chroma_uses_relative_target_not_absolute_one() {
        for wt in terms() {
            if let Term::RelativeChromaTarget(t) = &wt.term
                && let ScalarTarget::Target { value, .. } = &t.target
            {
                assert!((0.0..=1.0).contains(value));
            }
            assert!(
                !matches!(
                    &wt.term,
                    Term::ChromaTarget(t)
                        if matches!(&t.target, ScalarTarget::Target { value: 1.0, .. })
                ),
                "accent chroma must not be expressed as an absolute target of 1.0"
            );
        }
    }

    #[test]
    fn cover_salient_remains_strict_image_cap() {
        let relative_terms = terms()
            .into_iter()
            .filter_map(|weighted| match weighted.term {
                Term::RelativeChromaTarget(term) => Some((weighted.weight, term)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(relative_terms.len(), 2);
        for (expected_slot, (weight, term)) in (1..=2).zip(relative_terms) {
            assert_eq!(weight, 5.0);
            assert_eq!(term.slot, expected_slot);
            assert_eq!(
                term.reference,
                chromoxide::RelativeChromaReference::ImageCap
            );
            assert_eq!(term.hinge_delta, None);
            assert!(matches!(
                term.target,
                ScalarTarget::Target {
                    value: 0.90,
                    delta: 0.10
                }
            ));
        }
    }
}
