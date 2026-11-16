#!/usr/bin/env bash
# Drift runs (paper §3.5): the recipe file stays untouched, but what actually runs differs.
#   D1  a different container image than the pinned one (as when a tag like "latest" moves):
#       Ollama 0.40.15 instead of the declared 0.40.16
#   D2  same model name, different compression: qwen2.5:0.5b now points to the 8-bit build
#   D7  setup changed by hand outside OpenTofu: inspect-ai downgraded with pip inside the container
# Usage: experiments/run_drift.sh [D1|D2|D7 ...]   (default: all three)
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)

D1_IMAGE="ollama/ollama@sha256:0e59999e0e8f9fd4e704bd788a89b7e4cad399367fb1365f8c102ac662288761" # 0.40.15
D2_MODEL="qwen2.5:0.5b-instruct-q8_0"
D7_PACKAGE="inspect-ai==0.3.289"

restore_model() {
  # Put the declared 4-bit model back under its name (the model volume persists across runs).
  docker exec trustinfra-ollama ollama rm qwen2.5:0.5b >/dev/null 2>&1 || true
  docker exec trustinfra-ollama ollama pull qwen2.5:0.5b >/dev/null
}

for case in "${@:-D1 D2 D7}"; do
  for c in $case; do
    echo "=== $c $(date +%H:%M:%S)"
    case "$c" in
      D1) TF_EXTRA_VARS="-var ollama_image=$D1_IMAGE" "$HERE/run_eval.sh" D1 ;;
      D2) BEFORE_EVAL_HOOK="docker exec trustinfra-ollama ollama pull $D2_MODEL >/dev/null && docker exec trustinfra-ollama ollama cp $D2_MODEL qwen2.5:0.5b" \
            "$HERE/run_eval.sh" D2
          restore_model ;;
      D7) BEFORE_EVAL_HOOK="docker exec trustinfra-eval python -m pip install --quiet --no-deps --root-user-action=ignore $D7_PACKAGE" \
            "$HERE/run_eval.sh" D7 ;;
      *) echo "unknown case $c"; exit 1 ;;
    esac
  done
done
