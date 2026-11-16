pub mod artifacts;
pub mod error;
pub mod infra;
pub mod manifest;
pub mod report;
pub mod signature;

use crate::artifacts::{
    check_artifact, find_unlisted_artifact_files, hash_file, validate_artifact_path,
};
use crate::error::Error;
use crate::manifest::{Manifest, validate_schema_version};
use crate::signature::{check_signature, sign_value};
use base64::Engine;
use ed25519_dalek::{SigningKey, VerifyingKey};
pub use report::{Check, CheckStatus, InfraDiffEntry, Report, Verdict, fail, finish, pass};
use serde_json::Value;
use std::fs::{read_to_string, write};
use std::path::Path;

/// Looks for a `manifest.json` file in `bundle`, validates path for each artifact, signs the manifest, and writes it back to disk.
pub fn sign_bundle(bundle: &Path, key: &SigningKey) -> Result<(), Error> {
    let mut manifest = read_manifest_value(bundle)?;
    let manifest_path = bundle.join("manifest.json");

    let listed_paths = manifest
        .get("artifacts")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Manifest("missing `artifacts` object".into()))?
        .iter()
        .map(|(name, entry)| {
            let entry = entry
                .as_object()
                .ok_or_else(|| Error::Manifest(format!("artifact `{name}` is not an object")))?;
            entry
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| Error::Manifest(format!("artifact `{name}` has no `path`")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let unlisted = find_unlisted_artifact_files(bundle, &listed_paths)?;
    if !unlisted.is_empty() {
        return Err(Error::UnlistedFiles(unlisted));
    }

    let artifacts = manifest
        .get_mut("artifacts")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| Error::Manifest("missing `artifacts` object".into()))?;

    for (name, entry) in artifacts.iter_mut() {
        let entry = entry
            .as_object_mut()
            .ok_or_else(|| Error::Manifest(format!("artifact `{name}` is not an object")))?;
        let rel = entry
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Manifest(format!("artifact `{name}` has no `path`")))?;

        let full_path = validate_artifact_path(bundle, rel)?;
        let digest = hash_file(&full_path)?;
        entry.insert("sha256".into(), Value::String(digest));
    }

    sign_value(&mut manifest, key);

    write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    Ok(())
}

pub fn verify(bundle: &Path, trusted: &VerifyingKey) -> Report {
    let mut checks: Vec<Check> = Vec::new();

    let trusted_key = base64::engine::general_purpose::STANDARD.encode(trusted.to_bytes());

    let value: Value = match read_manifest_value(bundle) {
        Ok(v) => v,
        Err(e) => {
            checks.push(fail("schema", format!("cannot read manifest.json: {e}")));
            return finish(checks, trusted_key, Vec::new());
        }
    };

    let sig_check = check_signature(&value, trusted);
    let sig_ok = sig_check.status == CheckStatus::Pass;

    checks.push(sig_check);

    let manifest: Manifest = match Manifest::try_from(&value) {
        Ok(m) => m,
        Err(e) => {
            checks.push(fail("schema", e.to_string()));
            return finish(checks, trusted_key, Vec::new());
        }
    };
    checks.push(pass("schema", "manifest parsed"));

    match find_unlisted_artifact_files(
        bundle,
        &manifest
            .artifacts
            .values()
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>(),
    ) {
        Ok(paths) if paths.is_empty() => {
            checks.push(pass("unlisted_files", "all artifact files are listed"));
        }
        Ok(paths) => {
            checks.push(fail("unlisted_files", describe_unlisted_files(&paths)));
        }
        Err(error) => {
            checks.push(fail(
                "unlisted_files",
                format!("cannot inspect artifacts: {error}"),
            ));
        }
    }

    let infra_diff = infra::diff(&manifest.infra_declared, &manifest.infra_actual);
    if infra_diff.is_empty() {
        checks.push(pass(
            "infra_diff",
            "declared and actual infrastructure match",
        ));
    } else {
        checks.push(fail(
            "infra_diff",
            format!(
                "{} difference(s): first path {}",
                infra_diff.len(),
                infra_diff[0].path
            ),
        ));
    }

    let untrusted = !sig_ok;
    for (name, entry) in &manifest.artifacts {
        checks.push(check_artifact(bundle, name, entry, untrusted));
    }

    finish(checks, trusted_key, infra_diff)
}

fn describe_unlisted_files(paths: &[String]) -> String {
    const DISPLAY_LIMIT: usize = 5;

    let shown = paths
        .iter()
        .take(DISPLAY_LIMIT)
        .cloned()
        .collect::<Vec<_>>();
    let mut detail = format!("unlisted files: {}", shown.join(", "));
    if paths.len() > DISPLAY_LIMIT {
        detail.push_str(&format!(" and {} more", paths.len() - DISPLAY_LIMIT));
    }
    detail
}

fn read_manifest_value(bundle: &Path) -> Result<Value, Error> {
    let manifest_path = bundle.join("manifest.json");
    let raw_json = read_to_string(&manifest_path)?;
    let manifest: Value = serde_json::from_str(&raw_json)?;
    validate_schema_version(&manifest)?;
    Ok(manifest)
}
