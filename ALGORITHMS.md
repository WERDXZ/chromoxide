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
  - `Ignore`, `HardIntersect`, or `SoftPenalty`
- `relax`
  - soft-cap multiplier when `SoftPenalty` is used
- `cap_interpolation`
  - how cap values are queried from the cap surface

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

Important parameters:

- `count`
  - desired number of representatives
- `candidate_stride`
  - candidate downsampling stride for farthest-point methods
- `saliency_bias`
  - how much salient regions are preferred during selection

References:

- Farthest-first traversal: <https://en.wikipedia.org/wiki/Farthest-first_traversal>
- Stratified sampling: <https://en.wikipedia.org/wiki/Stratified_sampling>

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

The cap builder estimates the maximum plausible chroma as a function of lightness and hue.

Important parameters:

- `source`
  - whether cap is built from prepared pixels or exported samples
- `builder.*`
  - cap-builder-specific parameters from `chromoxide`

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

## 8. Contrast and formatting

Template filters are formatting functions, not solve terms.

Examples:

- `hex`
- `rgb`
- `oklch`
- `hdeg`

For contrast itself, the relevant concept is WCAG contrast ratio:

- <https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum.html>
