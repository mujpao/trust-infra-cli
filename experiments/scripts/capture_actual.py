#!/usr/bin/env python3
"""Record what ACTUALLY ran (paper §3.1 step 3, "infra_actual").

Every value comes from observing the running system, never from the recipe:
  image refs      <- `docker inspect` of the running containers
  model digest    <- Ollama's own model list (/api/tags), for the tag the eval used
  package versions<- importlib.metadata inside the eval container, after the run
  task settings   <- the Inspect log (self-reported by the eval; marked as such)

Writes <run_dir>/actual.json (same keys as declared.json) and <run_dir>/raw/*.
Usage: capture_actual.py <run_dir>
"""

import json
import pathlib
import subprocess
import sys

OLLAMA = "trustinfra-ollama"
EVAL = "trustinfra-eval"
INFRA = pathlib.Path(__file__).resolve().parents[2] / "infra"


def sh(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(args, capture_output=True, text=True, check=check)


def host_cpu() -> str:
    """CPU name on macOS or Linux; self-reported, for the record only."""
    try:
        name = sh("sysctl", "-n", "machdep.cpu.brand_string", check=False).stdout.strip()
        if name:
            return name
    except FileNotFoundError:
        pass
    try:
        for line in pathlib.Path("/proc/cpuinfo").read_text().splitlines():
            if line.lower().startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    import platform
    return platform.processor() or platform.machine()


def in_eval(code: str) -> str:
    return sh("docker", "exec", EVAL, "python", "-c", code).stdout


def image_ref(container: str, declared_ref: str) -> tuple[str, dict]:
    """repo@sha256 digest of the image the container is running."""
    image_id = sh("docker", "inspect", container, "--format", "{{.Image}}").stdout.strip()
    digests = json.loads(
        sh("docker", "image", "inspect", image_id, "--format", "{{json .RepoDigests}}").stdout
    )
    repo = declared_ref.split("@")[0]
    match = [d for d in digests if d.split("@")[0] == repo]
    ref = match[0] if match else image_id  # no repo digest: fall back to the local image id
    return ref, {"container": container, "image_id": image_id, "repo_digests": digests}


def meaningful_drift(plan_json: dict) -> list[dict]:
    """Attributes OpenTofu saw change outside it, minus the provider's null-vs-empty noise."""
    empty = (None, [], {})
    out = []
    for rd in plan_json.get("resource_drift", []):
        before, after = rd["change"].get("before") or {}, rd["change"].get("after") or {}
        for key in sorted(set(before) | set(after)):
            b, a = before.get(key), after.get(key)
            if b == a or (b in empty and a in empty):
                continue
            out.append({"address": rd["address"], "attribute": key, "before": b, "after": a})
    return out


def latest_log(run_dir: pathlib.Path) -> pathlib.Path:
    logs = sorted((run_dir / "logs").glob("*.json"))
    if not logs:
        sys.exit(f"no Inspect log in {run_dir / 'logs'}")
    return logs[-1]


def main() -> None:
    run_dir = pathlib.Path(sys.argv[1]).resolve()
    raw = run_dir / "raw"
    raw.mkdir(exist_ok=True)
    declared = json.loads((run_dir / "declared.json").read_text())

    log = json.loads(latest_log(run_dir).read_text())["eval"]
    model_tag = log["model"].removeprefix("ollama/")

    ollama_ref, ollama_img = image_ref(OLLAMA, declared["ollama_image"])
    eval_ref, eval_img = image_ref(EVAL, declared["eval_image"])

    tags = json.loads(in_eval(
        "import urllib.request,sys;sys.stdout.write(urllib.request.urlopen('http://ollama:11434/api/tags').read().decode())"
    ))
    model = next((m for m in tags["models"] if m["name"] == model_tag), None)
    show = json.loads(in_eval(
        "import urllib.request,json,sys;"
        f"r=urllib.request.Request('http://ollama:11434/api/show',data=json.dumps({{'model':{model_tag!r}}}).encode());"
        "sys.stdout.write(urllib.request.urlopen(r).read().decode())"
    )) if model else {}

    versions = json.loads(in_eval(
        "import importlib.metadata as m,json;"
        "print(json.dumps({p:m.version(p) for p in ['inspect_ai','inspect_evals']}))"
    ))

    actual = {
        "ollama_image": ollama_ref,
        "eval_image": eval_ref,
        "model_tag": model_tag,
        "model_digest": f"sha256:{model['digest']}" if model else "missing",
        "model_quantization": model["details"].get("quantization_level", "unknown") if model else "missing",
        "inspect_ai_version": versions["inspect_ai"],
        "inspect_evals_version": versions["inspect_evals"],
        "task": log["task"],
        "task_args": log["task_args"],
        "limit": log["config"].get("limit"),
        "temperature": log["model_generate_config"].get("temperature"),
        "seed": log["model_generate_config"].get("seed"),
        "max_tokens": log["model_generate_config"].get("max_tokens"),
    }
    # Where each actual value came from. Used for RQ1 grading: only values not
    # written by the eval itself can count as "Verified".
    sources = {
        "ollama_image": "docker inspect (runtime)",
        "eval_image": "docker inspect (runtime)",
        "model_tag": "Inspect log (self-reported)",
        "model_digest": "Ollama /api/tags (runtime)",
        "model_quantization": "Ollama /api/tags (runtime)",
        "inspect_ai_version": "importlib.metadata in eval container (runtime)",
        "inspect_evals_version": "importlib.metadata in eval container (runtime)",
        "task": "Inspect log (self-reported)",
        "task_args": "Inspect log (self-reported)",
        "limit": "Inspect log (self-reported)",
        "temperature": "Inspect log (self-reported)",
        "seed": "Inspect log (self-reported)",
        "max_tokens": "Inspect log (self-reported)",
    }
    assert actual.keys() == declared.keys(), "declared and actual must have identical keys (risk R1)"

    hardware = {
        "host_cpu": host_cpu(),
        "docker": json.loads(sh("docker", "info", "--format",
                                '{"cpus":{{.NCPU}},"mem_bytes":{{.MemTotal}},"arch":"{{.Architecture}}","server":"{{.ServerVersion}}"}').stdout),
        "note": "self-reported by the evaluator's machine; CPU only, no GPU",
    }

    (run_dir / "actual.json").write_text(json.dumps(actual, indent=2) + "\n")
    (raw / "capture.json").write_text(json.dumps(
        {"sources": sources, "images": [ollama_img, eval_img], "ollama_model": model, "hardware": hardware},
        indent=2) + "\n")
    (raw / "ollama_show.json").write_text(json.dumps(show, indent=2) + "\n")
    (raw / "pip_freeze.txt").write_text(sh("docker", "exec", EVAL, "python", "-m", "pip", "freeze").stdout)
    (raw / "tofu_state.json").write_text(sh("tofu", f"-chdir={INFRA}", "show", "-json").stdout)

    # Does OpenTofu itself notice anything changed outside it? (reported separately, D7)
    plan_file = raw / "tofu_refresh.plan"
    plan = sh("tofu", f"-chdir={INFRA}", "plan", "-refresh-only", "-detailed-exitcode", "-input=false",
              "-no-color", "-var", f"run_dir={run_dir}", f"-out={plan_file}", check=False)
    (raw / "tofu_refresh_plan.txt").write_text(f"exit_code={plan.returncode}\n{plan.stdout}{plan.stderr}")
    plan_json = json.loads(sh("tofu", f"-chdir={INFRA}", "show", "-json", str(plan_file)).stdout)
    plan_file.unlink()
    (raw / "tofu_refresh_drift.json").write_text(json.dumps({
        "exit_code": plan.returncode,
        "note": "exit_code 2 also fires on provider noise (null read back as []/{}); "
                "meaningful_changes ignores only that noise",
        "meaningful_changes": meaningful_drift(plan_json),
    }, indent=2) + "\n")

    print(f"captured actual setup -> {run_dir / 'actual.json'}")


if __name__ == "__main__":
    main()
