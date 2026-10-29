# Real-photo comparison — 2026-10-07

**Source anchoring improves the measured color coverage on this corpus, but the semantic adapter is not ready to be a general terminal theme.** Two concrete gaps remain: it can discard a distinctive source color to make room for semantic roles, and it only constrains three of the eight accent colors for text contrast. More iterations do not remove either problem.

## Corpus and protocol

13 photographs were selected for fog, snow, night lighting, sunsets, foliage, a red flower, a coast, and a rainy street. All were retained after selection. Wikimedia supplied the JPEG thumbnails; [photos.json](photos.json) pins their bytes with SHA-256 and records original pages, authors, and licenses as reported by Commons (CC0 or public domain). No recoloring or crop was added locally. The supplied thumbnail then follows the existing CLI preprocessing path (maximum dimension 256) and its 24-color support extraction. This is a curated diagnostic set, not a random sample or a user preference study.

Each image was run with the existing Base16 recipe (`baseline`), relaxed hue/chroma bounds with linear Support weights (`relaxed-linear`), source anchors, and semantic anchors. All four use eight starts and 120 iterations, with image seed 42 and master solve seeds 7, 23, 42: **156 solves**. The two anchor variants were also run at 360 iterations with the same starts, preparation, and seeds: **78 more solves**. This is an equal-budget Base16 comparison, not a run at the CLI's larger default start count. Recipe/solver objectives were not retuned during this evaluation.

[Full 120-iteration contact sheet](../../target/photo-fidelity-photos/preview.svg) · [360-iteration anchor comparison](../../target/photo-fidelity-photos-long/preview.svg) · [Numerical results](photo-results.csv)

Previews use seed 7 for both swatches and card measurements. Each uses the same code/terminal layout and slot mapping, without changing rendered colors to improve legibility. The three-role checks concern error/success/link; the separate text row checks all eight accent slots against base00. Supplementary statistics include all three seeds.

## Aggregate results at 120 iterations

| Variant | Coverage RMSE ↓ | Accent-to-source gap ↓ | Median seed spread ↓ | All 8 text accents pass | Solver-converged runs |
| --- | ---: | ---: | ---: | ---: | ---: |
| baseline | 0.05736 | 0.04833 | 0.02172 | 15/39 | 6/39 |
| relaxed-linear | 0.05211 | 0.00656 | 0.06104 | 13/39 | 7/39 |
| source-anchors | 0.03876 | 0.00339 | 0.00507 | 0/39 | 0/39 |
| semantic-anchors | 0.04696 | 0.01640 | 0.00403 | 3/39 | 0/39 |

Coverage is the area-weighted RMSE from prepared source pixels to their nearest exported color among all 16 slots. Accent gap is the mean distance from each of eight exported accents to the nearest prepared source pixel. These have different directions and neither proves perceptual fidelity. Seed spread is the maximum pairwise role-aligned Oklab RMSE across all 16 slots, summarized here by the median over photos. All exported measurements include clipping and 8-bit quantization. The 4.5:1 text threshold and hue/distance rules are experimental criteria, not a complete accessibility certification.

Source-anchor mean coverage error is about **32% lower** than the equal-budget Base16 baseline, and improves on all 13 photos. Semantic-anchor mean error is about **18% lower**, improving on 12/13 (Las Vegas is slightly worse). The added semantic colors increase the mean accent-to-source gap from 0.00339 to 0.01640 compared with source anchors. Background and neutral-lightness structure still largely follow the fixed recipe.

## Findings that change the next implementation step

### 1. The adapter can erase a distinctive color already extracted successfully

On [the coast preview](../../target/photo-fidelity-photos/previews/photo-11.png), a turquoise source mode `#23a5b3` accounts for **2.903%** of prepared support mass. At seed 7, source anchors reserve a slot within **0.00087 Oklab distance** of it. After semantic adaptation, no accent has it as its nearest source sample, and the nearest accent is **0.09148** away. The teal swatch visibly disappears.

The source selector did find this mode. Reserving two synthetic roles leaves fewer free seats, and the subsequent mode allocation drops it. The adapter currently has no requirement to retain a distinctive selected source mode. Better extraction alone would not fix this example. The color/role/area evidence is in the per-run provenance and source-matching CSV files.

### 2. Three readable status colors do not make readable code

All **117/117** semantic role/background contrasts exceed 4.5:1 at both budgets. Yet only **3/39 palettes** have all eight accent text colors above that floor, at both 120 and 360 iterations. The worst accent ratio is **1.62:1** at 120 iterations. Keywords or other syntax mapped to the five unconstrained source slots can remain difficult to read; the code previews show this directly. The all-accent audit is hypothetical use as foreground text, not a requirement that every decorative color must have text contrast.

The baseline has this issue too (15/39 complete accent sets pass), so its palette is not a ready-made usability gold standard. The fixed base03 comment color also remains dim in these previews; it is outside the eight-accent count above. A real theme contract needs to specify all text roles and their actual background combinations.

### 3. Final HEX hue checks expose a boundary issue

The semantic variant passes **116/117** full three-role checks at 120 iterations and **109/117** at 360. Both opt-in experiment commands therefore return nonzero **after saving all results**. All failures are error-role hue checks: ocean sunset at 120; ocean sunset, autumn, and rainy street at 360. Contrast, chroma, and pair distance pass.

These are small boundary crossings, not reversals such as green becoming red. Continuous hues near the 45° arc endpoint export to roughly **45.05–46.49°** after 8-bit conversion. For example, ocean sunset seed 23 exports `#897166` at about 46.01°. Increasing the iteration budget can drive the solution closer to the endpoint and increase the strict failure count. Output-aware margins or a justified quantization tolerance are needed; changing the threshold just to turn this run green would not establish visual correctness. Some source-adapted error colors also look brown rather than clearly red, which deserves a separate semantic judgment.

### 4. More iterations change the colors relatively little

| Variant | Median accent RMSE, 120 → 360 | Maximum paired accent RMSE | Solver convergence at 360 |
| --- | ---: | ---: | ---: |
| source-anchors | 0.00175 | 0.00644 | 9/39 |
| semantic-anchors | 0.00244 | 0.00489 | 5/39 |

Each pair compares the same master seed and byte-identical prepared support. These small paired changes support working on the missing theme constraints first; they do not establish global convergence. Different-seed variation remains: the semantic maximum role-aligned spread is 0.02254 at 120 iterations and 0.02441 at 360, both for winter sunset. Additional iterations do not make that spread disappear.

## Visual reading

This is an engineering visual review, not a blinded preference result. [Fog](../../target/photo-fidelity-photos/previews/photo-00.png), [fern](../../target/photo-fidelity-photos/previews/photo-08.png), and [red sunset](../../target/photo-fidelity-photos/previews/photo-07.png) retain clearer differences in the source-anchor swatches than in the fixed-hue baseline. [The poppy](../../target/photo-fidelity-photos/previews/photo-10.png) keeps its red accent with source anchors while the relaxed/linear control loses it. The semantic adapter correctly assigns that red to error. In the coast and neon scenes, however, its muted semantic additions and reduced source capacity still weaken some characteristic colors. Better average distance scores must not be treated as proof that these local features survived.

## Per-photo coverage

All values below average seeds 7, 23, 42 at 120 iterations.

| Photo / preview | Base16 | Source anchors | Semantic anchors |
| --- | ---: | ---: | ---: |
| [fog-road](../../target/photo-fidelity-photos/previews/photo-00.png) | 0.0379 | 0.0235 | 0.0284 |
| [mountain-mist](../../target/photo-fidelity-photos/previews/photo-01.png) | 0.0440 | 0.0299 | 0.0385 |
| [winter-mountain](../../target/photo-fidelity-photos/previews/photo-02.png) | 0.0495 | 0.0347 | 0.0405 |
| [winter-sunset](../../target/photo-fidelity-photos/previews/photo-03.png) | 0.0525 | 0.0448 | 0.0488 |
| [neon-streets](../../target/photo-fidelity-photos/previews/photo-04.png) | 0.0421 | 0.0363 | 0.0407 |
| [neon-vegas](../../target/photo-fidelity-photos/previews/photo-05.png) | 0.0466 | 0.0454 | 0.0491 |
| [ocean-sunset](../../target/photo-fidelity-photos/previews/photo-06.png) | 0.0642 | 0.0436 | 0.0539 |
| [red-sunset](../../target/photo-fidelity-photos/previews/photo-07.png) | 0.0709 | 0.0359 | 0.0408 |
| [fern](../../target/photo-fidelity-photos/previews/photo-08.png) | 0.0977 | 0.0336 | 0.0592 |
| [autumn](../../target/photo-fidelity-photos/previews/photo-09.png) | 0.0442 | 0.0323 | 0.0398 |
| [poppy](../../target/photo-fidelity-photos/previews/photo-10.png) | 0.0845 | 0.0566 | 0.0736 |
| [coast](../../target/photo-fidelity-photos/previews/photo-11.png) | 0.0515 | 0.0338 | 0.0403 |
| [rain-street](../../target/photo-fidelity-photos/previews/photo-12.png) | 0.0600 | 0.0534 | 0.0568 |

## Recommended next change

Keep a protected set of source modes through theme adaptation, with the coast turquoise as a regression case. If a semantic slot consumes a seat, explicitly decide which source modes can be merged or represented by that role; do not silently evict an already selected distinctive mode. Add foreground/background requirements to every slot actually used as text, allowing minimal source lightness adaptation and checking the final export. Re-run the same pinned corpus before exposing this as a general CLI theme mode.

The comparison does not yet justify a general source-identification or perceptual-stability claim. JPEG re-encoding, rescaling variants, user judgments, color-vision accessibility, broader corpora, and a complete gamut strategy remain outside this run.

## Reproduce and inspect

```sh
bash experiments/photo-fidelity/run_photos.sh
```

The script downloads/verifies pinned files, runs both budgets, renders SVG previews, audits all accent contrasts, compares budgets, and preserves the failed experiment status. Use `rsvg-convert` on the generated SVGs to obtain PNGs. Full solution TOML, colors, metrics, role checks, provenance, and support evidence remain under `target/photo-fidelity-photos{,-long}`. The downloaded images are under `target/photo-fidelity-corpus` and are not repository assets. Regular fidelity unit tests, Clippy with warnings denied, formatting, and Python/shell syntax checks passed. The opt-in real-photo acceptance checks intentionally remain failing as described above.

## Photo credits

License labels below reproduce the Commons metadata snapshot; original pages and exact downloaded bytes are pinned in [photos.json](photos.json).

| ID | Original page | Author | License |
| --- | --- | --- | --- |
| photo-00 | [fog-road](https://commons.wikimedia.org/wiki/File:Fog_(Unsplash_7CME6Wlgrdk).jpg) | Annie Spratt anniespratt | CC0 |
| photo-01 | [mountain-mist](https://commons.wikimedia.org/wiki/File:Mountain_mist_(876bb9ced7944bccba4f936d9584ed82).JPG) | Steve Redman (MORA) | Public domain |
| photo-02 | [winter-mountain](https://commons.wikimedia.org/wiki/File:Cutoff_Mountain_in_winter.jpg) | Yellowstone National Park NPS / Jacob W. Frank | Public domain |
| photo-03 | [winter-sunset](https://commons.wikimedia.org/wiki/File:Winter_forest_at_sunset_(52590345787).jpg) | Aleksandr Gorlov | Public domain |
| photo-04 | [neon-streets](https://commons.wikimedia.org/wiki/File:Neon_streets_(Unsplash).jpg) | JINDONG H jindong | CC0 |
| photo-05 | [neon-vegas](https://commons.wikimedia.org/wiki/File:City_of_Las_Vegas,_Las_Vegas_Boulevard_State_Scenic_Byway_-_Neon_Night_in_Las_Vegas_-_NARA_-_7717640.jpg) | Not provided in the NARA/Commons record | Public domain |
| photo-06 | [ocean-sunset](https://commons.wikimedia.org/wiki/File:Sunset_over_ocean_(27717086074).jpg) | Lisa Ann Yount from El Cerrito, CA, United States | CC0 |
| photo-07 | [red-sunset](https://commons.wikimedia.org/wiki/File:Sunset_over_an_ocean_(Corp2739).jpg) | Commander John Bortniak, NOAA Corps (ret.) | Public domain |
| photo-08 | [fern](https://commons.wikimedia.org/wiki/File:Leaves_of_a_green_fern.jpg) | Joselodos | CC0 |
| photo-09 | [autumn](https://commons.wikimedia.org/wiki/File:Autumn_leaves_(Unsplash).jpg) | Annie Spratt anniespratt | CC0 |
| photo-10 | [poppy](https://commons.wikimedia.org/wiki/File:Red_Poppy_(60672274).jpeg) | Stefanie Leuker | CC0 |
| photo-11 | [coast](https://commons.wikimedia.org/wiki/File:Beaches_of_Jeju_-_JejuBeaches2458.jpg) | lumoplank | CC0 |
| photo-12 | [rain-street](https://commons.wikimedia.org/wiki/File:Umbrella_California_St_rain_(Unsplash).jpg) | Todd Diemer todd_diemer | CC0 |
