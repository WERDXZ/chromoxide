use std::num::NonZeroU32;
use std::path::Path;

use chromoxide_image::{
    LocalContrastConfig, PreprocessConfig, ResizeFilter, SaliencyConfig, SaliencyMethod,
    compute_saliency, prepare_image, saliency_to_luma_image,
};
use image::{DynamicImage, Rgba, RgbaImage};

fn synthetic_image() -> DynamicImage {
    let mut rgba = RgbaImage::new(96, 64);
    for y in 0..64 {
        for x in 0..96 {
            rgba.put_pixel(x, y, Rgba([100, 110, 120, 255]));
        }
    }
    for y in 22..38 {
        for x in 40..56 {
            rgba.put_pixel(x, y, Rgba([250, 250, 250, 255]));
        }
    }
    DynamicImage::ImageRgba8(rgba)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = synthetic_image();
    let prepared = prepare_image(
        &img,
        &PreprocessConfig {
            max_working_dim: Some(NonZeroU32::new(96).expect("non-zero")),
            resize_filter: ResizeFilter::Triangle,
            background_rgb8: [255, 255, 255],
            min_alpha: 0.0,
            alpha_into_weight: false,
        },
    )?;

    let saliency = compute_saliency(
        &prepared,
        &SaliencyConfig {
            method: SaliencyMethod::LocalContrast(LocalContrastConfig {
                blur_radius: 4,
                color_weight: 0.6,
                luminance_weight: 1.0,
                global_mix: 0.2,
                robust_normalize: true,
            }),
        },
    )?;

    let out = saliency_to_luma_image(&saliency);
    std::fs::create_dir_all("target")?;
    let out_path = Path::new("target/saliency_demo.png");
    out.save(out_path)?;

    let min = saliency
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, |acc, v| acc.min(v));
    let max = saliency
        .values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, |acc, v| acc.max(v));
    let mean = saliency.values.iter().sum::<f64>() / saliency.values.len() as f64;

    println!("saliency map saved to: {}", out_path.display());
    println!("size: {}x{}", saliency.width, saliency.height);
    println!("stats: min={:.4}, max={:.4}, mean={:.4}", min, max, mean);

    Ok(())
}
