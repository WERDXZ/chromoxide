//! Experimental source assignments, deliberately outside the shipped recipe.
use chromoxide::{
    ChromaTargetTerm, HueTargetTerm, HueUnaryTarget, LightnessTargetTerm, Oklch, PaletteProblem,
    ScalarTarget, Term, WeightedSample, WeightedTerm,
};

// Hypotheses for this experiment, not validated perceptual thresholds.
const MIN_AREA: f64 = 0.005;
const MIN_DISTANCE: f64 = 0.04;

// Reserve one seat per selected source mode, then distribute the remaining seats
// toward area quotas. Repeated seats are intentional for near-monochrome images.
pub(super) fn source_anchors(samples: &[WeightedSample], count: usize) -> Vec<WeightedSample> {
    if samples.is_empty() || count == 0 {
        return Vec::new();
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| {
        a.lab
            .l
            .total_cmp(&b.lab.l)
            .then(a.lab.a.total_cmp(&b.lab.a))
            .then(a.lab.b.total_cmp(&b.lab.b))
            .then(a.weight.total_cmp(&b.weight))
    });
    let mut merged: Vec<WeightedSample> = Vec::new();
    for sample in sorted {
        if let Some(last) = merged.last_mut().filter(|last| last.lab == sample.lab) {
            let mass = last.weight + sample.weight;
            last.saliency = (last.saliency * last.weight + sample.saliency * sample.weight) / mass;
            last.weight = mass;
        } else {
            merged.push(sample);
        }
    }
    let mass: f64 = merged.iter().map(|s| s.weight).sum();
    for sample in &mut merged {
        sample.weight /= mass;
    }
    merged.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    let mut modes = vec![merged.remove(0)];
    while modes.len() < count {
        let candidate = merged
            .iter()
            .enumerate()
            .filter_map(|(i, sample)| {
                let distance2 = modes
                    .iter()
                    .map(|m| m.lab.distance2(sample.lab))
                    .fold(f64::INFINITY, f64::min);
                (sample.weight >= MIN_AREA && distance2 >= MIN_DISTANCE * MIN_DISTANCE)
                    .then_some((i, sample.weight * distance2))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let Some((index, _)) = candidate else { break };
        modes.push(merged.remove(index));
    }
    let selected_mass: f64 = modes.iter().map(|s| s.weight).sum();
    let mut seats = vec![1; modes.len()];
    let mut anchors = modes.clone();
    while anchors.len() < count {
        let index = (0..modes.len())
            .max_by(|&a, &b| {
                let deficit =
                    |i: usize| count as f64 * modes[i].weight / selected_mass - seats[i] as f64;
                deficit(a).total_cmp(&deficit(b))
            })
            .unwrap();
        seats[index] += 1;
        anchors.push(modes[index].clone());
    }
    anchors
}

/// Apply source targets with existing unary terms. Area chooses seat allocation;
/// it does not weaken the target of a seat reserved for a minority color.
pub(super) fn apply(
    problem: &mut PaletteProblem,
    keep_priors: bool,
) -> Vec<(usize, WeightedSample)> {
    let anchors = source_anchors(&problem.samples, 8);
    if anchors.is_empty() {
        return Vec::new();
    }
    if !keep_priors {
        problem.terms.retain(|weighted| match &weighted.term {
            Term::Support(term) => !term.slots.iter().all(|&slot| slot >= 8),
            Term::RelativeChromaTarget(term) => term.slot < 8,
            _ => true,
        });
    }
    let mut available: Vec<_> = (8..16).collect();
    let mut assignments = Vec::new();
    for source in anchors {
        let target = source.lab.to_oklch();
        // Deterministic greedy assignment: reserve each distinct mode first,
        // preferring an available role that needs the least L/C projection.
        let index = (0..available.len())
            .min_by(|&a, &b| {
                let cost = |i: usize| {
                    let domain = &problem.slots[available[i]].domain;
                    (target.l - target.l.clamp(domain.lightness.min, domain.lightness.max)).powi(2)
                        + (target.c - target.c.clamp(domain.chroma.min, domain.chroma.max)).powi(2)
                };
                cost(a).total_cmp(&cost(b))
            })
            .unwrap();
        let slot = available.remove(index);
        add_target(problem, slot, target, "source");
        assignments.push((slot, source));
    }
    assignments
}

pub(super) fn add_target(problem: &mut PaletteProblem, slot: usize, target: Oklch, origin: &str) {
    let domain = &problem.slots[slot].domain;
    let l = target.l.clamp(domain.lightness.min, domain.lightness.max);
    let c = target.c.clamp(domain.chroma.min, domain.chroma.max);
    // Locally approximates 10 * (dL² + dC² + C_source² * dh²).
    // This is not globally identical to squared Oklab distance.
    for (axis, weight, term) in [
        (
            "lightness",
            20.0,
            Term::LightnessTarget(LightnessTargetTerm {
                slot,
                target: ScalarTarget::Target {
                    value: l,
                    delta: 0.1,
                },
                hinge_delta: None,
            }),
        ),
        (
            "chroma",
            20.0,
            Term::ChromaTarget(ChromaTargetTerm {
                slot,
                target: ScalarTarget::Target {
                    value: c,
                    delta: 0.1,
                },
                hinge_delta: None,
            }),
        ),
        (
            "hue",
            20.0 * c * c,
            Term::HueTarget(HueTargetTerm {
                slot,
                target: HueUnaryTarget::Target {
                    center: target.h,
                    delta: 1.0,
                },
                use_hue_gate: false,
            }),
        ),
    ] {
        problem.terms.push(WeightedTerm {
            weight,
            name: Some(format!("{origin}-{slot}-{axis}")),
            term,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chromoxide::Oklab;

    fn evidence(red_mass: f64) -> Vec<WeightedSample> {
        vec![
            WeightedSample::new(
                Oklab {
                    l: 0.6,
                    a: 0.0,
                    b: 0.0,
                },
                1.0 - red_mass,
                0.1,
            ),
            WeightedSample::new(
                Oklab {
                    l: 0.56,
                    a: 0.18,
                    b: 0.10,
                },
                red_mass,
                0.9,
            ),
        ]
    }

    fn red_count(samples: &[WeightedSample]) -> usize {
        samples.iter().filter(|s| s.lab.a > 0.1).count()
    }

    #[test]
    fn semantic_assignment_reserves_error_for_the_source_red() {
        let mut problem = super::super::make_problem(
            &evidence(0.0125),
            None,
            true,
            true,
            super::super::experiment_config(),
        );
        let assignments = super::super::semantics::apply(&mut problem);
        let red = assignments
            .iter()
            .find(|a| a.source.as_ref().is_some_and(|s| s.lab.a > 0.1))
            .unwrap();
        assert_eq!(red.slot, 8, "source red must go to error, not success");
    }

    #[test]
    fn reserves_a_slot_for_small_region_without_promoting_one_pixel() {
        assert_eq!(red_count(&source_anchors(&evidence(0.0125), 8)), 1);
        assert_eq!(red_count(&source_anchors(&evidence(1.0 / 5120.0), 8)), 0);
    }

    #[test]
    fn follows_area_hierarchy_without_requiring_multiple_colors() {
        assert_eq!(red_count(&source_anchors(&evidence(0.2), 8)), 2);
        assert_eq!(red_count(&source_anchors(&evidence(0.8), 8)), 6);
        let gray = evidence(0.0)[0].clone();
        assert!(
            source_anchors(std::slice::from_ref(&gray), 8)
                .iter()
                .all(|s| s.lab == gray.lab)
        );
        assert!(source_anchors(&[], 8).is_empty());
        assert!(source_anchors(&[gray], 0).is_empty());
    }

    #[test]
    fn assignment_is_invariant_to_exact_color_splits_and_order() {
        let samples = evidence(0.0125);
        let expected = source_anchors(&samples, 8);
        // Each red piece is below the area cutoff; merging must precede filtering.
        let mut split = vec![
            WeightedSample {
                weight: samples[1].weight / 5.0,
                ..samples[1]
            };
            5
        ];
        split.push(samples[0].clone());
        for _ in 0..split.len() {
            let actual = source_anchors(&split, 8);
            for (a, b) in actual.iter().zip(&expected) {
                assert_eq!(a.lab, b.lab);
                assert!((a.weight - b.weight).abs() < 1e-12);
            }
            assert_eq!(actual.len(), expected.len());
            split.rotate_left(1);
        }
    }

    #[test]
    fn does_not_assign_a_second_mode_to_near_duplicate_colors() {
        let mut samples = evidence(0.2);
        samples[1].lab = Oklab {
            l: 0.601,
            a: 0.001,
            b: 0.001,
        };
        assert!(
            source_anchors(&samples, 8)
                .iter()
                .all(|s| s.lab == samples[0].lab)
        );
    }

    #[test]
    fn reserved_minority_slot_prefers_its_source_to_the_dominant_color() {
        use chromoxide::objective::ObjectiveEvaluator;
        use chromoxide::util::inv_sigmoid;

        let samples = evidence(0.0125);
        let mut problem = super::super::make_problem(
            &samples,
            None,
            true,
            true,
            super::super::experiment_config(),
        );
        let assignments = apply(&mut problem, false);
        // Isolate the source objective from image-cap penalties in this unit test.
        for slot in &mut problem.slots {
            slot.domain.cap_policy = chromoxide::CapPolicy::Ignore;
        }
        problem.validate().unwrap();
        let (slot, source) = assignments.iter().find(|(_, s)| s.lab.a > 0.1).unwrap();
        let domain = &problem.slots[*slot].domain;
        let candidate = |lab: Oklab| {
            let color = lab.to_oklch();
            let mut latent = vec![0.0; 48];
            latent[slot * 3] = inv_sigmoid(
                (color.l - domain.lightness.min) / (domain.lightness.max - domain.lightness.min),
            );
            latent[slot * 3 + 1] = inv_sigmoid(color.c / domain.chroma.max);
            latent[slot * 3 + 2] = color.h;
            latent
        };
        let eval = ObjectiveEvaluator::new(&problem).unwrap();
        let at_source = eval.evaluate_total(&candidate(source.lab)).unwrap();
        let at_majority = eval.evaluate_total(&candidate(samples[0].lab)).unwrap();
        assert!(
            at_source + 0.1 < at_majority,
            "{at_source} vs {at_majority}"
        );
    }
}
