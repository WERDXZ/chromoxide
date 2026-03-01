//! `chromoxide-image`: image entry pipeline for `chromoxide`.
//!
//! This crate converts image inputs into:
//! - `Vec<chromoxide::WeightedSample>`
//! - optional `chromoxide::ImageCap`
//! - image preprocessing / saliency / sampling diagnostics

pub mod assignment;
pub mod cap;
pub mod config;
pub mod diagnostics;
pub mod error;
pub mod load;
pub mod pipeline;
pub mod prepared;
pub mod preprocess;
pub mod saliency;
pub mod sampling;
pub mod util;

pub use assignment::export_samples;
pub use cap::build_image_cap;
pub use config::{
    CapConfig, CapSource, CenterMode, ExportConfig, FarthestPointLabConfig, GlobalContrastConfig,
    ImagePipelineConfig, LocalContrastConfig, PreprocessConfig, RandomUniformConfig, ResizeFilter,
    SaliencyConfig, SaliencyMethod, SamplingConfig, SamplingMethod, StratifiedConfig,
    UniformGridConfig,
};
pub use diagnostics::{ImagePipelineDiagnostics, SaliencyStats, saliency_to_luma_image};
pub use error::ImagePipelineError;
pub use load::load_image_from_path;
pub use pipeline::{
    ImageSupport, prepare_support_from_image, prepare_support_from_image_with_rng,
    prepare_support_from_path, prepare_support_from_path_with_rng,
};
pub use prepared::{PreparedImage, PreparedPixel};
pub use preprocess::prepare_image;
pub use saliency::{SaliencyMap, compute_saliency};
pub use sampling::{Representative, select_representatives, select_representatives_with_rng};
