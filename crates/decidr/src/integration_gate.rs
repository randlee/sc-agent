//! Boundary-checked prerelease integration results, shared with the wheel's publication gate.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::DecidrError;

/// Required live cases. Each runs against an installed binary or wheel, never source substitutes.
const REQUIRED_CASES: [&str; 10] = [
    "typesafe-models-cli",
    "typesafe-models-wheel",
    "typesafe-decision-cli",
    "typesafe-decision-wheel",
    "anthropic-decision-cli",
    "anthropic-decision-wheel",
    "typesafe-invalid-key-cli",
    "typesafe-invalid-key-wheel",
    "anthropic-invalid-key-cli",
    "anthropic-invalid-key-wheel",
];

/// Returns the required case identifiers for the installed wheel's integration contract.
#[must_use]
pub fn integration_cases() -> Vec<&'static str> {
    REQUIRED_CASES.to_vec()
}

/// Validates and serializes a complete passing platform run for the installed wheel.
///
/// Checked representations stay inside this module; callers receive the existing JSON contract.
///
/// # Errors
/// Returns `decidr.integration_evidence` for malformed, missing or non-passing case results,
/// or the existing serialization error if the validated records cannot be serialized.
pub fn validate_integration_results(
    json: &str,
    source_commit: &str,
) -> Result<String, DecidrError> {
    crate::output::to_json(&validate_results(json, source_commit)?)
}

fn invalid(field: &'static str, reason: &'static str) -> DecidrError {
    DecidrError::IntegrationEvidence {
        field,
        reason,
        source: None,
    }
}

/// An extensible case/platform identifier, checked once at the JSON boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
struct Identifier(String);

impl Identifier {
    fn new(value: String, field: &'static str) -> Result<Self, DecidrError> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err(invalid(
                field,
                "must contain 1..=128 ASCII letters, digits, dots, hyphens or underscores",
            ));
        }
        Ok(Self(value))
    }
}

/// A complete SHA-1 source revision or SHA-256 artifact digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
struct Digest(String);

impl Digest {
    fn new(value: String, len: usize, field: &'static str) -> Result<Self, DecidrError> {
        if value.len() != len || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid(
                field,
                "must be a complete hexadecimal digest (40 source or 64 artifact characters)",
            ));
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
}

/// A validated prerelease tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
struct PrereleaseTag(String);

impl PrereleaseTag {
    fn new(value: String) -> Result<Self, DecidrError> {
        let version = value
            .strip_prefix("prerelease/v")
            .ok_or_else(|| invalid("prerelease_tag", "must start with prerelease/v"))?;
        let parts: Vec<_> = version.split('.').collect();
        if parts.len() != 3
            || parts.iter().any(|p| {
                p.is_empty()
                    || !p.bytes().all(|b| b.is_ascii_digit())
                    || (p.len() > 1 && p.starts_with('0'))
                    || p.parse::<u64>().is_err()
            })
        {
            return Err(invalid(
                "prerelease_tag",
                "must carry a complete major.minor.patch version",
            ));
        }
        Ok(Self(value))
    }
}

/// A case outcome. SKIP and FAIL are preserved as evidence and never pass the gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum Status {
    /// The actual case passed.
    #[serde(rename = "PASS")]
    Pass,
    /// The actual case failed.
    #[serde(rename = "FAIL")]
    Fail,
    /// The case could not run.
    #[serde(rename = "SKIP")]
    Skip,
}

/// A bounded printable reason supplied by the producer after credential redaction.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
struct SanitizedReason(String);

/// A validated session ULID.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
struct Session(String);

/// Checked result contract. Fields cannot be instantiated without boundary validation.
#[derive(Debug, Clone, Serialize)]
struct CaseResult {
    case_id: Identifier,
    platform: Identifier,
    status: Status,
    reason: SanitizedReason,
    prerelease_tag: PrereleaseTag,
    source_commit: Digest,
    artifact_sha256: Digest,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<Session>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResult {
    case_id: String,
    platform: String,
    status: String,
    reason: String,
    prerelease_tag: String,
    source_commit: String,
    artifact_sha256: String,
    session_id: Option<String>,
}

impl TryFrom<RawResult> for CaseResult {
    type Error = DecidrError;
    fn try_from(raw: RawResult) -> Result<Self, Self::Error> {
        let status = match raw.status.as_str() {
            "PASS" => Status::Pass,
            "FAIL" => Status::Fail,
            "SKIP" => Status::Skip,
            _ => return Err(invalid("status", "must be PASS, FAIL or SKIP")),
        };
        if raw.reason.trim().is_empty()
            || raw.reason.len() > 4096
            || raw.reason.chars().any(char::is_control)
        {
            return Err(invalid(
                "reason",
                "must contain 1..=4096 printable UTF-8 bytes, redacted by the producer",
            ));
        }
        let session_id = raw
            .session_id
            .map(|id| {
                if id.len() != 26
                    || id.as_bytes().first().is_none_or(|b| *b > b'7')
                    || !id
                        .bytes()
                        .all(|b| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&b))
                {
                    return Err(invalid(
                        "session_id",
                        "must be a canonical 26-character ULID",
                    ));
                }
                Ok(Session(id))
            })
            .transpose()?;
        Ok(Self {
            case_id: Identifier::new(raw.case_id, "case_id")?,
            platform: Identifier::new(raw.platform, "platform")?,
            status,
            reason: SanitizedReason(raw.reason),
            prerelease_tag: PrereleaseTag::new(raw.prerelease_tag)?,
            source_commit: Digest::new(raw.source_commit, 40, "source_commit")?,
            artifact_sha256: Digest::new(raw.artifact_sha256, 64, "artifact_sha256")?,
            session_id,
        })
    }
}

/// Parses and validates one complete passing platform run, tied to `source_commit`.
///
/// # Errors
/// Returns `decidr.integration_evidence` for malformed fields, duplicate/missing cases,
/// mixed platforms/tags/sources, or any non-PASS outcome. No clock or external state is used.
fn validate_results(json: &str, source_commit: &str) -> Result<Vec<CaseResult>, DecidrError> {
    let source = Digest::new(source_commit.to_owned(), 40, "source_commit")?;
    let raw: Vec<RawResult> =
        serde_json::from_str(json).map_err(|error| DecidrError::IntegrationEvidence {
            field: "results",
            reason: "must be an array of complete case-result objects",
            source: Some(Box::new(sc_agent_types::ErrorDetail::foreign(
                "serde_json",
                &error,
            ))),
        })?;
    let results: Vec<_> = raw
        .into_iter()
        .map(CaseResult::try_from)
        .collect::<Result<_, _>>()?;
    let first = results
        .first()
        .ok_or_else(|| invalid("results", "must include every required case"))?;
    let mut seen = BTreeSet::new();
    for result in &results {
        if !seen.insert(&result.case_id.0) {
            return Err(invalid("case_id", "duplicate case in platform run"));
        }
        if result.status != Status::Pass {
            return Err(invalid(
                "status",
                "all required cases must PASS; SKIP cannot pass",
            ));
        }
        if result.source_commit != source
            || result.platform != first.platform
            || result.prerelease_tag != first.prerelease_tag
        {
            return Err(invalid(
                "results",
                "all cases must match one source, platform and prerelease tag",
            ));
        }
    }
    if REQUIRED_CASES
        .iter()
        .any(|case| !seen.contains(&case.to_string()))
    {
        return Err(invalid(
            "case_id",
            "one or more required live cases are missing",
        ));
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_agent_types::ErrorId;
    use serde_json::{Value, json};

    fn complete() -> Vec<Value> {
        REQUIRED_CASES.iter().map(|case| json!({
            "case_id": case, "platform": "macos-aarch64", "status": "PASS", "reason": "live case passed",
            "prerelease_tag": "prerelease/v0.1.1", "source_commit": "a".repeat(40), "artifact_sha256": "b".repeat(64),
            "session_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV"
        })).collect()
    }
    fn validate(results: Vec<Value>) -> Result<Vec<CaseResult>, DecidrError> {
        validate_results(&serde_json::to_string(&results).unwrap(), &"a".repeat(40))
    }
    #[test]
    fn complete_platform_passes_and_round_trips_without_reshaping() {
        let raw = complete();
        assert_eq!(integration_cases(), REQUIRED_CASES);
        assert_eq!(
            serde_json::from_str::<Value>(
                &validate_integration_results(
                    &serde_json::to_string(&raw).unwrap(),
                    &"a".repeat(40)
                )
                .unwrap(),
            )
            .unwrap(),
            json!(raw)
        );
        let mut extra = raw;
        extra[0]["platform"] = json!("future-platform.v2");
        assert!(
            CaseResult::try_from(serde_json::from_value::<RawResult>(extra[0].clone()).unwrap())
                .is_ok()
        );
    }
    #[test]
    fn skipped_failed_missing_duplicate_and_mixed_runs_fail_closed() {
        for status in ["FAIL", "SKIP", "unknown"] {
            let mut results = complete();
            results[0]["status"] = json!(status);
            assert_eq!(
                validate(results).unwrap_err().id(),
                "decidr.integration_evidence"
            );
        }
        let mut missing = complete();
        missing.pop();
        assert!(validate(missing).is_err());
        assert!(validate(vec![]).is_err());
        let mut duplicates = complete();
        duplicates.push(duplicates[0].clone());
        assert!(validate(duplicates).is_err());
        for (field, value) in [
            ("source_commit", "c".repeat(40)),
            ("platform", "linux-aarch64".to_owned()),
            ("prerelease_tag", "prerelease/v0.1.2".to_owned()),
        ] {
            let mut results = complete();
            results[0][field] = json!(value);
            assert!(validate(results).is_err());
        }
    }
    #[test]
    fn every_record_field_is_validated_at_the_boundary() {
        for (field, value) in [
            ("case_id", ""),
            ("case_id", "bad/path"),
            ("platform", "bad platform"),
            ("status", "pass"),
            ("reason", ""),
            ("reason", "line\nbreak"),
            ("prerelease_tag", "v0.1.1"),
            ("prerelease_tag", "prerelease/v01.2.3"),
            ("source_commit", "abc"),
            ("artifact_sha256", "zzzz"),
            ("session_id", "not-a-ulid"),
        ] {
            let mut result = complete().remove(0);
            result[field] = json!(value);
            let raw: RawResult = serde_json::from_value(result).unwrap();
            assert_eq!(
                CaseResult::try_from(raw).unwrap_err().id(),
                "decidr.integration_evidence",
                "{field}"
            );
        }
        for field in [
            "case_id",
            "platform",
            "status",
            "reason",
            "prerelease_tag",
            "source_commit",
            "artifact_sha256",
        ] {
            let mut results = complete();
            results[0].as_object_mut().unwrap().remove(field);
            assert!(validate(results).is_err(), "{field}");
        }
        for (field, value) in [("case_id", "x".repeat(129)), ("reason", "x".repeat(4097))] {
            let mut results = complete();
            results[0][field] = json!(value);
            assert!(validate(results).is_err());
        }
    }
}
