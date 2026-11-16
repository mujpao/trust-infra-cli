#!/usr/bin/env bash
# Level B reproduction: re-run every experiment from scratch with your own key.
# Needs Docker (running, >= 8 GB memory), OpenTofu, Rust >= 1.85, Python >= 3.11. Takes ~1-1.5 h on CPU.
# Usage: experiments/reproduce.sh
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/.." && pwd)
KEY_PREFIX=${KEY_PREFIX:-"$HERE/keys/local"}

need() { command -v "$1" >/dev/null || { echo "missing: $1 ($2)"; exit 1; }; }
need docker "https://docs.docker.com/get-docker/"
need tofu "https://opentofu.org/docs/intro/install/"
need cargo "https://rustup.rs"
need python3 "Python >= 3.11"
docker info >/dev/null 2>&1 || { echo "Docker is installed but not running"; exit 1; }
python3 -c 'import sys; sys.exit(sys.version_info < (3, 11))' || { echo "needs Python >= 3.11"; exit 1; }

cargo build --release --quiet --manifest-path "$ROOT/Cargo.toml"
# Your own key: your bundles verify with your local.pub, not ours.
[[ -f "$KEY_PREFIX.key" ]] || "$ROOT/target/release/trustinfra" keygen "$KEY_PREFIX"
tofu -chdir="$ROOT/infra" init -input=false >/dev/null

# Clean runs: same recipe three times. Each must verify (false-alarm check).
for i in 1 2 3; do "$HERE/run_eval.sh" "R0-$i"; done

# Drift runs: recipe untouched, what runs differs (D1 image, D2 model compression, D7 manual change).
"$HERE/run_drift.sh" D1 D2 D7

# RQ2: break copies of the clean and drift bundles; writes runs/results/rq2_matrix.{md,csv} (your copy).
python3 "$HERE/scripts/rq2.py" --clean "$HERE/runs/R0-1" \
  --drift "D1=$HERE/runs/D1" --drift "D2=$HERE/runs/D2" --drift "D7=$HERE/runs/D7" \
  --out "$HERE/runs/results"

# RQ1 scorecard is added here once built.

echo "done: outputs in $HERE/runs/ (compare runs/results/ with our experiments/results/)"
