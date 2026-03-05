use std::collections::HashMap;

use chromoxide::{
    CapPolicy, ChromaTargetTerm, CoverTerm, HueDomain, ImageCap, Interval, Oklch, PairDistanceTerm,
    PaletteProblem, SaliencyTarget, SaliencyTerm, ScalarTarget, SlotDomain, SlotSpec, SupportTerm,
    Term, WeightedSample, WeightedTerm,
};

use super::{solve_problem, Palette, SolveError};
use crate::solve_config::PartialSolveConfig;

#[derive(Debug, Clone)]
pub struct BuiltinPaletteDef {
    pub id: &'static str,
    pub name: &'static str,
    pub slots: Vec<SlotSpec>,
    pub terms: Vec<WeightedTerm>,
    pub config: PartialSolveConfig,
}

#[derive(Debug, Clone)]
pub struct BuiltinPalette {
    def: BuiltinPaletteDef,
}

impl BuiltinPalette {
    pub fn new(def: BuiltinPaletteDef) -> Self {
        Self { def }
    }

    fn build_problem(
        &self,
        samples: Vec<WeightedSample>,
        image_cap: Option<ImageCap>,
        global_config: &PartialSolveConfig,
    ) -> Result<PaletteProblem, super::user::BuildProblemError> {
        let solve_config = self.def.config.resolve_over(global_config)?;
        let problem = PaletteProblem {
            slots: self.def.slots.clone(),
            samples,
            image_cap,
            terms: self.def.terms.clone(),
            config: solve_config,
        };
        problem.validate()?;
        Ok(problem)
    }
}

impl Palette for BuiltinPalette {
    fn id(&self) -> String {
        self.def.id.to_string()
    }

    fn name(&self) -> String {
        self.def.name.to_string()
    }

    fn solve(
        &self,
        samples: Vec<WeightedSample>,
        image_cap: Option<ImageCap>,
        global_config: &PartialSolveConfig,
    ) -> Result<HashMap<String, Oklch>, SolveError> {
        let problem = self.build_problem(samples, image_cap, global_config)?;
        solve_problem(&problem)
    }
}

pub fn cover_salient() -> Box<dyn Palette> {
    Box::new(BuiltinPalette::new(cover_salient_def()))
}

pub fn cover_salient_def() -> BuiltinPaletteDef {
    BuiltinPaletteDef {
        id: "cover-salient",
        name: "Cover + Salient",
        slots: vec![cover_slot(), salient_slot()],
        terms: vec![
            WeightedTerm {
                weight: 5.0,
                name: Some("cover-fit".into()),
                term: Term::Cover(CoverTerm {
                    slots: vec![0],
                    tau: 0.02,
                    delta: 0.03,
                }),
            },
            WeightedTerm {
                weight: 2.0,
                name: Some("cover-support".into()),
                term: Term::Support(SupportTerm {
                    slots: vec![0],
                    tau: 0.02,
                    beta: 0.20,
                    epsilon: 1.0e-4,
                }),
            },
            WeightedTerm {
                weight: 2.5,
                name: Some("salient-support".into()),
                term: Term::Support(SupportTerm {
                    slots: vec![1],
                    tau: 0.02,
                    beta: 0.20,
                    epsilon: 1.0e-4,
                }),
            },
            WeightedTerm {
                weight: 4.0,
                name: Some("salient-saliency".into()),
                term: Term::Saliency(SaliencyTerm {
                    slot: 1,
                    sigma: 0.10,
                    target: SaliencyTarget::Min(0.65),
                    hinge_delta: Some(0.05),
                }),
            },
            WeightedTerm {
                weight: 2.0,
                name: Some("cover-low-chroma".into()),
                term: Term::ChromaTarget(ChromaTargetTerm {
                    slot: 0,
                    target: ScalarTarget::Max(0.05),
                    hinge_delta: Some(0.02),
                }),
            },
            WeightedTerm {
                weight: 3.0,
                name: Some("salient-high-chroma".into()),
                term: Term::ChromaTarget(ChromaTargetTerm {
                    slot: 1,
                    target: ScalarTarget::Min(0.10),
                    hinge_delta: Some(0.03),
                }),
            },
            WeightedTerm {
                weight: 2.5,
                name: Some("cover-salient-separation".into()),
                term: Term::Distance(PairDistanceTerm {
                    a: 0,
                    b: 1,
                    target: ScalarTarget::Min(0.10),
                    squared: false,
                    hinge_delta: Some(0.03),
                }),
            },
        ],
        config: PartialSolveConfig {
            seed_count: Some(24),
            keep_top_k: Some(6),
            ..Default::default()
        },
    }
}

fn cover_slot() -> SlotSpec {
    SlotSpec {
        name: "cover".into(),
        domain: SlotDomain {
            lightness: Interval {
                min: 0.18,
                max: 0.82,
            },
            chroma: Interval {
                min: 0.00,
                max: 0.10,
            },
            hue: HueDomain::Any,
            cap_policy: CapPolicy::Ignore,
            chroma_epsilon: 0.02,
        },
    }
}

fn salient_slot() -> SlotSpec {
    SlotSpec {
        name: "salient".into(),
        domain: SlotDomain {
            lightness: Interval {
                min: 0.25,
                max: 0.85,
            },
            chroma: Interval {
                min: 0.06,
                max: 0.22,
            },
            hue: HueDomain::Any,
            cap_policy: CapPolicy::Ignore,
            chroma_epsilon: 0.02,
        },
    }
}

#[cfg(test)]
mod tests {
    use chromoxide::Oklch;

    use super::cover_salient;
    use crate::solve_config::PartialSolveConfig;

    fn clustered_samples() -> Vec<chromoxide::WeightedSample> {
        vec![
            chromoxide::WeightedSample::new(
                Oklch {
                    l: 0.42,
                    c: 0.03,
                    h: 0.2,
                }
                .to_oklab(),
                4.0,
                0.15,
            ),
            chromoxide::WeightedSample::new(
                Oklch {
                    l: 0.40,
                    c: 0.04,
                    h: 0.3,
                }
                .to_oklab(),
                3.5,
                0.20,
            ),
            chromoxide::WeightedSample::new(
                Oklch {
                    l: 0.66,
                    c: 0.18,
                    h: 0.9,
                }
                .to_oklab(),
                1.0,
                0.95,
            ),
        ]
    }

    #[test]
    fn cover_salient_solves_and_exports_named_members() {
        let palette = cover_salient();
        let colors = palette
            .solve(clustered_samples(), None, &PartialSolveConfig::default())
            .expect("builtin palette should solve");

        assert_eq!(palette.id(), "cover-salient");
        assert!(colors.contains_key("cover"));
        assert!(colors.contains_key("salient"));

        let cover = colors["cover"];
        let salient = colors["salient"];
        assert!(cover.c <= 0.11);
        assert!(salient.c >= 0.08);
    }
}
