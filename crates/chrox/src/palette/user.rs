//! User-defined palette file model.
//!
//! ```toml
//! id = "my-palette"
//! name = "my-palette"
//!
//! [[slots]]
//! name = "bg"
//! domain = { lightness = { min = 0.10, max = 0.25 }, chroma = { min = 0.00, max = 0.06 }, hue = "Any", cap_policy = "Ignore", chroma_epsilon = 0.02 }
//!
//! [[terms]]
//! weight = 4.0
//! name = "cover"
//! term = { Cover = { slots = [0, 1], tau = 0.02, delta = 0.03 } }
//!
//! [config]
//! seed_count = 32
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chromoxide::{
    ImageCap, Oklch, PaletteError, PaletteProblem, SlotSpec, WeightedSample, WeightedTerm,
};
use serde::{Deserialize, Serialize};

use super::{solve_problem, Palette, SolveError};
use crate::solve_config::{Error as SolveConfigError, PartialSolveConfig};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PaletteFile {
    /// Stable palette id (slug). If omitted, derived from `name`.
    #[serde(default)]
    pub id: Option<String>,
    /// Human-facing palette name.
    pub name: String,
    #[serde(default)]
    pub slots: Vec<SlotSpec>,
    #[serde(default)]
    pub terms: Vec<WeightedTerm>,
    #[serde(default)]
    pub config: PartialSolveConfig,
}

impl FromStr for PaletteFile {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        toml::from_str(input).map_err(|source| Error::Parse { source })
    }
}

impl PaletteFile {
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();
        let input = std::fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;

        toml::from_str(&input).map_err(|source| Error::ParseAtPath { path, source })
    }

    /// Stable identifier derived from palette file name.
    pub fn id_from_path(path: &Path) -> String {
        let stem = path
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("palette");
        slugify(stem)
    }

    /// Resolve concrete solve config.
    ///
    /// Precedence is palette -> config -> chromoxide defaults.
    pub fn resolve_config(
        &self,
        global_config: &PartialSolveConfig,
    ) -> Result<chromoxide::SolveConfig, SolveConfigError> {
        self.config.resolve_over(global_config)
    }

    /// Build a problem once image samples are available.
    pub fn build_problem(
        self,
        samples: Vec<WeightedSample>,
        global_config: &PartialSolveConfig,
    ) -> Result<PaletteProblem, BuildProblemError> {
        self.build_problem_with_cap(samples, None, global_config)
    }

    /// Build a validated problem once image samples and optional c_cap are available.
    pub fn build_problem_with_cap(
        self,
        samples: Vec<WeightedSample>,
        image_cap: Option<ImageCap>,
        global_config: &PartialSolveConfig,
    ) -> Result<PaletteProblem, BuildProblemError> {
        let solve_config = self.config.resolve_over(global_config)?;
        let problem = PaletteProblem {
            slots: self.slots,
            samples,
            image_cap,
            terms: self.terms,
            config: solve_config,
        };
        problem.validate()?;
        Ok(problem)
    }
}

impl Palette for PaletteFile {
    fn id(&self) -> String {
        match self.id.as_deref() {
            Some(id) => slugify(id),
            None => slugify(&self.name),
        }
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn solve(
        &self,
        samples: Vec<WeightedSample>,
        image_cap: Option<ImageCap>,
        global_config: &PartialSolveConfig,
    ) -> Result<HashMap<String, Oklch>, SolveError> {
        let problem = self
            .clone()
            .build_problem_with_cap(samples, image_cap, global_config)?;
        solve_problem(&problem)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BuildProblemError {
    #[error("invalid solve config")]
    SolveConfig(#[from] SolveConfigError),
    #[error("invalid palette problem")]
    Problem(#[from] PaletteError),
}

fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_dash = false;

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }

    let out = out.trim_matches('-');
    if out.is_empty() {
        "palette".to_string()
    } else {
        out.to_string()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read palette file `{path}`")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse palette")]
    Parse {
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to parse palette file `{path}`")]
    ParseAtPath {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::str::FromStr;

    use chromoxide::{ImageCapBuilder, Oklch, WeightedSample};

    use super::PaletteFile;
    use crate::palette::{Palette, SolveError};
    use crate::solve_config::PartialSolveConfig;

    fn one_sample() -> Vec<WeightedSample> {
        vec![WeightedSample::new(
            Oklch {
                l: 0.5,
                c: 0.12,
                h: 1.0,
            }
            .to_oklab(),
            1.0,
            0.5,
        )]
    }

    #[test]
    fn parse_palette_entries() {
        let palette = PaletteFile::from_str(
            r#"
id = "custom-id"
name = "my-palette"

[[slots]]
name = "bg"
domain = { lightness = { min = 0.10, max = 0.25 }, chroma = { min = 0.00, max = 0.06 }, hue = "Any", cap_policy = "Ignore", chroma_epsilon = 0.02 }

[[slots]]
name = "fg"
domain = { lightness = { min = 0.70, max = 0.98 }, chroma = { min = 0.00, max = 0.08 }, hue = "Any", cap_policy = "Ignore", chroma_epsilon = 0.02 }

[[terms]]
weight = 4.0
name = "cover"
term = { Cover = { slots = [0, 1], tau = 0.02, delta = 0.03 } }

[config]
seed_count = 28
"#,
        )
        .expect("palette should parse");

        assert_eq!(palette.id.as_deref(), Some("custom-id"));
        assert_eq!(palette.name, "my-palette");
        assert_eq!(palette.slots.len(), 2);
        assert_eq!(palette.terms.len(), 1);
        assert_eq!(palette.terms[0].term.default_name(), "cover");
        assert_eq!(palette.config.seed_count, Some(28));
    }

    #[test]
    fn trait_id_uses_slugified_name_when_id_absent() {
        let palette = PaletteFile::from_str(
            r#"
name = "My Palette v1"
"#,
        )
        .expect("palette should parse");

        assert_eq!(palette.id(), "my-palette-v1");
    }

    #[test]
    fn trait_id_prefers_explicit_id() {
        let palette = PaletteFile::from_str(
            r#"
id = " Custom Palette@ID "
name = "My Palette"
"#,
        )
        .expect("palette should parse");

        assert_eq!(palette.id(), "custom-palette-id");
    }

    #[test]
    fn slugifies_id_from_filename() {
        assert_eq!(
            PaletteFile::id_from_path(Path::new("/tmp/My Palette@v1.toml")),
            "my-palette-v1"
        );
        assert_eq!(
            PaletteFile::id_from_path(Path::new("/tmp/---.toml")),
            "palette"
        );
    }

    #[test]
    fn builds_problem_after_sampling() {
        let palette = PaletteFile::from_str(
            r#"
name = "demo"

[[slots]]
name = "bg"
domain = { lightness = { min = 0.10, max = 0.25 }, chroma = { min = 0.00, max = 0.06 }, hue = "Any", cap_policy = "Ignore", chroma_epsilon = 0.02 }

[[terms]]
weight = 1.0
name = "cover"
term = { Cover = { slots = [0], tau = 0.02, delta = 0.03 } }
"#,
        )
        .expect("palette should parse");

        let problem = palette
            .build_problem(one_sample(), &PartialSolveConfig::default())
            .expect("problem should build");
        assert_eq!(problem.slots.len(), 1);
        assert_eq!(problem.terms.len(), 1);
        assert_eq!(problem.samples.len(), 1);
    }

    #[test]
    fn builds_problem_with_cap() {
        let palette = PaletteFile::from_str(
            r#"
name = "demo"

[[slots]]
name = "bg"
domain = { lightness = { min = 0.10, max = 0.25 }, chroma = { min = 0.00, max = 0.06 }, hue = "Any", cap_policy = "Ignore", chroma_epsilon = 0.02 }
"#,
        )
        .expect("palette should parse");

        let cap_samples = one_sample();
        let image_cap = ImageCapBuilder::default()
            .build(&cap_samples)
            .expect("image cap should build");

        let problem = palette
            .build_problem_with_cap(
                one_sample(),
                Some(image_cap),
                &PartialSolveConfig::default(),
            )
            .expect("problem should build");
        assert!(problem.image_cap.is_some());
    }

    #[test]
    fn config_precedence_palette_then_global_then_default() {
        let palette = PaletteFile::from_str(
            r#"
name = "demo"

[[slots]]
name = "bg"
domain = { lightness = { min = 0.10, max = 0.25 }, chroma = { min = 0.00, max = 0.06 }, hue = "Any", cap_policy = "Ignore", chroma_epsilon = 0.02 }

[config]
seed_count = 30
"#,
        )
        .expect("palette should parse");

        let global = PartialSolveConfig {
            seed_count: Some(12),
            keep_top_k: Some(4),
            ..Default::default()
        };

        let resolved = palette
            .resolve_config(&global)
            .expect("config should resolve");
        assert_eq!(resolved.seed_count.get(), 30);
        assert_eq!(resolved.keep_top_k.get(), 4);
    }

    #[test]
    fn trait_solve_propagates_build_problem_errors() {
        let palette = PaletteFile::from_str(
            r#"
name = "demo"

[[slots]]
name = "bg"
domain = { lightness = { min = 0.10, max = 0.25 }, chroma = { min = 0.00, max = 0.06 }, hue = "Any", cap_policy = "Ignore", chroma_epsilon = 0.02 }

[[terms]]
weight = 1.0
name = "cover"
term = { Cover = { slots = [0], tau = 0.02, delta = 0.03 } }
"#,
        )
        .expect("palette should parse");

        let err = palette
            .solve(Vec::new(), None, &PartialSolveConfig::default())
            .expect_err("empty samples should fail");
        assert!(matches!(err, SolveError::BuildProblem(_)));
    }
}
