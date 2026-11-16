#!/usr/bin/env python3
"""Pack one run into a TrustInfra bundle (paper §3.1 step 3, the "packing list").

<run_dir>/bundle/
  manifest.json      run metadata + infra_declared + infra_actual (signed later by `trustinfra sign`)
  artifacts/         every file of the run; `sign` writes each file's SHA-256 into the manifest

Usage: bundle.py <run_dir> <run_id>
"""

import datetime
import json
import pathlib
import shutil
import subprocess
import sys

EVAL = "trustinfra-eval"
EXP = pathlib.Path(__file__).resolve().parent.parent
SITE = "/usr/local/lib/python3.13/site-packages"
HF_SNAPSHOT = ("/hf/hub/datasets--truthfulqa--truthful_qa/snapshots/{rev}/multiple_choice/"
               "validation-00000-of-00001.parquet")
DATASET_REVISION = "741b8276f2d1982aa3d5b832d3ee81ed3b896490"  # pinned in inspect_evals 0.29.0


def norm(value):
    """One string form for every leaf, so declared and actual compare like with like."""
    if isinstance(value, dict):
        return {k: norm(v) for k, v in value.items()}
    if isinstance(value, bool):
        return "true" if value else "false"
    if value is None:
        return "none"
    if isinstance(value, (int, float)):
        return str(int(value)) if float(value).is_integer() else str(float(value))
    if isinstance(value, str):
        try:
            return norm(float(value))  # "42" and 42, "0" and 0.0 compare equal
        except ValueError:
            return value
    return str(value)


def docker_cp(src: str, dst: pathlib.Path) -> None:
    dst.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["docker", "cp", "-L", f"{EVAL}:{src}", str(dst)], check=True, capture_output=True)


def main() -> None:
    run_dir = pathlib.Path(sys.argv[1]).resolve()
    run_id = sys.argv[2]
    bundle = run_dir / "bundle"
    if bundle.exists():
        shutil.rmtree(bundle)
    art = bundle / "artifacts"
    art.mkdir(parents=True)

    log_path = sorted((run_dir / "logs").glob("*.json"))[-1]
    log = json.loads(log_path.read_text())

    files: dict[str, pathlib.Path] = {}

    def add(name: str, rel: str) -> pathlib.Path:
        files[name] = art / rel
        files[name].parent.mkdir(parents=True, exist_ok=True)
        return files[name]

    # results: the headline score and every per-sample mark, extracted from the log
    samples = [{
        "id": s["id"],
        "answer": next(iter(s["scores"].values())).get("answer"),
        "target": s["target"],
        "value": next(iter(s["scores"].values()))["value"],
    } for s in log["samples"]]
    metrics = log["results"]["scores"][0]["metrics"]
    add("results", "results.json").write_text(json.dumps({
        "accuracy": metrics["accuracy"]["value"],
        "stderr": metrics["stderr"]["value"],
        "n": len(samples),
        "samples": samples,
    }, indent=2) + "\n")

    shutil.copyfile(log_path, add("inspect_log", "inspect_log.json"))
    docker_cp(HF_SNAPSHOT.format(rev=DATASET_REVISION), add("dataset", "dataset/truthfulqa_mc_validation.parquet"))
    for name, src in {
        "task_code": "inspect_evals/truthfulqa/truthfulqa.py",
        "solver_code": "inspect_ai/solver/_multiple_choice.py",
        "scorer_code": "inspect_ai/scorer/_choice.py",
    }.items():
        docker_cp(f"{SITE}/{src}", add(name, f"code/{pathlib.Path(src).name}"))

    for name, src in {
        "recipe_main_tf": EXP.parent / "infra/main.tf",
        "recipe_tfvars": EXP.parent / "infra/declared.auto.tfvars",
        "recipe_provider_lock": EXP.parent / "infra/.terraform.lock.hcl",
        "recipe_requirements": EXP / "eval/requirements.txt",
        "recipe_run_sh": EXP / "eval/run.sh",
    }.items():
        shutil.copyfile(src, add(name, f"recipe/{src.name}"))

    for name, rel in {
        "capture": "capture.json",
        "tofu_state": "tofu_state.json",
        "tofu_refresh_plan": "tofu_refresh_plan.txt",
        "pip_freeze": "pip_freeze.txt",
        "ollama_show": "ollama_show.json",
    }.items():
        shutil.copyfile(run_dir / "raw" / rel, add(name, f"evidence/{rel}"))

    declared = norm(json.loads((run_dir / "declared.json").read_text()))
    actual = norm(json.loads((run_dir / "actual.json").read_text()))

    manifest = {
        "schema_version": "0.1",
        "run_id": run_id,
        "timestamp": datetime.datetime.now(datetime.UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "model": log["eval"]["model"],
        # No git commit exists for the installed scorer; the exact code files are hashed under artifacts/code.
        "scorer_commit": f"inspect_evals=={log['eval']['packages'].get('inspect_evals', 'unknown')}",
        "judge_model": "none",  # MC1 is scored by letter match; no AI judge
        "grading_prompt": None,
        "artifacts": {
            name: {"path": str(path.relative_to(bundle)), "sha256": "0" * 64}
            for name, path in sorted(files.items())
        },
        "infra_declared": declared,
        "infra_actual": actual,
    }
    (bundle / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"bundle written -> {bundle} ({len(files)} artifacts)")


if __name__ == "__main__":
    main()
