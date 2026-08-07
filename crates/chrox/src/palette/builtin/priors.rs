use std::f64::consts::TAU;

use chromoxide::{
    GroupAxis, GroupMember, GroupQuantileTerm, GroupTarget, HueTargetTerm, HueUnaryTarget, Oklch,
    SlotSpec, Term, WeightedSample, WeightedTerm,
};

use super::common::weighted;

const NEUTRAL_CHROMA_CUTOFF: f64 = 0.08;
const ACCENT_CHROMA_FLOOR: f64 = 0.05;

#[derive(Clone, Copy, Debug)]
struct ImagePriors {
    neutral_hue: Option<f64>,
    neutral_hue_weight: f64,
    neutral_chroma: f64,
    neutral_chroma_half_width: f64,
    accent_chroma: f64,
}

pub fn base16_terms(samples: &[WeightedSample], slots: &[SlotSpec]) -> Vec<WeightedTerm> {
    coherence_terms(
        samples,
        slots,
        &[0, 1, 2, 3, 4, 5, 6, 7],
        &[8, 9, 10, 11, 12, 13, 14, 15],
    )
}

pub fn base16_bright_terms(samples: &[WeightedSample], slots: &[SlotSpec]) -> Vec<WeightedTerm> {
    coherence_terms(
        samples,
        slots,
        &[0, 1, 2, 3, 4, 5, 6, 7],
        &[8, 9, 10, 11, 12, 13, 14, 15],
    )
}

pub fn ansi16_terms(samples: &[WeightedSample], slots: &[SlotSpec]) -> Vec<WeightedTerm> {
    coherence_terms(
        samples,
        slots,
        &[0, 8, 7, 15],
        &[1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14],
    )
}

pub fn ansi8_terms(samples: &[WeightedSample], slots: &[SlotSpec]) -> Vec<WeightedTerm> {
    coherence_terms(samples, slots, &[0, 7], &[1, 2, 3, 4, 5, 6])
}

fn coherence_terms(
    samples: &[WeightedSample],
    slots: &[SlotSpec],
    neutral_slots: &[usize],
    accent_slots: &[usize],
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

    let accent_members = group_members(accent_slots, slots.len());
    if !accent_members.is_empty() {
        terms.push(weighted(
            "accent-chroma-band",
            1.8,
            Term::GroupQuantile(GroupQuantileTerm {
                members: accent_members.clone(),
                axis: GroupAxis::Chroma,
                target: GroupTarget::ExplicitValues(vec![
                    priors.accent_chroma;
                    accent_members.len()
                ]),
                monotonic: None,
                huber_delta: 0.020,
            }),
        ));
    }

    terms
}

fn infer_image_priors(samples: &[WeightedSample]) -> ImagePriors {
    let mut neutral_weight = 0.0;
    let mut neutral_chroma_sum = 0.0;
    let mut neutral_chroma_sq_sum = 0.0;
    let mut neutral_a_sum = 0.0;
    let mut neutral_b_sum = 0.0;

    let mut accent_weight = 0.0;
    let mut accent_chroma_sum = 0.0;

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

        let accent_factor = ((lch.c - ACCENT_CHROMA_FLOOR) / 0.16).clamp(0.0, 1.0);
        let accent_w = base_weight * accent_factor;
        if accent_w > 0.0 {
            let capped_chroma = lch.c.min(0.22);
            accent_weight += accent_w;
            accent_chroma_sum += accent_w * capped_chroma;
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

    let accent_chroma = if accent_weight > 0.0 {
        (accent_chroma_sum / accent_weight).clamp(0.08, 0.16)
    } else {
        0.12
    };

    ImagePriors {
        neutral_hue,
        neutral_hue_weight,
        neutral_chroma,
        neutral_chroma_half_width,
        accent_chroma,
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
    use chromoxide::Oklab;

    use super::infer_image_priors;

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
    fn priors_pick_up_accent_chroma_band() {
        let samples = vec![
            chromoxide::WeightedSample::new(
                chromoxide::Oklch {
                    l: 0.55,
                    c: 0.14,
                    h: 0.4,
                }
                .to_oklab(),
                2.0,
                0.7,
            ),
            chromoxide::WeightedSample::new(
                chromoxide::Oklch {
                    l: 0.62,
                    c: 0.12,
                    h: 2.0,
                }
                .to_oklab(),
                2.0,
                0.8,
            ),
        ];
        let priors = infer_image_priors(&samples);
        assert!((0.08..=0.16).contains(&priors.accent_chroma));
    }
}
