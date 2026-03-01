//! Public configuration types for the image pipeline.

use std::num::{NonZeroU32, NonZeroUsize};

/// Top-level configuration for the full image support pipeline.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct ImagePipelineConfig {
    /// Preprocessing settings.
    pub preprocess: PreprocessConfig,
    /// Saliency settings.
    pub saliency: SaliencyConfig,
    /// Representative sampling settings.
    pub sampling: SamplingConfig,
    /// Export settings from clusters to weighted samples.
    pub export: ExportConfig,
    /// Optional image cap build settings.
    pub cap: Option<CapConfig>,
}

/// Configuration for image preprocessing.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PreprocessConfig {
    /// Optional max dimension for the working image (longest side).
    pub max_working_dim: Option<NonZeroU32>,
    /// Resize filter used when downscaling.
    pub resize_filter: ResizeFilter,
    /// sRGB background color for alpha compositing.
    pub background_rgb8: [u8; 3],
    /// Pixels with alpha below this threshold are invalid.
    pub min_alpha: f64,
    /// Whether alpha contributes to final sample mass.
    pub alpha_into_weight: bool,
}

impl Default for PreprocessConfig {
    fn default() -> Self {
        Self {
            max_working_dim: None,
            resize_filter: ResizeFilter::Lanczos3,
            background_rgb8: [255, 255, 255],
            min_alpha: 1.0 / 255.0,
            alpha_into_weight: false,
        }
    }
}

/// Resize filter choices for preprocessing.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResizeFilter {
    /// Nearest-neighbor filter.
    Nearest,
    /// Triangle (bilinear-like) filter.
    Triangle,
    /// Catmull-Rom cubic filter.
    CatmullRom,
    /// Gaussian filter.
    Gaussian,
    /// Lanczos3 filter.
    #[default]
    Lanczos3,
}

/// Configuration for saliency computation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct SaliencyConfig {
    /// Chosen saliency method.
    pub method: SaliencyMethod,
}

impl Default for SaliencyConfig {
    fn default() -> Self {
        Self {
            method: SaliencyMethod::None,
        }
    }
}

/// Built-in saliency methods.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum SaliencyMethod {
    /// Neutral saliency map (`1.0` everywhere).
    None,
    /// Global color contrast against image-wide mean Oklab.
    GlobalContrast(GlobalContrastConfig),
    /// Local contrast against a blurred neighborhood.
    LocalContrast(LocalContrastConfig),
}

/// Config for global-contrast saliency.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalContrastConfig {
    /// If true, normalize using p1/p99 instead of min/max.
    pub robust_normalize: bool,
}

impl Default for GlobalContrastConfig {
    fn default() -> Self {
        Self {
            robust_normalize: true,
        }
    }
}

/// Config for local-contrast saliency.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalContrastConfig {
    /// Box blur radius in pixels.
    pub blur_radius: u32,
    /// Weight for Oklab color contrast.
    ///
    /// Higher values prioritize chromatic edge/detail response.
    pub color_weight: f64,
    /// Weight for luminance contrast.
    ///
    /// Higher values prioritize brightness-edge response.
    pub luminance_weight: f64,
    /// Mix factor for adding global contrast term (`[0, 1]`).
    ///
    /// `0` keeps purely local behavior; `1` shifts fully to global contrast.
    pub global_mix: f64,
    /// If true, normalize using p1/p99 instead of min/max.
    pub robust_normalize: bool,
}

impl Default for LocalContrastConfig {
    fn default() -> Self {
        Self {
            blur_radius: 3,
            color_weight: 1.0,
            luminance_weight: 1.0,
            global_mix: 0.2,
            robust_normalize: true,
        }
    }
}

/// Configuration for representative sampling.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct SamplingConfig {
    /// Selected sampling method.
    pub method: SamplingMethod,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            method: SamplingMethod::UniformGrid(UniformGridConfig {
                step: NonZeroU32::new(8).expect("8 is non-zero"),
            }),
        }
    }
}

/// Built-in representative sampling methods.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum SamplingMethod {
    /// Regular grid selection in image space.
    UniformGrid(UniformGridConfig),
    /// Stratified random selection per tile.
    Stratified(StratifiedConfig),
    /// Uniform random sampling from valid pixels.
    RandomUniform(RandomUniformConfig),
    /// Greedy farthest-point sampling in Oklab space.
    FarthestPointLab(FarthestPointLabConfig),
}

/// Config for uniform-grid sampling.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UniformGridConfig {
    /// Grid step in pixels.
    pub step: NonZeroU32,
}

/// Config for stratified sampling.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StratifiedConfig {
    /// Number of tiles along x-axis.
    pub tiles_x: NonZeroU32,
    /// Number of tiles along y-axis.
    pub tiles_y: NonZeroU32,
    /// Samples drawn per tile.
    pub per_tile: NonZeroU32,
}

/// Config for uniform random sampling.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RandomUniformConfig {
    /// Number of representative points requested.
    pub count: NonZeroUsize,
}

/// Config for farthest-point sampling in Oklab.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FarthestPointLabConfig {
    /// Number of representative points requested.
    pub count: NonZeroUsize,
    /// Candidate downsampling stride over valid pixels.
    pub candidate_stride: NonZeroU32,
    /// Saliency bias factor used in selection score.
    ///
    /// Higher values pull selected representatives toward salient regions.
    pub saliency_bias: f64,
}

/// Configuration for converting clusters into weighted samples.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportConfig {
    /// Cluster center export mode.
    pub center_mode: CenterMode,
    /// Normalize final sample weights so `sum = 1`.
    pub normalize_weights: bool,
    /// Saliency-to-weight mix factor (`[0, 1]`).
    ///
    /// Higher values allocate more final mass to salient clusters.
    pub saliency_to_weight_mix: f64,
    /// Gamma for saliency term when mixed into weight.
    ///
    /// Values above `1` emphasize top-saliency clusters; below `1` flatten saliency contrast.
    pub saliency_weight_gamma: f64,
    /// Gamma for frequency/mass term.
    ///
    /// Values above `1` emphasize dominant clusters; below `1` spread mass more evenly.
    pub frequency_gamma: f64,
    /// Drop samples with weight below this threshold.
    pub min_cluster_weight: f64,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            center_mode: CenterMode::Centroid,
            normalize_weights: true,
            saliency_to_weight_mix: 0.0,
            saliency_weight_gamma: 1.0,
            frequency_gamma: 1.0,
            min_cluster_weight: 0.0,
        }
    }
}

/// Center selection mode for cluster color export.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CenterMode {
    /// Weighted Oklab centroid.
    #[default]
    Centroid,
    /// Real cluster pixel nearest to centroid.
    Medoid,
}

/// Configuration for optional `ImageCap` construction.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct CapConfig {
    /// Source data used to build the cap.
    pub source: CapSource,
    /// Reused `chromoxide` cap builder.
    pub builder: chromoxide::ImageCapBuilder,
}

impl Default for CapConfig {
    fn default() -> Self {
        Self {
            source: CapSource::PreparedPixels,
            builder: chromoxide::ImageCapBuilder::default(),
        }
    }
}

/// Input source for cap building.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CapSource {
    /// Build from all valid prepared pixels.
    #[default]
    PreparedPixels,
    /// Build from exported weighted samples.
    ExportedSamples,
}
