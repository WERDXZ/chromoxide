//! Term definitions and evaluation dispatch.
//!
//! All hue values and hue deltas are represented in radians.

use crate::color::{Oklab, Oklch};
use crate::support::WeightedSample;

/// Evaluation context shared across all terms.
///
/// The context is precomputed once per objective evaluation to avoid repeated
/// conversions and redundant calculations inside each term.
pub struct EvalContext<'a> {
    /// Slot colors in Oklab.
    pub slots_lab: &'a [Oklab],
    /// Slot colors in OkLCh.
    pub slots_lch: &'a [Oklch],
    /// Slot relative luminance values.
    pub luminance: &'a [f64],
    /// Hue reliability gates per slot.
    pub hue_gates: &'a [f64],
    /// Support samples.
    pub samples: &'a [WeightedSample],
}

impl EvalContext<'_> {
    /// Combined hue gate for a slot pair.
    pub fn pair_hue_gate(&self, i: usize, j: usize) -> f64 {
        self.hue_gates[i] * self.hue_gates[j]
    }
}

/// Per-term evaluation output.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct TermEvaluation {
    /// Unweighted term value.
    pub raw: f64,
    /// Optional components for diagnostics.
    pub components: Vec<f64>,
}

/// Cover term.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct CoverTerm {
    /// Slots participating in coverage.
    pub slots: Vec<usize>,
    /// Softmin temperature.
    ///
    /// Lower values make the term behave closer to a hard nearest-slot distance,
    /// which strengthens local matching but also makes the landscape sharper.
    /// Higher values smooth across more slots and reduce local sensitivity.
    pub tau: f64,
    /// Pseudo-Huber delta.
    ///
    /// Lower values penalize residuals more like absolute error.
    /// Higher values smooth the penalty and reduce outlier influence.
    pub delta: f64,
}

/// Support prior term.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct SupportTerm {
    /// Slots participating in support prior.
    pub slots: Vec<usize>,
    /// Softmin temperature.
    ///
    /// Lower values emphasize the single best matching sample for each slot.
    /// Higher values spread influence across more nearby samples.
    pub tau: f64,
    /// Weight prior strength.
    ///
    /// Higher values bias slots more strongly toward high-weight samples.
    /// Lower values reduce this prior and rely more on geometric proximity.
    pub beta: f64,
    /// Log epsilon.
    ///
    /// Stabilizes `ln(weight + epsilon)`.
    /// Larger values flatten differences between low-weight samples.
    pub epsilon: f64,
}

/// Saliency target shape.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum SaliencyTarget {
    /// Minimum saliency.
    Min(f64),
    /// Maximum saliency.
    Max(f64),
    /// Saliency range.
    Range { min: f64, max: f64 },
    /// Target saliency with pseudo-Huber delta.
    Target { value: f64, delta: f64 },
}

/// Saliency term.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct SaliencyTerm {
    /// Target slot.
    pub slot: usize,
    /// RBF sigma in Oklab distance.
    ///
    /// Smaller values make saliency estimation local and selective.
    /// Larger values smooth saliency over wider color neighborhoods.
    pub sigma: f64,
    /// Target type.
    pub target: SaliencyTarget,
    /// Optional pseudo-Huber delta used only for hinge-style targets.
    ///
    /// This affects `Min`, `Max`, and `Range`, but does not affect
    /// `Target { value, delta }`.
    pub hinge_delta: Option<f64>,
}

/// Reusable scalar target shape for unary and distance terms.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum ScalarTarget {
    /// Minimum allowed value.
    Min(f64),
    /// Maximum allowed value.
    Max(f64),
    /// Inclusive target range.
    Range { min: f64, max: f64 },
    /// Point target with pseudo-Huber delta.
    ///
    /// Unlike hinge-style targets, this uses the embedded `delta` directly.
    Target { value: f64, delta: f64 },
}

/// Delta-L target.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum DeltaLTarget {
    /// Minimum difference.
    Min(f64),
    /// Maximum difference.
    Max(f64),
    /// Range target.
    Range { min: f64, max: f64 },
    /// Point target with delta.
    Target { value: f64, delta: f64 },
}

/// Delta-C target.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum DeltaCTarget {
    /// Minimum difference.
    Min(f64),
    /// Maximum difference.
    Max(f64),
    /// Range target.
    Range { min: f64, max: f64 },
    /// Point target with delta.
    Target { value: f64, delta: f64 },
}

/// Delta-h target.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum DeltaHTarget {
    /// Minimum hue difference in radians.
    Min(f64),
    /// Maximum hue difference in radians.
    Max(f64),
    /// Inclusive hue-difference range in radians.
    Range { min: f64, max: f64 },
    /// Target hue difference in radians with pseudo-Huber delta.
    Target { value: f64, delta: f64 },
}

impl DeltaHTarget {
    /// Constructs `Min` with angle in radians.
    pub fn min_rad(value_rad: f64) -> Self {
        Self::Min(value_rad)
    }

    /// Constructs `Max` with angle in radians.
    pub fn max_rad(value_rad: f64) -> Self {
        Self::Max(value_rad)
    }

    /// Constructs `Range` with angles in radians.
    pub fn range_rad(min_rad: f64, max_rad: f64) -> Self {
        Self::Range {
            min: min_rad,
            max: max_rad,
        }
    }

    /// Constructs `Target` with angle/delta in radians.
    pub fn target_rad(value_rad: f64, delta_rad: f64) -> Self {
        Self::Target {
            value: value_rad,
            delta: delta_rad,
        }
    }
}

/// Pairwise lightness difference term.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PairDeltaLTerm {
    /// Slot A.
    pub a: usize,
    /// Slot B.
    pub b: usize,
    /// Target on absolute difference.
    pub target: DeltaLTarget,
    /// Optional pseudo-Huber delta used only for hinge-style targets.
    ///
    /// This affects `Min`, `Max`, and `Range`, but does not affect
    /// `Target { value, delta }`.
    pub hinge_delta: Option<f64>,
}

/// Pairwise chroma difference term.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PairDeltaCTerm {
    /// Slot A.
    pub a: usize,
    /// Slot B.
    pub b: usize,
    /// Target on absolute difference.
    pub target: DeltaCTarget,
    /// Optional pseudo-Huber delta used only for hinge-style targets.
    ///
    /// This affects `Min`, `Max`, and `Range`, but does not affect
    /// `Target { value, delta }`.
    pub hinge_delta: Option<f64>,
}

/// Pairwise hue difference term.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PairDeltaHTerm {
    /// Slot A.
    pub a: usize,
    /// Slot B.
    pub b: usize,
    /// Target on circular hue difference (radians).
    pub target: DeltaHTarget,
    /// Optional pseudo-Huber delta used only for hinge-style targets.
    ///
    /// This affects `Min`, `Max`, and `Range`, but does not affect
    /// `Target { value, delta }`.
    pub hinge_delta: Option<f64>,
}

/// Absolute lightness preference for a single slot.
///
/// This is useful for soft anchors such as keeping a background dark,
/// keeping text light, or nudging a slot toward a preferred lightness band.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct LightnessTargetTerm {
    /// Slot index.
    pub slot: usize,
    /// Scalar target applied to `slots_lch[slot].l`.
    pub target: ScalarTarget,
    /// Optional pseudo-Huber delta used only for hinge-style targets.
    ///
    /// This affects `Min`, `Max`, and `Range`, but does not affect
    /// `Target { value, delta }`.
    pub hinge_delta: Option<f64>,
}

/// Absolute chroma preference for a single slot.
///
/// This is useful for soft neutral anchors, accent minimum-chroma preferences,
/// or keeping a slot inside a preferred chroma band.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ChromaTargetTerm {
    /// Slot index.
    pub slot: usize,
    /// Scalar target applied to `slots_lch[slot].c`.
    pub target: ScalarTarget,
    /// Optional pseudo-Huber delta used only for hinge-style targets.
    ///
    /// This affects `Min`, `Max`, and `Range`, but does not affect
    /// `Target { value, delta }`.
    pub hinge_delta: Option<f64>,
}

/// Unary hue target for a single slot.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum HueUnaryTarget {
    /// Soft preference for a single absolute hue center in radians.
    Target { center: f64, delta: f64 },
    /// Soft preference for staying anywhere on a counter-clockwise arc.
    ArcPreference { start: f64, end: f64, delta: f64 },
}

/// Absolute hue preference for a single slot.
///
/// This term applies a unary preference to one slot's absolute hue. Unlike
/// [`PairDeltaHTerm`], it does not describe a relation between two slots.
/// Use it when a slot should stay near a specific hue or within a preferred hue
/// arc, while still remaining soft and optimizer-friendly.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct HueTargetTerm {
    /// Slot index.
    pub slot: usize,
    /// Unary hue target definition.
    pub target: HueUnaryTarget,
    /// Whether to multiply the penalty by this slot's hue gate.
    pub use_hue_gate: bool,
}

/// Pairwise Oklab distance preference between two slots.
///
/// This term constrains full geometric distance in Oklab, rather than only one
/// axis. Unlike [`PairDeltaLTerm`], [`PairDeltaCTerm`], or [`PairDeltaHTerm`],
/// it measures the combined Euclidean separation between two colors.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PairDistanceTerm {
    /// Slot A.
    pub a: usize,
    /// Slot B.
    pub b: usize,
    /// Target applied to Oklab distance or squared distance.
    pub target: ScalarTarget,
    /// Whether to operate on squared Oklab distance.
    pub squared: bool,
    /// Optional pseudo-Huber delta used only for hinge-style targets.
    ///
    /// This affects `Min`, `Max`, and `Range`, but does not affect
    /// `Target { value, delta }`.
    pub hinge_delta: Option<f64>,
}

/// Pairwise lightness order relation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum OrderRelation {
    /// A brighter than B by at least delta.
    BrighterBy { delta: f64 },
    /// A darker than B by at least delta.
    DarkerBy { delta: f64 },
}

/// Pairwise order term.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PairOrderTerm {
    /// Slot A.
    pub a: usize,
    /// Slot B.
    pub b: usize,
    /// Order relation.
    pub relation: OrderRelation,
    /// Optional pseudo-Huber delta used for the hinge-style order penalty.
    pub hinge_delta: Option<f64>,
}

/// Contrast term (foreground/background).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ContrastTerm {
    /// Foreground slot.
    pub fg: usize,
    /// Background slot.
    pub bg: usize,
    /// Minimum contrast ratio.
    pub min_ratio: f64,
    /// Optional pseudo-Huber delta used for the hinge-style contrast penalty.
    pub hinge_delta: Option<f64>,
}

/// Group axis.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum GroupAxis {
    /// Lightness axis.
    Lightness,
    /// Chroma axis.
    Chroma,
    /// Hue projected onto fixed arc, specified by start/end angles in radians.
    ///
    /// The arc is interpreted as the counter-clockwise span from `start` to `end`.
    HueArc { start: f64, end: f64 },
}

/// Group target values.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum GroupTarget {
    /// Uniform range target.
    UniformRange { min: f64, max: f64 },
    /// Explicit per-slot values.
    ExplicitValues(Vec<f64>),
    /// Explicit quantile/value mapping.
    ///
    /// Knots are interpreted in order of `quantile` and linearly interpolated.
    ExplicitQuantiles(Vec<QuantileKnot>),
}

/// Single control knot for a piecewise-linear quantile target.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug)]
pub struct QuantileKnot {
    /// Quantile position in `[0, 1]`.
    pub quantile: f64,
    /// Target value at this quantile.
    pub value: f64,
}

/// Optional monotonicity on ordered group slots.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum Monotonicity {
    /// Increasing with minimum adjacent gap.
    ///
    /// Higher `min_gap` enforces wider separation between adjacent group values.
    Increasing { min_gap: f64 },
    /// Decreasing with minimum adjacent gap.
    ///
    /// Higher `min_gap` enforces wider separation between adjacent group values.
    Decreasing { min_gap: f64 },
}

/// Slot/mass pair for group quantile terms.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug)]
pub struct GroupMember {
    /// Slot index in palette order.
    pub slot: usize,
    /// Relative mass for quantile-center computation.
    ///
    /// Higher mass gives this slot a larger share of the quantile axis.
    pub mass: f64,
}

/// Group mass-quantile term.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct GroupQuantileTerm {
    /// Ordered `(slot, mass)` entries.
    pub members: Vec<GroupMember>,
    /// Target axis.
    pub axis: GroupAxis,
    /// Target mapping.
    pub target: GroupTarget,
    /// Optional monotonicity penalty.
    pub monotonic: Option<Monotonicity>,
    /// Pseudo-Huber delta.
    ///
    /// Lower values penalize residuals more aggressively near target.
    /// Higher values smooth the residual penalty and tolerate deviations more.
    pub huber_delta: f64,
}

/// Any objective term.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum Term {
    /// Cover term.
    Cover(CoverTerm),
    /// Support term.
    Support(SupportTerm),
    /// Saliency term.
    Saliency(SaliencyTerm),
    /// Unary lightness target term.
    LightnessTarget(LightnessTargetTerm),
    /// Unary chroma target term.
    ChromaTarget(ChromaTargetTerm),
    /// Unary hue target term.
    HueTarget(HueTargetTerm),
    /// Pair delta-L term.
    DeltaL(PairDeltaLTerm),
    /// Pair delta-C term.
    DeltaC(PairDeltaCTerm),
    /// Pair delta-h term.
    DeltaH(PairDeltaHTerm),
    /// Pair Oklab distance term.
    Distance(PairDistanceTerm),
    /// Pair order term.
    Order(PairOrderTerm),
    /// Contrast term.
    Contrast(ContrastTerm),
    /// Group quantile term.
    GroupQuantile(GroupQuantileTerm),
}

impl Term {
    /// Evaluates this term in the provided context.
    pub fn evaluate(&self, ctx: &EvalContext<'_>) -> TermEvaluation {
        match self {
            Term::Cover(t) => crate::terms::cover::evaluate(t, ctx),
            Term::Support(t) => crate::terms::support::evaluate(t, ctx),
            Term::Saliency(t) => crate::terms::saliency::evaluate(t, ctx),
            Term::LightnessTarget(t) => crate::terms::lightness_target::evaluate(t, ctx),
            Term::ChromaTarget(t) => crate::terms::chroma_target::evaluate(t, ctx),
            Term::HueTarget(t) => crate::terms::hue_target::evaluate(t, ctx),
            Term::DeltaL(t) => crate::terms::pair_delta::evaluate_delta_l(t, ctx),
            Term::DeltaC(t) => crate::terms::pair_delta::evaluate_delta_c(t, ctx),
            Term::DeltaH(t) => crate::terms::pair_delta::evaluate_delta_h(t, ctx),
            Term::Distance(t) => crate::terms::pair_distance::evaluate(t, ctx),
            Term::Order(t) => crate::terms::pair_order::evaluate(t, ctx),
            Term::Contrast(t) => crate::terms::contrast::evaluate(t, ctx),
            Term::GroupQuantile(t) => crate::terms::group_quantile::evaluate(t, ctx),
        }
    }

    /// A short default display name.
    pub fn default_name(&self) -> &'static str {
        match self {
            Term::Cover(_) => "cover",
            Term::Support(_) => "support",
            Term::Saliency(_) => "saliency",
            Term::LightnessTarget(_) => "lightness_target",
            Term::ChromaTarget(_) => "chroma_target",
            Term::HueTarget(_) => "hue_target",
            Term::DeltaL(_) => "delta_l",
            Term::DeltaC(_) => "delta_c",
            Term::DeltaH(_) => "delta_h",
            Term::Distance(_) => "distance",
            Term::Order(_) => "order",
            Term::Contrast(_) => "contrast",
            Term::GroupQuantile(_) => "group_quantile",
        }
    }
}

/// Weighted term entry in a problem.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct WeightedTerm {
    /// Term weight.
    ///
    /// The contribution added to objective is `weight * term_raw_value`.
    /// Increasing this value makes the optimizer prioritize this term more strongly
    /// relative to other terms.
    pub weight: f64,
    /// Optional term name for diagnostics.
    pub name: Option<String>,
    /// Term payload.
    pub term: Term,
}
