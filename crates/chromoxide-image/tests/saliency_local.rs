use chromoxide_image::{
    LocalContrastConfig, PreprocessConfig, ResizeFilter, SaliencyConfig, SaliencyMethod,
    compute_saliency, prepare_image,
};
use image::{DynamicImage, Rgba, RgbaImage};

fn region_mean(map: &[f64], width: u32, x0: u32, y0: u32, x1: u32, y1: u32) -> f64 {
    let w = width as usize;
    let mut sum = 0.0;
    let mut count = 0usize;
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = y as usize * w + x as usize;
            sum += map[idx];
            count += 1;
        }
    }
    sum / count as f64
}

#[test]
fn local_contrast_highlights_small_bright_patch() {
    let mut rgba = RgbaImage::new(40, 30);
    for pixel in rgba.pixels_mut() {
        *pixel = Rgba([90, 90, 90, 255]);
    }
    for y in 12..16 {
        for x in 18..22 {
            rgba.put_pixel(x, y, Rgba([245, 245, 245, 255]));
        }
    }

    let pre_cfg = PreprocessConfig {
        max_working_dim: None,
        resize_filter: ResizeFilter::Nearest,
        background_rgb8: [255, 255, 255],
        min_alpha: 0.0,
        alpha_into_weight: false,
    };
    let prepared = prepare_image(&DynamicImage::ImageRgba8(rgba), &pre_cfg).unwrap();

    let saliency = compute_saliency(
        &prepared,
        &SaliencyConfig {
            method: SaliencyMethod::LocalContrast(LocalContrastConfig {
                blur_radius: 3,
                color_weight: 0.0,
                luminance_weight: 1.0,
                global_mix: 0.0,
                robust_normalize: false,
            }),
        },
    )
    .unwrap();

    let hotspot_mean = region_mean(&saliency.values, saliency.width, 18, 12, 22, 16);
    let background_mean = region_mean(&saliency.values, saliency.width, 0, 0, 8, 8);
    assert!(hotspot_mean > background_mean + 0.25);
}
