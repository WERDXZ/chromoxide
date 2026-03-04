//! Argument parsing and command dispatch.

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::palette::registry::PaletteRegistry;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing IMAGE. See --help")]
    MissingImage,
    #[error("image not found: {path}")]
    ImageNotFound { path: PathBuf },
    #[error("palette path not found: {path}")]
    PalettePathNotFound { path: PathBuf },
    #[error("config not found: {path}")]
    ConfigNotFound { path: PathBuf },
    #[error("failed to load config")]
    ConfigLoad(#[from] crate::config::Error),
    #[error("failed to discover palettes")]
    PaletteDiscovery(#[from] crate::palette::registry::Error),
    #[error("palette not found: {id}")]
    PaletteNotFound { id: String },
}

/// chromoxide CLI
#[derive(Parser, Debug)]
#[command(name = "chrox")]
#[command(about = "Colorscheme generator based on image.", long_about = None)]
#[command(arg_required_else_help = true)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Image to process (render mode if no subcommand is provided)
    #[arg(value_name = "IMAGE")]
    image: Option<PathBuf>,

    /// Additional palette search paths (comma-separated or repeated)
    #[arg(global = true, long, value_name = "DIR", value_delimiter = ',')]
    palettes: Vec<PathBuf>,

    /// Optional configuration file path (overrides defaults)
    #[arg(global = true, short, long, value_name = "CONFIG")]
    config: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List available palette families / templates
    List,

    /// Show details about a palette family or template pack
    Show {
        /// Identifier (e.g. ansi_light, base16_dark, ...)
        id: String,
    },
}

pub fn run(args: Args) -> Result<(), Error> {
    match args.command {
        Some(Commands::List) => {
            let ctx = load_context(args.config.as_ref(), &args.palettes)?;

            println!("Configured templates: {}", ctx.config.templates.len());
            println!("Palette search paths: {}", ctx.merged_palette_paths.len());

            let mut user_ids = ctx
                .registry
                .user_palettes()
                .map(|entry| entry.id.clone())
                .collect::<Vec<_>>();
            user_ids.sort();

            if user_ids.is_empty() {
                println!("User palettes: none discovered");
            } else {
                println!("User palettes:");
                for id in user_ids {
                    println!("  - {id}");
                }
            }

            Ok(())
        }
        Some(Commands::Show { id }) => {
            let ctx = load_context(args.config.as_ref(), &args.palettes)?;
            let record = ctx
                .registry
                .user_record(&id)
                .ok_or(Error::PaletteNotFound { id })?;

            println!("id: {}", record.id);
            println!("name: {}", record.palette.name);
            println!("path: {}", record.path.display());
            println!("slots: {}", record.palette.slots.len());
            println!("terms: {}", record.palette.terms.len());

            Ok(())
        }
        None => {
            // Render mode (config/template-driven palette inference)
            let image_path = args.image.ok_or(Error::MissingImage)?;

            if !image_path.exists() {
                return Err(Error::ImageNotFound { path: image_path });
            }
            let ctx = load_context(args.config.as_ref(), &args.palettes)?;

            // TODO:
            // 1) load and parse configured templates
            // 2) scan {{palette.member | filter}} references and infer required palettes
            // 3) chromoxide-image: samples + cap
            // 4) solve per required palette
            // 5) render template(s) -> output files

            println!("Processing image: {:?}", image_path);
            if let Some(cfg) = &args.config {
                println!("Using config: {:?}", cfg);
            }
            if !ctx.merged_palette_paths.is_empty() {
                println!("Palette search paths: {:?}", ctx.merged_palette_paths);
            }
            println!("Configured templates: {}", ctx.config.templates.len());
            println!("Discovered {} user palettes", ctx.registry.user_palette_count());

            Ok(())
        }
    }
}

#[derive(Debug)]
struct RunContext {
    config: Config,
    merged_palette_paths: Vec<PathBuf>,
    registry: PaletteRegistry,
}

fn load_context(config_path: Option<&PathBuf>, cli_palettes: &[PathBuf]) -> Result<RunContext, Error> {
    if let Some(cfg) = config_path
        && !cfg.exists()
    {
        return Err(Error::ConfigNotFound { path: cfg.clone() });
    }

    for palette_path in cli_palettes {
        if !palette_path.exists() {
            return Err(Error::PalettePathNotFound {
                path: palette_path.clone(),
            });
        }
    }

    let config = Config::load(config_path.cloned())?;
    let config_base_dir = config_base_dir(config_path);
    let merged_palette_paths = config.merged_palette_paths(&config_base_dir, cli_palettes);

    for palette_path in &merged_palette_paths {
        if !palette_path.exists() {
            return Err(Error::PalettePathNotFound {
                path: palette_path.clone(),
            });
        }
    }

    let registry = PaletteRegistry::discover(&merged_palette_paths)?;
    Ok(RunContext {
        config,
        merged_palette_paths,
        registry,
    })
}

fn config_base_dir(config_path: Option<&PathBuf>) -> PathBuf {
    match config_path {
        Some(path) => normalize_parent(path.parent()),
        None => {
            let default_path = Config::default_path();
            normalize_parent(default_path.parent())
        }
    }
}

fn normalize_parent(parent: Option<&Path>) -> PathBuf {
    let parent = parent.unwrap_or_else(|| Path::new("."));
    if parent.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        parent.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use clap::Parser;

    use super::{Args, Commands, Error, config_base_dir, run};

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("chrox-cli-test-{nanos}-{}", std::process::id()))
    }

    #[test]
    fn parse_palettes_supports_comma_and_repeat() {
        let args = Args::try_parse_from([
            "chrox",
            "image.png",
            "--palettes",
            "a,b",
            "--palettes",
            "c",
        ])
        .expect("args should parse");

        assert_eq!(args.image, Some(PathBuf::from("image.png")));
        assert_eq!(
            args.palettes,
            vec![PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")]
        );
    }

    #[test]
    fn subcommand_accepts_global_config_and_palettes() {
        let args = Args::try_parse_from([
            "chrox",
            "list",
            "--config",
            "cfg.toml",
            "--palettes",
            "a,b",
        ])
        .expect("args should parse");

        assert!(matches!(args.command, Some(Commands::List)));
        assert_eq!(args.config, Some(PathBuf::from("cfg.toml")));
        assert_eq!(args.palettes, vec![PathBuf::from("a"), PathBuf::from("b")]);
    }

    #[test]
    fn render_mode_requires_image() {
        let err = run(Args {
            command: None,
            image: None,
            palettes: Vec::new(),
            config: None,
        })
        .expect_err("missing image should fail");

        assert!(matches!(err, Error::MissingImage));
    }

    #[test]
    fn render_mode_rejects_missing_palette_path() {
        let image_path = std::env::current_exe().expect("current executable should be available");
        let missing = PathBuf::from("/definitely/not/a/real/chrox-palette-path");

        let dir = unique_temp_dir();
        std::fs::create_dir_all(&dir).expect("test temp dir should be created");
        let config_path = dir.join("config.toml");
        std::fs::write(&config_path, "").expect("test config file should be written");

        let err = run(Args {
            command: None,
            image: Some(image_path),
            palettes: vec![missing.clone()],
            config: Some(config_path.clone()),
        })
        .expect_err("missing palette path should fail");

        assert!(matches!(err, Error::PalettePathNotFound { path } if path == missing));

        let _ = std::fs::remove_file(config_path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn render_mode_merges_config_palette_paths() {
        let dir = unique_temp_dir();
        std::fs::create_dir_all(&dir).expect("test temp dir should be created");

        let image_path = std::env::current_exe().expect("current executable should be available");
        let config_path = dir.join("config.toml");
        std::fs::write(
            &config_path,
            r#"
[general]
palettes = ["palettes"]
"#,
        )
        .expect("test config file should be written");

        let expected_missing = dir.join("palettes");
        let err = run(Args {
            command: None,
            image: Some(image_path),
            palettes: Vec::new(),
            config: Some(config_path.clone()),
        })
        .expect_err("missing merged config palette path should fail");

        assert!(
            matches!(err, Error::PalettePathNotFound { ref path } if *path == expected_missing),
            "expected missing path {:?}, got {err}",
            expected_missing
        );

        let _ = std::fs::remove_file(config_path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn config_base_dir_uses_parent_or_dot() {
        assert_eq!(config_base_dir(Some(&PathBuf::from("config.toml"))), Path::new("."));
        assert_eq!(
            config_base_dir(Some(&PathBuf::from("cfg/chrox.toml"))),
            Path::new("cfg")
        );
    }

    #[test]
    fn show_returns_not_found_for_unknown_palette() {
        let dir = unique_temp_dir();
        std::fs::create_dir_all(&dir).expect("test temp dir should be created");

        let config_path = dir.join("config.toml");
        std::fs::write(&config_path, "").expect("test config file should be written");

        let err = run(Args {
            command: Some(Commands::Show {
                id: "missing".to_string(),
            }),
            image: None,
            palettes: Vec::new(),
            config: Some(config_path.clone()),
        })
        .expect_err("unknown palette should fail");

        assert!(matches!(err, Error::PaletteNotFound { id } if id == "missing"));

        let _ = std::fs::remove_file(config_path);
        let _ = std::fs::remove_dir_all(dir);
    }
}
