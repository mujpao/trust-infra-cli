#[allow(dead_code)]
mod common;

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::Value;
use tempfile::tempdir;

use common::{build_golden_unsigned, hash_file, read_manifest, write_manifest};

const BIN: &str = env!("CARGO_BIN_EXE_trustinfra");

fn keygen(prefix: &Path) -> Output {
    Command::new(BIN)
        .arg("keygen")
        .arg(prefix)
        .output()
        .unwrap()
}

fn sign_and_verify(prefix: &Path, bundle: &Path) -> (Output, PathBuf) {
    let generated = keygen(prefix);
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let key_path = suffixed(prefix, ".key");
    let public_path = suffixed(prefix, ".pub");
    build_golden_unsigned(bundle);
    let signed = Command::new(BIN)
        .arg("sign")
        .arg(bundle)
        .arg("--key")
        .arg(&key_path)
        .output()
        .unwrap();
    assert!(
        signed.status.success(),
        "{}",
        String::from_utf8_lossy(&signed.stderr)
    );
    let verified = Command::new(BIN)
        .arg("verify")
        .arg(bundle)
        .arg("--trusted-key")
        .arg(public_path)
        .output()
        .unwrap();
    (verified, key_path)
}

fn suffixed(prefix: &Path, suffix: &str) -> PathBuf {
    let mut path = prefix.as_os_str().to_owned();
    path.push(suffix);
    PathBuf::from(path)
}

#[test]
fn keygen_sign_and_verify_golden_bundle() {
    let temp = tempdir().unwrap();
    let prefix = temp.path().join("signing");
    let bundle = temp.path().join("bundle");
    let (verified, _) = sign_and_verify(&prefix, &bundle);

    assert_eq!(verified.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&verified.stdout).starts_with("VERDICT: VERIFIED"));
}

#[test]
fn signing_does_not_require_placeholder_artifact_hashes() {
    let temp = tempdir().unwrap();
    let prefix = temp.path().join("signing");
    let generated = keygen(&prefix);
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );

    let bundle = temp.path().join("bundle");
    build_golden_unsigned(&bundle);
    let mut manifest = read_manifest(&bundle);
    let artifacts = manifest["artifacts"].as_object_mut().unwrap();
    for entry in artifacts.values_mut() {
        entry.as_object_mut().unwrap().remove("sha256");
    }
    assert!(
        artifacts
            .values()
            .all(|entry| !entry.as_object().unwrap().contains_key("sha256"))
    );
    write_manifest(&bundle, &manifest);

    let signed = Command::new(BIN)
        .arg("sign")
        .arg(&bundle)
        .arg("--key")
        .arg(suffixed(&prefix, ".key"))
        .output()
        .unwrap();
    assert!(
        signed.status.success(),
        "{}",
        String::from_utf8_lossy(&signed.stderr)
    );

    let signed_manifest = read_manifest(&bundle);
    for entry in signed_manifest["artifacts"].as_object().unwrap().values() {
        let entry = entry.as_object().unwrap();
        let artifact_path = bundle.join(entry["path"].as_str().unwrap());
        assert_eq!(entry["sha256"].as_str().unwrap(), hash_file(&artifact_path));
    }

    let verified = Command::new(BIN)
        .arg("verify")
        .arg(&bundle)
        .arg("--trusted-key")
        .arg(suffixed(&prefix, ".pub"))
        .output()
        .unwrap();
    assert_eq!(verified.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&verified.stdout).starts_with("VERDICT: VERIFIED"));
}

#[test]
fn signing_rejects_unsupported_schema_version() {
    let temp = tempdir().unwrap();
    let prefix = temp.path().join("signing");
    let generated = keygen(&prefix);
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );

    let bundle = temp.path().join("bundle");
    build_golden_unsigned(&bundle);
    let mut manifest = read_manifest(&bundle);
    manifest["schema_version"] = "999.0".into();
    write_manifest(&bundle, &manifest);

    let signed = Command::new(BIN)
        .arg("sign")
        .arg(&bundle)
        .arg("--key")
        .arg(suffixed(&prefix, ".key"))
        .output()
        .unwrap();
    assert_ne!(
        signed.status.code(),
        Some(0),
        "signing unexpectedly accepted an unsupported schema version"
    );
    assert!(
        String::from_utf8_lossy(&signed.stderr).contains("unknown schema version: 999.0"),
        "{}",
        String::from_utf8_lossy(&signed.stderr)
    );
}

#[test]
fn modified_artifact_is_tampered() {
    let temp = tempdir().unwrap();
    let prefix = temp.path().join("signing");
    let bundle = temp.path().join("bundle");
    let (verified, _) = sign_and_verify(&prefix, &bundle);
    assert_eq!(verified.status.code(), Some(0));

    let artifact = bundle.join("artifacts/results.json");
    let mut contents = fs::read(&artifact).unwrap();
    contents[0] ^= 1;
    fs::write(artifact, contents).unwrap();

    let tampered = Command::new(BIN)
        .arg("verify")
        .arg(bundle)
        .arg("--trusted-key")
        .arg(suffixed(&prefix, ".pub"))
        .output()
        .unwrap();
    assert_eq!(tampered.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&tampered.stdout).starts_with("VERDICT: TAMPERED"));
}

#[test]
fn json_verification_output_is_tampered_report() {
    let temp = tempdir().unwrap();
    let prefix = temp.path().join("signing");
    let bundle = temp.path().join("bundle");
    let (verified, _) = sign_and_verify(&prefix, &bundle);
    assert_eq!(verified.status.code(), Some(0));

    fs::write(bundle.join("artifacts/results.json"), b"modified").unwrap();
    let output = Command::new(BIN)
        .arg("verify")
        .arg(bundle)
        .arg("--trusted-key")
        .arg(suffixed(&prefix, ".pub"))
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["verdict"], "tampered");
}

#[test]
fn missing_trusted_key_exits_two() {
    let temp = tempdir().unwrap();
    let output = Command::new(BIN)
        .arg("verify")
        .arg(temp.path())
        .arg("--trusted-key")
        .arg(temp.path().join("missing.pub"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn keygen_refuses_to_overwrite() {
    let temp = tempdir().unwrap();
    let prefix = temp.path().join("signing");
    let first = keygen(&prefix);
    assert!(first.status.success());
    let secret_before = fs::read(suffixed(&prefix, ".key")).unwrap();
    let second = keygen(&prefix);
    assert_eq!(second.status.code(), Some(1));
    assert_eq!(fs::read(suffixed(&prefix, ".key")).unwrap(), secret_before);

    let public_collision_prefix = temp.path().join("public-collision");
    let public_path = suffixed(&public_collision_prefix, ".pub");
    fs::write(&public_path, b"existing public key").unwrap();
    let collision = keygen(&public_collision_prefix);
    assert_eq!(collision.status.code(), Some(1));
    assert_eq!(fs::read(public_path).unwrap(), b"existing public key");
    assert!(!suffixed(&public_collision_prefix, ".key").exists());
}
