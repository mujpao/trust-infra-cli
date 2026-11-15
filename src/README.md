## Manifest schema (v0.1)

| Field | Type | Signed? | Notes |
|---|---|---|---|
| `schema_version` | string | yes | `"0.1"`. Verifier rejects unknown versions. |
| `run_id` | string | yes | Unique ID for the eval run. |
| `timestamp` | string | yes | RFC 3339, UTC. |
| `model` | string | yes | Model under evaluation. |
| `scorer_commit` | string | yes | Git commit of the scorer code. |
| `judge_model` | string | yes | Only if the scorer is LLM-graded. |
| `grading_prompt` | string | yes | Only if the scorer is LLM-graded. |
| `artifacts` | object | yes | Map of name to `{path, sha256}`. Paths are relative to the bundle directory. |
| `infra_declared` | object | yes | Declared environment (OpenTofu config, image digest, packages). |
| `infra_actual` | object | yes | Applied state captured after the run. Compared against `infra_declared`. |
| `signature` | string | n/a | Base64 (standard, padded) ed25519 signature over the JCS-canonical manifest with this field removed. |

### Rules

- Files get a SHA-256 in `artifacts`. Plain values (model name, commit, etc.) live directly in the signed manifest.
- Unknown top-level fields are rejected (`deny_unknown_fields`) and also break the signature.
- No floats in the manifest. Scores stay in the hashed results file, which avoids JCS number-formatting ambiguity.
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

## Bundle layout

```
my_run/
  manifest.json
  artifacts/
    dataset.json
    scorer.py
    prompts.json
    results.json
    log.json
```