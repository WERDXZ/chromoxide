#!/usr/bin/env python3
"""Render the opt-in Rust experiment as a standalone SVG and summary CSV.

Uses only the Python standard library. Swatches and card metrics use seed 7;
seed spread and the summary CSV use all three seeds. This is a diagnostic,
not a preference test.
"""

import base64
import argparse
import csv
import html
import itertools
import math
from pathlib import Path
import statistics
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", nargs="?", type=Path, default=Path("target/photo-fidelity"))
    parser.add_argument("--photos", type=Path, help="Downloaded photo manifest with source credits")
    args = parser.parse_args()
    out = args.out
    photo_meta = {}
    if args.photos:
        import json
        photo_meta = {r["file"]: r for r in json.loads(args.photos.read_text())}
    with (out / "inputs.toml").open("rb") as file:
        inputs = tomllib.load(file)
    previews = out / "previews"
    previews.mkdir(exist_ok=True)
    with (out / "metrics.csv").open() as file:
        metrics = list(csv.DictReader(file))
    with (out / "colors.csv").open() as file:
        colors = list(csv.DictReader(file))
    roles = {}
    if (out / "roles.csv").exists():
        with (out / "roles.csv").open() as file:
            for row in csv.DictReader(file):
                roles[row["input"], row["variant"], int(row["seed"]), int(row["slot"])] = row
    text_checks = {}
    if (out / "text-contrast.csv").exists():
        with (out / "text-contrast.csv").open() as file:
            for row in csv.DictReader(file):
                key = row["input"], row["variant"], int(row["seed"])
                text_checks.setdefault(key, []).append(float(row["contrast_hex"]))
    cases = list(dict.fromkeys(row["input"] for row in metrics))
    variants = list(dict.fromkeys(row["variant"] for row in metrics))
    palette = {}
    for row in colors:
        key = (row["input"], row["variant"], int(row["seed"]))
        palette.setdefault(key, {})[int(row["slot"])] = row

    width, row_height = max(820, 240 + 300 * len(variants)), 446
    height = 140 + row_height * len(cases)
    svg = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
           '<style>text{font-family:DejaVu Sans,sans-serif;fill:#202830}.small{font-size:12px}.mono{font-family:DejaVu Sans Mono,monospace;font-size:13px}</style>',
           f'<rect width="{width}" height="{height}" fill="#f3f5f7"/>']

    def text(x, y, content, size=14, fill=None):
        style = f' style="fill:{fill}"' if fill else ""
        svg.append(f'<text x="{x}" y="{y}" font-size="{size}"{style}>{html.escape(str(content))}</text>')

    def rect(x, y, w, h, color, radius=0):
        svg.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{radius}" fill="{color}"/>')

    def code(x, y, tokens):
        spans = "".join(f'<tspan fill="{color}">{html.escape(value)}</tspan>' for value, color in tokens)
        svg.append(f'<text x="{x}" y="{y}" xml:space="preserve" style="font-family:DejaVu Sans Mono,monospace" font-size="11">{spans}</text>')

    text(24, 32, "Chromoxide / photo fidelity ablation", 24)
    text(24, 57, "Base16 recipe experiments. Cards: seed 7. Seed spread: seeds 7, 23, 42. Summary CSV: three-seed means.")
    text(24, 78, "Source gap measures the 8 accents against prepared source pixels. Smaller is closer, not necessarily a better terminal theme.", 12)
    text(24, 95, "Role labels: source = adapted from photo; added = synthesized for the role; n/a = no semantic assignment.", 11)
    summaries = []
    source_matching = []
    for ci, case in enumerate(cases):
        top = 108 + ci * row_height
        row_start = len(svg)
        meta = photo_meta.get(Path(inputs[case]).name)
        text(24, top + 18, f"{case} / {meta['id']}" if meta else case, 12 if meta else 15)
        png = base64.b64encode((out / f"{case}.png").read_bytes()).decode()
        svg.append(f'<image x="24" y="{top+32}" width="176" height="130" href="data:image/png;base64,{png}"/>')
        with (out / f"{case}-samples.csv").open() as file:
            samples = list(csv.DictReader(file))
        with (out / f"{case}-support.toml").open("rb") as file:
            evidence = tomllib.load(file)["samples"]
        x = 24.0
        for sample in samples:
            w = 176 * float(sample["weight"])
            rect(x, top + 178, w, 20, sample["hex"])
            x += w
        text(24, top + 218, f"{len(samples)} samples / area weights", 12)
        if meta:
            text(24, top + 242, meta["category"], 11)
            svg.append(f'<a href="{html.escape(meta["source"], quote=True)}">')
            text(24, top + 264, f"Commons / {meta['license']}", 11)
            svg.append('</a>')

        for vi, variant in enumerate(variants):
            rows = [r for r in metrics if r["input"] == case and r["variant"] == variant]
            assert len(rows) == 3 and {int(r["seed"]) for r in rows} == {7, 23, 42}, "Incomplete run"
            stats = {key: statistics.mean(float(r[key]) for r in rows)
                     for key in metrics[0] if key not in ("input", "variant", "seed", "converged")}
            spread = []
            for sa, sb in itertools.combinations((7, 23, 42), 2):
                pa, pb = palette[case, variant, sa], palette[case, variant, sb]
                spread.append(math.sqrt(sum(sum((float(pa[i][axis]) - float(pb[i][axis]))**2
                                               for axis in ("hex_l", "hex_a", "hex_b"))
                                            for i in range(16)) / 16))
            worst = []
            for seed in (7, 23, 42):
                final = palette[case, variant, seed]
                distances = [[math.sqrt(sum((sample["lab"][axis] - float(final[slot]["hex_" + axis]))**2
                                            for axis in ("l", "a", "b")))
                              for slot in range(16)] for sample in evidence]
                worst.append(max(min(row) for row in distances))
                nearest_source = [min(range(len(evidence)), key=lambda i: distances[i][slot])
                                  for slot in range(8, 16)]
                for index, sample in enumerate(evidence):
                    chromatic_gap = min(math.hypot(sample["lab"]["a"] - float(final[slot]["hex_a"]),
                                                   sample["lab"]["b"] - float(final[slot]["hex_b"]))
                                        for slot in range(8, 16))
                    source_matching.append({"input": case, "variant": variant, "seed": seed,
                                            "sample": index, "area": sample["weight"],
                                            "nearest_output_distance": min(distances[index]),
                                            "nearest_accent_distance": min(distances[index][8:]),
                                            "nearest_accent_chromatic_distance": chromatic_gap,
                                            "nearest_accent_count": nearest_source.count(index)})
            summaries.append({"input": case, "variant": variant, **stats,
                              "seed_role_rmse_max": max(spread),
                              "worst_sample_distance_hex": statistics.mean(worst),
                              "accent_chroma_hex_min": min(float(r["accent_chroma_hex"]) for r in rows),
                              "accent_chroma_hex_max": max(float(r["accent_chroma_hex"]) for r in rows)})
            swatches = palette[case, variant, 7]
            assert len(swatches) == 16
            shown = next(r for r in rows if int(r["seed"]) == 7)
            role = {"baseline": "reference", "source-anchors": "source", "semantic-anchors": "theme", "protected-anchors": "theme", "staged-anchors": "text", "staged-decor": "mixed"}.get(variant, "control")
            converged = "yes" if shown["converged"].lower() == "true" else "no"
            x = 222 + vi * 300
            rect(x, top, 288, 424, "#ffffff", 8)
            text(x + 12, top + 23, f"{variant} [{role}]", 14)
            if variant.startswith("staged-"):
                use = "base04: decoration" if variant == "staged-decor" else "text projected"
                text(x + 12, top + 44, f"scene {shown['iterations']} iters | {use}", 11)
            else:
                text(x + 12, top + 44, f"seed 7 | {shown['iterations']} iters | converged: {converged}", 11)
            text(x + 12, top + 64, f"accent C {float(shown['accent_chroma_hex']):.4f}   source gap {float(shown['accent_source_distance_hex']):.4f}", 12)
            for slot in range(16):
                rect(x + 12 + (slot % 8) * 33, top + 76 + (slot // 8) * 24, 33, 24, swatches[slot]["hex"])
            bg, fg = swatches[0]["hex"], swatches[5]["hex"]
            rect(x + 12, top + 134, 264, 190, bg, 3)
            rect(x + 12, top + 134, 4, 190, swatches[4]["hex"], 2)
            text(x + 22, top + 155, "chromoxide / palette preview", 12, fg)
            text(x + 22, top + 175, "error", 13, swatches[8]["hex"])
            text(x + 91, top + 175, "success", 13, swatches[11]["hex"])
            text(x + 181, top + 175, "link", 13, swatches[13]["hex"])
            role_checks = []
            for slot, offset in ((8, 22), (11, 91), (13, 181)):
                check = roles.get((case, variant, 7, slot))
                if check:
                    role_checks.append(check)
                    origin = {"source-adapted": "source", "synthetic": "added"}.get(check["origin"], "n/a")
                    text(x + offset, top + 190, f"{origin} {float(check['contrast_hex']):.2f}:1", 9, fg)
            code(x + 22, top + 218, [("fn ", swatches[14]["hex"]), ("theme", swatches[13]["hex"]), ("(photo: &", fg), ("Image", swatches[10]["hex"]), (") {", fg)])
            code(x + 22, top + 236, [("  let ", swatches[14]["hex"]), ("accent = ", fg), ('"source";', swatches[11]["hex"])])
            code(x + 22, top + 254, [("  let ", swatches[14]["hex"]), ("slots = ", fg), ("16", swatches[9]["hex"]), (";", fg)])
            code(x + 22, top + 272, [("  // keep the photo character", swatches[3]["hex"])])
            code(x + 22, top + 290, [("}", fg)])
            text(x + 22, top + 311, "foreground on background", 12, fg)
            text(x + 12, top + 344, f"coverage {float(shown['coverage_rmse_hex']):.4f}   contrast {float(shown['bg_fg_contrast_hex']):.2f}:1", 12)
            text(x + 12, top + 364, f"max gap {worst[0]:.4f}   seed spread {max(spread):.4f}", 12)
            if role_checks:
                passed = all(r["passes"].lower() == "true" for r in role_checks)
                distance = min(float(r["nearest_role_distance_hex"]) for r in role_checks)
                text(x + 12, top + 386, f"3 role checks: {'pass' if passed else 'fail'} | distance {distance:.4f}", 11)
            if ratios := text_checks.get((case, variant, 7)):
                assert len(ratios) in (8, 10)  # Older audits only counted accents.
                text(x + 12, top + 406, f"text: {sum(r >= 4.5 for r in ratios)}/{len(ratios)} pass (4.5:1); min {min(ratios):.2f}:1", 11)
        (previews / f"{case}.svg").write_text("\n".join([
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{row_height}" viewBox="0 {top} {width} {row_height}">',
            svg[1], f'<rect y="{top}" width="{width}" height="{row_height}" fill="#f3f5f7"/>',
            *svg[row_start:], '</svg>',
        ]))
    svg.append("</svg>")
    (out / "preview.svg").write_text("\n".join(svg))
    with (out / "summary.csv").open("w", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=list(summaries[0]))
        writer.writeheader()
        writer.writerows(summaries)
    with (out / "source-matching.csv").open("w", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=list(source_matching[0]))
        writer.writeheader()
        writer.writerows(source_matching)
    print(f"Wrote preview.svg, summary.csv, and source-matching.csv in {out}")


if __name__ == "__main__":
    main()
