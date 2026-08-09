//! Multi-start L-BFGS solver.

use std::cmp::Ordering;

use argmin::core::{CostFunction, Error as ArgminError, Executor, Gradient};
use argmin::solver::linesearch::MoreThuenteLineSearch;
use argmin::solver::quasinewton::LBFGS;
use argmin_math as _;
use rand::Rng;

use crate::diagnostics::{PaletteSolution, SeedDiagnostics, SlotDiagnostics, SolverDiagnostics};
use crate::error::PaletteError;
use crate::objective::ObjectiveEvaluator;
use crate::problem::PaletteProblem;
use crate::seed::{SolveSeed, generate_seeds, generate_seeds_with_seed};
use crate::terms::saliency::estimate_saliency_at;
use crate::util::l2_norm;

struct ArgminAdapter<'a> {
    evaluator: ObjectiveEvaluator<'a>,
    fd_epsilon: f64,
}

impl CostFunction for ArgminAdapter<'_> {
    type Param = Vec<f64>;
    type Output = f64;

    fn cost(&self, param: &Self::Param) -> Result<Self::Output, ArgminError> {
        self.evaluator
            .evaluate_total(param)
            .map_err(|e| ArgminError::msg(e.to_string()))
    }
}

impl Gradient for ArgminAdapter<'_> {
    type Param = Vec<f64>;
    type Gradient = Vec<f64>;

    fn gradient(&self, param: &Self::Param) -> Result<Self::Gradient, ArgminError> {
        self.evaluator
            .finite_difference_gradient(param, self.fd_epsilon)
            .map_err(|e| ArgminError::msg(e.to_string()))
    }
}

#[derive(Clone, Debug)]
struct SeedRun {
    seed_index: usize,
    param: Vec<f64>,
    objective: f64,
    converged: bool,
    iterations: u64,
    grad_norm: Option<f64>,
}

/// Solves a palette optimization problem.
///
/// This convenience entrypoint uses a thread-local RNG.
/// Use [`solve_with_rng`] for caller-controlled randomness or [`solve_with_seed`]
/// for the fixed-width deterministic API.
pub fn solve(problem: &PaletteProblem) -> Result<PaletteSolution, PaletteError> {
    let mut rng = rand::rng();
    solve_with_rng(problem, &mut rng)
}

/// Solves a palette optimization problem with an explicit RNG.
///
/// Execution flow:
/// 1. Validate inputs (`PaletteProblem::validate`)
/// 2. Generate multi-start seeds
/// 3. Run L-BFGS from each seed using finite-difference gradients
/// 4. Select the best objective and emit detailed diagnostics
pub fn solve_with_rng(
    problem: &PaletteProblem,
    rng: &mut dyn Rng,
) -> Result<PaletteSolution, PaletteError> {
    problem.validate()?;
    let seeds = generate_seeds(problem, rng)?;
    solve_from_seeds(problem, seeds)
}

/// Solves a palette optimization problem with a fixed deterministic seed.
///
/// Each local start uses a separate ChaCha stream, so random consumption by
/// one start cannot affect any other start.
pub fn solve_with_seed(
    problem: &PaletteProblem,
    seed: SolveSeed,
) -> Result<PaletteSolution, PaletteError> {
    problem.validate()?;
    let seeds = generate_seeds_with_seed(problem, seed)?;
    solve_from_seeds(problem, seeds)
}

fn solve_from_seeds(
    problem: &PaletteProblem,
    seeds: Vec<Vec<f64>>,
) -> Result<PaletteSolution, PaletteError> {
    let evaluator = ObjectiveEvaluator::new(problem)?;

    let mut runs = Vec::with_capacity(seeds.len());
    for (seed_index, seed) in seeds.iter().enumerate() {
        match run_seed(&evaluator, problem, seed_index, seed.clone()) {
            Ok(run) => runs.push(run),
            Err(_) => {
                if let Ok(objective) = evaluator.evaluate_total(seed) {
                    runs.push(SeedRun {
                        seed_index,
                        param: seed.clone(),
                        objective,
                        converged: false,
                        iterations: 0,
                        grad_norm: None,
                    });
                }
            }
        }
    }

    if runs.is_empty() {
        return Err(PaletteError::SolverFailure(
            "all seed runs failed".to_string(),
        ));
    }

    runs.sort_by(compare_seed_runs);
    let best = runs.first().expect("non-empty runs");

    let (objective, term_breakdown, decoded) = evaluator.evaluate_breakdown(&best.param)?;

    let mut slot_diagnostics = Vec::with_capacity(problem.slots.len());
    for (i, slot) in problem.slots.iter().enumerate() {
        let cap_margin = decoded.slots[i]
            .cap_at_lh
            .map(|cap| cap - decoded.slots[i].lch.c);
        let near_cap = cap_margin.is_some_and(|m| m <= 0.01);

        let estimated_saliency = if decoded.estimated_saliency.len() == problem.slots.len() {
            decoded.estimated_saliency[i]
        } else {
            estimate_saliency_at(decoded.slots[i].lab, &problem.samples, 0.08)
        };

        slot_diagnostics.push(SlotDiagnostics {
            name: slot.name.clone(),
            final_lab: decoded.slots[i].lab,
            final_lch: decoded.slots[i].lch,
            relative_luminance: decoded.luminance[i],
            estimated_saliency,
            near_chroma_cap: near_cap,
            cap_margin,
        });
    }

    let seed_runs = runs
        .iter()
        .take(problem.config.keep_top_k.get())
        .map(|r| SeedDiagnostics {
            seed_index: r.seed_index,
            objective: r.objective,
            converged: r.converged,
            iterations: r.iterations,
        })
        .collect::<Vec<_>>();

    let solver_diagnostics = SolverDiagnostics {
        seed_count: seeds.len(),
        best_seed_index: best.seed_index,
        converged: best.converged,
        iterations: best.iterations,
        final_gradient_norm: best.grad_norm,
        seed_runs,
    };

    Ok(PaletteSolution {
        colors: decoded.slots.iter().map(|s| s.lab).collect(),
        colors_lch: decoded.slots.iter().map(|s| s.lch).collect(),
        objective,
        seed_index: best.seed_index,
        converged: best.converged,
        term_breakdown,
        slot_diagnostics,
        solver_diagnostics,
    })
}

fn compare_seed_runs(a: &SeedRun, b: &SeedRun) -> Ordering {
    a.objective
        .total_cmp(&b.objective)
        .then_with(|| a.seed_index.cmp(&b.seed_index))
}

/// Runs one local L-BFGS solve from a single starting seed.
fn run_seed(
    evaluator: &ObjectiveEvaluator<'_>,
    problem: &PaletteProblem,
    seed_index: usize,
    seed: Vec<f64>,
) -> Result<SeedRun, PaletteError> {
    let adapter = ArgminAdapter {
        evaluator: evaluator.clone(),
        fd_epsilon: problem.config.fd_epsilon,
    };

    let linesearch = MoreThuenteLineSearch::new();
    let solver = LBFGS::new(linesearch, 8)
        .with_tolerance_grad(problem.config.convergence_gtol)
        .map_err(|e| PaletteError::SolverFailure(format!("lbfgs grad tolerance: {e}")))?
        .with_tolerance_cost(problem.config.convergence_ftol)
        .map_err(|e| PaletteError::SolverFailure(format!("lbfgs cost tolerance: {e}")))?;

    let result = Executor::new(adapter, solver)
        .configure(|state| state.param(seed).max_iters(problem.config.max_iters.get()))
        .run()
        .map_err(|e| PaletteError::SolverFailure(format!("argmin run failed: {e}")))?;

    let state = result.state;
    let param = state
        .best_param
        .clone()
        .or(state.param.clone())
        .ok_or_else(|| {
            PaletteError::SolverFailure("solver did not return parameter vector".to_string())
        })?;

    let objective = if state.best_cost.is_finite() {
        state.best_cost
    } else {
        evaluator.evaluate_total(&param)?
    };
    let grad_norm = state.grad.as_ref().map(|g| l2_norm(g));

    let converged = state.termination_status.terminated() && state.iter < state.max_iters;

    Ok(SeedRun {
        seed_index,
        param,
        objective,
        converged,
        iterations: state.iter,
        grad_norm,
    })
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::num::{NonZeroU64, NonZeroUsize};

    use super::{SeedRun, compare_seed_runs, solve_with_seed};
    use crate::{
        CapPolicy, ChromaTargetTerm, HueDomain, Interval, Oklch, PaletteProblem, ScalarTarget,
        SlotDomain, SlotSpec, Term, WeightedSample, WeightedTerm,
    };

    fn seed_run(seed_index: usize, objective: f64) -> SeedRun {
        SeedRun {
            seed_index,
            param: vec![0.0; 3],
            objective,
            converged: true,
            iterations: 1,
            grad_norm: Some(0.0),
        }
    }

    fn repeatable_problem() -> PaletteProblem {
        let config = crate::SolveConfig {
            seed_count: NonZeroUsize::new(4).expect("seed count is non-zero"),
            keep_top_k: NonZeroUsize::new(2).expect("diagnostic count is non-zero"),
            max_iters: NonZeroU64::new(12).expect("iteration count is non-zero"),
            ..crate::SolveConfig::default()
        };

        PaletteProblem {
            slots: vec![SlotSpec {
                name: "accent".to_string(),
                domain: SlotDomain {
                    lightness: Interval { min: 0.3, max: 0.8 },
                    chroma: Interval { min: 0.0, max: 0.2 },
                    hue: HueDomain::Any,
                    cap_policy: CapPolicy::Ignore,
                    chroma_epsilon: 0.02,
                },
            }],
            samples: vec![WeightedSample::new(
                Oklch {
                    l: 0.55,
                    c: 0.1,
                    h: 1.4,
                }
                .to_oklab(),
                1.0,
                0.5,
            )],
            image_cap: None,
            terms: vec![WeightedTerm {
                weight: 1.0,
                name: Some("accent-chroma".to_string()),
                term: Term::ChromaTarget(ChromaTargetTerm {
                    slot: 0,
                    target: ScalarTarget::Target {
                        value: 0.1,
                        delta: 0.02,
                    },
                    hinge_delta: None,
                }),
            }],
            config,
        }
    }

    fn assert_solutions_exact(first: &crate::PaletteSolution, second: &crate::PaletteSolution) {
        assert_eq!(first.colors, second.colors);
        assert_eq!(first.colors_lch, second.colors_lch);
        assert_eq!(first.objective, second.objective);
        assert_eq!(first.seed_index, second.seed_index);
        assert_eq!(first.converged, second.converged);

        assert_eq!(first.term_breakdown.len(), second.term_breakdown.len());
        for (a, b) in first.term_breakdown.iter().zip(&second.term_breakdown) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.raw, b.raw);
            assert_eq!(a.weighted, b.weighted);
            assert_eq!(a.components, b.components);
        }

        assert_eq!(first.slot_diagnostics.len(), second.slot_diagnostics.len());
        for (a, b) in first.slot_diagnostics.iter().zip(&second.slot_diagnostics) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.final_lab, b.final_lab);
            assert_eq!(a.final_lch, b.final_lch);
            assert_eq!(a.relative_luminance, b.relative_luminance);
            assert_eq!(a.estimated_saliency, b.estimated_saliency);
            assert_eq!(a.near_chroma_cap, b.near_chroma_cap);
            assert_eq!(a.cap_margin, b.cap_margin);
        }

        let a = &first.solver_diagnostics;
        let b = &second.solver_diagnostics;
        assert_eq!(a.seed_count, b.seed_count);
        assert_eq!(a.best_seed_index, b.best_seed_index);
        assert_eq!(a.converged, b.converged);
        assert_eq!(a.iterations, b.iterations);
        assert_eq!(a.final_gradient_norm, b.final_gradient_norm);
        assert_eq!(a.seed_runs.len(), b.seed_runs.len());
        for (a, b) in a.seed_runs.iter().zip(&b.seed_runs) {
            assert_eq!(a.seed_index, b.seed_index);
            assert_eq!(a.objective, b.objective);
            assert_eq!(a.converged, b.converged);
            assert_eq!(a.iterations, b.iterations);
        }
    }

    #[test]
    fn solve_with_seed_is_exactly_repeatable() {
        let problem = repeatable_problem();
        let solve_seed = [31; 32];

        let first = solve_with_seed(&problem, solve_seed).unwrap();
        let second = solve_with_seed(&problem, solve_seed).unwrap();

        assert_solutions_exact(&first, &second);
        assert_eq!(first.seed_index, first.solver_diagnostics.best_seed_index);
        assert_eq!(
            first.solver_diagnostics.seed_runs.len(),
            problem.config.keep_top_k.get()
        );
        assert!(first.solver_diagnostics.seed_runs.windows(2).all(|pair| {
            pair[0]
                .objective
                .total_cmp(&pair[1].objective)
                .then_with(|| pair[0].seed_index.cmp(&pair[1].seed_index))
                != Ordering::Greater
        }));
    }

    #[test]
    fn equal_objective_prefers_lower_seed_index() {
        let lower_index = seed_run(2, 1.5);
        let higher_index = seed_run(7, 1.5);

        assert_eq!(
            compare_seed_runs(&lower_index, &higher_index),
            Ordering::Less
        );
        assert_eq!(
            compare_seed_runs(&higher_index, &lower_index),
            Ordering::Greater
        );
    }
}
