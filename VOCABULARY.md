# Vocabulary

This file defines the main terms used across the workspace.

## Core color terms

- `Oklab` - perceptual Cartesian color space used internally by the solver
- `Oklch` - perceptual polar color space used for palette-facing values: lightness, chroma, hue
- `lightness` / `L` - perceived brightness
- `chroma` / `C` - colorfulness or saturation-like strength
- `hue` / `h` - angle on the color wheel, usually in radians internally

## Solver / algorithm terms

- `constraint-driven palette optimization` - the overall approach used by `chromoxide`: solve colors by balancing evidence, domains, and weighted objectives
- `continuous optimization` - colors are solved in a continuous parameter space instead of chosen from a fixed discrete set
- `multi-start` - run the optimizer from multiple initial seeds and keep the best result
- `seed` - one initial guess for all slot variables before local optimization starts
- `seed_count` - number of starting guesses tried per solve
- `L-BFGS` - the local optimizer used after seed generation
- `finite-difference gradient` - numerical gradient estimate used when analytic gradients are not provided
- `objective` - the total weighted score being minimized
- `local optimum` - a good solution within one basin, but not necessarily the global best across all seeds
- `diagnostics` - extra solve metadata such as seed runs, objective breakdown, and slot stats
- `estimated saliency` - effective saliency value inferred at a solved slot position from image samples
- `conditional saliency` - mass-weighted RBF estimate of sample saliency at a query color, before any density gating
- `support density` / `normalized support density` - total RBF kernel mass at a query color divided by total sample mass, measuring how much image support exists near that color
- `effective saliency` - `conditional saliency * support-density gate`, the value used by `Saliency` terms and diagnostics

## Domain / constraint terms

- `interval` - closed min/max scalar range
- `hue arc` - allowed circular hue segment for a slot
- `chroma_epsilon` - low-chroma threshold below which hue-sensitive terms are softened
- `cap_policy` - how a slot interacts with the image cap
- `Ignore` - slot ignores the image cap
- `HardIntersect` - slot chroma is clamped to `min(user max, cap)` during decode; the user chroma minimum is never lowered
- `SoftPenalty` - slot decodes with the user chroma interval and pays `weight * pseudo_huber(max(0, C - cap), huber_delta)` when it exceeds the prebuilt cap
- `CapEstimator` - image-pipeline choice for how a cap surface is constructed from image evidence
- `MaxObserved` - cap estimator that records the maximum observed chroma per `(L, h)` cell
- `Statistical` - cap estimator (not a cap policy) that records a percentile-based cap surface
- `percentile` - weighted per-cell cutoff used when constructing a statistical cap surface
- `tolerance_factor` - extra headroom applied after percentile estimation so isolated outliers do not dominate the cap
- `conditional hue` - optional statistical-estimator filtering that keeps the cap tied to hue bins that actually carry mass at a given lightness; low-confidence hue cells are gated instead of restored by nearest-fill
- `cap confidence` - per-cell `cell_mass / row_mass` support, used by conditional-hue gating
- `neutralish` - a slot/domain with very small allowed chroma, effectively near-neutral

Respecting the original image means solved slot chroma stays inside, or only slightly above, the empirical `c_cap(L, h)` volume supported by the source image rather than borrowing chroma from unrelated lightness or hue regions.

## Palette model

- `palette` - a named color recipe that can be solved against image evidence
- `builtin palette` - a palette shipped by `chrox`
- `user palette` - a palette loaded from a TOML file
- `slot` - a solver variable; one color the optimizer solves for
- `member` - a named exported color available to templates
- `term` - one objective component in the optimization problem
- `weight` - multiplier controlling how strongly a term affects the solve
- `domain` - hard constraints for a slot: allowed lightness/chroma/hue/cap behavior
- `palette problem` - the full optimization input: slots, terms, samples, cap, config
- `solution` - the final solved colors and diagnostics

## Builtin palette/export terms

- `recipe` - the solve-side definition of a builtin palette: slots, terms, config
- `export` - the post-solve mapping from solved slot colors to final members
- `direct export` - members map 1:1 to solved slots
- `reorder export` - solved slots are reordered before being exposed as members
- `derive export` - extra members are generated from solved slots, such as bright ANSI variants

## Image pipeline terms

- `image pipeline` - the preprocessing + saliency + sampling + export + cap flow in `chromoxide-image`
- `preprocess` - image loading, resize, alpha handling, and conversion to working pixels
- `saliency` - signal for how visually prominent a region is
- `sampling` - selection of representative points/pixels from the image
- `export` - conversion from sampled/clustered image evidence into `WeightedSample`s
- `weighted sample` - one support color with a weight and saliency score
- `image cap` / `cap` - image-derived chroma limit used to keep solutions plausible for the source image
- `relative chroma` - chroma normalized to a slot's effective decode interval: `(C - lo) / (hi - lo)`, used by `RelativeChromaTarget`
- `relative chroma reference` - the interval a relative chroma ratio is measured against: `UserDomain`, `EffectiveDecodeDomain`, or `ImageCap`
- `representative anchor` - the real pixel index attached to a representative/cluster; it validates provenance but is not necessarily the assignment center
- `cluster center` - the Oklab color used for assignment and export (a Lloyd centroid or medoid), which may differ from the anchor pixel's color

## Term families

- `Cover` - encourages slots to explain broad cover colors in the image using soft-assignment expected squared distance
- `Support` - biases slots toward high-support image evidence
- `Saliency` - biases slots toward visually prominent image regions
- `LightnessTarget` / `ChromaTarget` / `HueTarget` - unary preferences on a single slot
- `DeltaL` / `DeltaC` / `DeltaH` - pairwise difference preferences between two slots
- `Distance` - pairwise Oklab distance preference
- `Order` - pairwise ordering preference, such as one slot being brighter than another
- `Contrast` - text/background contrast preference
- `GroupQuantile` - structured ladder/quantile target over a group of slots
- `RelativeChromaTarget` - unary preference on a slot's relative chroma inside its feasible interval

## CLI/template terms

- `template` - a text file containing placeholders like `{{base16.base00 | hex}}`
- `filter` - formatting function applied in templates, such as `hex` or `oklch`
- `render mode` - default CLI mode that solves required palettes and writes configured template outputs
- `test command` - CLI command that solves one or more palettes against an image and prints results to stdout

## Common palette-design phrases

- `neutral ladder` - an ordered set of low-chroma colors used for backgrounds, surfaces, comments, and foregrounds
- `accent` - a more colorful slot intended for semantic or emphasis use
- `cover color` - a color representing dominant image coverage, often useful for background/surface roles
- `salient color` - a color representing visually prominent image regions
- `regular vs bright` - ANSI pairing where `bright_*` is a systematic variant of the base color
- `reorder` - rename/reindex solved slots into a stable exported order
- `derive` - generate exported members from solved colors rather than solving them directly
- `tie-break` - a light extra preference added only to reduce unstable swaps between otherwise similar solutions

## Related docs

- See `ALGORITHMS.md` for pipeline overviews and parameter explanations.

## External references

- `Oklab / Oklch`
  - Bjorn Ottosson, "A perceptual color space for image processing"
  - <https://bottosson.github.io/posts/oklab/>
- `colorfulness / chroma`
  - Wikipedia: Colorfulness
  - <https://en.wikipedia.org/wiki/Colorfulness>
- `contrast ratio`
  - WCAG contrast minimum
  - <https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum.html>
- See `ALGORITHMS.md` for additional algorithm references.
