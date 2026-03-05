use std::collections::HashMap;

use chromoxide::{ImageCap, Oklch, PaletteError, PaletteProblem, WeightedSample, solve};

use crate::solve_config::PartialSolveConfig;

pub mod builtin;
pub mod user;
pub mod registry;

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
    fn solve(
        &self,
        samples: Vec<WeightedSample>,
        image_cap: Option<ImageCap>,
        global_config: &PartialSolveConfig,
    ) -> Result<HashMap<String, Oklch>, SolveError>;
}

fn solve_problem(problem: &PaletteProblem) -> Result<HashMap<String, Oklch>, SolveError> {
    let solution = solve(problem).map_err(SolveError::Solver)?;

    let mut out = HashMap::with_capacity(solution.slot_diagnostics.len());
    for (slot, lch) in solution
        .slot_diagnostics
        .iter()
        .zip(solution.colors_lch.iter().copied())
    {
        out.insert(slot.name.clone(), lch);
    }

    Ok(out)
}
