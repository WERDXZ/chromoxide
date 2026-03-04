use std::collections::HashMap;

use chromoxide::{ImageCap, Oklch, PaletteError, WeightedSample};

use crate::solve_config::PartialSolveConfig;

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
    fn solve(
        &self,
        samples: Vec<WeightedSample>,
        image_cap: Option<ImageCap>,
        global_config: &PartialSolveConfig,
    ) -> Result<HashMap<String, Oklch>, SolveError>;
}
