//! Configuration model and loader.
//!
//! ```toml
//! [general]
//! palettes = ["palettes", "/opt/chrox/palettes"]
//!
//! [[templates]]
//! name = "alacritty"
//! input = "templates/alacritty.toml"
//! output = ".config/alacritty/colors.toml"
//!
//! [config]
//! seed_count = 24
//! ```

use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::solve_config::PartialSolveConfig;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub templates: Vec<TemplateEntry>,
    #[serde(default, alias = "solve")]
    pub config: PartialSolveConfig,
}

impl FromStr for Config {
    type Err = Error;
    fn from_str(input: &str) -> Result<Self, Error> {
        toml::from_str(input).map_err(|source| Error::Parse { source })
    }
}

impl Config {
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("chrox")
            .join("config.toml")
    }

    pub fn load(path: Option<PathBuf>) -> Result<Self, Error> {
        match path {
            Some(path) => Self::from_path(path),
            None => {
                let path = Self::default_path();
                if path.exists() {
                    Self::from_path(path)
                } else {
                    Ok(Self::default())
                }
            }
        }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();
        let input = std::fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;

        toml::from_str(&input).map_err(|source| Error::ParseAtPath { path, source })
    }

    pub fn find_template(&self, name: &str) -> Option<&TemplateEntry> {
        self.templates.iter().find(|entry| entry.name == name)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GeneralConfig {
    #[serde(default)]
    pub palettes: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TemplateEntry {
    pub name: String,
    pub input: PathBuf,
    pub output: PathBuf,
}

impl TemplateEntry {
    pub fn resolve_input(&self, base_dir: &Path) -> PathBuf {
        resolve_path(base_dir, &self.input)
    }

    pub fn resolve_output(&self, base_dir: &Path) -> PathBuf {
        resolve_path(base_dir, &self.output)
    }
}

fn resolve_path(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read config file `{path}`")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config")]
    Parse {
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to parse config file `{path}`")]
    ParseAtPath {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::str::FromStr;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::Config;

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("chrox-config-test-{nanos}-{}", std::process::id()))
    }

    #[test]
    fn parse_general_and_template_entries() {
        let config = Config::from_str(
            r#"
[general]
palettes = ["palettes", "/opt/chrox/palettes"]

[[templates]]
name = "alacritty"
input = "templates/alacritty.toml"
output = ".config/alacritty/colors.toml"

[[templates]]
name = "hypr"
input = "templates/hyprland.conf"
output = ".config/hypr/colors.conf"

[config]
seed_count = 20
"#,
        )
        .expect("config should parse");

        assert_eq!(config.general.palettes.len(), 2);
        assert_eq!(config.config.seed_count, Some(20));

        assert_eq!(config.templates.len(), 2);
        assert_eq!(
            config.templates[0].resolve_input(Path::new("/tmp/chrox")),
            Path::new("/tmp/chrox/templates/alacritty.toml")
        );
    }

    #[test]
    fn find_template_by_name() {
        let config = Config::from_str(
            r#"
[[templates]]
name = "kitty"
input = "templates/kitty.conf"
output = ".config/kitty/colors.conf"
"#,
        )
        .expect("config should parse");

        let kitty = config
            .find_template("kitty")
            .expect("template should exist");
        assert_eq!(kitty.input, Path::new("templates/kitty.conf"));
        assert_eq!(config.find_template("missing"), None);
    }

    #[test]
    fn load_from_explicit_path() {
        let dir = unique_temp_dir();
        std::fs::create_dir_all(&dir).expect("test temp dir should be created");

        let config_path = dir.join("config.toml");
        std::fs::write(
            &config_path,
            r#"
[general]
palettes = ["palettes"]

[[templates]]
name = "wezterm"
input = "templates/wezterm.lua"
output = ".config/wezterm/colors.lua"

[config]
keep_top_k = 3
"#,
        )
        .expect("test config file should be written");

        let config = Config::load(Some(config_path.clone())).expect("config should load");
        assert_eq!(config.general.palettes, vec![PathBuf::from("palettes")]);
        assert_eq!(config.templates.len(), 1);
        assert_eq!(config.templates[0].name, "wezterm");
        assert_eq!(config.config.keep_top_k, Some(3));

        let _ = std::fs::remove_file(config_path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn default_path_suffix_is_stable() {
        assert!(Config::default_path().ends_with(Path::new("chrox/config.toml")));
    }
}
