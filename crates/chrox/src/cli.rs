//! Argument parsing and command dispatch.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

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
    #[arg(long, value_name = "DIR", value_delimiter = ',')]
    palettes: Vec<PathBuf>,

    /// Optional configuration file path (overrides defaults)
    #[arg(short, long, value_name = "CONFIG")]
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
            // TODO: list built-in families / template search paths
            println!("Listing palettes / templates...");
            Ok(())
        }
        Some(Commands::Show { id }) => {
            // TODO: show family definition / slots / aliases
            println!("Showing: {id}");
            Ok(())
        }
        None => {
            // Render mode (config/template-driven palette inference)
            let image_path = args.image.ok_or(Error::MissingImage)?;

            if !image_path.exists() {
                return Err(Error::ImageNotFound { path: image_path });
            }
            for palette_path in &args.palettes {
                if !palette_path.exists() {
                    return Err(Error::PalettePathNotFound {
                        path: palette_path.clone(),
                    });
                }
            }
            if let Some(cfg) = &args.config
                && !cfg.exists()
            {
                return Err(Error::ConfigNotFound { path: cfg.clone() });
            }

            // TODO:
            // 1) load config (or defaults)
            // 2) merge palette search paths: config.general.palettes + CLI --palettes
            // 3) load and parse configured templates
            // 4) scan {{palette.member | filter}} references and infer required palettes
            // 5) chromoxide-image: samples + cap
            // 6) solve per required palette
            // 7) render template(s) -> output files

            println!("Processing image: {:?}", image_path);
            if let Some(cfg) = &args.config {
                println!("Using config: {:?}", cfg);
            }
            if !args.palettes.is_empty() {
                println!("Additional palette paths: {:?}", args.palettes);
            }

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;

    use super::{Args, Error, run};

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

        let err = run(Args {
            command: None,
            image: Some(image_path),
            palettes: vec![missing.clone()],
            config: None,
        })
        .expect_err("missing palette path should fail");

        assert!(matches!(err, Error::PalettePathNotFound { path } if path == missing));
    }
}
