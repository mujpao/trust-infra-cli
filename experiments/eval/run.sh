#!/usr/bin/env bash
# Runs INSIDE the eval container (paper §3.2).
# Inputs come as env vars from experiments/run_eval.sh, which reads them from
# `tofu output -json declared`. Outputs go to /work (the run directory).
set -euo pipefail
# Two phases so a drift case can change the container between them: run.sh install | run.sh eval
PHASE=${1:?usage: run.sh install|eval}

if [[ "$PHASE" == "install" ]]; then
  python -m pip install --quiet --disable-pip-version-check --root-user-action=ignore \
    --require-hashes -r /exp/requirements.txt
  exit 0
fi

# shellcheck disable=SC2086  # TASK_ARGS is intentionally word-split: "-T k=v -T k=v"
inspect eval "$TASK" \
  --model "ollama/$MODEL_TAG" \
  $TASK_ARGS \
  --limit "$LIMIT" \
  --temperature "$TEMPERATURE" \
  --seed "$SEED" \
  --max-tokens "$MAX_TOKENS" \
  --log-dir /work/logs \
  --log-format json \
  --display plain
