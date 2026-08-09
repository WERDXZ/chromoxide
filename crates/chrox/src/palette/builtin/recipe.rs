use std::collections::HashMap;

use chromoxide::{
    ImageCap, Oklch, PaletteProblem, SlotSpec, WeightedSample, WeightedTerm, solve,
    solve_with_seed as core_solve_with_seed,
};

use super::export::BuiltinExport;
use crate::palette::{Palette, SolveError};
use crate::solve_config::PartialSolveConfig;

type DynamicTermBuilder = fn(&[WeightedSample], &[SlotSpec]) -> Vec<WeightedTerm>;

pub struct BuiltinPalette {
    id: &'static str,
    name: &'static str,
    slots: Vec<SlotSpec>,
    terms: Vec<WeightedTerm>,
    dynamic_terms: Option<DynamicTermBuilder>,
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
        Self::new_with_dynamic_terms(id, name, slots, terms, None, config, export)
    }

    pub fn new_with_dynamic_terms(
        id: &'static str,
        name: &'static str,
        slots: Vec<SlotSpec>,
        terms: Vec<WeightedTerm>,
        dynamic_terms: Option<DynamicTermBuilder>,
        config: PartialSolveConfig,
        export: Box<dyn BuiltinExport>,
    ) -> Self {
        Self {
            id,
            name,
            slots,
            terms,
            dynamic_terms,
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
        let mut terms = self.terms.clone();
        if let Some(builder) = self.dynamic_terms {
            terms.extend(builder(&samples, &self.slots));
        }
        let problem = PaletteProblem {
            slots: self.slots.clone(),
            samples,
            image_cap,
            terms,
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

    fn members(&self) -> Vec<String> {
        self.export.members(&self.slots)
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

    fn solve_with_seed(
        &self,
        samples: Vec<WeightedSample>,
        image_cap: Option<ImageCap>,
        global_config: &PartialSolveConfig,
        seed: chromoxide::SolveSeed,
    ) -> Result<HashMap<String, Oklch>, SolveError> {
        let problem = self.build_problem(samples, image_cap, global_config)?;
        let solution = core_solve_with_seed(&problem, seed).map_err(SolveError::Solver)?;
        Ok(self.export.export(&self.slots, &solution.colors_lch))
    }
}
