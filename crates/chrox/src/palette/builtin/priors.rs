use std::f64::consts::TAU;

use chromoxide::{
    GroupAxis, GroupMember, GroupQuantileTerm, GroupTarget, HueTargetTerm, HueUnaryTarget, Oklch,
    RelativeChromaReference, RelativeChromaTargetTerm, ScalarTarget, SlotSpec, Term,
    WeightedSample, WeightedTerm,
};

use super::common::weighted;

const NEUTRAL_CHROMA_CUTOFF: f64 = 0.08;

#[derive(Clone, Copy, Debug)]
struct ImagePriors {
    neutral_hue: Option<f64>,
    neutral_hue_weight: f64,
    neutral_chroma: f64,
    neutral_chroma_half_width: f64,
}

pub fn base16_terms(samples: &[WeightedSample], slots: &[SlotSpec]) -> Vec<WeightedTerm> {
    let mut terms = coherence_terms(samples, slots, &[0, 1, 2, 3, 4, 5, 6, 7]);
    append_adaptive_chroma_terms(&mut terms, slots, 8..=15, 0.84);
    terms
}

pub fn base16_bright_terms(samples: &[WeightedSample], slots: &[SlotSpec]) -> Vec<WeightedTerm> {
    let mut terms = coherence_terms(samples, slots, &[0, 1, 2, 3, 4, 5, 6, 7]);
    append_adaptive_chroma_terms(&mut terms, slots, 8..=15, 0.92);
    terms
}

pub fn ansi16_terms(samples: &[WeightedSample], slots: &[SlotSpec]) -> Vec<WeightedTerm> {
    let mut terms = coherence_terms(samples, slots, &[0, 8, 7, 15]);
    append_adaptive_chroma_terms(&mut terms, slots, 1..=6, 0.82);
    append_adaptive_chroma_terms(&mut terms, slots, 9..=14, 0.92);
    terms
}

pub fn ansi16_light_terms(samples: &[WeightedSample], slots: &[SlotSpec]) -> Vec<WeightedTerm> {
    let mut terms = coherence_terms(samples, slots, &[0, 8, 7, 15]);
    append_adaptive_chroma_terms(&mut terms, slots, 1..=6, 0.78);
    append_adaptive_chroma_terms(&mut terms, slots, 9..=14, 0.88);
    terms
}

pub fn ansi8_terms(samples: &[WeightedSample], slots: &[SlotSpec]) -> Vec<WeightedTerm> {
    let mut terms = coherence_terms(samples, slots, &[0, 7]);
    append_adaptive_chroma_terms(&mut terms, slots, 1..=6, 0.86);
    terms
}

pub fn ansi8_light_terms(samples: &[WeightedSample], slots: &[SlotSpec]) -> Vec<WeightedTerm> {
    let mut terms = coherence_terms(samples, slots, &[0, 7]);
    append_adaptive_chroma_terms(&mut terms, slots, 1..=6, 0.82);
    terms
}

fn coherence_terms(
    samples: &[WeightedSample],
    slots: &[SlotSpec],
    neutral_slots: &[usize],
) -> Vec<WeightedTerm> {
    let priors = infer_image_priors(samples);
    let mut terms = Vec::new();

    let neutral_members = group_members(neutral_slots, slots.len());
    if !neutral_members.is_empty() {
        terms.push(weighted(
            "neutral-chroma-band",
            2.2,
            Term::GroupQuantile(GroupQuantileTerm {
                members: neutral_members.clone(),
                axis: GroupAxis::Chroma,
                target: GroupTarget::UniformRange {
                    min: (priors.neutral_chroma - priors.neutral_chroma_half_width).max(0.006),
                    max: (priors.neutral_chroma + priors.neutral_chroma_half_width).min(0.045),
                },
                monotonic: None,
                huber_delta: 0.012,
            }),
        ));
    }

    for member in &neutral_members {
        if let Some(center) = priors.neutral_hue {
            terms.push(weighted(
                &format!("{}-neutral-hue-prior", slots[member.slot].name),
                priors.neutral_hue_weight,
                Term::HueTarget(HueTargetTerm {
                    slot: member.slot,
                    target: HueUnaryTarget::Target {
                        center,
                        delta: 0.45,
                    },
                    use_hue_gate: false,
                }),
            ));
        }
    }

    terms
}

fn append_adaptive_chroma_terms(
    terms: &mut Vec<WeightedTerm>,
    slots: &[SlotSpec],
    slot_range: impl Iterator<Item = usize>,
    target: f64,
) {
    terms.extend(
        slot_range
            .filter(|&slot| slot < slots.len())
            .map(|slot| adaptive_chroma_term(slot, &slots[slot].name, target, 2.4)),
    );
}

fn adaptive_chroma_term(slot: usize, slot_name: &str, target: f64, weight: f64) -> WeightedTerm {
    weighted(
        &format!("{slot_name}-adaptive-chroma"),
        weight,
        Term::RelativeChromaTarget(RelativeChromaTargetTerm {
            slot,
            target: ScalarTarget::Target {
                value: target,
                delta: 0.10,
            },
            hinge_delta: None,
            reference: RelativeChromaReference::AdaptiveImageCap,
        }),
    )
}

fn infer_image_priors(samples: &[WeightedSample]) -> ImagePriors {
    let mut neutral_weight = 0.0;
    let mut neutral_chroma_sum = 0.0;
    let mut neutral_chroma_sq_sum = 0.0;
    let mut neutral_a_sum = 0.0;
    let mut neutral_b_sum = 0.0;

    for sample in samples {
        let lch = Oklch::from_oklab(sample.lab);
        let base_weight = sample.weight.max(0.0) * (0.65 + 0.35 * sample.saliency.clamp(0.0, 1.0));
        if base_weight <= 0.0 {
            continue;
        }

        let neutral_factor =
            ((NEUTRAL_CHROMA_CUTOFF - lch.c) / NEUTRAL_CHROMA_CUTOFF).clamp(0.0, 1.0);
        let neutral_w = base_weight * neutral_factor * neutral_factor;
        if neutral_w > 0.0 {
            neutral_weight += neutral_w;
            neutral_chroma_sum += neutral_w * lch.c;
            neutral_chroma_sq_sum += neutral_w * lch.c * lch.c;
            neutral_a_sum += neutral_w * sample.lab.a;
            neutral_b_sum += neutral_w * sample.lab.b;
        }
    }

    let neutral_chroma = if neutral_weight > 0.0 {
        (neutral_chroma_sum / neutral_weight).clamp(0.010, 0.038)
    } else {
        0.018
    };
    let neutral_chroma_half_width = if neutral_weight > 0.0 {
        let mean_sq = neutral_chroma_sq_sum / neutral_weight;
        let variance = (mean_sq - neutral_chroma * neutral_chroma).max(0.0);
        (0.004 + variance.sqrt()).clamp(0.005, 0.012)
    } else {
        0.006
    };

    let neutral_bias_c = if neutral_weight > 0.0 {
        (neutral_a_sum.hypot(neutral_b_sum) / neutral_weight).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let neutral_hue = if neutral_bias_c >= 0.003 {
        Some(neutral_b_sum.atan2(neutral_a_sum).rem_euclid(TAU))
    } else {
        None
    };
    let neutral_hue_weight = (0.45 + 28.0 * neutral_bias_c).clamp(0.0, 1.8);

    ImagePriors {
        neutral_hue,
        neutral_hue_weight,
        neutral_chroma,
        neutral_chroma_half_width,
    }
}

fn group_members(slots: &[usize], max_len: usize) -> Vec<GroupMember> {
    slots
        .iter()
        .copied()
        .filter(|&slot| slot < max_len)
        .map(|slot| GroupMember { slot, mass: 1.0 })
        .collect()
}

#[cfg(test)]
mod tests {
    use chromoxide::{
        GroupTarget, Oklab, RelativeChromaReference, ScalarTarget, Term, WeightedTerm,
    };

    use super::{
        ansi8_light_terms, ansi8_terms, ansi16_light_terms, ansi16_terms, base16_bright_terms,
        base16_terms, infer_image_priors,
    };
    use crate::palette::builtin::common::unconstrained_slot;

    fn test_slots(count: usize) -> Vec<chromoxide::SlotSpec> {
        (0..count)
            .map(|slot| unconstrained_slot(&format!("slot-{slot}")))
            .collect()
    }

    fn assert_adaptive_targets(terms: &[WeightedTerm], expected: &[(usize, f64)]) {
        let actual = terms
            .iter()
            .filter_map(|weighted| match &weighted.term {
                Term::RelativeChromaTarget(term) => Some((weighted, term)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(actual.len(), expected.len());
        for ((weighted, term), &(slot, value)) in actual.into_iter().zip(expected) {
            assert_eq!(weighted.weight, 2.4);
            assert_eq!(term.slot, slot);
            assert_eq!(term.hinge_delta, None);
            assert_eq!(term.reference, RelativeChromaReference::AdaptiveImageCap);
            match &term.target {
                ScalarTarget::Target {
                    value: actual_value,
                    delta,
                } => {
                    assert_eq!(*actual_value, value);
                    assert_eq!(*delta, 0.10);
                }
                ref other => panic!("expected target for slot {slot}, got {other:?}"),
            }
        }
    }

    #[test]
    fn priors_pick_up_neutral_tint_and_low_chroma() {
        let samples = vec![
            chromoxide::WeightedSample::new(
                Oklab {
                    l: 0.40,
                    a: -0.010,
                    b: -0.020,
                },
                3.0,
                0.2,
            ),
            chromoxide::WeightedSample::new(
                Oklab {
                    l: 0.70,
                    a: -0.012,
                    b: -0.018,
                },
                2.0,
                0.1,
            ),
        ];
        let priors = infer_image_priors(&samples);
        assert!((0.010..=0.038).contains(&priors.neutral_chroma));
        assert!((0.005..=0.012).contains(&priors.neutral_chroma_half_width));
        assert!(priors.neutral_hue.is_some());
    }

    #[test]
    fn ansi16_regular_and_bright_use_expected_adaptive_targets() {
        let terms = ansi16_terms(&[], &test_slots(16));
        let expected = (1..=6)
            .map(|slot| (slot, 0.82))
            .chain((9..=14).map(|slot| (slot, 0.92)))
            .collect::<Vec<_>>();
        assert_adaptive_targets(&terms, &expected);
    }

    #[test]
    fn ansi16_light_uses_expected_adaptive_targets() {
        let terms = ansi16_light_terms(&[], &test_slots(16));
        let expected = (1..=6)
            .map(|slot| (slot, 0.78))
            .chain((9..=14).map(|slot| (slot, 0.88)))
            .collect::<Vec<_>>();
        assert_adaptive_targets(&terms, &expected);
    }

    #[test]
    fn ansi8_uses_expected_adaptive_target() {
        let terms = ansi8_terms(&[], &test_slots(8));
        let expected = (1..=6).map(|slot| (slot, 0.86)).collect::<Vec<_>>();
        assert_adaptive_targets(&terms, &expected);
    }

    #[test]
    fn ansi8_light_uses_expected_adaptive_target() {
        let terms = ansi8_light_terms(&[], &test_slots(8));
        let expected = (1..=6).map(|slot| (slot, 0.82)).collect::<Vec<_>>();
        assert_adaptive_targets(&terms, &expected);
    }

    #[test]
    fn base16_uses_expected_adaptive_target() {
        let terms = base16_terms(&[], &test_slots(16));
        let expected = (8..=15).map(|slot| (slot, 0.84)).collect::<Vec<_>>();
        assert_adaptive_targets(&terms, &expected);
    }

    #[test]
    fn base16_bright_uses_expected_adaptive_target() {
        let terms = base16_bright_terms(&[], &test_slots(16));
        let expected = (8..=15).map(|slot| (slot, 0.92)).collect::<Vec<_>>();
        assert_adaptive_targets(&terms, &expected);
    }

    #[test]
    fn accent_mean_prior_no_longer_exists() {
        let terms = ansi16_terms(&[], &test_slots(16));
        let groups = terms
            .iter()
            .filter_map(|weighted| match &weighted.term {
                Term::GroupQuantile(term) => Some(term),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0]
                .members
                .iter()
                .map(|member| member.slot)
                .collect::<Vec<_>>(),
            vec![0, 8, 7, 15]
        );
        assert!(matches!(
            &groups[0].target,
            GroupTarget::UniformRange { .. }
        ));
    }
}
