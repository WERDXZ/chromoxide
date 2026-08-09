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

## Cap handling

`chromoxide` consumes a prebuilt `ImageCap`; it never estimates image statistics
from `problem.samples`. Slots choose how to enforce that cap:

- `Ignore` - no cap lookup
- `HardIntersect` - decode chroma into `[user_min, min(user_max, cap)]`; the
  user minimum is never lowered, and infeasible domains fail validation
- `SoftPenalty { weight, huber_delta }` - decode with the user interval and add
  `weight * pseudo_huber(max(0, C - cap), huber_delta)` to the objective
- `AdaptiveSoftPenalty { weight, huber_delta }` - keep the same user interval,
  weight, and penalty shape, but use the support-confidence blend of strict
  conditional and same-lightness global caps

Relative chroma targets can reference the user domain, the decoded effective
domain, the strict conditional cap through `ImageCap`, or the confidence-aware
fallback through `AdaptiveImageCap`. Supported hues use conditional evidence;
unsupported semantic hues use the source image's global chroma profile at the
same lightness. The adaptive reference participates directly in optimization
and remains bounded by the slot's user interval and image-derived cap.

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

`solve()` remains the stochastic convenience API, while `solve_with_rng()` lets
the caller control a stochastic stream. Use `solve_with_seed()` for the stable
deterministic API: it assigns every local start its own ChaCha stream, so one
start's RNG consumption cannot perturb another start.

```rust
use chromoxide::solve_with_seed;

let solution = solve_with_seed(&problem, [42; 32])?;
# Ok::<(), chromoxide::PaletteError>(())
```

Bitwise OkLCh identity is not promised across algorithm versions. Within one
version, identical inputs and solve seed target stable final hex output.
