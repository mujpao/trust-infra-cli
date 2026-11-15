use std::{
    fs,
    panic::{AssertUnwindSafe, catch_unwind},
};

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use tempfile::tempdir;
use trust_infra_cli::{Verdict, verify};

fn assert_unverifiable_without_panicking(bundle: &std::path::Path) {
    let key = SigningKey::generate(&mut OsRng).verifying_key();
    let result = catch_unwind(AssertUnwindSafe(|| verify(bundle, &key)));
    assert!(result.is_ok(), "verify panicked for {}", bundle.display());
    assert_eq!(result.unwrap().verdict, Verdict::Unverifiable);
}

#[test]
fn truncated_manifest_is_unverifiable() {
    let bundle = tempdir().unwrap();
    fs::write(bundle.path().join("manifest.json"), "{\"schema_version\":").unwrap();
    assert_unverifiable_without_panicking(bundle.path());
}

#[test]
fn empty_manifest_is_unverifiable() {
    let bundle = tempdir().unwrap();
    fs::write(bundle.path().join("manifest.json"), "").unwrap();
    assert_unverifiable_without_panicking(bundle.path());
}

#[test]
fn non_json_manifest_is_unverifiable() {
    let bundle = tempdir().unwrap();
    fs::write(bundle.path().join("manifest.json"), "not json").unwrap();
    assert_unverifiable_without_panicking(bundle.path());
}

#[test]
fn missing_manifest_is_unverifiable() {
    let bundle = tempdir().unwrap();
    assert_unverifiable_without_panicking(bundle.path());
}
