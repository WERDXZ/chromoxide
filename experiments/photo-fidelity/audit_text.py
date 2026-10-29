#!/usr/bin/env python3
"""Measure final HEX contrast for accents, comments and main foreground text.

Uses the same sRGB transfer function and luminance coefficients as the Rust
pipeline. The 4.5 threshold is the experiment's acceptance criterion; color
meaning and other accessibility requirements are outside this check.
"""
import argparse
import csv
from pathlib import Path
import statistics

from compare_budgets import read


def luminance(value):
    def linear(channel):
        c = channel / 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055)**2.4
    return sum(k * linear(int(value[i:i+2], 16))
               for k, i in zip((0.2126, 0.7152, 0.0722), (1, 3, 5)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    colors = {}
    for row in read(args.out / "colors.csv"):
        colors[row["input"], row["variant"], row["seed"], int(row["slot"])] = row["hex"]
    role_checks = {(r["input"], r["variant"], r["seed"], int(r["slot"])): r
                   for r in read(args.out / "roles.csv")}
    rows = []
    groups = {}
    for (case, variant, seed, slot), color in colors.items():
        if slot not in (3, 5, *range(8, 16)):
            continue
        key = case, variant, seed
        bg = luminance(colors[*key, 0])
        fg = luminance(color)
        ratio = (max(bg, fg) + 0.05) / (min(bg, fg) + 0.05)
        if (check := role_checks.get((*key, slot))) is not None:
            assert abs(ratio - float(check["contrast_hex"])) < 1e-5, "Python/Rust contrast mismatch"
        rows.append(dict(input=case, variant=variant, seed=seed, slot=slot, hex=color,
                         contrast_hex=ratio, passes=ratio >= 4.5))
        groups.setdefault(key, []).append(ratio)
    assert groups and all(len(ratios) == 10 for ratios in groups.values()), "Incomplete text set"
    with (args.out / "text-contrast.csv").open("w", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    for variant in dict.fromkeys(row["variant"] for row in rows):
        ratios = [min(values) for (_, v, _), values in groups.items() if v == variant]
        print(variant, "all-ten pass", sum(r >= 4.5 for r in ratios), "/", len(ratios),
              "minimum", f"{min(ratios):.3f}", "median minimum", f"{statistics.median(ratios):.3f}")


if __name__ == "__main__":
    main()
