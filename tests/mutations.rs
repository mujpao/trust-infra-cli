mod common;

use std::{
    fs,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
};

use ed25519_dalek::SigningKey;
use serde::Serialize;
use serde_json::{Value, json};
use tempfile::tempdir;
use trust_infra_cli::{Report, Verdict, verify};

use common::{
    build_golden, bundle_copy_path, copy_dir, hash_file, read_manifest, resign, write_manifest,
};

struct Case {
    name: &'static str,
    mutate: fn(&Path, &SigningKey),
    expect_failed: &'static [&'static str],
    expect_verdict: Verdict,
}

#[derive(Serialize)]
struct MutationResult {
    case: &'static str,
    expected_failed_checks: Vec<String>,
    actual_failed_checks: Vec<String>,
    pass: bool,
}

#[test]
fn golden_bundle_verifies() {
    let root = tempdir().unwrap();
    let key = SigningKey::generate(&mut rand::rng());
    build_golden(root.path(), &key);

    let report = verify(root.path(), &key.verifying_key());
    assert_eq!(report.verdict, Verdict::Verified);
    assert!(report.failed_ids().is_empty());
}

#[test]
fn unknown_schema_version_is_unverifiable() {
    let root = tempdir().unwrap();
    let key = SigningKey::generate(&mut rand::rng());
    build_golden(root.path(), &key);

    let mut manifest = read_manifest(root.path());
    manifest["schema_version"] = json!("999.0");
    resign(root.path(), &manifest, &key);

    let report = verify(root.path(), &key.verifying_key());
    assert_eq!(report.verdict, Verdict::Unverifiable);
    assert!(report.failed_ids().contains(&"schema"));
}

#[test]
fn all_mutations_detected() {
    let root = tempdir().unwrap();
    let golden = root.path().join("golden");
    let trusted = SigningKey::generate(&mut rand::rng());
    build_golden(&golden, &trusted);

    let cases = [
        Case {
            name: "edit_results",
            mutate: edit_results,
            expect_failed: &["artifact:results"],
            expect_verdict: Verdict::Tampered,
        },
        Case {
            name: "edit_scorer",
            mutate: edit_scorer,
            expect_failed: &["artifact:scorer"],
            expect_verdict: Verdict::Tampered,
        },
        Case {
            name: "edit_dataset",
            mutate: edit_dataset,
            expect_failed: &["artifact:dataset"],
            expect_verdict: Verdict::Tampered,
        },
        Case {
            name: "delete_artifact",
            mutate: delete_results,
            expect_failed: &["artifact:results"],
            expect_verdict: Verdict::Tampered,
        },
        Case {
            name: "add_unlisted_file",
            mutate: add_unlisted_file,
            expect_failed: &["unlisted_files"],
            expect_verdict: Verdict::Tampered,
        },
        Case {
            name: "edit_model",
            mutate: edit_model,
            expect_failed: &["signature"],
            expect_verdict: Verdict::Tampered,
        },
        Case {
            name: "edit_and_rehash_results",
            mutate: edit_and_rehash_results,
            expect_failed: &["signature"],
            expect_verdict: Verdict::Tampered,
        },
        Case {
            name: "attacker_resign",
            mutate: attacker_resign,
            expect_failed: &["signature"],
            expect_verdict: Verdict::Tampered,
        },
        Case {
            name: "remove_signature",
            mutate: remove_signature,
            expect_failed: &["signature"],
            expect_verdict: Verdict::Tampered,
        },
        // CHANGED: unsigned edit + deny_unknown_fields means BOTH signature and
        // schema fail, and a schema failure is Unverifiable (exit code 2).
        // If you'd rather it be Tampered, re-sign in the mutation and decide
        // the expectation again.
        Case {
            name: "add_unknown_field",
            mutate: add_unknown_field,
            expect_failed: &["schema", "signature"],
            expect_verdict: Verdict::Unverifiable,
        },
        Case {
            name: "unsafe_artifact_path",
            mutate: unsafe_artifact_path,
            expect_failed: &["path_safety"],
            expect_verdict: Verdict::Tampered,
        },
        Case {
            name: "infra_mismatch",
            mutate: infra_mismatch,
            expect_failed: &["infra_diff"],
            expect_verdict: Verdict::Tampered,
        },
    ];

    let mut results = Vec::new();
    let mut failures = Vec::new();
    for case in cases {
        let bundle = bundle_copy_path(root.path(), case.name);
        copy_dir(&golden, &bundle);
        (case.mutate)(&bundle, &trusted);
        let verification = catch_unwind(AssertUnwindSafe(|| {
            verify(&bundle, &trusted.verifying_key())
        }));
        let (actual_failed_checks, pass, failure_detail) = match verification {
            Ok(report) => assess_report(&report, &case),
            Err(_) => (Vec::new(), false, "verify panicked".to_owned()),
        };
        if !pass {
            failures.push(format!("{}: {}", case.name, failure_detail));
        }
        results.push(MutationResult {
            case: case.name,
            expected_failed_checks: sorted(case.expect_failed.iter().map(|s| s.to_string())),
            actual_failed_checks,
            pass,
        });
    }

    let output_dir = Path::new("tests/out");
    fs::create_dir_all(output_dir).unwrap();
    fs::write(
        output_dir.join("mutation_results.json"),
        serde_json::to_vec_pretty(&results).unwrap(),
    )
    .unwrap();
    assert!(
        failures.is_empty(),
        "mutation failures:\n{}",
        failures.join("\n")
    );
}

fn sorted(ids: impl Iterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = ids.collect();
    v.sort();
    v
}

fn assess_report(report: &Report, case: &Case) -> (Vec<String>, bool, String) {
    let failed = catch_unwind(AssertUnwindSafe(|| report.failed_ids()));
    let Ok(failed_ids) = failed else {
        return (Vec::new(), false, "failed_ids panicked".to_owned());
    };
    let actual = sorted(failed_ids.into_iter().map(str::to_owned));
    let expected = sorted(case.expect_failed.iter().map(|s| s.to_string()));
    let pass = report.verdict == case.expect_verdict && actual == expected;
    let detail = format!(
        "expected verdict {:?} and failed checks {:?}; got {:?} and {:?}",
        case.expect_verdict, expected, report.verdict, actual
    );
    (actual, pass, detail)
}

fn edit_results(bundle: &Path, _: &SigningKey) {
    append(bundle.join("artifacts/results.json"), "\nchanged");
}

fn edit_scorer(bundle: &Path, _: &SigningKey) {
    append(bundle.join("artifacts/scorer.py"), "# changed\n");
}

fn edit_dataset(bundle: &Path, _: &SigningKey) {
    append(bundle.join("artifacts/dataset.json"), "\nchanged");
}

fn delete_results(bundle: &Path, _: &SigningKey) {
    fs::remove_file(bundle.join("artifacts/results.json")).unwrap();
}

fn add_unlisted_file(bundle: &Path, _: &SigningKey) {
    fs::write(bundle.join("artifacts/unlisted.txt"), "extra").unwrap();
}

fn edit_model(bundle: &Path, _: &SigningKey) {
    let mut manifest = read_manifest(bundle);
    manifest["model"] = json!("edited-model");
    write_manifest(bundle, &manifest);
}

fn edit_and_rehash_results(bundle: &Path, _: &SigningKey) {
    append(bundle.join("artifacts/results.json"), "\nchanged");
    let mut manifest = read_manifest(bundle);
    manifest["artifacts"]["results"]["sha256"] =
        Value::String(hash_file(&bundle.join("artifacts/results.json")));
    write_manifest(bundle, &manifest);
}

fn attacker_resign(bundle: &Path, _: &SigningKey) {
    let mut manifest = read_manifest(bundle);
    manifest["model"] = json!("attacker-model");
    let attacker = SigningKey::generate(&mut rand::rng());
    resign(bundle, &manifest, &attacker);
}

fn remove_signature(bundle: &Path, _: &SigningKey) {
    let mut manifest = read_manifest(bundle);
    manifest.as_object_mut().unwrap().remove("signature");
    write_manifest(bundle, &manifest);
}

fn add_unknown_field(bundle: &Path, _: &SigningKey) {
    let mut manifest = read_manifest(bundle);
    manifest["unknown"] = json!(true);
    write_manifest(bundle, &manifest);
}

fn unsafe_artifact_path(bundle: &Path, key: &SigningKey) {
    let mut manifest = read_manifest(bundle);
    manifest["artifacts"]["evil"] = json!({
        "path": "../outside.txt",
        "sha256": "0".repeat(64),
    });
    resign(bundle, &manifest, key);
}

fn infra_mismatch(bundle: &Path, key: &SigningKey) {
    let mut manifest = read_manifest(bundle);
    manifest["infra_actual"]["image"] = json!("sha256:bbb");
    resign(bundle, &manifest, key);
}

fn append(path: std::path::PathBuf, contents: &str) {
    use std::io::Write;

    fs::OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(contents.as_bytes())
        .unwrap();
}
