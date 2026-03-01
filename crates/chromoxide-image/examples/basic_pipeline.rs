use std::num::{NonZeroU32, NonZeroUsize};

use chromoxide::ImageCapBuilder;
use chromoxide_image::{
    CapConfig, CapSource, FarthestPointLabConfig, ImagePipelineConfig, LocalContrastConfig,
    PreprocessConfig, ResizeFilter, SaliencyConfig, SaliencyMethod, SamplingConfig, SamplingMethod,
    prepare_support_from_image_with_rng,
};
use image::{DynamicImage, Rgba, RgbaImage};
use rand::SeedableRng;
use rand::rngs::StdRng;

fn synthetic_image() -> DynamicImage {
    let mut rgba = RgbaImage::new(64, 64);
    for y in 0..64 {
        for x in 0..64 {
            let r = (x * 4).min(255) as u8;
            let g = (y * 4).min(255) as u8;
            rgba.put_pixel(x, y, Rgba([r, g, 110, 255]));
        }
    }
    for y in 20..34 {
        for x in 24..40 {
            rgba.put_pixel(x, y, Rgba([240, 40, 40, 255]));
        }
    }
    DynamicImage::ImageRgba8(rgba)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = synthetic_image();
    let cfg = ImagePipelineConfig {
        preprocess: PreprocessConfig {
            max_working_dim: Some(NonZeroU32::new(48).expect("non-zero")),
            resize_filter: ResizeFilter::Triangle,
            background_rgb8: [255, 255, 255],
            min_alpha: 0.0,
            alpha_into_weight: false,
        },
        saliency: SaliencyConfig {
            method: SaliencyMethod::LocalContrast(LocalContrastConfig {
                blur_radius: 3,
                color_weight: 1.0,
                luminance_weight: 0.8,
                global_mix: 0.2,
                robust_normalize: true,
            }),
        },
        sampling: SamplingConfig {
            method: SamplingMethod::FarthestPointLab(FarthestPointLabConfig {
                count: NonZeroUsize::new(12).expect("non-zero"),
                candidate_stride: NonZeroU32::new(2).expect("non-zero"),
                saliency_bias: 0.4,
            }),
        },
        export: chromoxide_image::ExportConfig::default(),
        cap: Some(CapConfig {
            source: CapSource::PreparedPixels,
            builder: ImageCapBuilder::default(),
        }),
    };

    let mut rng = StdRng::seed_from_u64(42);
    let support = prepare_support_from_image_with_rng(&img, &cfg, &mut rng)?;
    println!("samples: {}", support.samples.len());
    for (i, sample) in support.samples.iter().take(8).enumerate() {
        println!(
            "[{i}] lab=({:.4}, {:.4}, {:.4}) weight={:.6} saliency={:.4}",
            sample.lab.l, sample.lab.a, sample.lab.b, sample.weight, sample.saliency
        );
    }
    println!("diagnostics: {:?}", support.diagnostics);
    println!("has image_cap: {}", support.image_cap.is_some());

    Ok(())
}
