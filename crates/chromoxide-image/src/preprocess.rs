//! Image preprocessing: resize, alpha handling, compositing, and Oklab conversion.

use image::imageops;

use crate::config::PreprocessConfig;
use crate::error::ImagePipelineError;
use crate::prepared::{PreparedImage, PreparedPixel};
use crate::util::{
    checked_len, linear_rgb_to_oklab, relative_luminance, resize_filter_to_image, srgb_u8_to_linear,
};

/// Converts an input image into a working `PreparedImage`.
pub fn prepare_image(
    img: &image::DynamicImage,
    cfg: &PreprocessConfig,
) -> Result<PreparedImage, ImagePipelineError> {
    let (orig_width, orig_height) = image::GenericImageView::dimensions(img);
    if orig_width == 0 || orig_height == 0 {
        return Err(ImagePipelineError::EmptyImage);
    }
    if !cfg.min_alpha.is_finite() || !(0.0..=1.0).contains(&cfg.min_alpha) {
        return Err(ImagePipelineError::InvalidConfig(
            "preprocess.min_alpha must be finite and in [0, 1]".to_string(),
        ));
    }

    let mut rgba = img.to_rgba8();
    if let Some(max_dim) = cfg.max_working_dim {
        let max_dim = max_dim.get();
        let longest = rgba.width().max(rgba.height());
        if longest > max_dim {
            let scale = f64::from(max_dim) / f64::from(longest);
            let new_width = (f64::from(rgba.width()) * scale).round().max(1.0) as u32;
            let new_height = (f64::from(rgba.height()) * scale).round().max(1.0) as u32;
            rgba = imageops::resize(
                &rgba,
                new_width,
                new_height,
                resize_filter_to_image(cfg.resize_filter),
            );
        }
    }

    let width = rgba.width();
    let height = rgba.height();
    let len = checked_len(width, height)?;
    let bg_lin = [
        srgb_u8_to_linear(cfg.background_rgb8[0]),
        srgb_u8_to_linear(cfg.background_rgb8[1]),
        srgb_u8_to_linear(cfg.background_rgb8[2]),
    ];

    let mut pixels = Vec::with_capacity(len);
    let mut valid_indices = Vec::new();
    for (idx, px) in rgba.pixels().enumerate() {
        let [r, g, b, a] = px.0;
        let alpha_raw = f64::from(a) / 255.0;
        let valid = alpha_raw >= cfg.min_alpha;

        let fg_lin = [
            srgb_u8_to_linear(r),
            srgb_u8_to_linear(g),
            srgb_u8_to_linear(b),
        ];

        let lin_rgb = [
            alpha_raw * fg_lin[0] + (1.0 - alpha_raw) * bg_lin[0],
            alpha_raw * fg_lin[1] + (1.0 - alpha_raw) * bg_lin[1],
            alpha_raw * fg_lin[2] + (1.0 - alpha_raw) * bg_lin[2],
        ];
        let lab = linear_rgb_to_oklab(lin_rgb);
        let luminance = relative_luminance(lin_rgb);

        let alpha = if valid {
            if cfg.alpha_into_weight {
                alpha_raw
            } else {
                1.0
            }
        } else {
            0.0
        };

        if valid {
            valid_indices.push(idx);
        }

        pixels.push(PreparedPixel {
            lab,
            lin_rgb,
            luminance,
            alpha,
        });
    }

    if valid_indices.is_empty() {
        return Err(ImagePipelineError::NoValidPixels);
    }

    Ok(PreparedImage {
        width,
        height,
        pixels,
        valid_indices,
    })
}
