use chromoxide::ImageCapBuilder;
use chromoxide_image::{
    CapConfig, CapSource, PreprocessConfig, ResizeFilter, SaliencyConfig, build_image_cap,
    compute_saliency, prepare_image,
};
use image::{DynamicImage, Rgba, RgbaImage};

#[test]
fn can_build_image_cap_from_prepared_pixels() {
    let mut rgba = RgbaImage::new(6, 6);
    for y in 0..6 {
        for x in 0..6 {
            let r = (x * 40) as u8;
            let g = (y * 35) as u8;
            let b = 100u8;
            rgba.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }

    let prepared = prepare_image(
        &DynamicImage::ImageRgba8(rgba),
        &PreprocessConfig {
            max_working_dim: None,
            resize_filter: ResizeFilter::Nearest,
            background_rgb8: [255, 255, 255],
            min_alpha: 0.0,
            alpha_into_weight: false,
        },
    )
    .unwrap();
    let saliency = compute_saliency(&prepared, &SaliencyConfig::default()).unwrap();

    let cap = build_image_cap(
        &prepared,
        &saliency,
        None,
        &CapConfig {
            source: CapSource::PreparedPixels,
            builder: ImageCapBuilder::default(),
        },
    )
    .unwrap();

    assert!(!cap.grid.is_empty());
    assert!(cap.max_cap() > 0.0);
    let lch = prepared.pixels[prepared.valid_indices[0]].lab.to_oklch();
    let queried = cap.query(lch.l, lch.h);
    assert!(queried.is_finite());
    assert!(queried >= 0.0);
}
