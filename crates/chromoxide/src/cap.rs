//! Image-based chroma cap surface (`c_cap(L, h)`).

use std::f64::consts::TAU;

use crate::color::{Oklab, Oklch};
use crate::error::PaletteError;
use crate::support::WeightedSample;
use crate::util::{EPS, smoothstep01, wrap_hue};

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
    diagnostics: ImageCapDiagnostics,
}

impl ImageCap {
    /// Returns cap value at `(L, h)` with bilinear interpolation.
    ///
    /// This is equivalent to `query_with(..., CapInterpolation::Bilinear)`.
    pub fn query(&self, l: f64, h: f64) -> f64 {
        self.query_with(l, h, CapInterpolation::default())
    }

    /// Returns cap value at `(L, h)` with custom interpolation mode.
    ///
    /// Notes:
    /// - `Nearest` is piecewise constant and may make finite-difference gradients noisy.
    /// - `Bilinear` is smooth enough for robust optimization in most cases.
    /// - `BilinearBiased` keeps interpolation local but nudges towards local min/max corners.
    pub fn query_with(&self, l: f64, h: f64, interpolation: CapInterpolation) -> f64 {
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

        let bilinear = {
            let v0 = v00 * (1.0 - th) + v01 * th;
            let v1 = v10 * (1.0 - th) + v11 * th;
            v0 * (1.0 - tl) + v1 * tl
        };

        let value = match interpolation {
            CapInterpolation::Nearest => {
                let li = if tl < 0.5 { l0 } else { l1 };
                let hi = if th < 0.5 { h0 } else { h1 };
                self.grid[self.idx(li, hi)]
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

        value.max(0.0)
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

    /// Builds an image cap from an Oklab iterator factory.
    ///
    /// `make_iter` is called twice (first for lightness range, then for binning),
    /// allowing callers to avoid allocating temporary `Vec<WeightedSample>` values.
    pub fn build_from_oklab<F, I>(&self, make_iter: F) -> Result<ImageCap, PaletteError>
    where
        F: Fn() -> I,
        I: Iterator<Item = Oklab>,
    {
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

        let empty_cells = grid.iter().filter(|v| v.is_nan()).count();

        hue_nearest_fill(&mut grid, self.n_l, self.n_h);
        lightness_nearest_fill(&mut grid, self.n_l, self.n_h);
        for v in &mut grid {
            if v.is_nan() {
                *v = 0.0;
            }
        }

        let (mean_before_smooth, max_before_smooth) = stats(&grid);
        let mut smoothed = grid;
        if self.smooth_h_radius > 0 {
            smoothed = smooth_h(&smoothed, self.n_l, self.n_h, self.smooth_h_radius);
        }
        if self.smooth_l_radius > 0 {
            smoothed = smooth_l(&smoothed, self.n_l, self.n_h, self.smooth_l_radius);
        }

        for v in &mut smoothed {
            *v = (*v * self.relax).max(0.0);
        }
        let (mean_after_smooth, max_after_smooth) = stats(&smoothed);

        Ok(ImageCap {
            n_l: self.n_l,
            n_h: self.n_h,
            l_min,
            l_max,
            grid: smoothed,
            diagnostics: ImageCapDiagnostics {
                empty_cells,
                mean_before_smooth,
                max_before_smooth,
                mean_after_smooth,
                max_after_smooth,
            },
        })
    }
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
