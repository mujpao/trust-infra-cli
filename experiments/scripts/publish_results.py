#!/usr/bin/env python3
"""Publish sealed bundles for level A (experiments/verify_results.sh).

Copies runs/<id>/bundle/ -> results/bundles/<id>/ unchanged (any extra or changed
byte would make it TAMPERED), and writes the verdict that run_eval.sh recorded
(runs/<id>/verify.json) to results/verdicts/<id>.json.

Usage: publish_results.py [run_id ...]   (default: R0-1 R0-2 R0-3 D1 D2 D7)
"""

import json
import pathlib
import shutil
import sys

EXP = pathlib.Path(__file__).resolve().parents[1]
RUNS = EXP / "runs"
RESULTS = EXP / "results"
DEFAULT = ["R0-1", "R0-2", "R0-3", "D1", "D2", "D7"]


def main() -> int:
    ids = sys.argv[1:] or DEFAULT
    for run_id in ids:
        bundle = RUNS / run_id / "bundle"
        report = RUNS / run_id / "verify.json"
        if not bundle.is_dir() or not report.is_file():
            print(f"{run_id}: missing {bundle} or {report}; run run_eval.sh first")
            return 1
        verdict = json.loads(report.read_text())["verdict"]

        dest = RESULTS / "bundles" / run_id
        if dest.exists():
            shutil.rmtree(dest)
        shutil.copytree(bundle, dest)

        verdicts = RESULTS / "verdicts"
        verdicts.mkdir(parents=True, exist_ok=True)
        (verdicts / f"{run_id}.json").write_text(json.dumps({"verdict": verdict}, indent=2) + "\n")
        print(f"{run_id}: {verdict}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
