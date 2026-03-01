# chromoxide

`chromoxide` is a constraint-driven palette optimizer in Rust.

It optimizes slot colors in continuous `Oklab/OkLCh` space using:

- weighted color support samples
- slot domains (lightness/chroma/hue + cap policy)
- image evidence terms
- pairwise terms
- group mass-quantile terms
- multi-start + L-BFGS (argmin)

The crate focuses on optimization core behavior and diagnostics.

## Quick start

```rust
use chromoxide::*;

let samples = vec![
    WeightedSample::new(Oklch { l: 0.4, c: 0.12, h: 0.3 }.to_oklab(), 3.0, 0.5),
    WeightedSample::new(Oklch { l: 0.7, c: 0.10, h: 2.8 }.to_oklab(), 3.0, 0.8),
];

let slots = vec![
    SlotSpec {
        name: "a".into(),
        domain: SlotDomain {
            lightness: Interval { min: 0.2, max: 0.9 },
            chroma: Interval { min: 0.0, max: 0.22 },
            hue: HueDomain::Any,
            cap_policy: CapPolicy::Ignore,
            chroma_epsilon: 0.02,
        },
    },
    SlotSpec {
        name: "b".into(),
        domain: SlotDomain {
            lightness: Interval { min: 0.2, max: 0.9 },
            chroma: Interval { min: 0.0, max: 0.22 },
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
println!("objective = {}", solution.objective);
```

## Running

```bash
cargo check -p chromoxide
cargo test -p chromoxide
cargo run -p chromoxide --example two_cluster
cargo run -p chromoxide --example neutral_ladder
cargo run -p chromoxide --example synthetic_gradient
```

## Reproducibility

Pass a seeded RNG to `solve_with_rng` when you need reproducible runs.

```rust
use chromoxide::solve_with_rng;
use rand::rngs::StdRng;
use rand::SeedableRng;

let mut rng = StdRng::seed_from_u64(42);
let solution = solve_with_rng(&problem, &mut rng)?;
# Ok::<(), chromoxide::PaletteError>(())
```
