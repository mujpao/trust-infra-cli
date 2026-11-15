use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Verified,
    Tampered,
    Unverifiable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    pub id: String,
    pub status: CheckStatus,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InfraDiffEntry {
    pub path: String,
    pub declared: Option<String>,
    pub actual: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub verdict: Verdict,
    pub trusted_key: String,
    pub checks: Vec<Check>,
    pub infra_diff: Vec<InfraDiffEntry>,
}

impl Report {
    pub fn failed_ids(&self) -> Vec<&str> {
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::Fail)
            .map(|c| c.id.as_str())
            .collect()
    }
}

pub fn pass(id: impl Into<String>, detail: impl Into<String>) -> Check {
    Check {
        id: id.into(),
        status: CheckStatus::Pass,
        detail: detail.into(),
        expected: None,
        actual: None,
    }
}

pub fn fail(id: impl Into<String>, detail: impl Into<String>) -> Check {
    Check {
        id: id.into(),
        status: CheckStatus::Fail,
        detail: detail.into(),
        expected: None,
        actual: None,
    }
}

pub fn finish(checks: Vec<Check>, trusted_key: String, infra_diff: Vec<InfraDiffEntry>) -> Report {
    let verdict = if checks
        .iter()
        .any(|c| c.id == "schema" && c.status == CheckStatus::Fail)
    {
        Verdict::Unverifiable
    } else if checks.iter().any(|c| c.status == CheckStatus::Fail) {
        Verdict::Tampered
    } else {
        Verdict::Verified
    };
    Report {
        verdict,
        trusted_key,
        checks,
        infra_diff,
    }
}
