#!/usr/bin/env bash
# Reproduce the pinned photo comparison. fetch_photos.py needs network access.
set -euo pipefail
cd "$(dirname "$0")/../.."
phase=${1:-photos}
case "$phase" in
  photos) output=photo-fidelity-photos; long_variants=source-anchors,semantic-anchors ;;
  protected) output=photo-fidelity-protected; long_variants=protected-anchors ;;
  staged) output=photo-fidelity-staged; long_variants=staged-anchors,staged-decor ;;
  *) echo "Usage: $0 [photos|protected|staged]" >&2; exit 2 ;;
esac
python3 experiments/photo-fidelity/fetch_photos.py
export CHROMOXIDE_FIDELITY_PHASE="$phase"
export CHROMOXIDE_FIDELITY_IMAGES="$PWD/target/photo-fidelity-corpus"
export CHROMOXIDE_FIDELITY_CASES=photo-00,photo-01,photo-02,photo-03,photo-04,photo-05,photo-06,photo-07,photo-08,photo-09,photo-10,photo-11,photo-12
export CHROMOXIDE_FIDELITY_STARTS=8
unset CHROMOXIDE_FIDELITY_VARIANTS
unset CHROMOXIDE_FIDELITY_SCENES
export CHROMOXIDE_FIDELITY_ITERS=120
export CHROMOXIDE_FIDELITY_OUT="$PWD/target/$output"
# A failed final-HEX role check intentionally returns nonzero after all artifacts
# are saved. Preserve that status while still rendering and running the budget audit.
short_status=0
cargo test -p chrox --release photo_fidelity_comparison --offline -- --ignored --nocapture || short_status=$?
python3 experiments/photo-fidelity/audit_text.py "$CHROMOXIDE_FIDELITY_OUT"
python3 experiments/photo-fidelity/render.py "$CHROMOXIDE_FIDELITY_OUT" --photos experiments/photo-fidelity/photos.json
export CHROMOXIDE_FIDELITY_VARIANTS="$long_variants"
export CHROMOXIDE_FIDELITY_ITERS=360
export CHROMOXIDE_FIDELITY_OUT="$PWD/target/$output-long"
long_status=0
cargo test -p chrox --release photo_fidelity_comparison --offline -- --ignored --nocapture || long_status=$?
python3 experiments/photo-fidelity/audit_text.py "$CHROMOXIDE_FIDELITY_OUT"
python3 experiments/photo-fidelity/render.py "$CHROMOXIDE_FIDELITY_OUT" --photos experiments/photo-fidelity/photos.json
python3 experiments/photo-fidelity/compare_budgets.py "target/$output" "target/$output-long"
echo "120-iteration status: $short_status; 360-iteration status: $long_status"
if (( short_status != 0 || long_status != 0 )); then exit 1; fi
