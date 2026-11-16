"""The two simpler checkers TrustInfra is compared against (paper §3.5).

B-raw  "report alone": a reviewer has the Inspect log and the setup the paper claims.
       Checks the log against the claim and against itself. Has no independent copy of
       the score, the files, or what actually ran.
B-fp   "plain fingerprints": an unsigned SHA256SUMS list over every bundle file,
       published next to the bundle. Detects changed, missing or extra files.
       Never compares declared with actual setup. Two attacker models:
         static: the attacker does not touch SHA256SUMS
         regen:  the attacker regenerates SHA256SUMS (it is unsigned, so nothing stops this)
"""

import hashlib
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from bundle import norm  # noqa: E402  same normalisation as the manifest


# ---------------------------------------------------------------- B-raw
def b_raw(bundle: pathlib.Path, claim: dict) -> list[str]:
    """Issues a log-only reviewer can find. Empty list = nothing noticed."""
    path = bundle / "artifacts" / "inspect_log.json"
    if not path.exists():
        return ["inspect log missing"]
    try:
        log = json.loads(path.read_text())
        ev = log["eval"]
        samples = log["samples"]
        headline = log["results"]["scores"][0]["metrics"]["accuracy"]["value"]
    except (ValueError, KeyError, IndexError, TypeError) as e:
        return [f"inspect log unreadable: {e!r}"]

    issues = []

    def expect(what: str, got, want) -> None:
        if norm(got) != norm(want):
            issues.append(f"{what}: log says {got!r}, claim says {want!r}")

    gen = ev.get("model_generate_config") or {}
    expect("model", ev.get("model"), f"ollama/{claim['model_tag']}")
    expect("task", ev.get("task"), claim["task"])
    expect("task_args", ev.get("task_args"), claim["task_args"])
    expect("temperature", gen.get("temperature"), claim["temperature"])
    expect("seed", gen.get("seed"), claim["seed"])
    expect("max_tokens", gen.get("max_tokens"), claim["max_tokens"])
    expect("limit", (ev.get("config") or {}).get("limit"), claim["limit"])
    expect("inspect_ai version", (ev.get("packages") or {}).get("inspect_ai"), claim["inspect_ai_version"])
    expect("inspect_evals version", (ev.get("packages") or {}).get("inspect_evals"), claim["inspect_evals_version"])
    expect("number of samples", len(samples), claim["limit"])

    # Internal consistency: headline accuracy must equal the mean of per-sample marks.
    try:
        marks = [1 if next(iter(s["scores"].values()))["value"] == "C" else 0 for s in samples]
        mean = sum(marks) / len(marks) if marks else 0.0
        if abs(mean - float(headline)) > 1e-9:
            issues.append(f"headline accuracy {headline} != mean of samples {mean}")
    except (KeyError, StopIteration, TypeError, ValueError) as e:
        issues.append(f"per-sample scores unreadable: {e!r}")
    return issues


# ---------------------------------------------------------------- B-fp
def sha256sums(bundle: pathlib.Path) -> dict[str, str]:
    return {
        str(p.relative_to(bundle)): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(bundle.rglob("*")) if p.is_file()
    }


def b_fp(bundle: pathlib.Path, published: dict[str, str]) -> list[str]:
    """Issues found by checking the bundle against a published SHA256SUMS list."""
    now = sha256sums(bundle)
    issues = [f"changed: {f}" for f in published if f in now and now[f] != published[f]]
    issues += [f"missing: {f}" for f in published if f not in now]
    issues += [f"extra: {f}" for f in now if f not in published]
    return issues
