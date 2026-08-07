# Algorithms

This file explains the algorithmic pieces in the workspace.

## 1. Solve pipeline

At a high level, palette solving works like this:

1. Build a `PaletteProblem`
   - slots
   - terms
   - weighted samples
   - optional image cap
   - solve config
2. Generate multiple initial seeds
3. Run local optimization from each seed
4. Keep the best solution by objective value
5. Export solved slot colors into final palette members

This is a continuous, multi-start optimization workflow.

References:

- L-BFGS: <https://en.wikipedia.org/wiki/Limited-memory_BFGS>

## 2. Why multi-start matters

The objective usually has multiple good local optima.

Examples:

- two salient colors may swap places
- a cover color may drift among several similar dark neutrals
- two color families may compete with similar scores

Multi-start helps by trying several initial guesses and keeping the best one found.

Important parameters:

- `seed_count`
  - number of starting guesses
  - higher values usually improve stability and quality, but cost more time
- `keep_top_k`
  - how many of the best seed runs are kept in diagnostics

## 3. Local optimization

After seeding, each run uses local optimization in continuous color space.

Important parameters:

- `max_iters`
  - iteration cap per seed
- `fd_epsilon`
  - step size for finite-difference gradients
- `gradient_mode`
  - finite-difference style used to estimate gradients
- `convergence_ftol`
  - stop when objective improvement becomes very small
- `convergence_gtol`
  - stop when gradient norm becomes very small

References:

- Finite difference: <https://en.wikipedia.org/wiki/Finite_difference>

## 4. Objective design

The total objective is a weighted sum of terms.

Each term encodes one preference, for example:

- fit broad image coverage
- prefer salient regions
- keep a neutral ladder ordered
- separate two accents in hue
- enforce readable foreground/background contrast

Term shapes often use smooth robust penalties instead of hard cliffs.

Common term parameters:

- `weight`
  - how strongly the term influences the solve
- `target`
  - desired value, minimum, maximum, or range
- `hinge_delta`
  - smoothing width for hinge-style penalties
- `delta`
  - robust tolerance for target-style penalties
- `tau`
  - softmin temperature in terms like `Cover` and `Support`
- `beta`
  - support-weight influence strength in `Support`
- `sigma`
  - locality scale in `Saliency`
- `min_ratio`
  - required contrast ratio in `Contrast`
- `min_gap`
  - required spacing in monotonic group/order constraints
- `mass`
  - relative importance of members in `GroupQuantile`

### 4.1 Cover uses soft-assignment expected distance

`Cover` first converts each sample's squared Oklab distance to the participating
slots into a soft-assignment expected value:

```text
v_min = min(d0², d1², ...)
weight_i = exp(-(d_i² - v_min) / tau)
d_soft = sum(weight_i * d_i²) / sum(weight_i)
```

The `v_min` subtraction keeps the exponentials stable. The result is a convex
combination of the squared distances, so it is `0` when every participating slot
coincides with the sample and always lies between the minimum and maximum
squared distance. That non-negative value is then fed through pseudo-Huber with
`delta`.

This is deliberately different from a free-energy softmin: `softmin([0, 0])`
returns `-tau * ln(2)`, which would not be usable as a non-negative distance.

### 4.2 Support keeps free-energy softmin

`Support` scores each slot against all samples using prior-adjusted values
`d² - beta * ln(weight + epsilon)` and still uses the ordinary softmin

```text
-tau * ln(sum(exp(-score_i / tau)))
```

because those scores may legitimately be negative after the weight prior is
subtracted. `Cover` and `Support` therefore use different aggregation formulas.

### 4.3 Saliency: conditional, support density, gate, effective

At a query color, saliency is estimated with mass-weighted RBF regression:

```text
kernel_i = exp(-d_i² / (2 * sigma²))
weighted_kernel_i = max(weight_i, 0) * kernel_i
conditional = clamp(sum(weighted_kernel_i * clamp(saliency_i, 0, 1))
                    / sum(weighted_kernel_i), 0, 1)
normalized_density = clamp(sum(weighted_kernel_i) / total_mass, 0, 1)
gate = clamp(1 - exp(-normalized_density / support_scale), 0, 1)
effective = clamp(conditional * gate, 0, 1)
```

`total_mass` is `sum(max(weight_i, 0))`. `SaliencyTarget` values are evaluated
against `effective`; the diagnostics components are emitted in the fixed order
`[effective, conditional, normalized_density, gate]`.

### 4.4 Relative chroma target

`RelativeChromaTarget` normalizes a slot's chroma against its effective decode
interval:

```text
span = hi - lo
ratio = 1                          if span <= EPS
      = clamp((C - lo) / span, 0, 1) otherwise
```

The scalar target is applied to `ratio` (not to absolute chroma), so
`Target { value: 0.9, ... }` means "90% of the slot's feasible chroma interval"
instead of an impossible absolute target like `C = 1.0`.

The reference interval is explicit:

- `UserDomain` - `[user chroma min, user chroma max]`
- `EffectiveDecodeDomain` - the decoded effective interval (default; for
  `HardIntersect` this is the cap-intersected interval, otherwise the user
  interval)
- `ImageCap` - `[user min, max(user min, min(user max, image_cap(L, h)))]`

`ImageCap` requires `problem.image_cap`; validation rejects the problem
otherwise instead of silently falling back.

References:

- Huber loss: <https://en.wikipedia.org/wiki/Huber_loss>
- Smooth maximum / softmax family: <https://en.wikipedia.org/wiki/Smooth_maximum>

## 5. Domain and cap handling

Each slot has a domain:

- lightness interval
- chroma interval
- hue domain
- cap policy

The image cap is an image-derived chroma boundary used to keep solutions plausible for the source image.

Important parameters:

- `chroma_epsilon`
  - below this, hue-sensitive behavior is softened
- `cap_policy`
  - how a slot enforces an already-built cap: `Ignore`, `HardIntersect`, or
    `SoftPenalty { weight, huber_delta }`
- `cap_interpolation`
  - how cap values are queried from the cap surface

`HardIntersect` decodes chroma into
`[user_min, min(user_max, image_cap(L, h))]` and never lowers `user_min`; a slot
whose required minimum exceeds the minimum cap over its whole domain is
rejected during `PaletteProblem::validate` (`min_over_domain`). `SoftPenalty`
keeps the user chroma interval for decoding and adds
`weight * pseudo_huber(max(0, C - cap), huber_delta)` to the objective.

How the cap surface is constructed is a separate concern handled by
`chromoxide-image::CapEstimator` (see 6.5); `CapPolicy` never estimates
statistics from samples.

## 6. Image pipeline

The image pipeline produces weighted evidence for the solver.

Stages:

1. preprocess image
2. compute saliency
3. select representatives
4. export representatives/clusters to `WeightedSample`
5. optionally build image cap

### 6.1 Preprocess

Important parameters:

- `max_working_dim`
  - longest-side downscale limit
- `resize_filter`
  - resize kernel
- `background_rgb8`
  - compositing color for alpha
- `min_alpha`
  - alpha threshold for valid pixels
- `alpha_into_weight`
  - whether alpha affects sample mass

### 6.2 Saliency

Saliency estimates which regions are visually prominent.

Important parameters:

- `blur_radius`
  - neighborhood size for local contrast
- `color_weight`
  - chromatic contrast contribution
- `luminance_weight`
  - brightness contrast contribution
- `global_mix`
  - how much global contrast is mixed in
- `robust_normalize`
  - use percentile normalization instead of raw min/max

Reference:

- Salience: <https://en.wikipedia.org/wiki/Salience_(neuroscience)>

### 6.3 Sampling

Sampling chooses representative candidates from the image.

Available methods:

- `UniformGrid` - grid-aligned selection in image space
- `Stratified` - random per-tile selection
- `RandomUniform` - uniform random selection from valid pixels
- `FarthestPointLab` - greedy farthest-point selection in Oklab
- `KMeansPlusPlusLab` - weighted k-means++ seeding plus Lloyd refinement in Oklab

Important parameters:

- `count`
  - desired number of representatives
- `candidate_stride`
  - candidate downsampling stride for farthest-point / k-means++ seeding pools
- `saliency_bias`
  - how much salient regions are preferred during selection
- `max_iters`
  - Lloyd iteration cap
- `convergence_tol`
  - stop when the maximum Oklab center movement is below this value

References:

- Farthest-first traversal: <https://en.wikipedia.org/wiki/Farthest-first_traversal>
- k-means++: <https://en.wikipedia.org/wiki/K-means%2B%2B>
- Lloyd's algorithm: <https://en.wikipedia.org/wiki/Lloyd%27s_algorithm>
- Stratified sampling: <https://en.wikipedia.org/wiki/Stratified_sampling>

### 6.3.1 KMeansPlusPlusLab

The k-means++ method seeds clusters from a candidate pool:

1. Start with `valid_indices` stepped by `candidate_stride`; if the pool is
   smaller than the target cluster count, pad it with remaining valid pixels in
   index order until it has at least `min(count, valid_pixel_count)` entries.
2. Assign each candidate a base mass
   `alpha * (1 + saliency_bias * clamp(saliency, 0, 1))`.
3. Draw the first center proportional to base mass.
4. For each subsequent center, draw proportional to
   `base_mass * min_distance2_to_existing_centers`, without replacement, using a
   stable cumulative weighted selection; deterministic fallback chooses the
   smallest unselected pixel index when the weight sum is non-positive.

After seeding, Lloyd refinement runs over **all** valid pixels (not just the
candidate pool):

- assign each pixel to the nearest Oklab center (ties go to the lower cluster
  index);
- update each center with the alpha-weighted centroid
  `sum(max(alpha, 0) * lab) / sum(max(alpha, 0))`;
- empty clusters are repaired by choosing the valid pixel maximizing
  `alpha * min_distance2` to non-empty centers (ties and all-zero scores pick
  the smallest unused valid pixel index);
- stop when `max_center_movement² <= convergence_tol²` or `max_iters` is
  reached.

Saliency affects only the seeding probabilities, never the Lloyd centroids.
After refinement, each center gets a unique nearest-pixel anchor and is exported
as `Representative { pixel_index, lab }`; `lab` is the Lloyd center.

### 6.4 Export to weighted samples

After representative selection / clustering, the pipeline exports weighted support colors.

Important parameters:

- `center_mode`
  - `Centroid` or `Medoid`
- `normalize_weights`
  - normalize final weights to sum to 1
- `saliency_to_weight_mix`
  - how much saliency contributes to weight
- `saliency_weight_gamma`
  - curve shaping for saliency contribution
- `frequency_gamma`
  - curve shaping for cluster frequency contribution
- `min_cluster_weight`
  - drop tiny exported clusters

References:

- Medoid: <https://en.wikipedia.org/wiki/Medoid>

### 6.5 Image cap

`CapEstimator` decides how the cap surface is constructed from image evidence:

- `MaxObserved` records the maximum chroma per `(L, h)` cell.
- `Statistical(StatisticalCapConfig)` records a weighted percentile per cell.

`CapConfig` defaults to `PreparedPixels + Statistical`, so the statistical cap is
built from **all prepared valid pixels** (with their alpha as weight), not from
the 24 exported k-means samples.

Important parameters:

- `source`
  - whether cap is built from prepared pixels or exported samples
- `estimator`
  - `MaxObserved` or `Statistical`
- `percentile`
  - weighted per-cell chroma cutoff used by the statistical estimator
- `tolerance_factor`
  - headroom applied after percentile estimation
- `smoothing`
  - cap smoothing blend for the non-conditional statistical path
- `use_conditional_hue`
  - whether low-mass hue bins are suppressed within each lightness row
- `builder.*`
  - cap-builder-specific parameters from `chromoxide`

### 6.5.1 Conditional hue confidence

Every cap cell carries a support confidence
`cell_mass / row_mass` (clamped to `[0, 1]`). With `use_conditional_hue`, cells
below `StatisticalCapConfig::CONDITIONAL_HUE_THRESHOLD` get cap `0` and
confidence `0`, and hue nearest-fill is **not** used. Smoothing is
confidence-weighted normalized smoothing, and afterwards each cell is gated by
`smoothstep01(confidence / threshold)` using its own pre-smoothing confidence,
so a low-support hue cannot borrow full chroma from a neighboring hue. The
tolerance factor is applied last.

Each `ImageCap` stores the smoothed confidence grid and exposes it through
`query_with_confidence`; old serialized caps without a confidence grid report
confidence `1.0`.

## 7. Builtin export pipeline

Builtin palettes have two stages:

1. solve internal slots
2. export final members

Export can do three kinds of work:

- direct mapping
- reorder solved slots into stable names
- derive additional members from solved colors

Examples:

- `cover-salient`
  - solve `cover`, `salient-a`, `salient-b`
  - reorder/export as `cover`, `salient-1`, `salient-2`
- `ansi-8-derived`
  - solve 8 base colors
  - derive `bright_*` exports afterward

## 8. Default image sampling

`chrox`'s default image config now uses
`KMeansPlusPlusLab { count = 24, candidate_stride = 2, saliency_bias = 0.35,
max_iters = 20, convergence_tol = 1e-5 }`. `FarthestPointLab` remains available
and existing configs that select it still parse. The CLI default also caps the
working image longest side at `256` pixels so Lloyd refinement does not run
billions of distance computations on 1080p/4K images; library users can still
choose the full resolution by leaving `max_working_dim` unset.

## 9. Contrast and formatting

Template filters are formatting functions, not solve terms.

Examples:

- `hex`
- `rgb`
- `oklch`
- `hdeg`

For contrast itself, the relevant concept is WCAG contrast ratio:

- <https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum.html>
