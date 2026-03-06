//! Chromoxide: a constraint-driven palette optimizer.
//!
//! `chromoxide` solves for a palette of slot colors under:
//! - slot-wise hard domains (`SlotDomain`)
//! - image evidence terms (`Cover`, `Support`, `Saliency`)
//! - unary target terms (`LightnessTarget`, `ChromaTarget`, `HueTarget`)
//! - pairwise terms (`DeltaL`, `DeltaC`, `DeltaH`, `Distance`, `Order`, `Contrast`)
//! - group terms (`GroupQuantile`)
//!
//! Optimization is continuous in Oklab/OkLCh using multi-start L-BFGS.
//!
//! # Units and conventions
//!
//! - All numeric values are `f64`
//! - Hue values and hue differences are in radians
//! - `HueDomain::Arc` is represented as `Arc { start, len }` (counter-clockwise)
//! - Distances are computed in Oklab
//! - Domain decoding is performed in OkLCh
//!
//! # Typical workflow
//!
//! 1. Prepare weighted support samples (`WeightedSample`)
//! 2. Define palette slots (`SlotSpec`) and per-slot domains (`SlotDomain`)
//! 3. Add weighted objective terms (`WeightedTerm`)
//! 4. Configure solver settings (`SolveConfig`)
//! 5. Call [`solve`] and inspect diagnostics (`PaletteSolution`)
//!
//! # Minimal example
//!
//! ```
//! use chromoxide::*;
//!
//! let samples = vec![
//!     WeightedSample::new(Oklch { l: 0.35, c: 0.12, h: 0.2 }.to_oklab(), 2.0, 0.5),
//!     WeightedSample::new(Oklch { l: 0.75, c: 0.10, h: 2.8 }.to_oklab(), 2.0, 0.8),
//! ];
//!
//! let slots = vec![
//!     SlotSpec {
//!         name: "a".into(),
//!         domain: SlotDomain {
//!             lightness: Interval { min: 0.2, max: 0.9 },
//!             chroma: Interval { min: 0.0, max: 0.2 },
//!             hue: HueDomain::Any,
//!             cap_policy: CapPolicy::Ignore,
//!             chroma_epsilon: 0.02,
//!         },
//!     },
//!     SlotSpec {
//!         name: "b".into(),
//!         domain: SlotDomain {
//!             lightness: Interval { min: 0.2, max: 0.9 },
//!             chroma: Interval { min: 0.0, max: 0.2 },
//!             hue: HueDomain::Any,
//!             cap_policy: CapPolicy::Ignore,
//!             chroma_epsilon: 0.02,
//!         },
//!     },
//! ];
//!
//! let problem = PaletteProblem {
//!     slots,
//!     samples,
//!     image_cap: None,
//!     terms: vec![WeightedTerm {
//!         weight: 3.0,
//!         name: Some("cover".into()),
//!         term: Term::Cover(CoverTerm {
//!             slots: vec![0, 1],
//!             tau: 0.02,
//!             delta: 0.03,
//!         }),
//!     }],
//!     config: SolveConfig::default(),
//! };
//!
//! let solution = solve(&problem)?;
//! assert!(solution.objective.is_finite());
//! # Ok::<(), PaletteError>(())
//! ```

pub mod cap;
pub mod color;
pub mod convert;
pub mod decode;
pub mod diagnostics;
pub mod domain;
pub mod error;
pub mod objective;
pub mod problem;
pub mod seed;
pub mod solver;
pub mod support;
pub mod term;
pub mod terms;
pub mod util;

pub use cap::{CapBiasCurve, CapInterpolation, ImageCap, ImageCapBuilder, ImageCapDiagnostics};
pub use color::{Oklab, Oklch};
pub use diagnostics::{PaletteSolution, SlotDiagnostics, SolverDiagnostics, TermBreakdown};
pub use domain::{CapPolicy, HueDomain, Interval, SlotDomain};
pub use error::PaletteError;
pub use problem::{GradientMode, PaletteProblem, SlotSpec, SolveConfig};
pub use solver::{solve, solve_with_rng};
pub use support::WeightedSample;
pub use term::{
    ChromaTargetTerm, ContrastTerm, CoverTerm, DeltaCTarget, DeltaHTarget, DeltaLTarget,
    GroupAxis, GroupMember, GroupQuantileTerm, GroupTarget, HueTargetTerm, HueUnaryTarget,
    LightnessTargetTerm, Monotonicity, OrderRelation, PairDeltaCTerm, PairDeltaHTerm,
    PairDeltaLTerm, PairDistanceTerm, PairOrderTerm, QuantileKnot, SaliencyTarget,
    SaliencyTerm, ScalarTarget, SupportTerm, Term, WeightedTerm,
};
