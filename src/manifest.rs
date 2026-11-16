use crate::error::Error;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: &str = "0.1";

pub(crate) fn validate_schema_version(value: &Value) -> Result<(), Error> {
    let schema_version = value
        .as_object()
        .ok_or_else(|| Error::Manifest("manifest is not a JSON object".into()))?
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Manifest("missing or invalid `schema_version`".into()))?;

    if schema_version != SCHEMA_VERSION {
        return Err(Error::Schema(schema_version.to_owned()));
    }

    Ok(())
}

pub(crate) fn validate_no_float_numbers(value: &Value) -> Result<(), Error> {
    match value {
        Value::Object(object) => {
            for child in object.values() {
                validate_no_float_numbers(child)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                validate_no_float_numbers(child)?;
            }
        }
        Value::Number(number) if number.is_f64() => {
            return Err(Error::Manifest(
                "manifest contains a floating-point number".to_string(),
            ));
        }
        _ => {}
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: String,
    pub run_id: String,
    pub timestamp: DateTime<Utc>,
    pub model: String,
    pub scorer_commit: String,
    pub judge_model: String,
    pub grading_prompt: Option<String>,
    pub artifacts: BTreeMap<String, Artifact>,
    pub infra_declared: Value,
    pub infra_actual: Value,
}

impl TryFrom<&Value> for Manifest {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        validate_schema_version(value)?;
        validate_no_float_numbers(value)?;

        let mut obj = value
            .as_object()
            .ok_or_else(|| Error::Manifest("manifest is not a JSON object".into()))?
            .clone();
        obj.remove("signature");
        let manifest: Manifest = serde_json::from_value(Value::Object(obj))
            .map_err(|e| Error::Manifest(e.to_string()))?;

        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_manifest() -> Value {
        json!({
            "schema_version": "0.1",
            "run_id": "run-1",
            "timestamp": "2026-01-01T00:00:00Z",
            "model": "model-v1",
            "scorer_commit": "abc123",
            "judge_model": "judge-v1",
            "grading_prompt": "grade carefully",
            "artifacts": {
                "dataset": {
                    "path": "artifacts/dataset.json",
                    "sha256": "0".repeat(64)
                }
            },
            "infra_declared": {
                "image": "sha256:aaa",
                "packages": {
                    "python": 3
                }
            },
            "infra_actual": {
                "image": "sha256:aaa",
                "packages": {
                    "python": 3
                }
            }
        })
    }

    #[test]
    fn rejects_float_numbers_in_manifest() {
        let mut manifest = valid_manifest();
        manifest["infra_declared"] = json!({
            "image": "sha256:aaa",
            "packages": {
                "python": 3.12
            }
        });

        let err = Manifest::try_from(&manifest).unwrap_err();
        assert!(matches!(err, Error::Manifest(message) if message.contains("floating-point")));
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let mut manifest = valid_manifest();
        manifest["schema_version"] = json!("0.2");

        let err = Manifest::try_from(&manifest).unwrap_err();
        assert!(matches!(err, Error::Schema(_)));
    }
}
