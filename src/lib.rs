pub mod artifacts;
pub mod infra;
pub mod manifest;
pub mod report;
pub mod signature;

use ed25519_dalek::{SigningKey, VerifyingKey};
pub use report::{Check, CheckStatus, InfraDiffEntry, Report, Verdict};
use serde_json::Value;
use std::fs::{read_to_string, write};
use std::path::Path;

use crate::artifacts::{hash_file, validate_artifact_path};
use crate::signature::sign_value;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("malformed manifest: {0}")]
    Manifest(String),
    #[error("unsafe artifact path: {0}")]
    UnsafePath(String),
}

/// Looks for a `manifest.json` file in `bundle`, validates path for each artifact, signs the manifest, and writes it back to disk.
pub fn sign_bundle(bundle: &Path, key: &SigningKey) -> Result<(), Error> {
    let manifest_path = bundle.join("manifest.json");
    let raw_json = read_to_string(&manifest_path)?;
    let mut manifest_json: serde_json::Value = serde_json::from_str(&raw_json)?;

    let artifacts = manifest_json
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

    sign_value(&mut manifest_json, key);

    // TODO check if artifact dir contains artifacts not in manifest, return error

    write(&manifest_path, serde_json::to_vec_pretty(&manifest_json)?)?;

    Ok(())
}

pub fn verify(_bundle: &Path, _trusted: &VerifyingKey) -> Report {
    todo!()
}
