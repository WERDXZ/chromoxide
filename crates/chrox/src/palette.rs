use std::collections::HashMap;

use chromoxide::{
    ImageCap, Oklch, PaletteError, PaletteProblem, PaletteSolution, SolveSeed, WeightedSample,
    solve, solve_with_seed as core_solve_with_seed,
};

use crate::solve_config::PartialSolveConfig;

pub mod builtin;
pub mod registry;
pub mod user;

#[derive(Debug, thiserror::Error)]
pub enum SolveError {
    #[error("failed to build palette problem")]
    BuildProblem(#[from] user::BuildProblemError),
    #[error("failed to solve palette problem")]
    Solver(#[source] PaletteError),
}

pub trait Palette {
    fn id(&self) -> String;
    fn name(&self) -> String;
    fn members(&self) -> Vec<String>;
    fn solve(
        &self,
        samples: Vec<WeightedSample>,
        image_cap: Option<ImageCap>,
        global_config: &PartialSolveConfig,
    ) -> Result<HashMap<String, Oklch>, SolveError>;

    fn solve_with_seed(
        &self,
        samples: Vec<WeightedSample>,
        image_cap: Option<ImageCap>,
        global_config: &PartialSolveConfig,
        seed: SolveSeed,
    ) -> Result<HashMap<String, Oklch>, SolveError>;
}

fn solve_problem(problem: &PaletteProblem) -> Result<HashMap<String, Oklch>, SolveError> {
    let solution = solve(problem).map_err(SolveError::Solver)?;
    Ok(solution_to_map(&solution))
}

fn solve_problem_with_seed(
    problem: &PaletteProblem,
    seed: SolveSeed,
) -> Result<HashMap<String, Oklch>, SolveError> {
    let solution = core_solve_with_seed(problem, seed).map_err(SolveError::Solver)?;
    Ok(solution_to_map(&solution))
}

fn solution_to_map(solution: &PaletteSolution) -> HashMap<String, Oklch> {
    let mut out = HashMap::with_capacity(solution.slot_diagnostics.len());
    for (slot, lch) in solution
        .slot_diagnostics
        .iter()
        .zip(solution.colors_lch.iter().copied())
    {
        out.insert(slot.name.clone(), lch);
    }

    out
}
