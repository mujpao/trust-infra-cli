#!/usr/bin/env bash
# One run end to end: tofu apply -> pull model -> install -> eval -> capture -> bundle -> sign -> verify.
# Usage: experiments/run_eval.sh <run_id> [--limit N]   (--limit only for smoke tests)
# Drift runs (see run_drift.sh) set:
#   TF_EXTRA_VARS      extra `-var k=v` for tofu apply (changes what is applied, not the recipe)
#   BEFORE_EVAL_HOOK   shell command run after install, right before the eval
set -euo pipefail

RUN_ID=${1:?usage: run_eval.sh <run_id> [--limit N]}
LIMIT_OVERRIDE=""
if [[ "${2:-}" == "--limit" ]]; then LIMIT_OVERRIDE=${3:?}; fi

HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/.." && pwd)
INFRA="$ROOT/infra"
TRUSTINFRA="$ROOT/target/release/trustinfra"
KEY_PREFIX=${KEY_PREFIX:-"$HERE/keys/local"}
RUN_DIR="$HERE/runs/$RUN_ID"
mkdir -p "$RUN_DIR"

# Declared setup = the committed tfvars, read before apply and without overrides.
echo 'jsonencode(local.declared)' |
  tofu -chdir="$INFRA" console -var "run_dir=$RUN_DIR" |
  python3 -c "import json,sys; print(json.dumps(json.loads(json.loads(sys.stdin.read())), indent=2))" \
    >"$RUN_DIR/declared.json"
DECLARED=$(cat "$RUN_DIR/declared.json")
get() { python3 -c "import json,sys; print(json.loads(sys.argv[1])[sys.argv[2]])" "$DECLARED" "$1"; }

# shellcheck disable=SC2086  # TF_EXTRA_VARS is intentionally word-split
tofu -chdir="$INFRA" apply -auto-approve -input=false -var "run_dir=$RUN_DIR" ${TF_EXTRA_VARS:-} \
  -replace=docker_container.ollama -replace=docker_container.eval >"$RUN_DIR/tofu_apply.log"

MODEL_TAG=$(get model_tag)
TASK_ARGS=$(python3 -c "import json,sys; print(' '.join(f'-T {k}={v}' for k,v in json.loads(sys.argv[1])['task_args'].items()))" "$DECLARED")
LIMIT=${LIMIT_OVERRIDE:-$(get limit)}

# Wait for the Ollama server inside the container to accept requests.
for i in $(seq 1 60); do
  docker exec trustinfra-ollama ollama list >/dev/null 2>&1 && break
  [[ $i -eq 60 ]] && { echo "ollama did not become ready in 60s"; docker logs trustinfra-ollama | tail -20; exit 1; }
  sleep 1
done

# The model volume persists; pull re-syncs the tag with the registry.
docker exec trustinfra-ollama ollama pull "$MODEL_TAG" >/dev/null
docker exec trustinfra-eval bash /exp/run.sh install

if [[ -n "${BEFORE_EVAL_HOOK:-}" ]]; then
  echo "drift hook: $BEFORE_EVAL_HOOK" | tee "$RUN_DIR/drift_hook.txt"
  bash -c "$BEFORE_EVAL_HOOK"
fi

docker exec \
  -e TASK="$(get task)" -e MODEL_TAG="$MODEL_TAG" -e TASK_ARGS="$TASK_ARGS" \
  -e LIMIT="$LIMIT" -e TEMPERATURE="$(get temperature)" -e SEED="$(get seed)" -e MAX_TOKENS="$(get max_tokens)" \
  trustinfra-eval bash /exp/run.sh eval 2>&1 | tee "$RUN_DIR/eval_stdout.log"

# Record what actually ran, pack, seal, check.
python3 "$HERE/scripts/capture_actual.py" "$RUN_DIR"
python3 "$HERE/scripts/bundle.py" "$RUN_DIR" "$RUN_ID"
"$TRUSTINFRA" sign "$RUN_DIR/bundle" --key "$KEY_PREFIX.key"
set +e
"$TRUSTINFRA" verify "$RUN_DIR/bundle" --trusted-key "$KEY_PREFIX.pub" --json >"$RUN_DIR/verify.json"
echo "verify exit code: $?"
set -e
python3 -c "import json,sys; r=json.load(open(sys.argv[1])); print('VERDICT:', r['verdict'].upper()); [print('  infra diff:', d['path'], d['declared'], '->', d['actual']) for d in r['infra_diff']]" "$RUN_DIR/verify.json"

echo "run $RUN_ID done: $RUN_DIR"
