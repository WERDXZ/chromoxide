//! Multi-start latent seed generation.

use rand::{Rng, RngExt};

use crate::color::Oklch;
use crate::domain::{HueDomain, SlotDomain};
use crate::error::PaletteError;
use crate::problem::PaletteProblem;
use crate::term::{GroupAxis, Term};
use crate::terms::group_quantile::{compute_mass_quantile_centers, compute_targets};
use crate::util::{arc_length, inv_sigmoid, wrap_hue};

/// Generates multi-start seeds using the caller-provided RNG.
pub fn generate_seeds(
    problem: &PaletteProblem,
    rng: &mut dyn Rng,
) -> Result<Vec<Vec<f64>>, PaletteError> {
    let n_slots = problem.slots.len();
    let dim = n_slots * 3;
    let seed_count = problem.config.seed_count.get();
    let mut seeds = Vec::with_capacity(seed_count);

    let (n_random, n_targeted, n_support) = seed_mix_counts(seed_count);

    for _ in 0..n_random {
        seeds.push(random_seed(problem, rng));
    }

    for i in 0..n_targeted {
        seeds.push(group_targeted_seed(problem, rng, i));
    }

    for i in 0..n_support {
        seeds.push(support_aware_seed(problem, rng, i));
    }

    while seeds.len() < seed_count {
        seeds.push(random_seed(problem, rng));
    }
    if seeds.len() > seed_count {
        seeds.truncate(seed_count);
    }

    if seeds.iter().any(|s| s.len() != dim) {
        return Err(PaletteError::InvalidProblem(
            "internal seed dimensionality mismatch".to_string(),
        ));
    }
    Ok(seeds)
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
    use super::seed_mix_counts;

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
}
