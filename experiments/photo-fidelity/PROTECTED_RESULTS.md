# Protected source colors and text contrast — 2026-10-29

The new experimental adapter keeps the coast turquoise and makes all ten declared foreground text slots readable on base00. It does **not** improve overall photo fidelity: source coverage and seed stability regress at the original iteration budget. Keep this as an opt-in experiment, not a replacement for the shipped Base16 recipe.

## Changes and controls

`protected-anchors` extends the existing semantic adapter. The `semantic-anchors` control is retained; all 624 exported control swatches in this run exactly match the preceding photo experiment.

- Allocate the three semantic roles first, as before. Among remaining unrepresented source modes, prioritize colors at least 0.04 apart in Oklab's a/b plane from already assigned sources, ranked by area × squared chromatic distance. Then use the existing area-deficit rule. This preserves chromatic differences before redundant light/dark variants. It is a greedy capacity rule, not a guarantee that all extracted modes fit eight slots.
- Require contrast for base03 (comments), base05 (main foreground), and base08–base0F against base00. Use soft contrast targets of 4.7:1 with weight 20; acceptance remains **4.5:1 on final 8-bit HEX**. Raise the comment lightness ceiling to 0.68, the secondary neutral ceiling to 0.72, and their ladder targets to 0.60 and 0.65 so the old comment range does not make the requirement infeasible.
- For source-backed accents, use observed source chroma as the upper bound and target. Do not query the image cap at a newly adapted lightness. Synthesized roles keep their existing style-based chroma bounds. Source identity and true source area remain in `provenance.csv` even when the output is lightened.
- Inset semantic optimization hue arcs by 3° at each end to leave room for quantization. The final-HEX acceptance arcs are unchanged. This margin is an experimental safeguard, not a complete gamut strategy.

This is a combined adapter change, not an isolated causal ablation of each knob. Extraction, area/noise thresholds, image preprocessing, source-only control, and production palette recipes are unchanged. It adds no new dependencies or SIMD code.

## Protocol

Reuse all 13 pinned photographs and their SHA-256 manifest from [the previous report](PHOTO_RESULTS.md). Both adapters use eight starts, 120 iterations, image seed 42, and master seeds 7/23/42: **78 solves**. Check the new adapter again with eight starts and 360 iterations on all 13 photographs and the same three seeds: **39 more solves**. The latter tests whether additional optimization reintroduces the previous hue-boundary failures; it is not a new image sample.

The fixed preview maps the type `Image` to base0A for every variant and image. The text audit now counts comments and main foreground as well as all eight accents. The previous report's “all eight” numbers therefore should not be compared directly to the new “all ten” count.

[120-iteration comparison](../../target/photo-fidelity-protected/preview.svg) · [Coast detail](../../target/photo-fidelity-protected/previews/photo-11.png) · [360-iteration preview](../../target/photo-fidelity-protected-long/preview.svg)

[Per-photo numerical results](protected-results.csv) preserve the three-seed summaries for both budgets. All 13 downloaded hashes were verified, and the prepared support is byte-identical to the preceding experiment and between budgets.

## Results at 120 iterations

| Measurement | Semantic control | Protected adapter |
| --- | ---: | ---: |
| All ten text slots pass, per palette | 0/39 | **39/39** |
| Individual text/background checks pass | 259/390 | **390/390** |
| Lowest text contrast | 1.622:1 | **4.661:1** |
| Full error/success/link checks pass | 116/117 | **117/117** |
| Mean source coverage RMSE ↓ | 0.04696 | 0.07070 |
| Mean accent-to-source gap ↓ | 0.01640 | 0.01983 |
| Median seed spread ↓ | 0.00403 | 0.02411 |
| Maximum seed spread ↓ | 0.02254 | 0.03753 |
| Solver-converged runs | 0/39 | 0/39 |

Coverage and accent gap use prepared full-image pixels and final exported colors, as in the previous report. Coverage worsens on **all 13 photographs**, by about 51% on average. Raising dark source colors into readable foregrounds and removing the dim comment rung loses dark/midtone coverage. This is a meaningful cost; the fact that a text theme cannot copy every source lightness does not make that cost disappear.

The issue is not only lightness. The area-weighted representative-sample a/b coverage RMSE to the eight accents also increases, from 0.01186 to 0.01581. This supplemental measure uses the 24 support samples, not full-image pixels, and cannot replace the main coverage metric or a perceptual evaluation. At 120 iterations, accent-only seed spread has median 0.01104 versus 0.00236 in the control; neutral-only spread has median 0.03151 versus 0.00552. No selected solve reports convergence; all stop at the iteration limit.

### The missing turquoise is restored

The coast's 2.903% source sample is `#23a5b3`. It now receives base0A, while error and success remain explicitly marked synthetic. Across seeds 7/23/42:

| Adapter | Nearest accent HEX | Oklab distance to source |
| --- | --- | ---: |
| Control, seed 7 | `#7daad4` | 0.09148 |
| Protected, seed 7 | `#28a5b0` | 0.00498 |
| Protected, seed 23 | `#23a5b1` | 0.00340 |
| Protected, seed 42 | `#24a4b3` | 0.00214 |

This is a local success with explicit provenance. It does not imply that other minority colors survived equally well or that the whole coast palette is closer: its mean overall coverage error still increases from 0.04034 to 0.06497.

## Additional iterations

| Protected adapter | 120 iterations | 360 iterations |
| --- | ---: | ---: |
| Text checks pass | 390/390 | 390/390 |
| Semantic role checks pass | 117/117 | 117/117 |
| Lowest text contrast | 4.661:1 | 4.661:1 |
| Mean source coverage RMSE ↓ | 0.07070 | 0.07079 |
| Mean accent-to-source gap ↓ | 0.01983 | 0.01968 |
| Median seed spread ↓ | 0.02411 | 0.00760 |
| Maximum seed spread ↓ | 0.03753 | 0.01538 |
| Solver-converged runs | 0/39 | 0/39 |

The acceptance command exits successfully at both budgets. The prior hue-boundary failures do not recur. Extra iterations reduce seed variation, but do not recover source coverage. Paired 120→360 accent RMSE has median **0.01172**, maximum **0.02275**; these changes are larger than in the previous adapter. All selected runs still stop at the iteration limit, so passing output checks must not be confused with solver convergence.

Representative-sample a/b coverage improves to 0.01347 at 360, still above the control's 120-iteration 0.01186. The coast turquoise remains within 0.00079–0.00383 Oklab distance. Three raw colors are outside sRGB at each budget; the measurements include their actual channel clipping and quantization. This change therefore does not claim to solve gamut mapping.

## Engineering conclusion

The two concrete gaps are covered by regression tests and actual-image output checks. The adapter still combines source preservation and readability in competing soft objectives. Making a dark source target readable can move its chroma/hue too. The higher-budget comparison improves stability but leaves the fidelity cost in place; separate target adaptation needs evaluation before promotion.

The next useful experiment is to retain an unchanged scene palette alongside a theme palette, explicitly project source colors into readable text targets with minimal hue/chroma changes, and give decorative colors their own usage contract. Compare that staged adaptation against both controls here. Do not make every source color a text color or claim that greater average output distance proves success.

## Reproduce and verification

```sh
bash experiments/photo-fidelity/run_photos.sh protected
```

The runner verifies cached photographs or downloads missing pinned files, runs both budgets, audits final HEX, renders SVGs, and compares paired budgets. New acceptance failures return nonzero after saving artifacts; historical control failures remain visible in `roles.csv` but do not fail the new-adapter phase. Use `rsvg-convert` to generate PNGs from the SVGs.

Regression tests cover the coast allocation and actual exported color, invariance to exact-color splitting/reordering/mass scaling, and all ten exported text contrasts on a dark source. The coast fixture contains prepared support statistics, with its source photo recorded in `photos.json`. Regular workspace tests, the targeted regressions, Clippy with warnings denied, formatting, and Python/shell syntax checks pass. An equivalent `as_chunks_mut::<8>()` change in seed derivation resolves a new nightly Clippy lint; all 15 determinism tests, including golden vectors, pass.

The three synthetic smoke cases (gray, small red patch, isolated red pixel) also pass all ten new text contrasts for all three seeds: 9/9 palettes, minimum 4.667:1. These support specific boundary behaviors, not broad image fidelity. This corpus still does not establish perception, JPEG/resize stability, general accessibility, or complete sRGB gamut handling.
