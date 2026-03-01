//! Multi-start L-BFGS solver.

use argmin::core::{CostFunction, Error as ArgminError, Executor, Gradient};
use argmin::solver::linesearch::MoreThuenteLineSearch;
use argmin::solver::quasinewton::LBFGS;
use argmin_math as _;
use rand::Rng;

use crate::diagnostics::{PaletteSolution, SeedDiagnostics, SlotDiagnostics, SolverDiagnostics};
use crate::error::PaletteError;
use crate::objective::ObjectiveEvaluator;
use crate::problem::PaletteProblem;
use crate::seed::generate_seeds;
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
/// For reproducible runs, call [`solve_with_rng`] with a seeded RNG.
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

    let evaluator = ObjectiveEvaluator::new(problem);
    let seeds = generate_seeds(problem, rng)?;

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

    let best = runs
        .iter()
        .min_by(|a, b| a.objective.total_cmp(&b.objective))
        .expect("non-empty runs");

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
