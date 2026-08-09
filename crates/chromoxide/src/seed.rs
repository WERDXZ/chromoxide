//! Multi-start latent seed generation.

use rand::rngs::ChaCha8Rng;
use rand::{Rng, RngExt, SeedableRng};

use crate::color::Oklch;
use crate::domain::{HueDomain, SlotDomain};
use crate::error::PaletteError;
use crate::problem::PaletteProblem;
use crate::term::{GroupAxis, Term};
use crate::terms::group_quantile::{compute_mass_quantile_centers, compute_targets};
use crate::util::{arc_length, inv_sigmoid, wrap_hue};

/// Fixed-width seed used by deterministic solver entrypoints.
pub type SolveSeed = [u8; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SeedKind {
    Random,
    GroupTargeted,
    SupportAware,
}

/// Generates multi-start seeds using the caller-provided RNG.
pub fn generate_seeds(
    problem: &PaletteProblem,
    rng: &mut dyn Rng,
) -> Result<Vec<Vec<f64>>, PaletteError> {
    let seed_count = problem.config.seed_count.get();
    let seeds = build_seed_plan(seed_count)
        .into_iter()
        .map(|(kind, variant)| generate_one_seed(problem, kind, variant, rng))
        .collect::<Vec<_>>();

    validate_seed_dimensions(problem, &seeds)?;
    Ok(seeds)
}

/// Generates deterministic multi-start seeds using independent ChaCha streams.
pub fn generate_seeds_with_seed(
    problem: &PaletteProblem,
    seed: SolveSeed,
) -> Result<Vec<Vec<f64>>, PaletteError> {
    let plan = build_seed_plan(problem.config.seed_count.get());
    let mut seeds = Vec::with_capacity(plan.len());

    for (seed_index, (kind, variant)) in plan.into_iter().enumerate() {
        let mut rng = ChaCha8Rng::from_seed(seed);
        rng.set_stream(seed_index as u64);
        seeds.push(generate_one_seed(problem, kind, variant, &mut rng));
    }

    validate_seed_dimensions(problem, &seeds)?;
    Ok(seeds)
}

fn validate_seed_dimensions(
    problem: &PaletteProblem,
    seeds: &[Vec<f64>],
) -> Result<(), PaletteError> {
    let dim = problem.slots.len() * 3;

    if seeds.iter().any(|s| s.len() != dim) {
        return Err(PaletteError::InvalidProblem(
            "internal seed dimensionality mismatch".to_string(),
        ));
    }
    Ok(())
}

fn build_seed_plan(seed_count: usize) -> Vec<(SeedKind, usize)> {
    let (n_random, n_targeted, n_support) = seed_mix_counts(seed_count);
    let mut plan = Vec::with_capacity(seed_count);
    plan.extend((0..n_random).map(|variant| (SeedKind::Random, variant)));
    plan.extend((0..n_targeted).map(|variant| (SeedKind::GroupTargeted, variant)));
    plan.extend((0..n_support).map(|variant| (SeedKind::SupportAware, variant)));
    plan
}

fn generate_one_seed(
    problem: &PaletteProblem,
    kind: SeedKind,
    variant: usize,
    rng: &mut dyn Rng,
) -> Vec<f64> {
    match kind {
        SeedKind::Random => random_seed(problem, rng),
        SeedKind::GroupTargeted => group_targeted_seed(problem, rng, variant),
        SeedKind::SupportAware => support_aware_seed(problem, rng, variant),
    }
}

/// Splits total seeds into random / group-targeted / support-aware buckets.
fn seed_mix_counts(seed_count: usize) -> (usize, usize, usize) {
    match seed_count {
        0 => (0, 0, 0),
        1 => (1, 0, 0),
        2 => (1, 1, 0),
        _ => {
            let mut n_random = (seed_count * 5) / 10;
            let mut n_targeted = (seed_count * 3) / 10;
            let mut n_support = seed_count.saturating_sub(n_random + n_targeted);

            n_random = n_random.max(1);
            n_targeted = n_targeted.max(1);
            n_support = n_support.max(1);

            // Ensure every category keeps at least one member while shrinking.
            while n_random + n_targeted + n_support > seed_count {
                if n_random >= n_targeted && n_random >= n_support && n_random > 1 {
                    n_random -= 1;
                } else if n_targeted >= n_support && n_targeted > 1 {
                    n_targeted -= 1;
                } else if n_support > 1 {
                    n_support -= 1;
                } else {
                    break;
                }
            }

            while n_random + n_targeted + n_support < seed_count {
                n_random += 1;
            }

            (n_random, n_targeted, n_support)
        }
    }
}

/// Draws an unconstrained random latent seed for all slots.
fn random_seed(problem: &PaletteProblem, rng: &mut dyn Rng) -> Vec<f64> {
    let mut u = vec![0.0; problem.slots.len() * 3];
    for (i, slot) in problem.slots.iter().enumerate() {
        let base = i * 3;
        u[base] = rng.random_range(-2.0..2.0);

        if slot.domain.is_neutralish() {
            u[base + 1] = rng.random_range(-5.0..-2.5);
        } else {
            u[base + 1] = rng.random_range(-2.0..2.0);
        }

        u[base + 2] = match slot.domain.hue {
            HueDomain::Any => rng.random_range(0.0..std::f64::consts::TAU),
            HueDomain::Arc { .. } => rng.random_range(-2.0..2.0),
        };
    }
    u
}

/// Builds a seed biased toward configured group-quantile targets.
fn group_targeted_seed(problem: &PaletteProblem, rng: &mut dyn Rng, variant: usize) -> Vec<f64> {
    let mut u = random_seed(problem, rng);

    for wt in &problem.terms {
        let Term::GroupQuantile(g) = &wt.term else {
            continue;
        };
        if g.members.is_empty() {
            continue;
        }
        let masses = g
            .members
            .iter()
            .map(|member| member.mass)
            .collect::<Vec<_>>();
        let Ok(qs) = compute_mass_quantile_centers(&masses) else {
            continue;
        };
        let Ok(targets) = compute_targets(&qs, &g.target, g.members.len()) else {
            continue;
        };

        for (k, member) in g.members.iter().enumerate() {
            let slot_idx = member.slot;
            let base = slot_idx * 3;
            let domain = problem.slots[slot_idx].domain;
            let jitter = ((variant as f64 * 0.017 + k as f64 * 0.011).sin()) * 0.02;
            let t = targets[k] + jitter;

            match g.axis {
                GroupAxis::Lightness => {
                    u[base] = map_to_interval_latent(t, domain.lightness.min, domain.lightness.max);
                }
                GroupAxis::Chroma => {
                    u[base + 1] = map_to_interval_latent(t, domain.chroma.min, domain.chroma.max);
                }
                GroupAxis::HueArc { start, end } => {
                    let len = arc_length(start, end).max(1.0e-9);
                    let h = wrap_hue(start + t.clamp(0.0, len));
                    u[base + 2] = map_hue_to_latent(h, domain.hue);
                }
            }

            if domain.is_neutralish() {
                u[base + 1] = -4.0;
            }
        }
    }

    u
}

/// Builds a seed by snapping each slot near high-scoring support samples.
fn support_aware_seed(problem: &PaletteProblem, rng: &mut dyn Rng, variant: usize) -> Vec<f64> {
    let mut u = random_seed(problem, rng);

    let sample_lch: Vec<_> = problem
        .samples
        .iter()
        .map(|s| Oklch::from_oklab(s.lab))
        .collect();

    for (slot_idx, slot) in problem.slots.iter().enumerate() {
        let candidates = ranked_sample_candidates(problem, slot_idx, &sample_lch);
        if candidates.is_empty() {
            continue;
        }
        let pick = candidates[(variant + slot_idx) % candidates.len()];
        let lch = sample_lch[pick];
        let base = slot_idx * 3;

        u[base] =
            map_to_interval_latent(lch.l, slot.domain.lightness.min, slot.domain.lightness.max);
        u[base + 1] = if slot.domain.is_neutralish() {
            -4.0
        } else {
            map_to_interval_latent(lch.c, slot.domain.chroma.min, slot.domain.chroma.max)
        };
        u[base + 2] = map_hue_to_latent(lch.h, slot.domain.hue);
    }

    u
}

/// Ranks support samples for a slot by domain fit and proximity.
fn ranked_sample_candidates(
    problem: &PaletteProblem,
    slot_idx: usize,
    sample_lch: &[Oklch],
) -> Vec<usize> {
    let domain = problem.slots[slot_idx].domain;
    let center = domain_center_lch(domain);
    let center_lab = center.to_oklab();

    let mut scored = Vec::new();
    for (i, sample) in problem.samples.iter().enumerate() {
        let lch = sample_lch[i];
        if !domain.lightness.contains(lch.l) {
            continue;
        }
        if !domain.hue.contains(lch.h) {
            continue;
        }
        let penalty_c = if lch.c < domain.chroma.min {
            domain.chroma.min - lch.c
        } else if lch.c > domain.chroma.max {
            lch.c - domain.chroma.max
        } else {
            0.0
        };
        let dist2 = center_lab.distance2(sample.lab);
        let score = sample.weight - 2.5 * penalty_c - 6.0 * dist2;
        scored.push((score, i));
    }

    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.into_iter().map(|(_, i)| i).collect()
}

/// Returns the geometric center of a slot domain in OkLCh.
fn domain_center_lch(domain: SlotDomain) -> Oklch {
    let h = match domain.hue {
        HueDomain::Any => 0.0,
        HueDomain::Arc { start, len } => wrap_hue(start + 0.5 * len),
    };
    Oklch {
        l: domain.lightness.midpoint(),
        c: domain.chroma.midpoint(),
        h,
    }
}

/// Encodes a clamped scalar into an unconstrained interval latent.
fn map_to_interval_latent(v: f64, min: f64, max: f64) -> f64 {
    if (max - min).abs() < 1.0e-12 {
        return 0.0;
    }
    let t = ((v - min) / (max - min)).clamp(1.0e-6, 1.0 - 1.0e-6);
    inv_sigmoid(t)
}

/// Encodes hue into the latent parameterization of a hue domain.
fn map_hue_to_latent(h: f64, hue_domain: HueDomain) -> f64 {
    match hue_domain {
        HueDomain::Any => wrap_hue(h),
        HueDomain::Arc { start, len } => {
            let len = len.max(1.0e-9);
            let d = wrap_hue(wrap_hue(h) - wrap_hue(start));
            let t = (d / len).clamp(1.0e-6, 1.0 - 1.0e-6);
            inv_sigmoid(t)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use rand::SeedableRng;
    use rand::rngs::ChaCha8Rng;

    use super::{
        SeedKind, build_seed_plan, generate_one_seed, generate_seeds_with_seed, seed_mix_counts,
    };
    use crate::{
        CapPolicy, GroupAxis, GroupMember, GroupQuantileTerm, GroupTarget, HueDomain, Interval,
        Oklch, PaletteProblem, SlotDomain, SlotSpec, Term, WeightedSample, WeightedTerm,
    };

    fn test_problem(seed_count: usize) -> PaletteProblem {
        let config = crate::SolveConfig {
            seed_count: NonZeroUsize::new(seed_count).expect("seed count is non-zero"),
            ..crate::SolveConfig::default()
        };

        PaletteProblem {
            slots: vec![
                SlotSpec {
                    name: "dark".to_string(),
                    domain: SlotDomain {
                        lightness: Interval { min: 0.2, max: 0.6 },
                        chroma: Interval { min: 0.0, max: 0.2 },
                        hue: HueDomain::Any,
                        cap_policy: CapPolicy::Ignore,
                        chroma_epsilon: 0.02,
                    },
                },
                SlotSpec {
                    name: "light".to_string(),
                    domain: SlotDomain {
                        lightness: Interval { min: 0.4, max: 0.9 },
                        chroma: Interval { min: 0.0, max: 0.2 },
                        hue: HueDomain::Any,
                        cap_policy: CapPolicy::Ignore,
                        chroma_epsilon: 0.02,
                    },
                },
            ],
            samples: vec![
                WeightedSample::new(
                    Oklch {
                        l: 0.35,
                        c: 0.08,
                        h: 0.4,
                    }
                    .to_oklab(),
                    1.0,
                    0.4,
                ),
                WeightedSample::new(
                    Oklch {
                        l: 0.75,
                        c: 0.12,
                        h: 2.2,
                    }
                    .to_oklab(),
                    2.0,
                    0.8,
                ),
            ],
            image_cap: None,
            terms: vec![WeightedTerm {
                weight: 1.0,
                name: Some("lightness-ladder".to_string()),
                term: Term::GroupQuantile(GroupQuantileTerm {
                    members: vec![
                        GroupMember { slot: 0, mass: 1.0 },
                        GroupMember { slot: 1, mass: 1.0 },
                    ],
                    axis: GroupAxis::Lightness,
                    target: GroupTarget::UniformRange { min: 0.3, max: 0.8 },
                    monotonic: None,
                    huber_delta: 0.02,
                }),
            }],
            config,
        }
    }

    #[test]
    fn seed_mix_counts_respects_total() {
        for n in 0..32 {
            let (a, b, c) = seed_mix_counts(n);
            assert_eq!(a + b + c, n);
        }
    }

    #[test]
    fn seed_mix_small_counts_are_intuitive() {
        assert_eq!(seed_mix_counts(0), (0, 0, 0));
        assert_eq!(seed_mix_counts(1), (1, 0, 0));
        assert_eq!(seed_mix_counts(2), (1, 1, 0));
        assert_eq!(seed_mix_counts(3), (1, 1, 1));
    }

    #[test]
    fn seed_plan_has_stable_kind_order_and_variants() {
        assert_eq!(
            build_seed_plan(8),
            vec![
                (SeedKind::Random, 0),
                (SeedKind::Random, 1),
                (SeedKind::Random, 2),
                (SeedKind::Random, 3),
                (SeedKind::GroupTargeted, 0),
                (SeedKind::GroupTargeted, 1),
                (SeedKind::SupportAware, 0),
                (SeedKind::SupportAware, 1),
            ]
        );
    }

    #[test]
    fn seed_bank_is_identical_for_same_solve_seed() {
        let problem = test_problem(8);
        let solve_seed = [17; 32];

        let first = generate_seeds_with_seed(&problem, solve_seed).unwrap();
        let second = generate_seeds_with_seed(&problem, solve_seed).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn seed_bank_changes_for_different_solve_seed() {
        let problem = test_problem(8);

        let first = generate_seeds_with_seed(&problem, [17; 32]).unwrap();
        let second = generate_seeds_with_seed(&problem, [18; 32]).unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn local_seed_streams_are_independent() {
        let problem = test_problem(8);
        let solve_seed = [23; 32];
        let plan = build_seed_plan(problem.config.seed_count.get());
        let bank = generate_seeds_with_seed(&problem, solve_seed).unwrap();

        for (seed_index, &(kind, variant)) in plan.iter().enumerate() {
            let mut rng = ChaCha8Rng::from_seed(solve_seed);
            rng.set_stream(seed_index as u64);
            let independently_generated = generate_one_seed(&problem, kind, variant, &mut rng);
            assert_eq!(bank[seed_index], independently_generated);
        }
    }
}
