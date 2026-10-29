//! Opt-in theme adaptation, separate from source palette extraction.
use chromoxide::{
    CapPolicy, ContrastTerm, HueDomain, Oklch, PaletteProblem, Term, WeightedSample, WeightedTerm,
};

use super::anchors;

// Experimental role conventions and validation thresholds, not perceptual guarantees.
pub(super) const MIN_CONTRAST: f64 = 4.5;
pub(super) const MIN_ROLE_DISTANCE: f64 = 0.04;
const MIN_ROLE_CHROMA: f64 = 0.035;
pub(super) const TEXT_SLOTS: [usize; 10] = [3, 5, 8, 9, 10, 11, 12, 13, 14, 15];

#[derive(Clone, Copy)]
pub(super) struct Role {
    pub name: &'static str,
    pub slot: usize,
    start: f64,
    len: f64,
}

impl Role {
    pub fn hue(self) -> HueDomain {
        HueDomain::Arc {
            start: self.start.to_radians(),
            len: self.len.to_radians(),
        }
    }
}

pub(super) const ROLES: [Role; 3] = [
    Role {
        name: "error",
        slot: 8,
        start: 350.0,
        len: 55.0,
    },
    Role {
        name: "success",
        slot: 11,
        start: 125.0,
        len: 45.0,
    },
    Role {
        name: "link",
        slot: 13,
        start: 225.0,
        len: 55.0,
    },
];

pub(super) struct Assignment {
    pub slot: usize,
    // Source-backed means adapted from this color, not necessarily an exact copy.
    pub source: Option<WeightedSample>,
    pub target: Oklch,
}

pub(super) fn apply(problem: &mut PaletteProblem) -> Vec<Assignment> {
    apply_policy(problem, false)
}

pub(super) fn apply_protected(problem: &mut PaletteProblem) -> Vec<Assignment> {
    apply_policy(problem, true)
}

fn apply_policy(problem: &mut PaletteProblem, protect: bool) -> Vec<Assignment> {
    let mut modes = Vec::<WeightedSample>::new();
    for sample in anchors::source_anchors(&problem.samples, 8) {
        if !modes.iter().any(|mode| mode.lab == sample.lab) {
            modes.push(sample);
        }
    }
    if modes.is_empty() {
        return Vec::new();
    }
    problem.terms.retain(|weighted| match &weighted.term {
        Term::Support(t) => !t.slots.iter().all(|&slot| slot >= 8),
        Term::RelativeChromaTarget(t) => t.slot < 8,
        _ => true,
    });
    if protect {
        // Comments are foreground text too. The original L <= .46 cannot
        // satisfy this contract. Keep room between comment, secondary and
        // main foreground targets rather than fighting the old ladder.
        problem.slots[3].domain.lightness.max = 0.68;
        problem.slots[4].domain.lightness.max = 0.72;
        for weighted in &mut problem.terms {
            if weighted.name.as_deref() == Some("base-ladder")
                && let Term::GroupQuantile(term) = &mut weighted.term
                && let chromoxide::GroupTarget::ExplicitValues(values) = &mut term.target
            {
                values[3] = 0.60;
                values[4] = 0.65;
            }
        }
        for slot in TEXT_SLOTS {
            problem.terms.push(WeightedTerm {
                name: Some(format!("text-{slot}-contrast")),
                weight: 20.0,
                term: Term::Contrast(ContrastTerm {
                    fg: slot,
                    bg: 0,
                    min_ratio: MIN_CONTRAST + 0.2,
                    hinge_delta: Some(0.25),
                }),
            });
        }
    }
    let mass: f64 = modes.iter().map(|s| s.weight).sum();
    let mean = |axis: fn(Oklch) -> f64| {
        modes
            .iter()
            .map(|s| s.weight * axis(s.lab.to_oklch()))
            .sum::<f64>()
            / mass
    };
    // A small semantic floor is intentional here; the source-only mode has none.
    // Area weighting prevents one small vivid patch from saturating every role.
    let style_c = mean(|c| c.c).clamp(0.04, 0.12);
    let style_l = mean(|c| c.l).clamp(0.60, 0.70);
    let mut seats = vec![0; modes.len()];
    let mut assignments = Vec::new();
    for role in ROLES {
        let source_index = (0..modes.len())
            .filter(|&i| {
                let c = modes[i].lab.to_oklch();
                c.c >= MIN_ROLE_CHROMA && role.hue().contains(c.h)
            })
            .max_by(|&a, &b| modes[a].weight.total_cmp(&modes[b].weight));
        let source = source_index.map(|i| {
            seats[i] += 1;
            modes[i].clone()
        });
        let mut target = source.as_ref().map_or(
            Oklch {
                l: style_l,
                c: style_c,
                h: (role.start + role.len / 2.0).to_radians(),
            },
            |s| s.lab.to_oklch(),
        );
        let domain = &mut problem.slots[role.slot].domain;
        domain.hue = role.hue();
        if protect {
            // Leave an experimental margin for 8-bit export near arc edges;
            // final HEX is still checked against the original role arc.
            let margin = 3.0;
            domain.hue = HueDomain::Arc {
                start: (role.start + margin).to_radians(),
                len: (role.len - 2.0 * margin).to_radians(),
            };
            let offset = (target.h.to_degrees() - role.start).rem_euclid(360.0);
            target.h = (role.start + offset.clamp(margin, role.len - margin)).to_radians();
        }
        domain.chroma.min = MIN_ROLE_CHROMA;
        if source.is_none() {
            domain.chroma.max = style_c;
        }
        // A role may move in L for readability or have no source hue at all.
        // Its chroma comes from the selected source/style target, not a cap at
        // the adapted coordinates. The control keeps the cap on free accents.
        domain.cap_policy = CapPolicy::Ignore;
        if !protect {
            problem.terms.push(WeightedTerm {
                name: Some(format!("semantic-{}-contrast", role.name)),
                weight: 10.0,
                term: Term::Contrast(ContrastTerm {
                    fg: role.slot,
                    bg: 0,
                    min_ratio: MIN_CONTRAST + 0.1,
                    hinge_delta: Some(0.25),
                }),
            });
        }
        assignments.push(Assignment {
            slot: role.slot,
            source,
            target,
        });
    }
    // The protected adapter reserves distinct chromatic modes first. Both
    // policies then keep unrepresented modes and fill by area deficit. Semantic
    // source seats count toward the quota; synthesized colors claim no area.
    for slot in (8..16).filter(|slot| !ROLES.iter().any(|r| r.slot == *slot)) {
        let distinct = if protect {
            distinct_unrepresented(&modes, &seats)
        } else {
            None
        };
        let index = distinct.unwrap_or_else(|| {
            (0..modes.len())
                .max_by(|&a, &b| {
                    let deficit = |i: usize| 8.0 * modes[i].weight / mass - seats[i] as f64;
                    (seats[a] == 0)
                        .cmp(&(seats[b] == 0))
                        .then(deficit(a).total_cmp(&deficit(b)))
                })
                .unwrap()
        });
        seats[index] += 1;
        let source = modes[index].clone();
        assignments.push(Assignment {
            slot,
            target: source.lab.to_oklch(),
            source: Some(source),
        });
    }
    for assignment in &mut assignments {
        if protect {
            let domain = &mut problem.slots[assignment.slot].domain;
            // Theme lightness may be unsupported at the source coordinates.
            // Preserve observed chroma instead of applying the image cap at
            // that new L. Synthetic roles retain their separate style cap.
            domain.cap_policy = CapPolicy::Ignore;
            if assignment.source.is_some() {
                domain.chroma.max = assignment.target.c.max(domain.chroma.min);
            }
        }
        let domain = &problem.slots[assignment.slot].domain;
        assignment.target.l = assignment
            .target
            .l
            .clamp(domain.lightness.min, domain.lightness.max);
        assignment.target.c = assignment
            .target
            .c
            .clamp(domain.chroma.min, domain.chroma.max);
        anchors::add_target(
            problem,
            assignment.slot,
            assignment.target,
            if assignment.source.is_some() {
                "source"
            } else {
                "synthetic"
            },
        );
    }
    assignments
}

fn distinct_unrepresented(modes: &[WeightedSample], seats: &[usize]) -> Option<usize> {
    if seats.iter().all(|&count| count == 0) {
        return None; // The usual area rule reserves the dominant source first.
    }
    (0..modes.len())
        .filter(|&i| seats[i] == 0)
        .filter_map(|i| {
            // Ignore L here: theme adaptation must be able to lighten dark
            // text. Reserve distinct chromatic modes before tonal variants.
            let distance2 = modes
                .iter()
                .zip(seats)
                .filter(|(_, count)| **count > 0)
                .map(|(other, _)| {
                    (modes[i].lab.a - other.lab.a).powi(2) + (modes[i].lab.b - other.lab.b).powi(2)
                })
                .fold(f64::INFINITY, f64::min);
            (distance2 >= 0.04_f64.powi(2)).then_some((i, modes[i].weight * distance2))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coast_samples() -> Vec<WeightedSample> {
        let data: toml::Value =
            toml::from_str(include_str!("fixtures/coast-samples.toml")).unwrap();
        data["samples"].clone().try_into().unwrap()
    }

    #[test]
    fn protected_theme_keeps_the_coast_turquoise_when_semantic_roles_need_seats() {
        let samples = coast_samples();
        let turquoise = samples[8].lab;
        assert!(
            anchors::source_anchors(&samples, 8)
                .iter()
                .any(|s| s.lab == turquoise)
        );
        let assignments = apply_protected(&mut super::super::make_problem(
            &samples,
            None,
            true,
            true,
            super::super::experiment_config(),
        ));
        assert!(
            assignments
                .iter()
                .any(|a| a.source.as_ref().is_some_and(|s| s.lab == turquoise)),
            "the extracted 2.9% turquoise must survive theme allocation"
        );
        let cap = chromoxide::ImageCapBuilder::default()
            .build_statistical(&samples, Default::default())
            .unwrap();
        let mut config = super::super::experiment_config();
        config.seed_count = std::num::NonZeroUsize::new(2).unwrap();
        let mut problem = super::super::make_problem(&samples, Some(&cap), true, true, config);
        let assigned = apply_protected(&mut problem)
            .into_iter()
            .find(|a| a.source.as_ref().is_some_and(|s| s.lab == turquoise))
            .unwrap();
        let solved = chromoxide::solve_with_seed(&problem, [7; 32]).unwrap();
        let (r, g, b) = crate::filter::srgb_u8(solved.colors_lch[assigned.slot]);
        let path =
            std::env::temp_dir().join(format!("chrox-coast-export-{}.ppm", std::process::id()));
        super::super::write_ppm(&path, 1, 1, &[[r, g, b]]);
        let image = chromoxide_image::load_image_from_path(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        let displayed = chromoxide_image::prepare_image(&image, &Default::default()).unwrap();
        assert!(
            displayed.pixels[0].lab.distance2(turquoise).sqrt() < 0.02,
            "the reserved turquoise must also survive optimization and export"
        );
    }

    #[test]
    fn protected_theme_allocation_survives_splitting_reordering_and_mass_scaling() {
        let samples = coast_samples();
        let mut split: Vec<_> = samples
            .iter()
            .flat_map(|s| {
                (0..4).map(move |_| WeightedSample {
                    weight: s.weight * 2.5,
                    ..s.clone()
                })
            })
            .collect();
        split.reverse();
        let allocate = |samples: &[WeightedSample]| {
            apply_protected(&mut super::super::make_problem(
                samples,
                None,
                true,
                true,
                super::super::experiment_config(),
            ))
        };
        for (before, after) in allocate(&samples).iter().zip(allocate(&split)) {
            assert_eq!(before.slot, after.slot);
            assert_eq!(
                before.source.as_ref().map(|s| s.lab),
                after.source.as_ref().map(|s| s.lab)
            );
            assert!(before.target.to_oklab().distance2(after.target.to_oklab()) < 1e-20);
        }
    }

    #[test]
    fn protected_theme_makes_all_used_text_colors_readable_after_hex_export() {
        let samples = vec![WeightedSample::new(
            Oklch {
                l: 0.2,
                c: 0.01,
                h: 3.5,
            }
            .to_oklab(),
            1.0,
            0.5,
        )];
        let mut config = super::super::experiment_config();
        config.seed_count = std::num::NonZeroUsize::new(2).unwrap();
        let mut problem = super::super::make_problem(&samples, None, true, true, config);
        for slot in &mut problem.slots {
            slot.domain.cap_policy = CapPolicy::Ignore;
        }
        apply_protected(&mut problem);
        let solution = chromoxide::solve_with_seed(&problem, [7; 32]).unwrap();
        let luminance = |color| {
            let (r, g, b) = crate::filter::srgb_u8(color);
            0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
        };
        let bg = luminance(solution.colors_lch[0]);
        for slot in [3, 5, 8, 9, 10, 11, 12, 13, 14, 15] {
            let ratio =
                chromoxide::convert::contrast_ratio(bg, luminance(solution.colors_lch[slot]));
            assert!(
                ratio >= MIN_CONTRAST,
                "text slot {slot} only reaches {ratio:.3}:1"
            );
        }
    }

    fn linear(v: u8) -> f64 {
        let c = v as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn sample(c: f64, hue: f64, weight: f64) -> WeightedSample {
        WeightedSample::new(
            Oklch {
                l: 0.6,
                c,
                h: hue.to_radians(),
            }
            .to_oklab(),
            weight,
            0.5,
        )
    }

    fn plan(samples: &[WeightedSample]) -> Vec<Assignment> {
        apply(&mut super::super::make_problem(
            samples,
            None,
            true,
            true,
            super::super::experiment_config(),
        ))
    }

    #[test]
    fn reserves_matching_roles_before_area_seats_and_marks_missing_hues() {
        for (hue, slot) in [(28.0, 8), (145.0, 11), (250.0, 13)] {
            let samples = [sample(0.0, 0.0, 0.9875), sample(0.18, hue, 0.0125)];
            let assignments = plan(&samples);
            assert_eq!(assignments.len(), 8);
            let assigned = assignments.iter().find(|a| a.slot == slot).unwrap();
            assert_eq!(assigned.source.as_ref().unwrap().lab, samples[1].lab);
            for role in ROLES.into_iter().filter(|r| r.slot != slot) {
                let other = assignments.iter().find(|a| a.slot == role.slot).unwrap();
                assert!(other.source.is_none());
                assert!(
                    other.target.c <= 0.04,
                    "minority chroma must not saturate missing roles"
                );
            }
        }
    }

    #[test]
    fn gray_and_isolated_pixel_do_not_claim_semantic_source_evidence() {
        for samples in [
            vec![sample(0.0, 0.0, 1.0)],
            vec![
                sample(0.0, 0.0, 1.0 - 1.0 / 5120.0),
                sample(0.2, 28.0, 1.0 / 5120.0),
            ],
        ] {
            let assignments = plan(&samples);
            for role in ROLES {
                assert!(
                    assignments
                        .iter()
                        .find(|a| a.slot == role.slot)
                        .unwrap()
                        .source
                        .is_none()
                );
            }
            assert_eq!(assignments.iter().filter(|a| a.source.is_some()).count(), 5);
        }
        assert!(plan(&[]).is_empty());
    }

    #[test]
    fn matching_and_provenance_survive_color_splitting_and_reordering() {
        let samples = [
            sample(0.0, 0.0, 0.8),
            sample(0.18, 28.0, 0.1),
            sample(0.1, 145.0, 0.1),
        ];
        let expected = plan(&samples);
        let mut split: Vec<_> = samples
            .iter()
            .flat_map(|s| {
                (0..5).map(move |_| WeightedSample {
                    weight: s.weight / 5.0,
                    ..s.clone()
                })
            })
            .collect();
        split.reverse();
        let actual = plan(&split);
        for (a, b) in expected.iter().zip(&actual) {
            assert_eq!(a.slot, b.slot);
            assert_eq!(
                a.source.as_ref().map(|s| s.lab),
                b.source.as_ref().map(|s| s.lab)
            );
            assert!(a.target.to_oklab().distance2(b.target.to_oklab()) < 1e-20);
        }
        for slot in [8, 11] {
            assert!(
                actual
                    .iter()
                    .find(|a| a.slot == slot)
                    .unwrap()
                    .source
                    .is_some()
            );
        }
    }
}
