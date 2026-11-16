# TrustInfra CLI

**Tamper-evident AI evaluation results.** TrustInfra binds an eval result to the inputs and infrastructure that produced it, so a third party can check that nothing changed after the fact.

## How it works
 
1. **Bundle.** When your eval finishes, collect everything it used and produced (dataset, scorer, prompts, results, logs) into an `artifacts/` folder, next to a [`manifest.json`](#manifest-schema-v01) that records the run metadata plus the infrastructure you *declared* and the infrastructure that *actually* ran.
2. **Sign.** `trustinfra sign` hashes every artifact (SHA-256) into the manifest, then signs the manifest with an [Ed25519](https://en.wikipedia.org/wiki/EdDSA#Ed25519) key.
3. **Verify.** Anyone with your public key runs `trustinfra verify`. It recomputes every hash, checks the signature, and compares declared against actual infrastructure. The verdict is `VERIFIED`, `TAMPERED` or `UNVERIFIABLE`.
`VERIFIED` means the bundle is exactly what the key holder signed. It does not prove the run was honest: someone could sign a fabricated run. What the manifest does give you is a precise record of the environment, so an independent team can re-run the evaluation under the same conditions and compare.

## Install

**Option A: prebuilt binary** Download the build for your platform from the [Releases page](https://github.com/mujpao/trust-infra-cli/releases), rename to trustinfra if desired, then:

```sh
chmod +x trustinfra          # Linux / macOS
./trustinfra --help
```

In a pipeline:

```sh
curl -L -o trustinfra https://github.com/mujpao/trust-infra-cli/releases/download/v0.1.0/trustinfra-x86_64-linux && chmod +x trustinfra
```

**Option B: install with Cargo** (needs [Rust](https://rustup.rs)):

```sh
cargo install --git https://github.com/mujpao/trust-infra-cli --locked
```

**Option C: build from source:**

```sh
git clone https://github.com/mujpao/trust-infra-cli && cd trust-infra-cli
cargo build --release        # binary at target/release/trustinfra
```

The examples below assume `trustinfra` is on your PATH. If it isn't, use `./trustinfra`.

## How to use

1. **(One time) Generate a keypair.**
   `trustinfra keygen mykey` writes `mykey.key` (secret) and `mykey.pub` (public). Keep `*.key` private and never commit it. Share only the `.pub`.
2. **Run your evaluation.** For a worked example that declares the setup with OpenTofu and captures what actually ran, see [Experiments](#experiments).
3. **Assemble the bundle.** Put the run's files in `bundle/artifacts/` and write `bundle/manifest.json` ([schema below](#manifest-schema-v01)).
4. **Sign.** `trustinfra sign ./bundle --key mykey.key`. Don't change the bundle after this.
5. **Verify.** `trustinfra verify ./bundle --trusted-key mykey.pub` (add `--json` for a machine-readable report).

If the CLI prints `VERDICT: VERIFIED`, every file and manifest field is unchanged since signing, and the declared and actual infrastructure match.
 
## Exit codes
 
| Code | Verdict        | Meaning |
|------|----------------|---------|
| 0    | `verified`     | All checks passed |
| 1    | `tampered`     | At least one check failed (hash mismatch, bad signature, unlisted file, unsafe path, infra mismatch) |
| 2    | `unverifiable` | The manifest could not be read or failed schema validation, so later checks could not run. The same code is used for a missing or invalid trusted-key file |
 
`sign` exits 0 on success and 1 on any error.
 
## Manifest schema (v0.1)
 
| Field | Type | Signed? | Notes |
|---|---|---|---|
| `schema_version` | string | yes | `"0.1"`. Verifier rejects unknown versions. |
| `run_id` | string | yes | Unique ID for the eval run. |
| `timestamp` | string | yes | RFC 3339, UTC. |
| `model` | string | yes | Model under evaluation. |
| `scorer_commit` | string | yes | Git commit of the scorer code. |
| `judge_model` | string | yes | The LLM judge. |
| `grading_prompt` | string | yes | Prompt used to instruct the LLM judge. |
| `artifacts` | object | yes | Map of name to `{path, sha256}`. Paths are relative to the bundle directory. |
| `infra_declared` | object | yes | Declared environment (OpenTofu config, image digest, packages). |
| `infra_actual` | object | yes | Applied state captured after the run. Compared against `infra_declared`. |
| `signature` | string | n/a | Base64 (standard, padded) ed25519 signature over the JCS-canonical manifest with this field removed. |
 
### Schema Rules
 
- Files get a SHA-256 in `artifacts`. Plain values (model name, commit, etc.) live directly in the signed manifest.
- Unknown top-level fields are rejected during manifest parsing (`deny_unknown_fields`), and the verification fails as `UNVERIFIABLE`.
- Floats are not allowed in the manifest. Scores stay in the hashed results file, which avoids JCS number-formatting ambiguity.
- Artifact paths must be relative, with no `..`, and must not be symlinks.
- Every file under `artifacts/` must be listed. Unlisted files fail the `unlisted_files` check.
## Example `manifest.json`
 
```json
{
  "schema_version": "0.1",
  "run_id": "run-2026-11-14-001",
  "timestamp": "2026-11-14T22:15:03Z",
  "model": "example-model-v1",
  "scorer_commit": "3f9c2a7d1e5b8046a9c3d7e2f1b50a84c6d9e3f1",
  "judge_model": "example-judge-v1",
  "grading_prompt": "Grade the answer as correct or incorrect.",
  "artifacts": {
    "dataset": {
      "path": "artifacts/dataset.json",
      "sha256": "9f2b7c1d4e8a3b6055c7e1a2d9f04b8c3a6e5d7f1b2c4a8e0d9f3b6a7c5e1d2f"
    },
    "scorer": {
      "path": "artifacts/scorer.py",
      "sha256": "1a3c5e7f9b2d4f608a1c3e5b7d9f0a2c4e6b8d1f3a5c7e9b0d2f4a6c8e1b3d5f"
    },
    "prompts": {
      "path": "artifacts/prompts.json",
      "sha256": "b4d6f801a3c5e7f92b4d6f801a3c5e7f9b2d4f608a1c3e5b7d9f0a2c4e6b8d1f"
    },
    "results": {
      "path": "artifacts/results.json",
      "sha256": "5c7e9b0d2f4a6c8e1b3d5f7a9c0e2b4d6f8a1c3e5b7d9f0a2c4e6b8d1f3a5c7e"
    },
    "log": {
      "path": "artifacts/log.json",
      "sha256": "d1f3a5c7e9b0d2f4a6c8e1b3d5f7a9c0e2b4d6f8a1c3e5b7d9f0a2c4e6b8d1f3"
    }
  },
  "infra_declared": {
    "image": "sha256:7a1e4c9b2d5f8a036c9e1b4d7f0a3c5e8b2d4f619a3c5e7b0d2f4a681c3e5b7d",
    "packages": {
      "inspect-ai": "0.3.100",
      "python": "3.12.4"
    }
  },
  "infra_actual": {
    "image": "sha256:7a1e4c9b2d5f8a036c9e1b4d7f0a3c5e8b2d4f619a3c5e7b0d2f4a681c3e5b7d",
    "packages": {
      "inspect-ai": "0.3.100",
      "python": "3.12.4"
    }
  },
  "signature": "BASE64_ED25519_SIGNATURE_88_CHARS"
}
```
 
(`signature` is added by `sign`; leave it out of the manifest you write.)
 
## Bundle layout
 
A bundle is a directory:
 
```
bundle/
├── manifest.json        # signed
└── artifacts/
    ├── results.json     # scores live here (hashed), not in the manifest
    ├── scorer.py
    └── dataset.json
```

## Experiments

An end-to-end run on a real evaluation: TruthfulQA MC1 with Inspect, against `qwen2.5:0.5b` served by Ollama. Both run in Docker containers declared with OpenTofu.

```
infra/                  OpenTofu recipe; declared.auto.tfvars is the declared setup
experiments/
  eval/                 runs inside the eval container (run.sh, requirements.txt)
  scripts/              capture_actual.py (what actually ran), bundle.py (packs the bundle)
  keys/demo.pub         public key for the experiment bundles
  run_eval.sh           one run: tofu apply → eval → capture → bundle → sign → verify
  reproduce.sh          all runs from scratch (level B)
  verify_results.sh     check our published bundles (level A)
  results/              our sealed bundles and tables (committed)
  runs/<run_id>/        your local outputs (not committed)
```

### Reproduce

**A. Check our results** (Rust ≥ 1.85 only, ~2 min). Verifies every published bundle with `experiments/keys/demo.pub` and compares the verdict with the one we recorded:

```sh
experiments/verify_results.sh
```

**B. Re-run everything** (Docker with ≥ 8 GB memory, [OpenTofu](https://opentofu.org), Rust ≥ 1.85, Python ≥ 3.11; ~1–1.5 h on CPU). Generates your own key, so your bundles verify with your `local.pub`, not ours:

```sh
experiments/reproduce.sh
```

For a 2-minute smoke test instead of the full reproduction, set up the release binary, local key, and OpenTofu state first:

```sh
cargo build --release
target/release/trustinfra keygen experiments/keys/local
tofu -chdir=infra init -input=false
experiments/run_eval.sh smoke --limit 5
```

Set `KEY_PREFIX` to use a different key path; it defaults to `experiments/keys/local`.

The host scripts use only the Python standard library. Poetry is needed only to change versions: edit `pyproject.toml`, run `poetry lock`, then `poetry export --without dev -o experiments/eval/requirements.txt` (needs `poetry-plugin-export`).

**What can legitimately differ on your machine**
- Scores can differ slightly on another CPU. Detection results (RQ2) do not depend on the score.
- If Ollama's registry has changed `qwen2.5:0.5b` since our runs, your clean runs report `TAMPERED` with `model_digest` in the infra diff. That is the check working: what ran differs from the recipe.
- Tested on Apple M1 Pro (arm64, Docker Desktop). Image digests are multi-arch, but x86-64/Linux is untested.

`infra_actual` is measured from the running system (`docker inspect`, Ollama's model list, installed package versions), not copied from the recipe. `experiments/runs/<run_id>/raw/capture.json` records the source of each value.
