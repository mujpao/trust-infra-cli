#!/usr/bin/env python3
"""RQ2: break copies of real bundles on purpose and record which checker notices (paper Table 4).

Methods:  B-raw (report alone), B-fp static, B-fp regen (see baselines.py), TrustInfra (`verify`).
Detected = the method reports at least one issue (TrustInfra: verdict is not VERIFIED).

The expected outcome of every case is written down here BEFORE running, and printed next to
what was observed. The result table holds only observations; mismatches are listed, not hidden.

Usage:
  rq2.py --clean experiments/runs/R0-1 [--drift D1=experiments/runs/D1 ...] [--out experiments/results]
"""

import argparse
import csv
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from baselines import b_fp, b_raw, sha256sums  # noqa: E402

EXP = pathlib.Path(__file__).resolve().parents[1]
ROOT = EXP.parent
TRUSTINFRA = ROOT / "target" / "release" / "trustinfra"
TRUSTED_PUB = EXP / "keys" / "demo.pub"
TRUSTED_KEY = EXP / "keys" / "demo.key"
METHODS = ["B-raw", "B-fp static", "B-fp regen", "TrustInfra"]


# ---------------------------------------------------------------- helpers
def jload(p: pathlib.Path):
    return json.loads(p.read_text())


def jdump(p: pathlib.Path, v) -> None:
    p.write_text(json.dumps(v, indent=2) + "\n")


def manifest(b):
    return jload(b / "manifest.json")


def set_manifest(b, m):
    jdump(b / "manifest.json", m)


def sign(b: pathlib.Path, key: pathlib.Path) -> None:
    m = manifest(b)
    m.pop("signature", None)
    set_manifest(b, m)
    subprocess.run([TRUSTINFRA, "sign", b, "--key", key], check=True, capture_output=True)


def verify(b: pathlib.Path) -> dict:
    r = subprocess.run([TRUSTINFRA, "verify", b, "--trusted-key", TRUSTED_PUB, "--json"],
                       capture_output=True, text=True)
    report = json.loads(r.stdout)
    return {
        "verdict": report["verdict"],
        "exit_code": r.returncode,
        "failed_checks": sorted(c["id"] for c in report["checks"] if c["status"] != "pass"),
        "infra_diff": report["infra_diff"],
    }


ATTACKER_KEY: pathlib.Path | None = None


def attacker_key(work: pathlib.Path) -> pathlib.Path:
    global ATTACKER_KEY
    if ATTACKER_KEY is None:
        prefix = work / "attacker"
        subprocess.run([TRUSTINFRA, "keygen", prefix], check=True, capture_output=True)
        ATTACKER_KEY = prefix.with_suffix(".key")
    return ATTACKER_KEY


def flip_one_sample(log: dict) -> tuple[dict, float]:
    """Mark the first wrong sample as correct and recompute the headline accuracy."""
    for s in log["samples"]:
        score = next(iter(s["scores"].values()))
        if score["value"] != "C":
            score["value"] = "C"
            break
    marks = [1 if next(iter(s["scores"].values()))["value"] == "C" else 0 for s in log["samples"]]
    acc = sum(marks) / len(marks)
    log["results"]["scores"][0]["metrics"]["accuracy"]["value"] = acc
    return log, acc


# ---------------------------------------------------------------- tampering cases (clean bundle)
# Each: (name, group, description, mutate(bundle, work), expected detection per method)
A = "artifacts"


def t_untouched(b, w): pass


def t_manifest_reformat(b, w):
    m = manifest(b)
    (b / "manifest.json").write_text(json.dumps(dict(reversed(list(m.items())))) + "\n")


def t_artifact_whitespace(b, w):
    with open(b / A / "results.json", "a") as f:
        f.write("\n")


def t_edit_results(b, w):
    r = jload(b / A / "results.json")
    r["accuracy"] = round(r["accuracy"] + 0.2, 4)
    jdump(b / A / "results.json", r)


def t_edit_log_headline_only(b, w):
    p = b / A / "inspect_log.json"
    log = jload(p)
    log["results"]["scores"][0]["metrics"]["accuracy"]["value"] += 0.2
    jdump(p, log)


def t_edit_log_consistently(b, w):
    p = b / A / "inspect_log.json"
    log, acc = flip_one_sample(jload(p))
    jdump(p, log)
    r = jload(b / A / "results.json")
    r["accuracy"] = acc
    jdump(b / A / "results.json", r)


def t_edit_scorer(b, w):
    with open(b / A / "code" / "_choice.py", "a") as f:
        f.write("\n# changed after the run\n")


def t_edit_dataset(b, w):
    p = b / A / "dataset" / "truthfulqa_mc_validation.parquet"
    data = bytearray(p.read_bytes())
    data[len(data) // 2] ^= 0x01
    p.write_bytes(bytes(data))


def t_delete_artifact(b, w):
    (b / A / "results.json").unlink()


def t_add_unlisted_file(b, w):
    (b / A / "extra.json").write_text('{"note": "added after signing"}\n')


def t_edit_manifest_model(b, w):
    m = manifest(b)
    m["model"] = "ollama/qwen2.5:7b"
    set_manifest(b, m)


def t_edit_and_rehash(b, w):
    t_edit_results(b, w)
    m = manifest(b)
    m["artifacts"]["results"]["sha256"] = sha256sums(b)["artifacts/results.json"]
    set_manifest(b, m)


def t_attacker_resign(b, w):
    t_edit_results(b, w)
    sign(b, attacker_key(w))


def t_remove_signature(b, w):
    m = manifest(b)
    m.pop("signature")
    set_manifest(b, m)


def t_add_unknown_field(b, w):
    m = manifest(b)
    m["note"] = "added after signing"
    set_manifest(b, m)


def t_unsafe_path(b, w):
    # `trustinfra sign` refuses unsafe paths, so this entry cannot be re-signed via the CLI:
    # the verifier sees both a bad signature and an unsafe path. (tests/mutations.rs covers the
    # re-signed variant at the library level.)
    m = manifest(b)
    m["artifacts"]["evil"] = {"path": "../outside.txt", "sha256": "0" * 64}
    set_manifest(b, m)
    (b.parent / "outside.txt").write_text("outside the bundle\n")


def t_infra_edit_real_key(b, w):
    m = manifest(b)
    m["infra_actual"]["model_quantization"] = "Q8_0"
    set_manifest(b, m)
    sign(b, TRUSTED_KEY)


Y, N = True, False
TAMPER_CASES = [
    # name, group, description, mutate, expected {B-raw, B-fp static, B-fp regen, TrustInfra}
    ("untouched copy", "false alarm", "copy of the clean bundle, no change", t_untouched, (N, N, N, N)),
    ("manifest re-formatted", "false alarm", "manifest.json re-indented, keys reordered; same content",
     t_manifest_reformat, (N, Y, N, N)),
    ("artifact whitespace", "byte change", "one newline appended to results.json",
     t_artifact_whitespace, (N, Y, N, Y)),
    ("edit results", "artifact edit", "results.json accuracy raised by 0.2", t_edit_results, (N, Y, N, Y)),
    ("edit log headline only", "artifact edit", "log headline accuracy raised by 0.2, samples untouched",
     t_edit_log_headline_only, (Y, Y, N, Y)),
    ("edit log consistently", "artifact edit", "one sample flipped to correct, headline recomputed, results.json too",
     t_edit_log_consistently, (N, Y, N, Y)),
    ("edit scorer code", "artifact edit", "line appended to the hashed scorer source", t_edit_scorer, (N, Y, N, Y)),
    ("edit dataset", "artifact edit", "one bit flipped in the dataset file", t_edit_dataset, (N, Y, N, Y)),
    ("delete artifact", "artifact edit", "results.json deleted", t_delete_artifact, (N, Y, N, Y)),
    ("add unlisted file", "artifact edit", "extra file added under artifacts/", t_add_unlisted_file, (N, Y, N, Y)),
    ("edit manifest model", "manifest edit", "manifest model changed, not re-signed", t_edit_manifest_model,
     (N, Y, N, Y)),
    ("edit and rehash", "manifest edit", "results edited and its manifest hash updated, not re-signed",
     t_edit_and_rehash, (N, Y, N, Y)),
    ("attacker re-signs", "manifest edit", "results edited, bundle re-signed with an attacker's key",
     t_attacker_resign, (N, Y, N, Y)),
    ("remove signature", "manifest edit", "signature field deleted", t_remove_signature, (N, Y, N, Y)),
    ("add unknown field", "manifest edit", "unknown top-level field added (verdict UNVERIFIABLE)",
     t_add_unknown_field, (N, Y, N, Y)),
    ("unsafe artifact path", "manifest edit", "artifact entry pointing outside the bundle added, not re-signed",
     t_unsafe_path, (N, Y, N, Y)),
    ("infra edit, real key", "insider", "infra_actual changed and re-signed with the real key (diff check)",
     t_infra_edit_real_key, (N, Y, N, Y)),
]


# ---------------------------------------------------------------- drift cases (drift bundles)
def d_as_run(b, w): pass


def d_hide(b, w):
    m = manifest(b)
    m["infra_actual"] = m["infra_declared"]
    set_manifest(b, m)


def d_hide_attacker(b, w):
    d_hide(b, w)
    sign(b, attacker_key(w))


def d_insider_lie(b, w):
    d_hide(b, w)
    sign(b, TRUSTED_KEY)


# Expected detection for drift depends on whether the Inspect log records the changed field,
# so B-raw is "None" (= no prediction recorded; observed value reported as is).
DRIFT_VARIANTS = [
    ("as run", "the bundle exactly as the drifted run produced it", d_as_run, (None, N, N, Y)),
    ("hide drift", "infra_actual overwritten with declared values, not re-signed", d_hide, (None, Y, N, Y)),
    ("hide drift, attacker re-signs", "same, re-signed with an attacker's key", d_hide_attacker, (None, Y, N, Y)),
    ("insider signs a lie", "same, re-signed with the real key (outside the threat model)", d_insider_lie,
     (None, Y, N, N)),
]


# ---------------------------------------------------------------- run
def run_case(src_bundle, claim, name, mutate, work):
    case_dir = work / name.replace(" ", "_").replace(",", "")
    bundle = case_dir / "bundle"
    shutil.copytree(src_bundle, bundle)
    for p in bundle.rglob("*"):  # dataset arrives read-only from the HF cache
        if p.is_file():
            p.chmod(0o644)
    published_sums = sha256sums(bundle)  # what the honest publisher shipped
    mutate(bundle, work)
    regen_sums = sha256sums(bundle)  # attacker regenerates the unsigned list
    ti = verify(bundle)
    issues = {
        "B-raw": b_raw(bundle, claim),
        "B-fp static": b_fp(bundle, published_sums),
        "B-fp regen": b_fp(bundle, regen_sums),
        "TrustInfra": [] if ti["verdict"] == "verified" else [f"{ti['verdict']}: {', '.join(ti['failed_checks'])}"],
    }
    return {"observed": {m: bool(issues[m]) for m in METHODS}, "issues": issues, "trustinfra": ti}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--clean", required=True, type=pathlib.Path)
    ap.add_argument("--drift", action="append", default=[], help="ID=run_dir, e.g. D2=experiments/runs/D2")
    ap.add_argument("--out", type=pathlib.Path, default=EXP / "results")
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)

    rows = []
    with tempfile.TemporaryDirectory() as tmp:
        work = pathlib.Path(tmp)
        claim = jload(args.clean / "declared.json")
        for name, group, desc, mutate, expected in TAMPER_CASES:
            r = run_case(args.clean / "bundle", claim, name, mutate, work)
            rows.append({"source": args.clean.name, "case": name, "group": group, "description": desc,
                         "expected": dict(zip(METHODS, expected)), **r})
        for spec in args.drift:
            did, run_dir = spec.split("=", 1)
            run_dir = pathlib.Path(run_dir)
            claim = jload(run_dir / "declared.json")
            for name, desc, mutate, expected in DRIFT_VARIANTS:
                r = run_case(run_dir / "bundle", claim, f"{did} {name}", mutate, work)
                rows.append({"source": run_dir.name, "case": f"{did}: {name}", "group": f"drift {did}",
                             "description": desc, "expected": dict(zip(METHODS, expected)), **r})

    mark = {True: "✓", False: "✗"}
    mismatches = []
    md = ["| Case | Group | " + " | ".join(METHODS) + " | TrustInfra checks failed |",
          "|---|---|" + "---|" * len(METHODS) + "---|"]
    for r in rows:
        cells = []
        for m in METHODS:
            obs, exp = r["observed"][m], r["expected"][m]
            cells.append(mark[obs] + ("" if exp is None or exp == obs else " (!)"))
            if exp is not None and exp != obs:
                mismatches.append(f"{r['case']} / {m}: expected {mark[exp]}, observed {mark[obs]}")
        failed = ", ".join(r["trustinfra"]["failed_checks"]) or "none"
        md.append(f"| {r['case']} | {r['group']} | " + " | ".join(cells) + f" | {r['trustinfra']['verdict']}: {failed} |")
    md += ["", "✓ = noticed, ✗ = not noticed, (!) = differs from the expectation written before the run.",
           "", "Expectation mismatches:" if mismatches else "No expectation mismatches."]
    md += [f"- {x}" for x in mismatches]

    (args.out / "rq2_matrix.md").write_text("\n".join(md) + "\n")
    with open(args.out / "rq2_matrix.csv", "w", newline="") as f:
        wr = csv.writer(f)
        wr.writerow(["case", "group", *METHODS, "trustinfra_verdict", "trustinfra_failed_checks"])
        for r in rows:
            wr.writerow([r["case"], r["group"], *[int(r["observed"][m]) for m in METHODS],
                         r["trustinfra"]["verdict"], ";".join(r["trustinfra"]["failed_checks"])])
    jdump(args.out / "rq2_details.json", rows)
    print("\n".join(md))


if __name__ == "__main__":
    main()
