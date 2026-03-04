//! Argument parsing and command dispatch.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing IMAGE. See --help")]
    MissingImage,
    #[error("missing --palette <PALETTE>. See --help")]
    MissingPalette,
    #[error("image not found: {path}")]
    ImageNotFound { path: PathBuf },
    #[error("template not found: {path}")]
    TemplateNotFound { path: PathBuf },
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

    /// Palette file/dir used to infer required palette families and render output
    #[arg(short, long, value_name = "PALETTE")]
    palette: Option<PathBuf>,

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
            // Render mode (template-driven palette inference)
            let image_path = args.image.ok_or(Error::MissingImage)?;
            let template_path = args
                .palette
                .ok_or(Error::MissingPalette)?;

            if !image_path.exists() {
                return Err(Error::ImageNotFound { path: image_path });
            }
            if !template_path.exists() {
                return Err(Error::TemplateNotFound {
                    path: template_path,
                });
            }
            if let Some(cfg) = &args.config
                && !cfg.exists()
            {
                return Err(Error::ConfigNotFound { path: cfg.clone() });
            }

            // TODO:
            // 1) load template
            // 2) scan {{family.slot | filter}} references
            // 3) infer required families
            // 4) chromoxide-image: samples + cap
            // 5) solve per family
            // 6) render template -> string
            // 7) write to out or stdout

            println!("Processing image: {:?}", image_path);
            println!("Using template: {:?}", template_path);
            if let Some(cfg) = &args.config {
                println!("Using config: {:?}", cfg);
            }

            Ok(())
        }
    }
}
