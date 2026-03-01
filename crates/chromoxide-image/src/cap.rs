//! `ImageCap` build bridge to `chromoxide::ImageCapBuilder`.

use crate::config::{CapConfig, CapSource};
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

    let built = match cfg.source {
        CapSource::PreparedPixels => cfg.builder.build_from_oklab(|| {
            prepared
                .valid_indices
                .iter()
                .map(|&idx| prepared.pixels[idx].lab)
        }),
        CapSource::ExportedSamples => {
            let samples = exported_samples.ok_or_else(|| {
                ImagePipelineError::InvalidConfig(
                    "cap source is ExportedSamples but exported_samples is None".to_string(),
                )
            })?;
            cfg.builder
                .build_from_oklab(|| samples.iter().map(|sample| sample.lab))
        }
    };

    built.map_err(|err| ImagePipelineError::CapBuild(err.to_string()))
}
