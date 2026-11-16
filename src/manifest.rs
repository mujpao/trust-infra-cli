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
