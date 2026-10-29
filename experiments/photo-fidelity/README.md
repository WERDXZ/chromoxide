# Photo fidelity experiment

This is an opt-in experiment, not a change to the shipped Base16 recipe. It
tests whether fixed accent domains and the support mass prior suppress source
differences. It does not claim that unconstrained palettes make better terminal
themes.

## Protocol

- Run the existing image pipeline once per input with an explicit seed. All
  variants receive identical samples and image cap.
- Compare a 2 × 2 design: original Base16; linear-mass Support only; relaxed
  accent domains only; both changes together.
- Linear-mass Support uses the existing API with `beta = tau` and `epsilon = 0`.
  Inputs have strictly positive, normalized sample weights. This avoids adding
  pseudo-mass per sample; the existing stable softmin handles the log sum.
- Relaxed accent domains set the eight accent hue domains to `Any` and their
  chroma lower bounds to zero. Lightness bounds, upper chroma bounds, all term
  weights, cap policy, neutral slots, and dynamic priors stay the same.
- Use the same solve budget and explicit master seeds across variants. Changed
  hue domains can change physical initial colors; report between-seed variation
  rather than claiming identical starting palettes.
- First use gray, a small red region on gray, and two images with identical
  colors but exchanged area proportions. Optional local photos extend the set.
- Measure source coverage, accent-to-source distance, chroma, out-of-gamut
  colors, and final RGB clipping drift. Show source, weighted samples, all final
  HEX colors, and a fixed theme preview. These are diagnostics, not a combined
  aesthetic score.
- Audit Cover's loss scale separately. Do not replace it with squared distance
  under its old weight and interpret the result as an isolated improvement.

## Checks and scope

The fast regression tests compare scalar loss and finite-difference gradients
before and after splitting same-color evidence while preserving total mass.
They also verify the linear weighted-kernel formula. The slow image comparison
is ignored by the normal test suite and must be requested explicitly.

Implementation uses existing Rust APIs, `f64`, explicit ChaCha seeds, and normal
workspace formatting. Generated results live under `target/photo-fidelity`.
No new dependency, public term variant, default recipe, solver algorithm, or
export mapping is needed for this experiment.

```sh
cargo test -p chromoxide --test support_mass --offline
cargo test -p chrox --release photo_fidelity_comparison --offline -- --ignored --nocapture
python3 experiments/photo-fidelity/render.py
# Optional PNG rendering; the SVG is already self-contained.
rsvg-convert target/photo-fidelity/preview.svg -o target/photo-fidelity/preview.png
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
cargo test --workspace --all-features --offline
```

The default experiment uses eight starts and 120 iterations per start, with
three fixed master seeds. This is a smaller budget than shipped Base16's
32 starts and 180 iterations. To audit the difficult cases with more compute:

```sh
CHROMOXIDE_FIDELITY_CASES=gray-red-small,cool80-warm20 \
CHROMOXIDE_FIDELITY_STARTS=16 CHROMOXIDE_FIDELITY_ITERS=360 \
CHROMOXIDE_FIDELITY_OUT="$PWD/target/photo-fidelity-long" \
cargo test -p chrox --release photo_fidelity_comparison --offline -- --ignored --nocapture
python3 experiments/photo-fidelity/render.py target/photo-fidelity-long
```

Set `CHROMOXIDE_FIDELITY_IMAGES` to a local image file, directory, or a
platform-separated list of paths to append photographs. Directories are scanned
non-recursively for JPEG, PNG, and WebP. They receive sorted `photo-00`,
`photo-01`, etc. identifiers; `inputs.toml` records their paths. No network
download is performed. Use `CHROMOXIDE_FIDELITY_CASES=photo-00` to restrict a run.

Each output directory contains per-run solution TOML (including weighted loss
breakdowns), the image-support TOML, final HEX and raw/roundtripped Oklab CSV,
metrics, a separate Cover scale audit, and the renderer's summary/preview.
Reusing the directory replaces the run's CSV files; use a different `OUT` to
preserve a previous comparison.

Metrics use full prepared pixels for source coverage and accent-to-source
distance. `max_export_drift` includes both clipping and 8-bit quantization.
The renderer also reports the worst nearest-palette distance over the exported
support samples, to reveal minority colors hidden by an area-weighted mean.
Preview cards use seed 7 for both colors and their numerical metrics, and show
the solver's convergence flag and iteration count. They label the reference,
controls, and source-anchor prototype explicitly. The summary CSV and tables in
this document still average all three seeds. Earlier previews mixed seed-7
colors with three-seed average annotations; regenerating them corrects that
presentation without changing any solution or numerical CSV.
Seed spread is maximum pairwise, role-aligned palette RMSE across the three
seeds; free-hue accents can permute, so this is not an unordered palette metric.
Only the `base00`/`base05` contrast pair is checked, not all semantic roles.

## Acceptance

1. Linear-mass support preserves loss and gradient under mass-preserving splits
   within floating-point tolerance; the legacy prior's dependence is measured.
2. The original variant is built from the actual Base16 slot/term factories.
3. Every variant uses identical prepared evidence and equal solve budgets.
4. Results and previews are reproducible and distinguish source errors from
   final RGB clipping. No human preference or real-photo result is inferred
   from synthetic fixtures.

## Results — 2026-10-06

Code baseline: `d3f02bf`; rustc `1.97.0-nightly (ca9a134e0 2026-04-26)`.
Four synthetic images × four variants × three master seeds = 48 solves.
The gray/red fixture is an 8 × 8 red patch on an 80 × 64 gray image: 1.25% red.
The two-color fixtures have the same RGB colors and exchanged 80%/20% areas.
The image pipeline retained exactly the expected one or two source colors and
area weights; loss of those colors therefore occurs downstream in these cases.

All distances below are Euclidean Oklab distances after final HEX export,
averaged across the three seeds. They are not CIEDE2000 or preference scores.

| Variant | Gray: mean accent chroma | Red patch: nearest output distance | Cool 80%: worst source-sample distance |
| --- | ---: | ---: | ---: |
| Baseline | 0.07248 | 0.12432 | 0.10575 |
| Linear Support | 0.07248 | 0.08163 | 0.07902 |
| Relaxed accents | ≈ 0 | 0.18809 | 0.16123 |
| Relaxed + linear | ≈ 0 | 0.19788 | 0.16001 |

The fixed accent domains introduce chroma into a gray source. Relaxing them
removes that effect but lets multiple accents cluster near the dominant source
color. The relaxed/linear cool-dominant palette is closer to source evidence on
average while failing to retain the orange minority. A lower average distance
alone is therefore an inadequate acceptance criterion.

The small-budget red-patch result favors linear Support within the existing
semantic recipe. This is not yet evidence of general improvement: 23 of the 48
selected runs reached the 120-iteration limit, and results vary with the seed.
The larger-budget follow-up is recorded separately below.

### Larger-budget follow-up

The red-patch and cool-dominant cases were repeated with 16 starts and 360
iterations per start, again at all three master seeds (24 more solves).

| Variant | Red patch: nearest output distance | Cool 80%: worst source-sample distance |
| --- | ---: | ---: |
| Baseline | 0.12423 | 0.08466 |
| Linear Support | 0.11042 | 0.07892 |
| Relaxed accents | 0.20662 | 0.15939 |
| Relaxed + linear | 0.20675 | 0.19093 |

The minority-loss pattern persists with more compute. Linear Support's
red-patch improvement is smaller than in the first run, and it is not better
for every master seed. Eight of these 24 selected runs still hit the iteration
limit; this is not proof of globally optimal solutions. The solver-reported
gradient norm is unavailable for one run (`NaN` in the CSV), and should not be
treated as a convergence certificate. Objective totals are not compared across
different definitions of Support.

There is also a direct explanation for the remaining minority pressure. The
red/gray source distance squared is 0.047233. Even under **linear** mass weights,
at the red sample and `tau = 0.03`, the gray kernel contributes
`0.9875 * exp(-0.047233 / 0.03) = 0.20454`, versus `0.0125` from red itself:
about 16.36 times as much. Every free accent can seek the same dominant mode;
Base16's Cover still only involves the three dark background slots. Neither
mass linearity nor relaxed domains alone impose minority coverage.

This experiment therefore does **not** promote the relaxed/linear combination
to a default or claim to deliver a finished photo-faithful theme. The follow-up
below gives accent slots explicit responsibility for source coverage while
retaining the same minority-loss checks.

The Support tests initially failed with the existing Base16 `beta`/`epsilon`,
then passed with `beta = tau` and zero epsilon, using the same implementation.
They check both loss and gradients at three queries, splitting a color into
2, 3, and 11 equal-mass pieces. No solver equality is inferred from loss
invariance, since representation can also affect seed construction.

The Cover audit confirms that at distance 0.05 and delta 0.03, the existing
single-slot loss is about 0.00000312 versus 0.0025 for squared distance. This
run does not change Cover or claim a visual improvement from changing it.

These are mechanism experiments. No real-photo corpus, geometric/re-encoding
stability study, all-role readability audit, or preference test was performed.
The production recipe and export mapping are unchanged.

## Source coverage follow-up — 2026-10-07

This phase compares four variants built on the relaxed/linear recipe:

| Variant | Change |
| --- | --- |
| `relaxed-linear` | Previous control. |
| `accent-cover` | Add Cover over slots 8–15 with the existing weight 5, tau 0.02, delta 0.03. This tests scope only, not a retuned coverage loss. |
| `anchors-with-priors` | Add assigned source targets; retain Support and relative-chroma priors. |
| `source-anchors` | Same assignments, replacing accent Support and relative-chroma priors with source targets. |

Assignment merges **exactly identical** source colors before considering area.
It always selects the largest mode, then greedily selects separated colors
using `area * nearest_selected_distance²`, up to eight modes. Additional modes
need at least 0.5% area and 0.04 Oklab separation. These are experimental
thresholds, not validated perceptual constants or a spatial noise detector.
The pipeline's original sample weights and cap are unchanged. Saliency is not
used for selection in this phase.

Each selected mode receives one reserved accent seat. Remaining seats go to the
largest deficits from proportional area quotas, normalized over selected modes.
Repeated source colors are allowed. The quotas are a simple theme heuristic;
eight equal swatches do not model actual screen area. Source colors are assigned
greedily to available slots with the smallest L/C projection error, reserving
one seat for each mode before filling duplicates. This is not a globally optimal
assignment or an ANSI semantic-role mapping.

The prototype reuses unary lightness, chroma, and hue terms. Targets are the
assigned source L/C/H with L and C clamped to the existing role bounds. L and C
use weight 20 and Huber delta 0.1; hue uses weight `20 * target_C²` and delta
1 radian, without an output-dependent hue gate. Near the target this resembles
`10 * (dL² + dC² + target_C² * dh²)`, not globally squared Oklab distance.
The target strength of a reserved minority seat is independent of its area.
Neutral roles, accent L bounds/C ceilings, image-cap penalties, solver, and HEX
export all remain in use. No public API or shipped recipe changes are required.

Fast tests cover monochrome input, exchanged area quotas, a small minority
versus one pixel, near-duplicate colors, exact-color splitting/reordering, and
whether the reserved minority slot prefers its own source to the dominant
color. End-to-end measurements use five fixtures (adding a single red pixel on
gray), four variants, three master seeds, eight starts, and 120 iterations.

```sh
CHROMOXIDE_FIDELITY_PHASE=coverage \
CHROMOXIDE_FIDELITY_OUT="$PWD/target/photo-fidelity-coverage" \
cargo test -p chrox --release photo_fidelity_comparison --offline -- --ignored --nocapture
python3 experiments/photo-fidelity/render.py target/photo-fidelity-coverage
rsvg-convert target/photo-fidelity-coverage/preview.svg -o target/photo-fidelity-coverage/preview.png
```

`CHROMOXIDE_FIDELITY_VARIANTS` optionally restricts the selected phase to a
comma-separated subset. `assignments.csv` records source Lab/area per assigned
slot. The renderer's `source-matching.csv` records nearest final-color distance
and the number of accents nearest each source sample, per seed. Assignment is
deterministic; the solved and exported colors still require stability checks.

### Measured results

All 60 solves completed. The table reports mean worst source-sample distance
to final HEX colors across the three seeds (lower is closer):

| Variant | 1.25% red patch | Cool 80% / warm 20% | Cool 20% / warm 80% |
| --- | ---: | ---: | ---: |
| Relaxed + linear | 0.19788 | 0.16001 | 0.05313 |
| Add accent Cover | 0.20380 | 0.15988 | 0.04812 |
| Anchors with old priors | 0.02633 | 0.01276 | 0.02371 |
| Source anchors replacing old priors | **0.00276** | **0.00463** | **0.00522** |

At all three seeds, source anchors produced:

- Eight gray accents for the gray fixture, with effectively zero chroma.
- Seven gray accents and one red accent for the 1.25% patch. The largest
  red-to-output distance among seeds was 0.00313.
- Six cool/two warm accents for the cool-dominant image; two cool/six warm
  accents when areas were exchanged, counting nearest-source matches after HEX.
- Eight accents nearest gray and none nearest the single red pixel. The red
  pixel **was present** in the extracted evidence at weight 0.0001953125. Its
  omission by the assignment is deliberate, so the worst-sample score around
  0.207 on this fixture should not be interpreted as a regression.

Replacing old priors reduced mean coverage RMSE from 0.02241 to 0.00031 for
the red-patch case, from 0.07173 to 0.00262 for cool-dominant, and from 0.02594
to 0.00501 for warm-dominant. Source anchors had no out-of-gamut colors in these
15 solves. This does not establish gamut safety for other images. Role-aligned
seed RMSE for the warm-dominant case decreased from 0.05346 to 0.00723, but
was not universally lower (cool-dominant increased from 0.00232 to 0.00449 as
the minority was restored).

The matched `anchors-with-priors` versus `source-anchors` comparison supports
replacing generic accent density/relative-chroma goals with source-specific
targets in this prototype. It does not isolate Support from relative chroma;
both are removed together. The scope-only Cover control is not evidence against
all coverage objectives: its original scale is weak and was not recalibrated.

Twenty-eight of 60 selected solves reached the 120-iteration limit, including
six of the 15 source-anchor solves. Thus the result is a finite-budget mechanism
test, not a global-optimum claim. A larger-budget check is recorded below.

There is still no real-photo evaluation, region-connectivity model, or all-role
contrast check. The 0.5% cutoff can discard legitimate tiny highlights and
cannot distinguish a coherent object from equally colored scattered noise.
The assignment can place red in Base16's nominal green/success slot (visible in
the preview); a separate semantic-role adaptation is needed for terminal use.
This remains an opt-in prototype, not a promoted theme preset.

### Larger-budget source-anchor check

Nine additional solves repeated the three colored cases with 16 starts and
360 iterations, using the same three master seeds. All retained the minority
colors and the same 7/1 or 6/2 accent counts after HEX export. Mean worst-source
distances were 0.00240 (red patch), 0.00516 (cool dominant), and 0.00541 (warm
dominant). Every individual source-to-output distance was below 0.01, and no
output was out of gamut. Two of nine selected runs still hit the iteration
limit. Distances do not decrease monotonically with budget because the full
objective includes role and cap constraints, and is not this diagnostic alone.

```sh
CHROMOXIDE_FIDELITY_PHASE=coverage \
CHROMOXIDE_FIDELITY_VARIANTS=source-anchors \
CHROMOXIDE_FIDELITY_CASES=gray-red-small,cool80-warm20,cool20-warm80 \
CHROMOXIDE_FIDELITY_STARTS=16 CHROMOXIDE_FIDELITY_ITERS=360 \
CHROMOXIDE_FIDELITY_OUT="$PWD/target/photo-fidelity-anchors-long" \
cargo test -p chrox --release photo_fidelity_comparison --offline -- --ignored --nocapture
python3 experiments/photo-fidelity/render.py target/photo-fidelity-anchors-long
```

Validation: the five new fast assignment/objective tests, the workspace suite
(including doctests), formatting, and Clippy with warnings denied passed. Both
opt-in experiment runs completed (60 + 9 solves). Final CSV checks verified
minority distances and source-count assignments for every seed, not only the
seed-7 swatches shown in the previews. The renderer was also run on the earlier
factorial artifacts to check compatibility.

## Audit of the blue/purple single-pixel output

The `gray-red-pixel / relaxed-linear / seed 7` result in the small-budget
comparison contains blue/cyan/purple accents absent from the source. These
are actual solved colors, not a rendering or channel-order error. Its mean
exported accent chroma is **0.0618**; the old preview's 0.0386 was the
three-seed average. The selected solve reached 120 iterations with
`converged = false` and final gradient norm approximately 0.235.

The cap/target interaction also deserves separate investigation:

- The single red pixel has area 1/5120 but occupies its own lightness row.
  The statistical cap's percentile and hue confidence normalize within a row.
  The red row therefore remains highly chromatic even with one pixel.
- Empty lightness rows are filled from neighbors. The resulting global
  chroma profile reaches 0.22996; unsupported hues fall back to this profile.
  The cap grid, global profile, and support-confidence arrays are exactly
  equal for the one-pixel and 64-pixel red fixtures in these artifacts.
- The relaxed/linear control still targets 84% relative chroma. Its blue
  `base09` reaches C=0.12932 against an adaptive cap of 0.15461 (ratio 0.83642),
  closely satisfying this term while paying a larger Support penalty. An
  allowed chroma range is not evidence that the corresponding hue exists.

This does **not** establish that blue/purple is the objective's optimum.
Re-running this same control with 16 starts and 360 iterations at all three
master seeds yields low-chroma gray accents (mean C about 0.0091). Thus the
vivid screenshot is sensitive to solve budget; it should not be presented as
the recipe's stable output. The source-anchor prototype also produces gray
accents on this fixture at the smaller budget (seed 7: all eight HEX values
are `#7d8287`). Neither observation proves general convergence or repairs the
low-evidence cap fallback in the production implementation.

```sh
CHROMOXIDE_FIDELITY_PHASE=coverage \
CHROMOXIDE_FIDELITY_VARIANTS=relaxed-linear \
CHROMOXIDE_FIDELITY_CASES=gray-red-pixel \
CHROMOXIDE_FIDELITY_STARTS=16 CHROMOXIDE_FIDELITY_ITERS=360 \
CHROMOXIDE_FIDELITY_OUT="$PWD/target/photo-fidelity-pixel-audit" \
cargo test -p chrox --release photo_fidelity_comparison --offline -- --ignored --nocapture
python3 experiments/photo-fidelity/render.py target/photo-fidelity-pixel-audit
```

The renderer update was verified on all four earlier artifact directories;
existing summary/source-matching CSV hashes stayed unchanged. Card labels were
checked against the seed-7 CSV, and SVG/PNG output was visually inspected.

## Sparse fallback and solver-reporting fixes — 2026-10-07

Unlike the earlier recipe experiments, this step changes the core statistical
cap builder and solver diagnostics. It does not install the source-anchor
prototype as a production recipe.

For newly built statistical caps, each row's all-hue fallback is now
`min(weighted_quantile(row, p), weighted_quantile(full_image, p))`, with the
existing `global_chroma_percentile` supplying `p`. The bound is applied before
hole filling, smoothing, and tolerance headroom. Direct conditional evidence
is unchanged. Thus a rare saturated row can support its own observed hue but
cannot supply the same high fallback to arbitrary unsupported hues. No new
area cutoff or configuration field is introduced. Existing serialized caps
must be rebuilt to get this behavior; max-observed builders are unchanged.

The single-pixel fixture's maximum global fallback falls from 0.22996 to
0.01080. Unit tests first reproduced the leak, then verified the bound across
multiple lightness-grid sizes and smoothing settings, retained directly
supported minority chroma, preserved abundant/vivid cases, and checked mass
splitting and weight scaling.

Solver reporting previously classified any early termination as convergence,
including argmin's `SolverExit` after line-search failure. It now requires an
explicit `SolverConverged` reason, which means the solver met its gradient **or
cost** tolerance. A solver-converged stop on the final allowed iteration is
also recognized. Optional stop-reason text is serialized globally and per seed;
older diagnostics still deserialize. The finite-difference gradient is
re-evaluated at the returned best parameters rather than taken from a potentially
different last iterate. Regression tests cover failed termination, missing/stale
cached gradients, last-iteration convergence, and serialization compatibility.

### Equal-budget checks

The five fixtures were repeated for `relaxed-linear` and `source-anchors` at
the original eight starts / 120 iterations and three master seeds: 30 solves.
Artifacts are in `target/photo-fidelity-cap-fix`; previous directories retain
the earlier results. The before/after table uses three-seed means, except seed
spread, which is the maximum pairwise role-aligned RMSE:

| Check | Before | After |
| --- | ---: | ---: |
| Single pixel, relaxed/linear: mean accent C | 0.03856 | 0.00918 |
| Single pixel, relaxed/linear: seed spread | 0.07206 | 0.00540 |
| Small red patch, source anchors: nearest output distance | 0.00276 | 0.00915 |
| Cool-dominant, source anchors: worst source distance | 0.00463 | 0.00463 |
| Warm-dominant, source anchors: worst source distance | 0.00522 | 0.00522 |

All three relaxed/linear single-pixel runs now have maximum raw accent C below
0.0091; the blue/purple excursions disappear at the original budget. Source
anchors still produce 7 gray / 1 red for the patch, 6/2 and 2/6 for exchanged
cool/warm proportions, and no red accent for the single-pixel fixture. The red
patch does move slightly farther from its source under the more conservative
fallback: individual nearest-output distances range from 0.00851 to 0.00968.
This tradeoff is recorded rather than treating every metric as improved.

Of the selected runs, 17 report solver convergence and 13 reach the iteration
limit. Every returned gradient norm is finite in this run. The fixes improve
the demonstrated failure case and reporting; they do not establish general
convergence or real-photo quality. Full workspace tests (including doctests),
Clippy with warnings denied, formatting, and artifact checks passed.

```sh
CHROMOXIDE_FIDELITY_PHASE=coverage \
CHROMOXIDE_FIDELITY_VARIANTS=relaxed-linear,source-anchors \
CHROMOXIDE_FIDELITY_OUT="$PWD/target/photo-fidelity-cap-fix" \
cargo test -p chrox --release photo_fidelity_comparison --offline -- --ignored --nocapture
python3 experiments/photo-fidelity/render.py target/photo-fidelity-cap-fix
```

## Semantic role adaptation — 2026-10-07

The source-anchor prototype can preserve a red patch yet put it in `base0B`
(success): its greedy assignment only minimizes L/C projection. A regression
test first reproduced that exact slot-11-versus-slot-8 failure. The new opt-in
`semantic-anchors` variant reserves error (`base08`), success (`base0B`), and
link (`base0D`) before filling the other five accent slots. The source-only
control remains available. This is still a test-only experiment, not a new CLI
recipe or a complete Base16/ANSI semantic policy.

The theme adapter reuses source-mode selection and its area/noise cutoff. Each
role takes the largest retained mode with C >= 0.035 inside its Oklch hue arc:
error 350–45 degrees, success 125–170, link 225–280. Missing roles use the arc
midpoint with `C = clamp(area-weighted mean source-mode C, 0.04, 0.12)` and
`L = clamp(area-weighted mean source-mode L, 0.60, 0.70)`. These are experimental
conventions, not universal semantic or perceptual thresholds. The positive
chroma floor belongs only to the three theme roles; the source-only palette
can still be entirely gray. A small vivid patch therefore supplies its own
role without forcing equally vivid synthesized colors.

Role domains enforce the hue arc and a 0.035 chroma floor. Synthesized roles
also cap chroma at their style target. Source-backed roles use their selected
source's L/C/h targets, projected into the existing slot bounds. All three
roles use a soft contrast target of 4.6:1 against `base00`, with final HEX
checked against an experimental acceptance floor of 4.5:1. They bypass the
image cap at adapted coordinates: lightening a supported red for readability
does not turn it into an unsupported source color, and synthesized green has
no source hue cap to consult. The remaining five accent slots retain the
image-cap policy. This does not add general gamut mapping; final clipped and
quantized HEX values are measured using the existing export path.

Remaining seats prioritize unrepresented source modes, then area deficits;
source-backed semantic seats count toward their mode's quota. Synthetic seats
claim no photo area. With only five free seats, complex images may lose a
source mode to semantic requirements. Area proportions are consequently a
preference, not an exact output constraint.

Artifacts now include `provenance.csv` (all eight theme accent assignments,
original source/area when present, pre-contrast targets) and `roles.csv`
(per-seed final HEX, origin, background contrast, nearest other role distance,
hue/chroma checks, pass/fail). A semantic run fails after saving artifacts if
any role misses its hue arc, C >= 0.03, contrast >= 4.5, or distance >= 0.04
from another role. These checks are numerical diagnostics, not a guarantee
of color-vision accessibility. Preview labels distinguish `source` from
`added`; controls say `n/a`, since no semantic assignment was performed.

Two extra fixtures exercise a small green patch and red/green patches together.
Unit tests cover matching red/green/blue, missing-role provenance, gray and
isolated-pixel behavior, exact-color splitting/reordering, and simultaneous
red/green evidence. The seven-fixture comparison uses the same eight starts,
120 iterations, and three master seeds for both variants.

All 63 final-HEX role checks passed across the 21 theme runs (42 solves with
the control). Minimum role/background contrast was 4.5769:1 and minimum pair
distance was 0.05405. However, only 3/21 theme runs reported solver convergence;
the others exhausted the iteration budget. One theme run also contained an
out-of-gamut raw color, which the existing exporter clipped. Passing the
final-output checks does not establish convergence or general gamut safety.
Full workspace tests, Clippy with warnings denied, and formatting checks passed.
The fixtures are synthetic; real-photo evaluation remains outstanding.

For the reported gray/red case at seed 7, the new error is `#dd3a2f`
(source-adapted, 4.61:1), success `#738776` (synthetic, 5.32:1), and link
`#728396` (synthetic, 5.27:1). Error is slightly lighter than the source to
meet the contrast target; nearest red-source distance increases from 0.0097
to 0.0350. The mean accent-to-source gap also increases from 0.0037 to 0.0129
because the theme deliberately adds green and blue. This is a usability versus
fidelity tradeoff, not an across-the-board source-fidelity improvement.

```sh
CHROMOXIDE_FIDELITY_PHASE=semantics \
CHROMOXIDE_FIDELITY_OUT="$PWD/target/photo-fidelity-semantics" \
cargo test -p chrox --release photo_fidelity_comparison --offline -- --ignored --nocapture
python3 experiments/photo-fidelity/render.py target/photo-fidelity-semantics
```

## Real-photo evaluation — 2026-10-07

See [PHOTO_RESULTS.md](PHOTO_RESULTS.md) for the 13-photo corpus, source credits,
per-photo previews, measurements, and limitations. [photos.json](photos.json)
pins the downloaded files and their SHA-256 hashes. Run
`bash experiments/photo-fidelity/run_photos.sh` to download/verify the corpus,
compare four variants at 120 iterations, and compare the two anchor variants
again at 360 iterations. All use eight starts and three master seeds.

The 234 solves improve the evidence for source anchoring, but expose two theme
adapter gaps: a distinctive coast turquoise is dropped during role allocation,
and only 3/39 semantic palettes make all eight accent text colors reach 4.5:1,
at either budget. Additional final-HEX hue-boundary failures mean the opt-in
real-photo acceptance runs return nonzero after saving their complete artifacts.
The report preserves these failures; the prototype is not promoted to a CLI
recipe. The extended preview uses a fixed code/terminal layout and separately
reports three semantic role checks and all-eight accent text contrast.

## Protected colors and all-text contrast — 2026-10-29

[PROTECTED_RESULTS.md](PROTECTED_RESULTS.md) evaluates the next adapter against
the same semantic control. The new variant keeps the coast turquoise and makes
comments, main foreground, and all eight accents meet 4.5:1 after HEX export.
At 120 iterations, all 39 palettes pass the ten-slot text contract, but average
source coverage and seed stability regress. This remains an experiment.

Run `bash experiments/photo-fidelity/run_photos.sh protected` to reproduce both
the 120-iteration comparison and the new adapter's 360-iteration budget check.
The current text audit includes base03 and base05; older reports explicitly
counted only eight accents. Both old and new palette colors remain visible in
the same preview layout and provenance CSVs.

## Separate scene and theme adaptation — 2026-10-29

[STAGED_RESULTS.md](STAGED_RESULTS.md) compares an immutable source palette with
explicit foreground projection and a separate decorative-color ablation.
At 120 iterations, the combined prototype reduces mean full-image coverage
error by 36% and median seed spread by 84% relative to protected anchors, with
all ten text slots and three semantic roles passing on the same 13 photos.
The higher-budget check also passes.

Run `bash experiments/photo-fidelity/run_photos.sh staged` for a fresh comparison.
`staged-decor` reserves base04 for decoration and does not preserve the ordered
Base16 neutral ladder; consumers must honor that usage contract. The production
registry is unchanged. Per-run TOML separates scene evidence and final theme
colors, and the report distinguishes cached source replay from fresh solving.
