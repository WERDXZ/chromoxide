use std::collections::HashMap;

use chromoxide::{solve, ImageCap, Oklch, PaletteProblem, SlotSpec, WeightedSample, WeightedTerm};

use super::export::BuiltinExport;
use crate::palette::{Palette, SolveError};
use crate::solve_config::PartialSolveConfig;

pub struct BuiltinPalette {
    id: &'static str,
    name: &'static str,
    slots: Vec<SlotSpec>,
    terms: Vec<WeightedTerm>,
    config: PartialSolveConfig,
    export: Box<dyn BuiltinExport>,
}

impl BuiltinPalette {
    pub fn new(
        id: &'static str,
        name: &'static str,
        slots: Vec<SlotSpec>,
        terms: Vec<WeightedTerm>,
        config: PartialSolveConfig,
        export: Box<dyn BuiltinExport>,
    ) -> Self {
        Self {
            id,
            name,
            slots,
            terms,
            config,
            export,
        }
    }

    fn build_problem(
        &self,
        samples: Vec<WeightedSample>,
        image_cap: Option<ImageCap>,
        global_config: &PartialSolveConfig,
    ) -> Result<PaletteProblem, super::super::user::BuildProblemError> {
        let solve_config = self.config.resolve_over(global_config)?;
        let problem = PaletteProblem {
            slots: self.slots.clone(),
            samples,
            image_cap,
            terms: self.terms.clone(),
            config: solve_config,
        };
        problem.validate()?;
        Ok(problem)
    }
}

impl Palette for BuiltinPalette {
    fn id(&self) -> String {
        self.id.to_string()
    }

    fn name(&self) -> String {
        self.name.to_string()
    }

    fn solve(
        &self,
        samples: Vec<WeightedSample>,
        image_cap: Option<ImageCap>,
        global_config: &PartialSolveConfig,
    ) -> Result<HashMap<String, Oklch>, SolveError> {
        let problem = self.build_problem(samples, image_cap, global_config)?;
        let solution = solve(&problem).map_err(SolveError::Solver)?;
        Ok(self.export.export(&self.slots, &solution.colors_lch))
    }
}
