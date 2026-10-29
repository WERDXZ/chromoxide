# Scene palette and theme adaptation — 2026-10-29

**Separating text adaptation improves stability; reserving a non-text color recovers much of the lost source coverage.** On the same 13 photographs, the combined prototype reduces mean coverage error by 36% and median seed spread by 84% relative to `protected-anchors`, while retaining all ten text contrast requirements. These are measured diagnostic improvements, not a perceptual preference result or a drop-in Base16 compatibility claim.

[Four-way comparison](../../target/photo-fidelity-staged/preview.svg) · [Focused coast comparison](../../target/photo-fidelity-staged/previews/photo-11-focused.png) · [Higher-budget comparison](../../target/photo-fidelity-staged-long/preview.svg) · [Per-photo results](staged-results.csv)

## What changed

The prototype has two explicit data objects. `ScenePalette` contains an unchanged source-anchor solution and all prepared support samples with their original area weights. `ThemePalette` contains the adapted colors, the ten declared text slots, and a per-slot record of the pre-projection target and final color. The source optimizer and extraction pipeline are unchanged. Original evidence remains available even when a theme must brighten a source color.

Two ablations use the same source scene and the same semantic assignments as the protected adapter:

- **`staged-anchors`:** allocate the semantic/source roles first. Project each foreground independently against the actual exported background. Keep hue and chroma fixed, find the smallest upward lightness adjustment on a bounded search, and check actual 8-bit contrast against 4.6:1. Already readable in-gamut targets remain exact. If no such lightness exists in sRGB, reduce chroma as little as the search permits, retaining hue and the semantic chroma floor. Map non-text scene colors into gamut by reducing chroma at fixed L/H. The original slot lightness ceilings do not restrict the projected text colors.
- **`staged-decor`:** use the identical text colors, but declare **base04 decorative** and choose its color from the existing selector's eligible source modes. Pick the candidate that reduces area-weighted support coverage error most; retain the original color if none improves it. The remaining fifteen slots are byte-identical to `staged-anchors`. This ablation keeps the palette at 16 colors.

The projection searches L upward from the target, on a grid with gaps at most 1/1024, then refines a valid entry point. Chroma fallback uses a bounded binary search. This is a lexicographic, numerical policy favoring C/H retention, not an exact global minimum of Oklab displacement. It does not search lower source lightness or optimize relations among all foregrounds jointly. Semantic hue conventions and their existing 3° inset are applied before this projection; retaining target hue does not mean that semantic assignment never moved the original hue.

The decoration search uses support samples as its objective, but candidates must pass the existing merged-mass/source-mode selection. An initial version using all raw samples promoted one isolated red pixel to a full decorative swatch. A failing regression test exposed this, and the final implementation restricts eligibility. The final real-photo and synthetic results below use that restriction.

## Usage and compatibility

The explicit foreground contract remains base03, base05, and base08–base0F against base00, with a final acceptance floor of **4.5:1**. The fixed preview also shows base04 as a narrow decorative border, without placing text over it. That border uses the same slot in every card.

In the decoration variant, base04 may be dark or chromatic and may fall below base03 in lightness. **Consumers expecting an ordered Base16 neutral ladder or using base04 as foreground cannot use this variant unchanged.** This prototype requires a usage-aware theme contract. The production palette registry has not been changed.

## Protocol and controls

Use all 13 [pinned photographs](photos.json), image seed 42, and solve seeds 7/23/42. At 120 iterations and eight starts, compare source-only anchors, the previous protected adapter, staged text adaptation, and staged text plus decoration: 156 palette records. The two staged variants share the source-only solve within each image/seed, so this requires 78 fresh optimizer executions plus 78 projections, rather than 156 optimizer executions.

All 624 source-control HEX values match the preceding source-anchor experiment; all 624 protected-control HEX values match the preceding protected experiment. All downloaded SHA-256 hashes were verified. No photographs were dropped after looking at results.

The 360-iteration check replays the **existing 39 source-only solutions** from `target/photo-fidelity-photos-long`, then runs both new projections. Configuration and master/image seeds are checked, and every freshly prepared support TOML must match the saved input byte for byte. Replay is explicit, not an automatic cache; the source recipe must still be known to match. Current 120-iteration control equality supports that assumption here. The new 360-iteration run therefore contains 78 projected palette records, not 78 fresh optimizer executions. Its total execution time is recorded as NaN and is excluded from timing claims.

As a replay cross-check, the coast was also solved freshly at 360 iterations for all three master seeds. Its source colors and both projected theme records exactly match replay, including all 96 exported HEX values. Those three fresh solves and six projections are saved separately under `target/photo-fidelity-staged-replay-check`.

In staged CSV rows, objective, iterations, gradient, convergence, and seed diagnostics refer to the **source solve**. Projection has no L-BFGS convergence flag. Each staged TOML separates `source_solution`, `scene`, and `theme`; `adaptation_seconds` measures only the new adaptation. Small seed changes are not evidence of JPEG/resize or perceptual stability.

## Results at 120 iterations

| Measurement | Source only | Protected | Staged text | Staged + decoration |
| --- | ---: | ---: | ---: | ---: |
| All ten text slots pass, palettes | 0/39 | 39/39 | 39/39 | 39/39 |
| Individual text checks pass | 204/390 | 390/390 | 390/390 | 390/390 |
| Lowest text contrast | 1.543:1 | 4.661:1 | 4.600:1 | 4.600:1 |
| Full semantic role checks pass | 3/117 | 117/117 | 117/117 | 117/117 |
| Mean source coverage RMSE ↓ | 0.03876 | 0.07070 | 0.06683 | **0.04521** |
| Mean accent-to-source gap ↓ | 0.00339 | 0.01983 | 0.01853 | 0.01853 |
| Median seed spread ↓ | 0.00507 | 0.02411 | **0.00406** | **0.00387** |
| Maximum seed spread ↓ | 0.01820 | 0.03753 | 0.01797 | 0.01608 |
| Raw colors outside sRGB tolerance | 8/624 | 3/624 | 0/624 | 0/624 |

Coverage is the full prepared image's area-weighted nearest-color RMSE to the exported 16-color palette. Accent gap is the reverse nearest-source distance averaged over the eight accents. Seed spread is the largest pairwise role-aligned Oklab RMSE over the three seeds, summarized across photographs. These are the same definitions as in the earlier reports. Source-only colors have no semantic assignment, so semantic check counts there describe compatibility with the role conventions, not a failed promise of that extractor.

Staged text alone improves mean coverage by about **5.5%** relative to protected anchors. Allowing one decorative slot produces a further **32%** reduction relative to staged text, and improves full-image coverage on **all 13 photographs**. Its aggregate error is about 36% below protected anchors, but remains about **17% above source-only anchors**. The decoration ablation changes only base04 in all 39 palettes; it does not improve the eight-accent source-gap measurement because those colors are identical.

The area-weighted representative-sample a/b coverage RMSE to the eight text accents falls from the protected adapter's 0.01581 to **0.01110**, independently of decoration. This supplementary measurement uses support samples rather than full-image pixels. All 390 text projections retain their pre-projection C and H exactly on this corpus; the chroma-reduction fallback is exercised by a separate saturated-blue test. The mean upward L shift is 0.0821 and the maximum is **0.4533**, so the need to brighten dark colors has not disappeared. Relative source lightness structure can still flatten.

### Coast and remaining costs

The coast turquoise `#23a5b3` now exports exactly as that HEX for all three seeds, within **0.00079 Oklab distance** of the prepared support sample. Its input chroma and hue survive projection. Mean coast coverage improves from protected's 0.06497 to **0.04180** with decoration; staged text alone stays at 0.06572. The recovered dark decorative swatch is visible in the palette and its preview border.

The sunset and rainy-street scenes still pay a substantial text-adaptation cost relative to source-only extraction. A small decorative border also occupies little actual interface area; counting its color in palette coverage does not prove that a user would identify the source photo more accurately. The figures support keeping separate source and usage representations, not claiming that a lower coverage number alone makes a better terminal theme.

## Higher-budget check

Both staged variants still pass **390/390** text checks and **117/117** semantic checks at the 360-iteration source budget; both have zero raw colors beyond the 1e-8 gamut tolerance. Staged text's mean coverage is 0.06602 and decoration's is **0.04481**. Decoration's median/max seed spread is 0.00377/0.02090. Some neutral variation therefore persists, and the worst spread does not monotonically improve with extra source iterations.

For each staged variant, the paired 120→360 accent RMSE has median zero at displayed precision and maximum **0.00133**, substantially below protected's previously measured 0.01172 median. The projected accents depend mainly on fixed source evidence and actual background contrast; they are not independently re-optimized at each budget. The reused source solver reports convergence on 9/39 scenes at 360, versus 0/39 at 120. Passing projected output checks does not establish source optimizer convergence.

## Reproduce and verification

Fresh execution of both budgets:

```sh
bash experiments/photo-fidelity/run_photos.sh staged
```

To replay the previously generated source scenes, run the ignored experiment at 360 iterations with `CHROMOXIDE_FIDELITY_PHASE=staged`, `CHROMOXIDE_FIDELITY_VARIANTS=staged-anchors,staged-decor`, and `CHROMOXIDE_FIDELITY_SCENES="$PWD/target/photo-fidelity-photos-long"`. Use the same absolute image path and photo-00 through photo-12 case list as the runner, and a separate output directory. Replay rejects different solve parameters, different support, missing scene files, or reuse of its input directory as output. It does not detect arbitrary historical source-code changes, so it must be explicitly chosen for a compatible source recipe.

After a run, `audit_text.py`, `render.py`, and `compare_budgets.py` produce the text audit, SVGs, source matching, summaries, and paired drift CSV. `rsvg-convert` produces the PNG previews. Full artifacts are under `target/photo-fidelity-staged{,-long}`. The CSV linked above preserves per-photo means at both budgets.

Five new regressions cover dark-color lightening at fixed C/H, readable-color identity, gamut-limited saturated blue, recovery of a missing dark decorative mode, and rejection of isolated-pixel decoration. The final synthetic gray, isolated-red-pixel, and red-plus-green cases also pass all ten text contrasts for all three seeds in both variants: 9/9 palettes per variant, minimum 4.601:1. Workspace tests with all features, Clippy with warnings denied, formatting, and Python/shell syntax checks pass. Both actual-photo acceptance commands exit successfully.

This is still an experimental usage-aware adapter. A next CLI integration should expose explicit source/theme outputs and role usage, preserve the existing Base16 recipe contract, and evaluate an actual consumer before broader promotion. Encoding/resize variants, user judgments, alternate backgrounds, and a full theme accessibility contract remain outside these measurements.
