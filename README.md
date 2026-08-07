# Chromoxide

A constraint‑driven palette optimizer for Rust.

Chromoxide solves for optimal color palettes given image evidence, slot‑wise hard domains, and pairwise constraints. It uses continuous optimization in Oklab/OkLCh color space with multi‑start L‑BFGS.

> **Note:** This project is in early development. APIs may change.

## Crates

- **`chromoxide`** – Core optimization engine, domain definitions, and solver.
- **`chromoxide‑image`** – Image preprocessing, saliency detection, sampling, and support extraction.
- **`chrox`** – CLI palette generation tool built on top of `chromoxide` and `chromoxide-image`.

## Cap estimation vs. enforcement

Cap construction and cap enforcement are separate:

- `chromoxide-image::CapEstimator` decides how a cap surface is built from image
  evidence (`MaxObserved` or `Statistical`). The default builds a statistical
  cap from all prepared pixels, not from the exported 24 samples.
- `CapPolicy` decides how a slot enforces the already-built cap
  (`Ignore`, `HardIntersect`, or `SoftPenalty`).

`HardIntersect` never lowers a slot's user `chroma.min`; problems whose required
minimum exceeds the cap over the whole slot domain are rejected during
validation.

## Usage

Add to your `Cargo.toml` (replace the git URL with your own):

```toml
[dependencies]
chromoxide = { git = "https://github.com/werdxz/chromoxide" }
chromoxide-image = { git = "https://github.com/werdxz/chromoxide" }
```

Install the CLI with:

```bash
cargo install --git https://github.com/werdxz/chromoxide chrox
```

Basic example using pre‑computed samples:

```rust
use chromoxide::*;

let samples = vec![
    WeightedSample::new(Oklch { l: 0.35, c: 0.12, h: 0.2 }.to_oklab(), 2.0, 0.5),
    WeightedSample::new(Oklch { l: 0.75, c: 0.10, h: 2.8 }.to_oklab(), 2.0, 0.8),
];

let slots = vec![
    SlotSpec {
        name: "a".into(),
        domain: SlotDomain {
            lightness: Interval { min: 0.2, max: 0.9 },
            chroma: Interval { min: 0.0, max: 0.2 },
            hue: HueDomain::Any,
            cap_policy: CapPolicy::Ignore,
            chroma_epsilon: 0.02,
        },
    },
    SlotSpec {
        name: "b".into(),
        domain: SlotDomain {
            lightness: Interval { min: 0.2, max: 0.9 },
            chroma: Interval { min: 0.0, max: 0.2 },
            hue: HueDomain::Any,
            cap_policy: CapPolicy::Ignore,
            chroma_epsilon: 0.02,
        },
    },
];

let problem = PaletteProblem {
    slots,
    samples,
    image_cap: None,
    terms: vec![WeightedTerm {
        weight: 3.0,
        name: Some("cover".into()),
        term: Term::Cover(CoverTerm {
            slots: vec![0, 1],
            tau: 0.02,
            delta: 0.03,
        }),
    }],
    config: SolveConfig::default(),
};

let solution = solve(&problem)?;
```

For a full image‑based pipeline, see the examples in `chromoxide‑image`.

For CLI-driven palette generation and template rendering, see `crates/chrox/README.md`.

## Examples

Run the workspace examples with:

```bash
cargo run --example neutral_ladder --release
cargo run --example basic_pipeline --release
```

## Documentation

Build local documentation:

```bash
cargo doc --workspace --open
```

For project terminology, see [VOCABULARY.md](VOCABULARY.md).
For pipeline and parameter explanations, see [ALGORITHMS.md](ALGORITHMS.md).

## License

This project is licensed under the MIT License – see the [LICENSE](LICENSE) file for details.
