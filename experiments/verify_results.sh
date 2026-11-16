#!/usr/bin/env bash
# Level A reproduction: check our published, sealed results. Needs Rust only.
# Every bundle in experiments/results/bundles/ is verified with experiments/keys/demo.pub,
# and the verdict is compared with the one we recorded.
# Usage: experiments/verify_results.sh
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/.." && pwd)
RESULTS="$HERE/results"

command -v cargo >/dev/null || { echo "needs Rust >= 1.85 (https://rustup.rs)"; exit 1; }
cargo build --release --quiet --manifest-path "$ROOT/Cargo.toml"
TRUSTINFRA="$ROOT/target/release/trustinfra"

[[ -d "$RESULTS/bundles" ]] || { echo "no published bundles in $RESULTS/bundles yet"; exit 1; }

fail=0
for bundle in "$RESULTS"/bundles/*/; do
  id=$(basename "$bundle")
  expected=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['verdict'])" "$RESULTS/verdicts/$id.json")
  set +e
  got=$("$TRUSTINFRA" verify "$bundle" --trusted-key "$HERE/keys/demo.pub" --json |
        python3 -c "import json,sys; print(json.load(sys.stdin)['verdict'])")
  set -e
  if [[ "$got" == "$expected" ]]; then mark=ok; else mark=MISMATCH; fail=1; fi
  printf '%-40s expected %-13s got %-13s %s\n' "$id" "$expected" "$got" "$mark"
done

exit $fail
