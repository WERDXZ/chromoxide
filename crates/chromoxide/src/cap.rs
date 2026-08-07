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
            tolerance_factor: 0.12,
            smoothing: 1.0,
            use_conditional_hue: true,
        }
    }
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
    /// Row-major per-cell support confidence, length `n_l * n_h` when populated.
    ///
    /// Old serialized caps without this field deserialize as empty, in which
    /// case queries report confidence `1.0`.
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
        if self.confidence.len() == self.grid.len() {
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

        if config.use_conditional_hue {
            Ok(self.finish_conditional_grid(grid, confidence, config, l_min, l_max))
        } else {
            Ok(self.finish_grid_with_confidence(
                grid,
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
        }

        Ok(self.finish_grid_with_confidence(grid, self.relax - 1.0, 1.0, l_min, l_max))
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

        ImageCap {
            n_l: self.n_l,
            n_h: self.n_h,
            l_min,
            l_max,
            grid: smoothed,
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
        let gate_confidence = confidence.clone();

        let (mean_before_smooth, max_before_smooth) = stats(&grid);

        if self.smooth_h_radius > 0 {
            (grid, confidence) = smooth_h_conf_weighted(
                &grid,
                &confidence,
                self.n_l,
                self.n_h,
                self.smooth_h_radius,
            );
        }
        if self.smooth_l_radius > 0 {
            (grid, confidence) = smooth_l_conf_weighted(
                &grid,
                &confidence,
                self.n_l,
                self.n_h,
                self.smooth_l_radius,
            );
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

        ImageCap {
            n_l: self.n_l,
            n_h: self.n_h,
            l_min,
            l_max,
            grid,
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
