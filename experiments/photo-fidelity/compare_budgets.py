#!/usr/bin/env python3
"""Compare final HEX colors at two solver budgets with identical photo/seed inputs.

Run render.py on each completed experiment first. No perceptual pass threshold
is imposed on the Oklab deltas: this reports how much the output changes.
"""
import argparse
import csv
import math
from pathlib import Path
import statistics


def read(path):
    with path.open() as file:
        return list(csv.DictReader(file))


def palettes(out):
    result = {}
    for row in read(out / "colors.csv"):
        key = row["input"], row["variant"], int(row["seed"])
        slots = result.setdefault(key, {})
        slot = int(row["slot"])
        assert slot not in slots, f"Duplicate slot: {key}/{slot}"
        slots[slot] = tuple(float(row[axis]) for axis in ("hex_l", "hex_a", "hex_b"))
    assert all(set(slots) == set(range(16)) for slots in result.values()), "Incomplete palette"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("short", type=Path)
    parser.add_argument("long", type=Path)
    args = parser.parse_args()
    short, long = palettes(args.short), palettes(args.long)
    variants = {key[1] for key in long}
    expected = {key for key in short if key[1] in variants}
    assert long and long.keys() == expected, "Budgets have different photo/variant/seed inputs"
    # Preparation must be identical; otherwise this would confound image and solver changes.
    for case in {key[0] for key in long}:
        assert (args.short / f"{case}-support.toml").read_bytes() == (args.long / f"{case}-support.toml").read_bytes()
    rows = []
    for (case, variant, seed), after in long.items():
        before = short[case, variant, seed]
        distances = [math.dist(before[i], after[i]) for i in range(16)]
        rows.append({"input": case, "variant": variant, "seed": seed,
                     "all_slots_rmse": math.sqrt(statistics.mean(d*d for d in distances)),
                     "accent_rmse": math.sqrt(statistics.mean(d*d for d in distances[8:])),
                     "max_slot_distance": max(distances),
                     "max_semantic_role_distance": max(distances[i] for i in (8, 11, 13))})
    path = args.long / "budget-drift.csv"
    with path.open("w", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    for variant in sorted({r["variant"] for r in rows}):
        selected = [r for r in rows if r["variant"] == variant]
        print(variant, "paired runs", len(selected), "median accent RMSE",
              f"{statistics.median(r['accent_rmse'] for r in selected):.5f}",
              "max accent RMSE", f"{max(r['accent_rmse'] for r in selected):.5f}")
    print(f"Wrote {path}")


if __name__ == "__main__":
    main()
