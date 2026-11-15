use crate::error::Error;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

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
        let mut obj = value
            .as_object()
            .ok_or_else(|| Error::Manifest("manifest is not a JSON object".into()))?
            .clone();
        obj.remove("signature");
        serde_json::from_value(Value::Object(obj)).map_err(|e| Error::Manifest(e.to_string()))
    }
}
