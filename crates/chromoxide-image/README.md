# chromoxide-image

`chromoxide-image` is the image entry companion crate for `chromoxide`.

It provides the utilities that convert an input image into:

- `Vec<chromoxide::WeightedSample>`
- `Option<chromoxide::ImageCap>`
- detailed preprocessing / saliency / sampling diagnostics

Pipeline:

`Image -> PreparedImage -> SaliencyMap -> Representatives -> WeightedSample / ImageCap`

## Scope

This crate handles image I/O and support extraction. It does **not** implement:

- palette optimization / solver logic
- downstream theme/template generation
- ML saliency / segmentation / superpixel / GPU

## Features

- preprocess: resize, alpha filtering, linear-sRGB compositing, Oklab conversion
- saliency: `None`, `GlobalContrast`, `LocalContrast`
- representative sampling:
  - `UniformGrid`
  - `Stratified`
  - `RandomUniform`
  - `FarthestPointLab`
- assignment/export to `WeightedSample`
- optional `ImageCap` build via `chromoxide::ImageCapBuilder`
- diagnostics and saliency debug rendering (`saliency_to_luma_image`)

## Quick start

```rust
use chromoxide_image::{
    ImagePipelineConfig, PreprocessConfig, ResizeFilter, SaliencyConfig, SaliencyMethod,
    SamplingConfig, SamplingMethod, UniformGridConfig, prepare_support_from_image_with_rng,
};
use image::{DynamicImage, Rgba, RgbaImage};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::num::NonZeroU32;

let mut rgba = RgbaImage::new(32, 32);
for pixel in rgba.pixels_mut() {
    *pixel = Rgba([120, 140, 180, 255]);
}

let cfg = ImagePipelineConfig {
    preprocess: PreprocessConfig {
        max_working_dim: None,
        resize_filter: ResizeFilter::Triangle,
        background_rgb8: [255, 255, 255],
        min_alpha: 0.0,
        alpha_into_weight: false,
    },
    saliency: SaliencyConfig {
        method: SaliencyMethod::None,
    },
    sampling: SamplingConfig {
        method: SamplingMethod::UniformGrid(UniformGridConfig {
            step: NonZeroU32::new(4).expect("non-zero"),
        }),
    },
    export: Default::default(),
    cap: None,
};

let mut rng = StdRng::seed_from_u64(42);
let support = prepare_support_from_image_with_rng(&DynamicImage::ImageRgba8(rgba), &cfg, &mut rng)?;
println!("samples = {}", support.samples.len());
# Ok::<(), chromoxide_image::ImagePipelineError>(())
```

## Run

```bash
cargo check -p chromoxide-image
cargo test -p chromoxide-image
cargo run -p chromoxide-image --example basic_pipeline
cargo run -p chromoxide-image --example compare_sampling
cargo run -p chromoxide-image --example saliency_demo
```
