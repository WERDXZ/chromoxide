//! Image loading helpers.

use std::path::Path;

use image::{DynamicImage, ImageReader};

use crate::error::ImagePipelineError;

/// Loads an image from disk and decodes it into `DynamicImage`.
pub fn load_image_from_path<P: AsRef<Path>>(path: P) -> Result<DynamicImage, ImagePipelineError> {
    let path_ref = path.as_ref();
    let reader = ImageReader::open(path_ref)
        .map_err(|err| ImagePipelineError::Io(format!("{} ({})", path_ref.display(), err)))?;
    reader
        .decode()
        .map_err(|err| ImagePipelineError::ImageDecode(format!("{} ({})", path_ref.display(), err)))
}
