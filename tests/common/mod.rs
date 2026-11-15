use std::{
    fs,
    path::{Path, PathBuf},
};

use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use trust_infra_cli::{sign_bundle, signature::sign_value};

pub fn build_golden(dir: &Path, key: &SigningKey) {
    let artifacts = dir.join("artifacts");
    fs::create_dir_all(&artifacts).unwrap();
    let files = [
        ("dataset", "dataset.json", r#"{"rows":[{"x":1}]}"#),
        ("scorer", "scorer.py", "def score(x): return x\n"),
        ("prompts", "prompts.json", r#"{"prompt":"grade"}"#),
        ("results", "results.json", r#"{"score":0.95}"#),
        ("log", "log.json", r#"{"event":"complete"}"#),
    ];

    let artifact_entries: serde_json::Map<String, Value> = files
        .iter()
        .map(|(name, file, contents)| {
            fs::write(artifacts.join(file), contents).unwrap();
            (
                (*name).to_owned(),
                json!({
                    "path": format!("artifacts/{file}"),
                    "sha256": sha256(contents.as_bytes()),
                }),
            )
        })
        .collect();

    let infra = json!({
        "image": "sha256:aaa",
        "packages": {"python": "3.12", "numpy": "2.0"}
    });
    let manifest = json!({
        "schema_version": "0.1",
        "run_id": "golden-run",
        "timestamp": "2026-01-01T00:00:00Z",
        "model": "model-v1",
        "scorer_commit": "abc123",
        "judge_model": "judge-v1",
        "grading_prompt": "grade carefully",
        "artifacts": artifact_entries,
        "infra_declared": infra,
        "infra_actual": infra,
    });

    write_manifest(dir, &manifest);
    sign_bundle(dir, key).unwrap();
}

pub fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

pub fn read_manifest(dir: &Path) -> Value {
    serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap()
}

pub fn write_manifest(dir: &Path, manifest: &Value) {
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(manifest).unwrap(),
    )
    .unwrap();
}

pub fn resign(dir: &Path, manifest: &Value, key: &SigningKey) {
    let mut manifest = manifest.clone();
    manifest.as_object_mut().unwrap().remove("signature");
    sign_value(&mut manifest, key);
    write_manifest(dir, &manifest);
}

pub fn hash_file(path: &Path) -> String {
    sha256(&fs::read(path).unwrap())
}

fn sha256(contents: &[u8]) -> String {
    hex::encode(Sha256::digest(contents))
}

pub fn bundle_copy_path(root: &Path, name: &str) -> PathBuf {
    root.join(name)
}
