//! Image-based chroma cap surface (`c_cap(L, h)`).

use std::f64::consts::TAU;

use crate::color::{Oklab, Oklch};
use crate::domain::{HueDomain, Interval};
use crate::error::PaletteError;
use crate::support::WeightedSample;
use crate::util::{EPS, smoothstep01, wrap_hue};

fn circular_close(a: f64, b: f64) -> bool {
    let d = (wrap_hue(a) - wrap_hue(b)).abs();
    d.min(TAU - d) <= 1.0e-10
}

/// Query-time interpolation mode for [`ImageCap`].
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CapInterpolation {
    /// Nearest-neighbor lookup (piecewise constant; non-smooth).
    Nearest,
    /// Bilinear interpolation (smooth and optimization-friendly).
    #[default]
    Bilinear,
    /// Bilinear interpolation with directional bias to prefer higher/lower local cap.
    ///
    /// `alpha` controls direction and strength:
    /// - `alpha > 0`: prefer higher corner values
    /// - `alpha < 0`: prefer lower corner values
    /// - `alpha = 0`: identical to bilinear
    ///
    /// The magnitude `|alpha|` is clamped to `[0, 1]` and passed through `curve`.
    BilinearBiased {
        /// Bias strength and direction in `[-1, 1]`.
        alpha: f64,
        /// Easing curve applied to `|alpha|`.
        curve: CapBiasCurve,
    },
}

impl CapInterpolation {
    /// Validates interpolation parameters.
    pub fn validate(self) -> Result<(), PaletteError> {
        if let Self::BilinearBiased { alpha, curve } = self {
            if !alpha.is_finite() {
                return Err(PaletteError::InvalidProblem(
                    "cap interpolation alpha must be finite".to_string(),
                ));
            }
            if alpha.abs() > 1.0 {
                return Err(PaletteError::InvalidProblem(
                    "cap interpolation alpha must be in [-1, 1]".to_string(),
                ));
            }
            curve.validate()?;
        }
        Ok(())
    }
}

/// Easing curve used by [`CapInterpolation::BilinearBiased`].
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CapBiasCurve {
    /// Linear easing.
    Linear,
    /// Smoothstep easing.
    #[default]
    Smoothstep,
    /// Cubic Bezier easing on `[0, 1]` with fixed endpoints `0` and `1`.
    ///
    /// `c1` and `c2` are y-control values and should be within `[0, 1]`.
    Bezier01 { c1: f64, c2: f64 },
}

impl CapBiasCurve {
    /// Validates Bezier control values when this curve variant is selected.
    fn validate(self) -> Result<(), PaletteError> {
        if let Self::Bezier01 { c1, c2 } = self {
            if !c1.is_finite() || !c2.is_finite() {
                return Err(PaletteError::InvalidProblem(
                    "Bezier01 controls must be finite".to_string(),
                ));
            }
            if !(0.0..=1.0).contains(&c1) || !(0.0..=1.0).contains(&c2) {
                return Err(PaletteError::InvalidProblem(
                    "Bezier01 controls must be in [0, 1]".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Applies easing to a clamped input in `[0, 1]`.
    fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::Smoothstep => smoothstep01(t),
            Self::Bezier01 { c1, c2 } => {
                let omt = 1.0 - t;
                let y = 3.0 * omt * omt * t * c1 + 3.0 * omt * t * t * c2 + t * t * t;
                y.clamp(0.0, 1.0)
            }
        }
    }
}

/// Diagnostics from building an [`ImageCap`].
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct ImageCapDiagnostics {
    /// Number of empty cells before hole filling.
    pub empty_cells: usize,
    /// Mean cap value before smoothing.
    pub mean_before_smooth: f64,
    /// Max cap value before smoothing.
    pub max_before_smooth: f64,
    /// Mean cap value after smoothing.
    pub mean_after_smooth: f64,
    /// Max cap value after smoothing.
    pub max_after_smooth: f64,
}

/// Parameters for building a statistical image cap from weighted samples.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatisticalCapConfig {
    /// Weighted percentile used per populated `(L, h)` cell.
    pub percentile: f64,
    /// Weighted chroma percentile across all hues within one lightness row,
    /// used as the low-confidence fallback profile.
    #[cfg_attr(feature = "serde", serde(default = "default_global_chroma_percentile"))]
    pub global_chroma_percentile: f64,
    /// Headroom multiplier applied after percentile estimation.
    pub tolerance_factor: f64,
    /// Blend factor in `[0, 1]` between unsmoothed and smoothed cap grids.
    pub smoothing: f64,
    /// Whether to suppress low-mass hue bins within each lightness row.
    pub use_conditional_hue: bool,
}

impl Default for StatisticalCapConfig {
    fn default() -> Self {
        Self {
            percentile: 0.95,
            global_chroma_percentile: default_global_chroma_percentile(),
            tolerance_factor: 0.12,
            smoothing: 1.0,
            use_conditional_hue: true,
        }
    }
}

fn default_global_chroma_percentile() -> f64 {
    0.90
}

impl StatisticalCapConfig {
    /// Small conditional-hue mass threshold used to suppress incidental hue bins.
    pub const CONDITIONAL_HUE_THRESHOLD: f64 = 0.02;

    /// Validates config ranges.
    pub fn validate(self) -> Result<(), PaletteError> {
        if !self.percentile.is_finite()
            || !(0.0..=1.0).contains(&self.percentile)
            || self.percentile <= 0.0
        {
            return Err(PaletteError::InvalidProblem(
                "statistical cap percentile must be finite and in (0, 1]".to_string(),
            ));
        }
        if !self.global_chroma_percentile.is_finite()
            || !(0.0..=1.0).contains(&self.global_chroma_percentile)
            || self.global_chroma_percentile <= 0.0
        {
            return Err(PaletteError::InvalidProblem(
                "statistical cap global_chroma_percentile must be finite and in (0, 1]".to_string(),
            ));
        }
        if !self.tolerance_factor.is_finite() || self.tolerance_factor < 0.0 {
            return Err(PaletteError::InvalidProblem(
                "statistical cap tolerance_factor must be finite and >= 0".to_string(),
            ));
        }
        if !self.smoothing.is_finite() || !(0.0..=1.0).contains(&self.smoothing) {
            return Err(PaletteError::InvalidProblem(
                "statistical cap smoothing must be finite and in [0, 1]".to_string(),
            ));
        }
        Ok(())
    }
}

/// Query result for an [`ImageCap`] with per-cell support confidence.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CapQuery {
    /// Interpolated cap chroma.
    pub chroma: f64,
    /// Interpolated support confidence in `[0, 1]`.
    pub confidence: f64,
}

/// Conditional and global evidence used by an adaptive image-cap query.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AdaptiveCapQuery {
    /// Chroma from the strict conditional `(L, h)` cap.
    pub conditional_chroma: f64,
    /// Chroma from the all-hue profile at the queried lightness.
    pub global_chroma: f64,
    /// Original pre-smoothing support confidence for the queried hue.
    pub support_confidence: f64,
    /// Confidence-adaptive blend of conditional and global chroma.
    pub chroma: f64,
}

/// 2D grid approximation of `c_cap(L, h)`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ImageCap {
    /// Number of lightness bins.
    pub n_l: usize,
    /// Number of hue bins.
    pub n_h: usize,
    /// Minimum L used by the cap grid.
    pub l_min: f64,
    /// Maximum L used by the cap grid.
    pub l_max: f64,
    /// Row-major cap values, length `n_l * n_h`.
    pub grid: Vec<f64>,
    /// Per-lightness global chroma profile, length `n_l` when populated.
    #[cfg_attr(feature = "serde", serde(default))]
    pub global_chroma_by_lightness: Vec<f64>,
    /// Original pre-smoothing support confidence used for adaptive fallback.
    #[cfg_attr(feature = "serde", serde(default))]
    pub support_confidence: Vec<f64>,
    /// Row-major smoothed/blended confidence used for diagnostics.
    ///
    /// Old serialized caps without `support_confidence` fall back to this
    /// field when its length matches the cap grid.
    #[cfg_attr(feature = "serde", serde(default))]
    pub confidence: Vec<f64>,
    diagnostics: ImageCapDiagnostics,
}

impl ImageCap {
    /// Returns cap value at `(L, h)` with bilinear interpolation.
    ///
    /// This is equivalent to `query_with(..., CapInterpolation::Bilinear)`.
    pub fn query(&self, l: f64, h: f64) -> f64 {
        self.query_with_confidence(l, h, CapInterpolation::default())
            .chroma
    }

    /// Returns cap value at `(L, h)` with custom interpolation mode.
    ///
    /// Notes:
    /// - `Nearest` is piecewise constant and may make finite-difference gradients noisy.
    /// - `Bilinear` is smooth enough for robust optimization in most cases.
    /// - `BilinearBiased` keeps interpolation local but nudges towards local min/max corners.
    pub fn query_with(&self, l: f64, h: f64, interpolation: CapInterpolation) -> f64 {
        self.query_with_confidence(l, h, interpolation).chroma
    }

    /// Returns cap chroma and support confidence at `(L, h)`.
    ///
    /// Confidence is interpolated with the same local neighborhood as the cap
    /// value; `BilinearBiased` uses plain bilinear confidence interpolation.
    pub fn query_with_confidence(
        &self,
        l: f64,
        h: f64,
        interpolation: CapInterpolation,
    ) -> CapQuery {
        let l_span = (self.l_max - self.l_min).max(EPS);
        let l_norm = ((l - self.l_min) / l_span).clamp(0.0, 1.0);
        let h_norm = wrap_hue(h) / TAU;

        let lf = l_norm * (self.n_l.saturating_sub(1)) as f64;
        let hf = h_norm * self.n_h as f64;

        let l0 = lf.floor() as usize;
        let l1 = (l0 + 1).min(self.n_l - 1);
        let h0 = (hf.floor() as usize) % self.n_h;
        let h1 = (h0 + 1) % self.n_h;

        let tl = lf - l0 as f64;
        let th = hf - hf.floor();

        let v00 = self.grid[self.idx(l0, h0)];
        let v01 = self.grid[self.idx(l0, h1)];
        let v10 = self.grid[self.idx(l1, h0)];
        let v11 = self.grid[self.idx(l1, h1)];
        let c00 = self.confidence_at(l0, h0);
        let c01 = self.confidence_at(l0, h1);
        let c10 = self.confidence_at(l1, h0);
        let c11 = self.confidence_at(l1, h1);

        let bilinear = {
            let v0 = v00 * (1.0 - th) + v01 * th;
            let v1 = v10 * (1.0 - th) + v11 * th;
            v0 * (1.0 - tl) + v1 * tl
        };
        let bilinear_confidence = {
            let c0 = c00 * (1.0 - th) + c01 * th;
            let c1 = c10 * (1.0 - th) + c11 * th;
            c0 * (1.0 - tl) + c1 * tl
        };

        let value = match interpolation {
            CapInterpolation::Nearest => {
                let li = if tl < 0.5 { l0 } else { l1 };
                let hi = if th < 0.5 { h0 } else { h1 };
                let confidence = self.confidence_at(li, hi);
                return CapQuery {
                    chroma: self.grid[self.idx(li, hi)].max(0.0),
                    confidence,
                };
            }
            CapInterpolation::Bilinear => bilinear,
            CapInterpolation::BilinearBiased { alpha, curve } => {
                let alpha = alpha.clamp(-1.0, 1.0);
                let amount = curve.apply(alpha.abs());
                let local_min = v00.min(v01).min(v10).min(v11);
                let local_max = v00.max(v01).max(v10).max(v11);
                if alpha >= 0.0 {
                    bilinear + amount * (local_max - bilinear)
                } else {
                    bilinear + amount * (local_min - bilinear)
                }
            }
        };

        CapQuery {
            chroma: value.max(0.0),
            confidence: bilinear_confidence.clamp(0.0, 1.0),
        }
    }

    /// Returns the all-hue chroma profile interpolated at lightness `l`.
    ///
    /// Legacy caps without an explicit profile use the maximum cap in each
    /// neighboring lightness row before interpolation.
    pub fn query_global_chroma(&self, l: f64) -> f64 {
        if self.n_l == 0 {
            return 0.0;
        }

        let profile = if self.global_chroma_by_lightness.len() == self.n_l {
            self.global_chroma_by_lightness
                .iter()
                .map(|&value| finite_nonnegative(value))
                .collect::<Vec<_>>()
        } else {
            (0..self.n_l)
                .map(|li| {
                    (0..self.n_h)
                        .filter_map(|hi| self.grid.get(li * self.n_h + hi).copied())
                        .map(finite_nonnegative)
                        .fold(0.0_f64, f64::max)
                })
                .collect::<Vec<_>>()
        };

        interpolate_lightness_profile(&profile, l, self.l_min, self.l_max)
    }

    /// Returns a confidence-adaptive blend of conditional and global cap evidence.
    pub fn query_adaptive_with(
        &self,
        l: f64,
        h: f64,
        interpolation: CapInterpolation,
    ) -> AdaptiveCapQuery {
        let local = self.query_with_confidence(l, h, interpolation);
        let global = self.query_global_chroma(l);
        let gate = smoothstep01(local.confidence / StatisticalCapConfig::CONDITIONAL_HUE_THRESHOLD);
        let adaptive = gate * local.chroma + (1.0 - gate) * global;

        AdaptiveCapQuery {
            conditional_chroma: local.chroma,
            global_chroma: global,
            support_confidence: local.confidence,
            chroma: adaptive.max(0.0),
        }
    }

    /// Conservative minimum cap over a slot's full `(L, h)` domain.
    ///
    /// Samples the domain boundaries plus every cap grid coordinate that falls
    /// inside the domain, then returns the minimum queried cap value.
    pub fn min_over_domain(
        &self,
        lightness: Interval,
        hue: HueDomain,
        interpolation: CapInterpolation,
    ) -> f64 {
        let l_span = (self.l_max - self.l_min).max(EPS);
        let mut ls = vec![lightness.min, lightness.max];
        for i in 0..self.n_l {
            let l = self.l_min + l_span * (i as f64 / (self.n_l - 1) as f64);
            if l >= lightness.min && l <= lightness.max {
                ls.push(l);
            }
        }
        ls.sort_by(f64::total_cmp);
        ls.dedup_by(|a, b| (*a - *b).abs() <= 1.0e-10);

        let mut hs: Vec<f64> = Vec::new();
        match hue {
            HueDomain::Any => {
                for i in 0..self.n_h {
                    hs.push(TAU * (i as f64 / self.n_h as f64));
                }
            }
            HueDomain::Arc { start, len } => {
                hs.push(wrap_hue(start));
                hs.push(wrap_hue(start + len));
                for i in 0..self.n_h {
                    let h = TAU * (i as f64 / self.n_h as f64);
                    if hue.contains(h) {
                        hs.push(h);
                    }
                }
            }
        }
        hs.sort_by(f64::total_cmp);
        hs.dedup_by(|a, b| circular_close(*a, *b));

        let mut min_v = f64::INFINITY;
        for &l in &ls {
            for &h in &hs {
                min_v = min_v.min(self.query_with(l, h, interpolation));
            }
        }
        min_v.max(0.0)
    }

    /// Returns builder diagnostics.
    pub fn diagnostics(&self) -> &ImageCapDiagnostics {
        &self.diagnostics
    }

    /// Maximum cap value.
    pub fn max_cap(&self) -> f64 {
        self.grid
            .iter()
            .copied()
            .fold(0.0_f64, |acc, v| if v > acc { v } else { acc })
    }

    /// Returns row-major grid index for `(l, h)`.
    fn idx(&self, l: usize, h: usize) -> usize {
        l * self.n_h + h
    }

    fn confidence_at(&self, l: usize, h: usize) -> f64 {
        if self.support_confidence.len() == self.grid.len() {
            self.support_confidence[self.idx(l, h)].clamp(0.0, 1.0)
        } else if self.confidence.len() == self.grid.len() {
            self.confidence[self.idx(l, h)].clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

/// Builder for [`ImageCap`].
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ImageCapBuilder {
    /// Number of lightness bins.
    pub n_l: usize,
    /// Number of hue bins.
    pub n_h: usize,
    /// Lightness smoothing radius.
    pub smooth_l_radius: usize,
    /// Hue smoothing radius.
    pub smooth_h_radius: usize,
    /// Global cap relaxation multiplier.
    pub relax: f64,
}

impl Default for ImageCapBuilder {
    fn default() -> Self {
        Self {
            n_l: 24,
            n_h: 72,
            smooth_l_radius: 1,
            smooth_h_radius: 2,
            relax: 1.0,
        }
    }
}

impl ImageCapBuilder {
    /// Builds an image cap from weighted samples.
    ///
    /// Construction pipeline:
    /// 1. Convert samples to OkLCh
    /// 2. Record per-cell max chroma on an `(L, h)` grid
    /// 3. Fill empty cells (hue-nearest then lightness-nearest)
    /// 4. Apply separable smoothing (circular in hue, linear in lightness)
    /// 5. Scale by `relax` and expose bilinear query interface
    ///
    /// The build is deterministic for fixed inputs.
    ///
    /// Only `sample.lab` is used for cap construction.
    pub fn build(&self, samples: &[WeightedSample]) -> Result<ImageCap, PaletteError> {
        self.build_from_oklab(|| samples.iter().map(|sample| sample.lab))
    }

    /// Builds a statistical image cap from weighted samples.
    ///
    /// The grid is populated with a weighted percentile per `(L, h)` cell instead of the
    /// absolute max chroma. Optional conditional-hue filtering blanks low-mass hue bins
    /// within each lightness row before the usual hole filling and smoothing steps.
    pub fn build_statistical(
        &self,
        samples: &[WeightedSample],
        config: StatisticalCapConfig,
    ) -> Result<ImageCap, PaletteError> {
        self.build_statistical_from_weighted_oklab(
            || samples.iter().map(|sample| (sample.lab, sample.weight)),
            config,
        )
    }

    /// Builds a statistical image cap from a weighted Oklab iterator factory.
    ///
    /// Each iterator item is `(Oklab, weight)`. `make_iter` is called twice
    /// (once for lightness bounds, once for binning), so callers can stream
    /// prepared pixels without allocating a `Vec<WeightedSample>`.
    pub fn build_statistical_from_weighted_oklab<F, I>(
        &self,
        make_iter: F,
        config: StatisticalCapConfig,
    ) -> Result<ImageCap, PaletteError>
    where
        F: Fn() -> I,
        I: Iterator<Item = (Oklab, f64)>,
    {
        config.validate()?;
        self.validate_builder()?;
        let (l_min, l_max) = weighted_lightness_bounds(&make_iter)?;
        let l_span = (l_max - l_min).max(EPS);

        let cell_count = self.n_l * self.n_h;
        let mut cells = vec![Vec::<(f64, f64)>::new(); cell_count];
        let mut row_samples = vec![Vec::<(f64, f64)>::new(); self.n_l];
        let mut hue_mass = vec![0.0; cell_count];
        let mut row_mass = vec![0.0; self.n_l];

        for (lab, weight) in make_iter() {
            let lch = Oklch::from_oklab(lab);
            let li = (((lch.l - l_min) / l_span).clamp(0.0, 1.0) * (self.n_l - 1) as f64).floor()
                as usize;
            let hi = ((wrap_hue(lch.h) / TAU) * self.n_h as f64).floor() as usize % self.n_h;
            let idx = li * self.n_h + hi;
            let weight = weight.max(0.0);
            cells[idx].push((lch.c.max(0.0), weight));
            row_samples[li].push((lch.c.max(0.0), weight));
            hue_mass[idx] += weight;
            row_mass[li] += weight;
        }

        let mut grid = vec![f64::NAN; cell_count];
        let mut confidence = vec![0.0; cell_count];
        for (li, &row_total) in row_mass.iter().enumerate() {
            for hi in 0..self.n_h {
                let idx = li * self.n_h + hi;
                if cells[idx].is_empty() {
                    continue;
                }
                let cell_confidence = if row_total > 0.0 {
                    (hue_mass[idx] / row_total).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                if config.use_conditional_hue
                    && cell_confidence < StatisticalCapConfig::CONDITIONAL_HUE_THRESHOLD
                {
                    confidence[idx] = 0.0;
                    continue;
                }
                confidence[idx] = cell_confidence;
                grid[idx] = weighted_percentile(&mut cells[idx], config.percentile);
            }
        }

        let global_raw = row_samples
            .iter_mut()
            .map(|samples| {
                if samples.is_empty() {
                    f64::NAN
                } else {
                    weighted_percentile(samples, config.global_chroma_percentile)
                }
            })
            .collect::<Vec<_>>();

        if config.use_conditional_hue {
            Ok(self.finish_conditional_grid(grid, confidence, global_raw, config, l_min, l_max))
        } else {
            Ok(self.finish_grid_with_confidence(
                grid,
                global_raw,
                config.tolerance_factor,
                config.smoothing,
                l_min,
                l_max,
            ))
        }
    }

    /// Builds an image cap from an Oklab iterator factory.
    ///
    /// `make_iter` is called twice (first for lightness range, then for binning),
    /// allowing callers to avoid allocating temporary `Vec<WeightedSample>` values.
    pub fn build_from_oklab<F, I>(&self, make_iter: F) -> Result<ImageCap, PaletteError>
    where
        F: Fn() -> I,
        I: Iterator<Item = Oklab>,
    {
        self.validate_builder()?;
        let (l_min, l_max) = lightness_bounds(&make_iter)?;

        let mut grid = vec![f64::NAN; self.n_l * self.n_h];
        let mut global_raw = vec![f64::NAN; self.n_l];
        let l_span = (l_max - l_min).max(EPS);

        for lab in make_iter() {
            let lch = Oklch::from_oklab(lab);
            let li = (((lch.l - l_min) / l_span).clamp(0.0, 1.0) * (self.n_l - 1) as f64).floor()
                as usize;
            let hi = ((wrap_hue(lch.h) / TAU) * self.n_h as f64).floor() as usize % self.n_h;
            let idx = li * self.n_h + hi;
            let c = lch.c.max(0.0);
            if grid[idx].is_nan() || c > grid[idx] {
                grid[idx] = c;
            }
            if global_raw[li].is_nan() || c > global_raw[li] {
                global_raw[li] = c;
            }
        }

        Ok(self.finish_grid_with_confidence(grid, global_raw, self.relax - 1.0, 1.0, l_min, l_max))
    }

    fn validate_builder(&self) -> Result<(), PaletteError> {
        if self.n_l < 2 || self.n_h < 2 {
            return Err(PaletteError::InvalidProblem(
                "image cap grid must be at least 2x2".to_string(),
            ));
        }
        if !self.relax.is_finite() || self.relax <= 0.0 {
            return Err(PaletteError::InvalidProblem(
                "image cap relax must be finite and > 0".to_string(),
            ));
        }
        Ok(())
    }

    fn finish_grid_with_confidence(
        &self,
        mut grid: Vec<f64>,
        global_raw: Vec<f64>,
        tolerance_factor: f64,
        smoothing: f64,
        l_min: f64,
        l_max: f64,
    ) -> ImageCap {
        let empty_cells = grid.iter().filter(|v| v.is_nan()).count();

        hue_nearest_fill(&mut grid, self.n_l, self.n_h);
        lightness_nearest_fill(&mut grid, self.n_l, self.n_h);
        for v in &mut grid {
            if v.is_nan() {
                *v = 0.0;
            }
        }

        let (mean_before_smooth, max_before_smooth) = stats(&grid);
        let mut smoothed = grid.clone();
        if self.smooth_h_radius > 0 {
            smoothed = smooth_h(&smoothed, self.n_l, self.n_h, self.smooth_h_radius);
        }
        if self.smooth_l_radius > 0 {
            smoothed = smooth_l(&smoothed, self.n_l, self.n_h, self.smooth_l_radius);
        }

        let blend = smoothing.clamp(0.0, 1.0);
        if blend < 1.0 {
            for (base, smooth) in grid.iter_mut().zip(smoothed.iter()) {
                *base = *base * (1.0 - blend) + *smooth * blend;
            }
            smoothed = grid;
        }

        let scale = (1.0 + tolerance_factor).max(0.0);
        for v in &mut smoothed {
            *v = (*v * scale).max(0.0);
        }
        let (mean_after_smooth, max_after_smooth) = stats(&smoothed);
        let global_chroma_by_lightness = self.finish_global_profile(global_raw, smoothing, scale);

        ImageCap {
            n_l: self.n_l,
            n_h: self.n_h,
            l_min,
            l_max,
            grid: smoothed,
            global_chroma_by_lightness,
            support_confidence: vec![1.0; self.n_l * self.n_h],
            confidence: vec![1.0; self.n_l * self.n_h],
            diagnostics: ImageCapDiagnostics {
                empty_cells,
                mean_before_smooth,
                max_before_smooth,
                mean_after_smooth,
                max_after_smooth,
            },
        }
    }

    fn finish_conditional_grid(
        &self,
        mut grid: Vec<f64>,
        mut confidence: Vec<f64>,
        global_raw: Vec<f64>,
        config: StatisticalCapConfig,
        l_min: f64,
        l_max: f64,
    ) -> ImageCap {
        let empty_cells = grid.iter().filter(|v| v.is_nan()).count();
        for v in &mut grid {
            if v.is_nan() {
                *v = 0.0;
            }
        }
        for v in &mut confidence {
            if !v.is_finite() {
                *v = 0.0;
            } else {
                *v = v.clamp(0.0, 1.0);
            }
        }

        // The support gate must use each cell's own pre-smoothing confidence;
        // otherwise a low-support hue could borrow full chroma from neighbors
        // through confidence-weighted smoothing.
        let base_grid = grid.clone();
        let base_confidence = confidence.clone();
        let gate_confidence = base_confidence.clone();

        let (mean_before_smooth, max_before_smooth) = stats(&base_grid);

        let mut smoothed_grid = base_grid.clone();
        let mut smoothed_confidence = base_confidence.clone();
        if self.smooth_h_radius > 0 {
            (smoothed_grid, smoothed_confidence) = smooth_h_conf_weighted(
                &smoothed_grid,
                &smoothed_confidence,
                self.n_l,
                self.n_h,
                self.smooth_h_radius,
            );
        }
        if self.smooth_l_radius > 0 {
            (smoothed_grid, smoothed_confidence) = smooth_l_conf_weighted(
                &smoothed_grid,
                &smoothed_confidence,
                self.n_l,
                self.n_h,
                self.smooth_l_radius,
            );
        }

        let blend = config.smoothing.clamp(0.0, 1.0);
        for i in 0..grid.len() {
            grid[i] = base_grid[i] * (1.0 - blend) + smoothed_grid[i] * blend;
            confidence[i] = base_confidence[i] * (1.0 - blend) + smoothed_confidence[i] * blend;
        }

        for (cap, conf) in grid.iter_mut().zip(gate_confidence.iter()) {
            let gate = smoothstep01(conf / StatisticalCapConfig::CONDITIONAL_HUE_THRESHOLD);
            *cap *= gate;
        }

        let scale = (1.0 + config.tolerance_factor).max(0.0);
        for v in &mut grid {
            *v = (*v * scale).max(0.0);
        }
        let (mean_after_smooth, max_after_smooth) = stats(&grid);
        let global_chroma_by_lightness =
            self.finish_global_profile(global_raw, config.smoothing, scale);

        ImageCap {
            n_l: self.n_l,
            n_h: self.n_h,
            l_min,
            l_max,
            grid,
            global_chroma_by_lightness,
            support_confidence: gate_confidence,
            confidence,
            diagnostics: ImageCapDiagnostics {
                empty_cells,
                mean_before_smooth,
                max_before_smooth,
                mean_after_smooth,
                max_after_smooth,
            },
        }
    }

    fn finish_global_profile(&self, mut profile: Vec<f64>, smoothing: f64, scale: f64) -> Vec<f64> {
        lightness_nearest_fill_profile(&mut profile);
        for value in &mut profile {
            *value = finite_nonnegative(*value);
        }

        let smoothed = if self.smooth_l_radius > 0 {
            smooth_l_profile(&profile, self.smooth_l_radius)
        } else {
            profile.clone()
        };
        let blend = smoothing.clamp(0.0, 1.0);
        for (base, smoothed) in profile.iter_mut().zip(smoothed) {
            *base = finite_nonnegative((*base * (1.0 - blend) + smoothed * blend) * scale);
        }
        profile
    }
}

fn lightness_bounds<F, I>(make_iter: &F) -> Result<(f64, f64), PaletteError>
where
    F: Fn() -> I,
    I: Iterator<Item = Oklab>,
{
    let mut l_min = f64::INFINITY;
    let mut l_max = f64::NEG_INFINITY;
    let mut has_any = false;
    for lab in make_iter() {
        has_any = true;
        l_min = l_min.min(lab.l);
        l_max = l_max.max(lab.l);
    }
    if !has_any {
        return Err(PaletteError::EmptySamples);
    }

    if !l_min.is_finite() || !l_max.is_finite() {
        return Err(PaletteError::NumericInstability(
            "non-finite sample lightness".to_string(),
        ));
    }
    if (l_max - l_min).abs() < 1.0e-6 {
        l_min = (l_min - 1.0e-3).max(0.0);
        l_max = (l_max + 1.0e-3).min(1.0);
    }
    Ok((l_min, l_max))
}

fn weighted_lightness_bounds<F, I>(make_iter: &F) -> Result<(f64, f64), PaletteError>
where
    F: Fn() -> I,
    I: Iterator<Item = (Oklab, f64)>,
{
    let mut l_min = f64::INFINITY;
    let mut l_max = f64::NEG_INFINITY;
    let mut has_any = false;
    for (lab, _) in make_iter() {
        has_any = true;
        l_min = l_min.min(lab.l);
        l_max = l_max.max(lab.l);
    }
    if !has_any {
        return Err(PaletteError::EmptySamples);
    }

    if !l_min.is_finite() || !l_max.is_finite() {
        return Err(PaletteError::NumericInstability(
            "non-finite sample lightness".to_string(),
        ));
    }
    if (l_max - l_min).abs() < 1.0e-6 {
        l_min = (l_min - 1.0e-3).max(0.0);
        l_max = (l_max + 1.0e-3).min(1.0);
    }
    Ok((l_min, l_max))
}

fn weighted_percentile(samples: &mut [(f64, f64)], percentile: f64) -> f64 {
    samples.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total_weight: f64 = samples.iter().map(|(_, weight)| *weight).sum();
    if total_weight <= EPS {
        return samples.last().map_or(0.0, |(value, _)| *value);
    }

    let target = percentile * total_weight;
    let mut accum = 0.0;
    for (value, weight) in samples.iter().copied() {
        accum += weight;
        if accum + EPS >= target {
            return value;
        }
    }
    samples.last().map_or(0.0, |(value, _)| *value)
}

fn finite_nonnegative(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn interpolate_lightness_profile(profile: &[f64], l: f64, l_min: f64, l_max: f64) -> f64 {
    if profile.is_empty() {
        return 0.0;
    }
    if profile.len() == 1 {
        return finite_nonnegative(profile[0]);
    }

    let l_span = (l_max - l_min).max(EPS);
    let l_norm = ((l - l_min) / l_span).clamp(0.0, 1.0);
    let lf = l_norm * (profile.len() - 1) as f64;
    let l0 = lf.floor() as usize;
    let l1 = (l0 + 1).min(profile.len() - 1);
    let t = lf - l0 as f64;
    finite_nonnegative(
        finite_nonnegative(profile[l0]) * (1.0 - t) + finite_nonnegative(profile[l1]) * t,
    )
}

fn lightness_nearest_fill_profile(profile: &mut [f64]) {
    let valid_indices = profile
        .iter()
        .enumerate()
        .filter_map(|(index, value)| value.is_finite().then_some(index))
        .collect::<Vec<_>>();
    if valid_indices.is_empty() {
        profile.fill(0.0);
        return;
    }

    for index in 0..profile.len() {
        if profile[index].is_finite() {
            continue;
        }
        let mut nearest = valid_indices[0];
        let mut nearest_distance = nearest.abs_diff(index);
        for &candidate in &valid_indices[1..] {
            let distance = candidate.abs_diff(index);
            if distance < nearest_distance {
                nearest = candidate;
                nearest_distance = distance;
            }
        }
        profile[index] = profile[nearest];
    }
}

fn smooth_l_profile(profile: &[f64], radius: usize) -> Vec<f64> {
    let mut smoothed = vec![0.0; profile.len()];
    let width = 2 * radius + 1;
    for (index, value) in smoothed.iter_mut().enumerate() {
        let mut sum = 0.0;
        for offset in 0..width {
            let delta = offset as isize - radius as isize;
            let neighbor = (index as isize + delta).clamp(0, profile.len() as isize - 1) as usize;
            sum += profile[neighbor];
        }
        *value = sum / width as f64;
    }
    smoothed
}

/// Returns `(mean, max)` summary for a cap grid.
fn stats(grid: &[f64]) -> (f64, f64) {
    if grid.is_empty() {
        return (0.0, 0.0);
    }
    let mut sum = 0.0;
    let mut max_v = f64::NEG_INFINITY;
    for &v in grid {
        sum += v;
        if v > max_v {
            max_v = v;
        }
    }
    (sum / grid.len() as f64, max_v.max(0.0))
}

/// Fills NaN cells by nearest neighbors along circular hue rows.
fn hue_nearest_fill(grid: &mut [f64], n_l: usize, n_h: usize) {
    for l in 0..n_l {
        let row_start = l * n_h;
        let row_end = row_start + n_h;
        let row = &grid[row_start..row_end];
        if row.iter().all(|v| v.is_nan()) {
            continue;
        }

        let mut out = row.to_vec();
        for h in 0..n_h {
            if !row[h].is_nan() {
                continue;
            }
            let mut found = None;
            for d in 1..=n_h {
                let left = (h + n_h - (d % n_h)) % n_h;
                let right = (h + d) % n_h;
                let left_valid = !row[left].is_nan();
                let right_valid = !row[right].is_nan();
                if left_valid || right_valid {
                    found = Some(match (left_valid, right_valid) {
                        (true, true) => 0.5 * (row[left] + row[right]),
                        (true, false) => row[left],
                        (false, true) => row[right],
                        (false, false) => unreachable!(),
                    });
                    break;
                }
            }
            out[h] = found.unwrap_or(0.0);
        }
        grid[row_start..row_end].copy_from_slice(&out);
    }
}

/// Fills remaining NaN cells by nearest neighbors along lightness columns.
fn lightness_nearest_fill(grid: &mut [f64], n_l: usize, n_h: usize) {
    for h in 0..n_h {
        let mut col = vec![f64::NAN; n_l];
        for l in 0..n_l {
            col[l] = grid[l * n_h + h];
        }
        if col.iter().all(|v| v.is_nan()) {
            continue;
        }

        let filled_indices: Vec<usize> = col
            .iter()
            .enumerate()
            .filter_map(|(idx, v)| if v.is_nan() { None } else { Some(idx) })
            .collect();

        for l in 0..n_l {
            if !col[l].is_nan() {
                continue;
            }
            let mut nearest = filled_indices[0];
            let mut nearest_dist = nearest.abs_diff(l);
            for &idx in &filled_indices[1..] {
                let d = idx.abs_diff(l);
                if d < nearest_dist {
                    nearest = idx;
                    nearest_dist = d;
                }
            }
            col[l] = col[nearest];
        }

        for l in 0..n_l {
            grid[l * n_h + h] = col[l];
        }
    }
}

/// Box-smooths cap grid along circular hue axis.
fn smooth_h(grid: &[f64], n_l: usize, n_h: usize, radius: usize) -> Vec<f64> {
    let mut out = vec![0.0; grid.len()];
    let width = 2 * radius + 1;
    for l in 0..n_l {
        for h in 0..n_h {
            let mut sum = 0.0;
            for d in 0..width {
                let ofs = d as isize - radius as isize;
                let hh = ((h as isize + ofs).rem_euclid(n_h as isize)) as usize;
                sum += grid[l * n_h + hh];
            }
            out[l * n_h + h] = sum / width as f64;
        }
    }
    out
}

/// Box-smooths cap grid along clamped lightness axis.
fn smooth_l(grid: &[f64], n_l: usize, n_h: usize, radius: usize) -> Vec<f64> {
    let mut out = vec![0.0; grid.len()];
    let width = 2 * radius + 1;
    for l in 0..n_l {
        for h in 0..n_h {
            let mut sum = 0.0;
            for d in 0..width {
                let ofs = d as isize - radius as isize;
                let ll = (l as isize + ofs).clamp(0, n_l as isize - 1) as usize;
                sum += grid[ll * n_h + h];
            }
            out[l * n_h + h] = sum / width as f64;
        }
    }
    out
}

/// Confidence-weighted smoothing along circular hue.
///
/// Cap values are averaged with neighbor confidence as weight; confidence is
/// averaged with an ordinary box mean.
fn smooth_h_conf_weighted(
    grid: &[f64],
    confidence: &[f64],
    n_l: usize,
    n_h: usize,
    radius: usize,
) -> (Vec<f64>, Vec<f64>) {
    let mut out_cap = vec![0.0; grid.len()];
    let mut out_conf = vec![0.0; grid.len()];
    let width = 2 * radius + 1;
    for l in 0..n_l {
        for h in 0..n_h {
            let mut numerator = 0.0;
            let mut denominator = 0.0;
            let mut conf_sum = 0.0;
            for d in 0..width {
                let ofs = d as isize - radius as isize;
                let hh = ((h as isize + ofs).rem_euclid(n_h as isize)) as usize;
                let idx = l * n_h + hh;
                let conf = confidence[idx].max(0.0);
                numerator += grid[idx] * conf;
                denominator += conf;
                conf_sum += conf;
            }
            out_cap[l * n_h + h] = if denominator > EPS {
                numerator / denominator
            } else {
                0.0
            };
            out_conf[l * n_h + h] = (conf_sum / width as f64).clamp(0.0, 1.0);
        }
    }
    (out_cap, out_conf)
}

/// Confidence-weighted smoothing along clamped lightness.
fn smooth_l_conf_weighted(
    grid: &[f64],
    confidence: &[f64],
    n_l: usize,
    n_h: usize,
    radius: usize,
) -> (Vec<f64>, Vec<f64>) {
    let mut out_cap = vec![0.0; grid.len()];
    let mut out_conf = vec![0.0; grid.len()];
    let width = 2 * radius + 1;
    for l in 0..n_l {
        for h in 0..n_h {
            let mut numerator = 0.0;
            let mut denominator = 0.0;
            let mut conf_sum = 0.0;
            for d in 0..width {
                let ofs = d as isize - radius as isize;
                let ll = (l as isize + ofs).clamp(0, n_l as isize - 1) as usize;
                let idx = ll * n_h + h;
                let conf = confidence[idx].max(0.0);
                numerator += grid[idx] * conf;
                denominator += conf;
                conf_sum += conf;
            }
            out_cap[l * n_h + h] = if denominator > EPS {
                numerator / denominator
            } else {
                0.0
            };
            out_conf[l * n_h + h] = (conf_sum / width as f64).clamp(0.0, 1.0);
        }
    }
    (out_cap, out_conf)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use crate::cap::{
        CapInterpolation, ImageCap, ImageCapBuilder, ImageCapDiagnostics, StatisticalCapConfig,
    };
    use crate::color::Oklch;

    fn statistical_profile(pixels: &[(crate::color::Oklab, f64)], percentile: f64) -> ImageCap {
        ImageCapBuilder {
            n_l: 2,
            n_h: 8,
            smooth_l_radius: 0,
            smooth_h_radius: 0,
            relax: 1.0,
        }
        .build_statistical_from_weighted_oklab(
            || pixels.iter().copied(),
            StatisticalCapConfig {
                percentile: 1.0,
                global_chroma_percentile: percentile,
                tolerance_factor: 0.0,
                smoothing: 0.0,
                use_conditional_hue: false,
            },
        )
        .unwrap()
    }

    fn direct_cap(
        conditional_chroma: f64,
        global_chroma: f64,
        support_confidence: f64,
    ) -> ImageCap {
        ImageCap {
            n_l: 2,
            n_h: 2,
            l_min: 0.0,
            l_max: 1.0,
            grid: vec![conditional_chroma; 4],
            global_chroma_by_lightness: vec![global_chroma; 2],
            support_confidence: vec![support_confidence; 4],
            confidence: vec![0.75; 4],
            diagnostics: ImageCapDiagnostics::default(),
        }
    }

    #[test]
    fn global_chroma_percentile_validation_enforces_finite_open_closed_range() {
        let mut config = StatisticalCapConfig {
            percentile: 0.95,
            global_chroma_percentile: 0.90,
            tolerance_factor: 0.12,
            smoothing: 1.0,
            use_conditional_hue: true,
        };

        for invalid in [f64::NAN, f64::NEG_INFINITY, 0.0, -0.1, 1.01] {
            config.global_chroma_percentile = invalid;
            assert!(
                config.validate().is_err(),
                "accepted invalid value {invalid}"
            );
        }
        for valid in [f64::MIN_POSITIVE, 0.90, 1.0] {
            config.global_chroma_percentile = valid;
            assert!(config.validate().is_ok(), "rejected valid value {valid}");
        }
    }

    fn supported_two_hue_cap(smoothing: f64) -> crate::cap::ImageCap {
        let mut pixels = Vec::new();
        for _ in 0..50 {
            pixels.push((
                Oklch {
                    l: 0.5,
                    c: 0.05,
                    h: 0.0,
                }
                .to_oklab(),
                1.0,
            ));
            pixels.push((
                Oklch {
                    l: 0.5,
                    c: 0.20,
                    h: TAU / 16.0,
                }
                .to_oklab(),
                1.0,
            ));
        }
        ImageCapBuilder {
            n_l: 2,
            n_h: 16,
            smooth_l_radius: 0,
            smooth_h_radius: 2,
            relax: 1.0,
        }
        .build_statistical_from_weighted_oklab(
            || pixels.iter().copied(),
            StatisticalCapConfig {
                percentile: 1.0,
                global_chroma_percentile: 0.90,
                tolerance_factor: 0.0,
                smoothing,
                use_conditional_hue: true,
            },
        )
        .unwrap()
    }

    #[test]
    fn conditional_smoothing_zero_preserves_supported_cell_caps() {
        let cap = supported_two_hue_cap(0.0);
        let cap_a = cap.query_with(cap.l_min, 0.0, CapInterpolation::Nearest);
        let cap_b = cap.query_with(cap.l_min, TAU / 16.0, CapInterpolation::Nearest);
        assert!((cap_a - 0.05).abs() < 1.0e-6);
        assert!((cap_b - 0.20).abs() < 1.0e-6);
    }

    #[test]
    fn conditional_smoothing_one_blends_supported_cell_caps() {
        let base = supported_two_hue_cap(0.0);
        let smoothed = supported_two_hue_cap(1.0);
        let base_a = base.query_with(base.l_min, 0.0, CapInterpolation::Nearest);
        let base_b = base.query_with(base.l_min, TAU / 16.0, CapInterpolation::Nearest);
        let smooth_a = smoothed.query_with(smoothed.l_min, 0.0, CapInterpolation::Nearest);
        let smooth_b = smoothed.query_with(smoothed.l_min, TAU / 16.0, CapInterpolation::Nearest);

        assert!(smooth_a > base_a);
        assert!(smooth_b < base_b);
        assert!((smooth_b - smooth_a).abs() < (base_b - base_a).abs());
    }

    #[test]
    fn global_profile_uses_weighted_row_percentile() {
        let pixels = [
            (
                Oklch {
                    l: 0.4,
                    c: 0.03,
                    h: 0.0,
                }
                .to_oklab(),
                9.0,
            ),
            (
                Oklch {
                    l: 0.4,
                    c: 0.18,
                    h: 1.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.6,
                    c: 0.10,
                    h: 0.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.6,
                    c: 0.20,
                    h: 1.0,
                }
                .to_oklab(),
                9.0,
            ),
        ];

        let cap = statistical_profile(&pixels, 0.90);

        assert_eq!(cap.global_chroma_by_lightness.len(), 2);
        assert!((cap.global_chroma_by_lightness[0] - 0.03).abs() < 1.0e-9);
        assert!((cap.global_chroma_by_lightness[1] - 0.20).abs() < 1.0e-9);
    }

    #[test]
    fn global_profile_is_low_for_muted_image() {
        let pixels = vec![
            (
                Oklch {
                    l: 0.4,
                    c: 0.03,
                    h: 0.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.4,
                    c: 0.05,
                    h: 1.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.6,
                    c: 0.04,
                    h: 0.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.6,
                    c: 0.05,
                    h: 1.0,
                }
                .to_oklab(),
                1.0,
            ),
        ];

        let cap = statistical_profile(&pixels, 0.90);

        assert!(
            cap.global_chroma_by_lightness
                .iter()
                .all(|&c| { (0.049..=0.051).contains(&c) })
        );
    }

    #[test]
    fn global_profile_is_high_for_vivid_image() {
        let muted = vec![
            (
                Oklch {
                    l: 0.4,
                    c: 0.03,
                    h: 0.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.4,
                    c: 0.05,
                    h: 1.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.6,
                    c: 0.04,
                    h: 0.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.6,
                    c: 0.05,
                    h: 1.0,
                }
                .to_oklab(),
                1.0,
            ),
        ];
        let vivid = vec![
            (
                Oklch {
                    l: 0.4,
                    c: 0.14,
                    h: 0.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.4,
                    c: 0.18,
                    h: 1.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.6,
                    c: 0.15,
                    h: 0.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.6,
                    c: 0.18,
                    h: 1.0,
                }
                .to_oklab(),
                1.0,
            ),
        ];
        let muted_cap = statistical_profile(&muted, 0.90);
        let vivid_cap = statistical_profile(&vivid, 0.90);

        for (&muted_chroma, &vivid_chroma) in muted_cap
            .global_chroma_by_lightness
            .iter()
            .zip(&vivid_cap.global_chroma_by_lightness)
        {
            assert!((0.179..=0.181).contains(&vivid_chroma));
            assert!(vivid_chroma > muted_chroma + 0.08);
        }
    }

    #[test]
    fn max_observed_global_profile_uses_row_max_and_relax() {
        let pixels = [
            Oklch {
                l: 0.4,
                c: 0.03,
                h: 0.0,
            }
            .to_oklab(),
            Oklch {
                l: 0.4,
                c: 0.12,
                h: 1.0,
            }
            .to_oklab(),
            Oklch {
                l: 0.6,
                c: 0.08,
                h: 0.0,
            }
            .to_oklab(),
            Oklch {
                l: 0.6,
                c: 0.20,
                h: 1.0,
            }
            .to_oklab(),
        ];
        let cap = ImageCapBuilder {
            n_l: 2,
            n_h: 8,
            smooth_l_radius: 0,
            smooth_h_radius: 0,
            relax: 1.25,
        }
        .build_from_oklab(|| pixels.iter().copied())
        .unwrap();

        assert_eq!(cap.global_chroma_by_lightness.len(), 2);
        assert!((cap.global_chroma_by_lightness[0] - 0.15).abs() < 1.0e-9);
        assert!((cap.global_chroma_by_lightness[1] - 0.25).abs() < 1.0e-9);
        assert!(
            cap.support_confidence
                .iter()
                .all(|&confidence| confidence == 1.0)
        );
    }

    #[test]
    fn adaptive_cap_equals_conditional_for_supported_hue() {
        let cap = direct_cap(0.05, 0.18, 1.0);

        let query = cap.query_adaptive_with(0.5, 0.0, CapInterpolation::Nearest);

        assert_eq!(query.conditional_chroma, 0.05);
        assert_eq!(query.global_chroma, 0.18);
        assert_eq!(query.support_confidence, 1.0);
        assert_eq!(query.chroma, 0.05);
    }

    #[test]
    fn adaptive_cap_equals_global_for_unsupported_hue() {
        let cap = direct_cap(0.0, 0.18, 0.0);

        let query = cap.query_adaptive_with(0.5, 0.0, CapInterpolation::Nearest);

        assert_eq!(query.conditional_chroma, 0.0);
        assert_eq!(query.global_chroma, 0.18);
        assert_eq!(query.support_confidence, 0.0);
        assert_eq!(query.chroma, 0.18);
    }

    #[test]
    fn adaptive_cap_smoothly_blends_low_confidence_hue() {
        let cap = direct_cap(
            0.04,
            0.16,
            StatisticalCapConfig::CONDITIONAL_HUE_THRESHOLD / 2.0,
        );

        let query = cap.query_adaptive_with(0.5, 0.0, CapInterpolation::Nearest);

        assert_eq!(query.conditional_chroma, 0.04);
        assert_eq!(query.global_chroma, 0.16);
        assert_eq!(query.support_confidence, 0.01);
        assert!((query.chroma - 0.10).abs() < 1.0e-12);
    }

    #[test]
    fn support_confidence_is_pre_smoothing() {
        let pixels = [
            (
                Oklch {
                    l: 0.4,
                    c: 0.20,
                    h: 0.0,
                }
                .to_oklab(),
                1.0,
            ),
            (
                Oklch {
                    l: 0.6,
                    c: 0.20,
                    h: 0.0,
                }
                .to_oklab(),
                1.0,
            ),
        ];
        let cap = ImageCapBuilder {
            n_l: 2,
            n_h: 16,
            smooth_l_radius: 0,
            smooth_h_radius: 1,
            relax: 1.0,
        }
        .build_statistical_from_weighted_oklab(
            || pixels.iter().copied(),
            StatisticalCapConfig {
                percentile: 1.0,
                global_chroma_percentile: 0.90,
                tolerance_factor: 0.0,
                smoothing: 1.0,
                use_conditional_hue: true,
            },
        )
        .unwrap();
        let neighbor = 1;

        assert_eq!(cap.support_confidence[neighbor], 0.0);
        assert!(cap.confidence[neighbor] > 0.0);
        let query = cap.query_with_confidence(cap.l_min, TAU / 16.0, CapInterpolation::Nearest);
        assert_eq!(query.confidence, 0.0);
    }

    #[test]
    fn old_cap_without_global_profile_has_finite_fallback() {
        let cap = ImageCap {
            n_l: 2,
            n_h: 2,
            l_min: 0.0,
            l_max: 1.0,
            grid: vec![0.04, 0.08, 0.12, 0.16],
            global_chroma_by_lightness: Vec::new(),
            support_confidence: Vec::new(),
            confidence: vec![0.0; 4],
            diagnostics: ImageCapDiagnostics::default(),
        };

        let global = cap.query_global_chroma(0.5);
        let adaptive = cap.query_adaptive_with(0.5, 0.0, CapInterpolation::Bilinear);

        assert!(global.is_finite());
        assert!((global - 0.12).abs() < 1.0e-12);
        assert_eq!(adaptive.support_confidence, 0.0);
        assert!((adaptive.chroma - 0.12).abs() < 1.0e-12);
    }

    #[test]
    fn old_cap_without_any_confidence_defaults_to_full_support() {
        let cap = ImageCap {
            n_l: 2,
            n_h: 2,
            l_min: 0.0,
            l_max: 1.0,
            grid: vec![0.08; 4],
            global_chroma_by_lightness: Vec::new(),
            support_confidence: Vec::new(),
            confidence: Vec::new(),
            diagnostics: ImageCapDiagnostics::default(),
        };

        let query = cap.query_adaptive_with(0.5, 0.0, CapInterpolation::Nearest);

        assert_eq!(query.support_confidence, 1.0);
        assert_eq!(query.conditional_chroma, 0.08);
        assert_eq!(query.chroma, 0.08);
    }
}
