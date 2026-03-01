use std::num::{NonZeroU32, NonZeroUsize};

use chromoxide_image::{
    FarthestPointLabConfig, ImagePipelineConfig, PreprocessConfig, RandomUniformConfig,
    ResizeFilter, SaliencyConfig, SaliencyMethod, SamplingConfig, SamplingMethod, StratifiedConfig,
    UniformGridConfig, prepare_support_from_image_with_rng,
};
use image::{DynamicImage, Rgba, RgbaImage};
use rand::SeedableRng;
use rand::rngs::StdRng;

fn synthetic_image() -> DynamicImage {
    let mut rgba = RgbaImage::new(80, 48);
    for y in 0..48 {
        for x in 0..80 {
            let r = (x * 3).min(255) as u8;
            let g = (y * 5).min(255) as u8;
            let b = 90u8;
            rgba.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }
    for y in 14..30 {
        for x in 30..50 {
            rgba.put_pixel(x, y, Rgba([245, 60, 40, 255]));
        }
    }
    DynamicImage::ImageRgba8(rgba)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = synthetic_image();

    let methods = vec![
        (
            "UniformGrid",
            SamplingMethod::UniformGrid(UniformGridConfig {
                step: NonZeroU32::new(6).expect("non-zero"),
            }),
        ),
        (
            "RandomUniform",
            SamplingMethod::RandomUniform(RandomUniformConfig {
                count: NonZeroUsize::new(20).expect("non-zero"),
            }),
        ),
        (
            "Stratified",
            SamplingMethod::Stratified(StratifiedConfig {
                tiles_x: NonZeroU32::new(5).expect("non-zero"),
                tiles_y: NonZeroU32::new(3).expect("non-zero"),
                per_tile: NonZeroU32::new(2).expect("non-zero"),
            }),
        ),
        (
            "FarthestPointLab",
            SamplingMethod::FarthestPointLab(FarthestPointLabConfig {
                count: NonZeroUsize::new(20).expect("non-zero"),
                candidate_stride: NonZeroU32::new(2).expect("non-zero"),
                saliency_bias: 0.5,
            }),
        ),
    ];

    for (name, method) in methods {
        let cfg = ImagePipelineConfig {
            preprocess: PreprocessConfig {
                max_working_dim: Some(NonZeroU32::new(64).expect("non-zero")),
                resize_filter: ResizeFilter::Triangle,
                background_rgb8: [255, 255, 255],
                min_alpha: 0.0,
                alpha_into_weight: false,
            },
            saliency: SaliencyConfig {
                method: SaliencyMethod::None,
            },
            sampling: SamplingConfig { method },
            export: chromoxide_image::ExportConfig::default(),
            cap: None,
        };

        let mut rng = StdRng::seed_from_u64(123);
        let support = prepare_support_from_image_with_rng(&img, &cfg, &mut rng)?;
        println!("\n== {name} ==");
        println!(
            "sample_count={} weight_sum={:.6}",
            support.samples.len(),
            support.diagnostics.weight_sum
        );
        for sample in support.samples.iter().take(5) {
            println!(
                "lab=({:.4}, {:.4}, {:.4}) weight={:.6} saliency={:.4}",
                sample.lab.l, sample.lab.a, sample.lab.b, sample.weight, sample.saliency
            );
        }
    }

    Ok(())
}
