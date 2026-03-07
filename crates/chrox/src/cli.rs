//! Argument parsing and command dispatch.

use std::num::{NonZeroU32, NonZeroUsize};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use chromoxide::convert::{oklab_to_linear_srgb, relative_luminance};
use chromoxide_image::{
    CapConfig, FarthestPointLabConfig, ImagePipelineConfig, LocalContrastConfig,
    SamplingConfig, SamplingMethod, SaliencyConfig, SaliencyMethod, prepare_support_from_path,
};

use crate::config::Config;
use crate::filter;
use crate::palette::Palette;
use crate::palette::registry::{PaletteRecordRef, PaletteRegistry};
use crate::template::{TemplateEngine, Token};

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
    #[error("failed to load config: {0}")]
    ConfigLoad(#[from] crate::config::Error),
    #[error("failed to discover palettes: {0}")]
    PaletteDiscovery(#[source] Box<crate::palette::registry::Error>),
    #[error("palette not found: {id}")]
    PaletteNotFound { id: String },
    #[error("failed to prepare image support")]
    ImageSupport(#[from] chromoxide_image::ImagePipelineError),
    #[error("no templates configured")]
    NoTemplates,
    #[error("failed to parse template")]
    Template(#[from] crate::template::Error),
    #[error("palette `{palette}` missing member `{member}`")]
    MissingPaletteMember { palette: String, member: String },
    #[error("unsupported filter `{name}`")]
    UnsupportedFilter { name: String },
    #[error("failed to create output directory `{path}`")]
    CreateOutputDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write output file `{path}`")]
    WriteOutput {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to solve palette `{id}`")]
    PaletteSolve {
        id: String,
        #[source]
        source: Box<crate::palette::SolveError>,
    },
}

impl From<crate::palette::registry::Error> for Error {
    fn from(source: crate::palette::registry::Error) -> Self {
        Self::PaletteDiscovery(Box::new(source))
    }
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

    /// Solve one or more palettes against an image and print to stdout
    Test {
        /// Palette ids to solve in order
        #[arg(value_name = "PALETTE", required = true, num_args = 1..)]
        palette_ids: Vec<String>,
        /// Image to process; pass after `--`
        #[arg(value_name = "IMAGE", last = true)]
        image: PathBuf,
    },
}

pub fn run(args: Args) -> Result<(), Error> {
    match args.command {
        Some(Commands::List) => {
            let ctx = load_context(args.config.as_ref(), &args.palettes)?;

            println!("Configured templates: {}", ctx.config.templates.len());
            print_palette_paths(&ctx.merged_palette_paths);

            let mut builtin_ids = ctx
                .registry
                .builtin_palettes()
                .map(|entry| entry.id)
                .collect::<Vec<_>>();
            builtin_ids.sort_unstable();

            if builtin_ids.is_empty() {
                println!("Builtin palettes: none registered");
            } else {
                println!("Builtin palettes:");
                for id in builtin_ids {
                    println!("  - {id}");
                }
            }

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
            match ctx.registry.resolve(&id) {
                Some(PaletteRecordRef::User(record)) => {
                    println!("source: user");
                    println!("id: {}", record.id);
                    println!("name: {}", record.palette.name);
                    println!("path: {}", record.path.display());
                    println!("slots: {}", record.palette.slots.len());
                    println!("terms: {}", record.palette.terms.len());
                }
                Some(PaletteRecordRef::Builtin(record)) => {
                    println!("source: builtin");
                    println!("id: {}", record.id);
                    println!("name: {}", record.name);
                }
                None => return Err(Error::PaletteNotFound { id }),
            }

            Ok(())
        }
        Some(Commands::Test { palette_ids, image }) => {
            if !image.exists() {
                return Err(Error::ImageNotFound { path: image });
            }

            let ctx = load_context(args.config.as_ref(), &args.palettes)?;
            let support = prepare_support_from_path(&image, &default_test_pipeline_config())?;

            for (idx, palette_id) in palette_ids.iter().enumerate() {
                let record = ctx
                    .registry
                    .resolve(palette_id)
                    .ok_or_else(|| Error::PaletteNotFound {
                        id: palette_id.clone(),
                    })?;

                let rendered = match record {
                    PaletteRecordRef::User(record) => {
                        let colors = record
                            .palette
                            .solve(
                                support.samples.clone(),
                                support.image_cap.clone(),
                                &ctx.config.config,
                            )
                            .map_err(|source| Error::PaletteSolve {
                                id: record.id.clone(),
                                source: Box::new(source),
                            })?;
                        format_palette_output(&record.id, &record.palette.name, &colors)
                    }
                    PaletteRecordRef::Builtin(record) => {
                        let palette = (record.build)();
                        let colors = palette
                            .solve(
                                support.samples.clone(),
                                support.image_cap.clone(),
                                &ctx.config.config,
                            )
                            .map_err(|source| Error::PaletteSolve {
                                id: record.id.to_string(),
                                source: Box::new(source),
                            })?;
                        format_palette_output(record.id, record.name, &colors)
                    }
                };

                if idx > 0 {
                    println!();
                }
                print!("{rendered}");
            }

            Ok(())
        }
        None => {
            let image_path = args.image.ok_or(Error::MissingImage)?;

            if !image_path.exists() {
                return Err(Error::ImageNotFound { path: image_path });
            }
            let ctx = load_context(args.config.as_ref(), &args.palettes)?;

            render_mode(image_path, &ctx, args.config.as_ref())
        }
    }
}

fn render_mode(image_path: PathBuf, ctx: &RunContext, config_path: Option<&PathBuf>) -> Result<(), Error> {
    if ctx.config.templates.is_empty() {
        return Err(Error::NoTemplates);
    }

    let config_base_dir = config_base_dir(config_path);
    let mut engine = TemplateEngine::new();
    let mut render_jobs = Vec::with_capacity(ctx.config.templates.len());
    for entry in &ctx.config.templates {
        let input = entry.resolve_input(&config_base_dir);
        let output = entry.resolve_output(&config_base_dir);
        let source = engine.parse_file(&input)?;
        render_jobs.push(RenderJob {
            name: entry.name.clone(),
            output,
            source,
        });
    }

    let support = prepare_support_from_path(&image_path, &default_test_pipeline_config())?;
    let palette_ids = engine.required_palettes();
    let mut solved = std::collections::HashMap::with_capacity(palette_ids.len());
    for palette_id in palette_ids {
        let record = ctx
            .registry
            .resolve(&palette_id)
            .ok_or_else(|| Error::PaletteNotFound {
                id: palette_id.clone(),
            })?;
        let colors = solve_palette_record(record, &support.samples, support.image_cap.clone(), &ctx.config.config)?;
        solved.insert(palette_id, colors);
    }

    for job in render_jobs {
        let source = engine.source(job.source).expect("parsed source should exist");
        let rendered = render_template_source(source, &engine, &solved)?;
        if let Some(parent) = job.output.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|source| Error::CreateOutputDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(&job.output, rendered).map_err(|source| Error::WriteOutput {
            path: job.output.clone(),
            source,
        })?;
        println!("rendered {} -> {}", job.name, job.output.display());
    }

    Ok(())
}

fn solve_palette_record(
    record: PaletteRecordRef<'_>,
    samples: &[chromoxide::WeightedSample],
    image_cap: Option<chromoxide::ImageCap>,
    config: &crate::solve_config::PartialSolveConfig,
) -> Result<std::collections::HashMap<String, chromoxide::Oklch>, Error> {
    match record {
        PaletteRecordRef::User(record) => record
            .palette
            .solve(samples.to_vec(), image_cap, config)
            .map_err(|source| Error::PaletteSolve {
                id: record.id.clone(),
                source: Box::new(source),
            }),
        PaletteRecordRef::Builtin(record) => (record.build)()
            .solve(samples.to_vec(), image_cap, config)
            .map_err(|source| Error::PaletteSolve {
                id: record.id.to_string(),
                source: Box::new(source),
            }),
    }
}

fn render_template_source(
    source: &crate::template::TemplateSource,
    engine: &TemplateEngine,
    palettes: &std::collections::HashMap<String, std::collections::HashMap<String, chromoxide::Oklch>>,
) -> Result<String, Error> {
    let mut out = String::with_capacity(source.content().len());
    for token in source.tokens() {
        match token {
            Token::Text(span) => out.push_str(source.slice(span)),
            Token::Slot(index) => {
                let template = engine.template(*index).expect("template should exist");
                let palette_name = template.palette_name(source);
                let member_name = template.member_name(source);
                let filter_name = template.filter_name(source).unwrap_or("hex");
                let palette = palettes.get(palette_name).ok_or_else(|| Error::PaletteNotFound {
                    id: palette_name.to_string(),
                })?;
                let color = palette.get(member_name).ok_or_else(|| Error::MissingPaletteMember {
                    palette: palette_name.to_string(),
                    member: member_name.to_string(),
                })?;
                let rendered = filter::apply(filter_name, *color).ok_or_else(|| Error::UnsupportedFilter {
                    name: filter_name.to_string(),
                })?;
                out.push_str(&rendered);
            }
        }
    }
    Ok(out)
}

fn default_test_pipeline_config() -> ImagePipelineConfig {
    ImagePipelineConfig {
        saliency: SaliencyConfig {
            method: SaliencyMethod::LocalContrast(LocalContrastConfig::default()),
        },
        sampling: SamplingConfig {
            method: SamplingMethod::FarthestPointLab(FarthestPointLabConfig {
                count: NonZeroUsize::new(24).expect("24 is non-zero"),
                candidate_stride: NonZeroU32::new(2).expect("2 is non-zero"),
                saliency_bias: 0.35,
            }),
        },
        cap: Some(CapConfig::default()),
        ..Default::default()
    }
}

fn format_palette_output(
    id: &str,
    name: &str,
    colors: &std::collections::HashMap<String, chromoxide::Oklch>,
) -> String {
    let mut entries = colors.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(name, _)| *name);

    let rows = entries
        .into_iter()
        .map(|(slot, color)| PaletteRow {
            slot: slot.as_str(),
            hex: filter::apply("hex", *color).expect("hex filter should exist"),
            oklch: filter::apply("oklch", *color).expect("oklch filter should exist"),
            preview: color_preview(*color, preview_label(slot)),
        })
        .collect::<Vec<_>>();

    let slot_width = rows
        .iter()
        .map(|row| row.slot.len())
        .max()
        .unwrap_or(4)
        .max("slot".len());
    let hex_width = rows
        .iter()
        .map(|row| row.hex.len())
        .max()
        .unwrap_or(4)
        .max("hex".len());
    let oklch_width = rows
        .iter()
        .map(|row| row.oklch.len())
        .max()
        .unwrap_or(5)
        .max("oklch".len());

    let mut out = String::new();
    out.push_str(&format!("palette: {id}\n"));
    out.push_str(&format!("name:    {name}\n"));
    out.push_str(&format!(
        "{: <slot_width$}\t{: <hex_width$}\t{: <oklch_width$}\t{}\n",
        "slot", "hex", "oklch", "preview"
    ));
    for row in rows {
        out.push_str(&format!(
            "{: <slot_width$}\t{: <hex_width$}\t{: <oklch_width$}\t{}\n",
            row.slot, row.hex, row.oklch, row.preview
        ));
    }
    out
}

struct PaletteRow<'a> {
    slot: &'a str,
    hex: String,
    oklch: String,
    preview: String,
}

fn color_preview(color: chromoxide::Oklch, label: &str) -> String {
    let (bg_r, bg_g, bg_b) = srgb_u8(color);
    let (fg_r, fg_g, fg_b) = readable_text_rgb(color);
    format!(
        "\x1b[48;2;{bg_r};{bg_g};{bg_b}m\x1b[38;2;{fg_r};{fg_g};{fg_b}m {:<4} \x1b[0m",
        label
    )
}

fn preview_label(slot: &str) -> &str {
    match slot {
        "cover" => "cvr",
        "salient-1" => "s1",
        "salient-2" => "s2",
        _ => slot,
    }
}

fn srgb_u8(color: chromoxide::Oklch) -> (u8, u8, u8) {
    let linear = oklab_to_linear_srgb(color.to_oklab());
    (
        to_srgb_u8(linear.r),
        to_srgb_u8(linear.g),
        to_srgb_u8(linear.b),
    )
}

fn readable_text_rgb(color: chromoxide::Oklch) -> (u8, u8, u8) {
    let linear = oklab_to_linear_srgb(color.to_oklab());
    let bg_luma = relative_luminance(linear);
    let contrast_with_black = contrast_ratio(bg_luma, 0.0);
    let contrast_with_white = contrast_ratio(bg_luma, 1.0);
    if contrast_with_black >= contrast_with_white {
        (0, 0, 0)
    } else {
        (255, 255, 255)
    }
}

fn contrast_ratio(a: f64, b: f64) -> f64 {
    let lighter = a.max(b);
    let darker = a.min(b);
    (lighter + 0.05) / (darker + 0.05)
}

fn to_srgb_u8(channel: f64) -> u8 {
    let srgb = if channel <= 0.003_130_8 {
        12.92 * channel
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    };
    (srgb.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[derive(Debug)]
struct RunContext {
    config: Config,
    merged_palette_paths: Vec<PathBuf>,
    registry: PaletteRegistry,
}

#[derive(Debug)]
struct RenderJob {
    name: String,
    output: PathBuf,
    source: crate::template::SourceIndex,
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

fn print_palette_paths(paths: &[PathBuf]) {
    println!("Palette search paths: {}", paths.len());
    for path in paths {
        println!("  - {}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use chromoxide::Oklch;
    use clap::Parser;

    use super::{Args, Commands, Error, config_base_dir, format_palette_output, run};

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("chrox-cli-test-{nanos}-{}", std::process::id()))
    }

    fn write_test_image(path: &Path) {
        std::fs::write(path, b"P3\n1 1\n255\n200 80 120\n").expect("test image should be written");
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
    fn test_subcommand_parses_palettes_and_image_after_double_dash() {
        let args = Args::try_parse_from(["chrox", "test", "cover-salient", "base16", "--", "wall.png"])
            .expect("args should parse");

        match args.command {
            Some(Commands::Test { palette_ids, image }) => {
                assert_eq!(palette_ids, vec!["cover-salient", "base16"]);
                assert_eq!(image, PathBuf::from("wall.png"));
            }
            other => panic!("expected test command, got {other:?}"),
        }
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
    fn render_mode_requires_templates() {
        let dir = unique_temp_dir();
        std::fs::create_dir_all(&dir).expect("test temp dir should be created");
        let image_path = dir.join("tiny.ppm");
        let config_path = dir.join("config.toml");
        write_test_image(&image_path);
        std::fs::write(&config_path, "").expect("config should be written");

        let err = run(Args {
            command: None,
            image: Some(image_path),
            palettes: Vec::new(),
            config: Some(config_path.clone()),
        })
        .expect_err("missing templates should fail");

        assert!(matches!(err, Error::NoTemplates));

        let _ = std::fs::remove_file(config_path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn render_mode_renders_templates_to_output_files() {
        let dir = unique_temp_dir();
        let templates_dir = dir.join("templates");
        let out_dir = dir.join("out");
        std::fs::create_dir_all(&templates_dir).expect("templates dir should be created");

        let image_path = dir.join("tiny.ppm");
        let config_path = dir.join("config.toml");
        let template_path = templates_dir.join("demo.txt");
        let output_path = out_dir.join("demo.txt");
        write_test_image(&image_path);
        std::fs::write(
            &template_path,
            "bg={{base16.base00|hex}}\nfg={{base16.base05|hex}}\nred={{ansi-8-derived.red|hex}}\n",
        )
        .expect("template should be written");
        std::fs::write(
            &config_path,
            r#"
[[templates]]
name = "demo"
input = "templates/demo.txt"
output = "out/demo.txt"
"#,
        )
        .expect("config should be written");

        run(Args {
            command: None,
            image: Some(image_path),
            palettes: Vec::new(),
            config: Some(config_path.clone()),
        })
        .expect("render mode should succeed");

        let rendered = std::fs::read_to_string(&output_path).expect("output should exist");
        assert!(rendered.contains("bg=#"));
        assert!(rendered.contains("fg=#"));
        assert!(rendered.contains("red=#"));

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

    #[test]
    fn format_palette_output_sorts_slots_and_includes_color_formats() {
        let mut colors = HashMap::new();
        colors.insert(
            "salient".to_string(),
            Oklch {
                l: 0.7,
                c: 0.14,
                h: 0.8,
            },
        );
        colors.insert(
            "cover".to_string(),
            Oklch {
                l: 0.4,
                c: 0.02,
                h: 0.2,
            },
        );

        let output = format_palette_output("cover-salient", "Cover + Salient", &colors);
        let lines = output.lines().collect::<Vec<_>>();

        assert_eq!(lines[0], "palette: cover-salient");
        assert_eq!(lines[1], "name:    Cover + Salient");
        assert!(lines[2].starts_with("slot"));
        assert!(lines[2].contains("\thex"));
        assert!(lines[3].starts_with("cover"));
        assert!(lines[3].contains("\t#"));
        assert!(lines[3].contains("\toklch("));
        assert!(lines[3].contains("\x1b[48;2;"));
        assert!(lines[3].contains("cvr"));
        assert!(lines[4].starts_with("salient"));
    }
}
