//! Unified error types for the image pipeline.

use thiserror::Error;

/// Errors produced by `chromoxide-image` pipeline stages.
#[derive(Debug, Error)]
pub enum ImagePipelineError {
    /// Input image is empty (zero width or height).
    #[error("empty image")]
    EmptyImage,

    /// No pixels remained after preprocessing validity checks.
    #[error("no valid pixels")]
    NoValidPixels,

    /// Configuration is invalid.
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    /// I/O error while loading image input.
    #[error("io error: {0}")]
    Io(String),

    /// Image decode error.
    #[error("image decode error: {0}")]
    ImageDecode(String),

    /// Numeric issue (overflow, non-finite value, etc).
    #[error("numeric error: {0}")]
    Numeric(String),

    /// `chromoxide::ImageCapBuilder` build error.
    #[error("image cap build error: {0}")]
    CapBuild(String),
}
