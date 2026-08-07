//! `ImageCap` build bridge to `chromoxide::ImageCapBuilder`.

use crate::config::{CapConfig, CapEstimator, CapSource};
use crate::error::ImagePipelineError;
use crate::prepared::PreparedImage;
use crate::saliency::SaliencyMap;
use crate::util::checked_len;

/// Builds `chromoxide::ImageCap` from prepared pixels or exported samples.
pub fn build_image_cap(
    prepared: &PreparedImage,
    saliency: &SaliencyMap,
    exported_samples: Option<&[chromoxide::WeightedSample]>,
    cfg: &CapConfig,
) -> Result<chromoxide::ImageCap, ImagePipelineError> {
    let len = checked_len(prepared.width, prepared.height)?;
    if saliency.width != prepared.width
        || saliency.height != prepared.height
        || saliency.values.len() != len
    {
        return Err(ImagePipelineError::InvalidConfig(
            "saliency map dimensions must match prepared image".to_string(),
        ));
    }

    let built = match (&cfg.source, &cfg.estimator) {
        (CapSource::PreparedPixels, CapEstimator::MaxObserved) => {
            cfg.builder.build_from_oklab(|| {
                prepared
                    .valid_indices
                    .iter()
                    .map(|&idx| prepared.pixels[idx].lab)
            })
        }
        (CapSource::ExportedSamples, CapEstimator::MaxObserved) => {
            let samples = exported_samples.ok_or_else(|| {
                ImagePipelineError::InvalidConfig(
                    "cap source is ExportedSamples but exported_samples is None".to_string(),
                )
            })?;
            cfg.builder
                .build_from_oklab(|| samples.iter().map(|sample| sample.lab))
        }
        (CapSource::PreparedPixels, CapEstimator::Statistical(config)) => {
            cfg.builder.build_statistical_from_weighted_oklab(
                || {
                    prepared.valid_indices.iter().map(|&idx| {
                        let px = prepared.pixels[idx];
                        (px.lab, px.alpha)
                    })
                },
                *config,
            )
        }
        (CapSource::ExportedSamples, CapEstimator::Statistical(config)) => {
            let samples = exported_samples.ok_or_else(|| {
                ImagePipelineError::InvalidConfig(
                    "cap source is ExportedSamples but exported_samples is None".to_string(),
                )
            })?;
            cfg.builder.build_statistical(samples, *config)
        }
    };

    built.map_err(|err| ImagePipelineError::CapBuild(err.to_string()))
}
