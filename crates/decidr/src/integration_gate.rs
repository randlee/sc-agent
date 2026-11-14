//! Boundary-checked prerelease integration results, shared with the wheel's publication gate.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::DecidrError;

// Required case identifiers are generated from the packaged case record manifest.
include!(concat!(env!("OUT_DIR"), "/integration_cases.rs"));

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

/// A complete hexadecimal digest whose required length is part of its type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
struct Digest<const LENGTH: usize>(String);

impl<const LENGTH: usize> Digest<LENGTH> {
    fn new(value: String, field: &'static str) -> Result<Self, DecidrError> {
        if value.len() != LENGTH || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl Status {
    fn parse(value: &str) -> Result<Self, DecidrError> {
        match value {
            "PASS" => Ok(Self::Pass),
            "FAIL" => Ok(Self::Fail),
            "SKIP" => Ok(Self::Skip),
            _ => Err(invalid("status", "must be PASS, FAIL or SKIP")),
        }
    }
}

/// A bounded printable reason supplied by the producer after credential redaction.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
struct SanitizedReason(String);

impl SanitizedReason {
    fn new(value: String) -> Result<Self, DecidrError> {
        if value.trim().is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
            return Err(invalid(
                "reason",
                "must contain 1..=4096 printable UTF-8 bytes, redacted by the producer",
            ));
        }
        Ok(Self(value))
    }
}

/// A validated session ULID.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
struct Session(String);

impl Session {
    fn new(value: String) -> Result<Self, DecidrError> {
        if value.len() != 26
            || value.as_bytes().first().is_none_or(|b| *b > b'7')
            || !value
                .bytes()
                .all(|b| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&b))
        {
            return Err(invalid(
                "session_id",
                "must be a canonical 26-character ULID",
            ));
        }
        Ok(Self(value))
    }
}

/// Checked result contract. Fields cannot be instantiated without boundary validation.
#[derive(Debug, Clone, Serialize)]
struct CaseResult {
    case_id: Identifier,
    platform: Identifier,
    status: Status,
    reason: SanitizedReason,
    prerelease_tag: PrereleaseTag,
    source_commit: Digest<40>,
    artifact_sha256: Digest<64>,
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
        let status = Status::parse(&raw.status)?;
        let session_id = raw.session_id.map(Session::new).transpose()?;
        Ok(Self {
            case_id: Identifier::new(raw.case_id, "case_id")?,
            platform: Identifier::new(raw.platform, "platform")?,
            status,
            reason: SanitizedReason::new(raw.reason)?,
            prerelease_tag: PrereleaseTag::new(raw.prerelease_tag)?,
            source_commit: Digest::<40>::new(raw.source_commit, "source_commit")?,
            artifact_sha256: Digest::<64>::new(raw.artifact_sha256, "artifact_sha256")?,
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
    let source = Digest::<40>::new(source_commit.to_owned(), "source_commit")?;
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
    let mut seen = BTreeSet::<&str>::new();
    for result in &results {
        if !seen.insert(result.case_id.0.as_str()) {
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
    if REQUIRED_CASES.iter().any(|case| !seen.contains(*case)) {
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
    fn status_owns_parsing_and_serde_for_the_wire_values() {
        for (wire, expected) in [
            ("PASS", Status::Pass),
            ("FAIL", Status::Fail),
            ("SKIP", Status::Skip),
        ] {
            assert_eq!(Status::parse(wire).unwrap(), expected);
            assert_eq!(
                serde_json::from_value::<Status>(json!(wire)).unwrap(),
                expected
            );
            assert_eq!(serde_json::to_value(expected).unwrap(), json!(wire));
        }
    }

    #[test]
    fn generated_required_cases_match_the_packaged_case_manifest() {
        let records: Vec<Value> =
            serde_json::from_str(include_str!("../integration-cases.json")).unwrap();
        let manifest_cases: Vec<_> = records
            .iter()
            .map(|record| record["id"].as_str().unwrap())
            .collect();
        assert_eq!(integration_cases(), manifest_cases);
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
            if field == "status" {
                match validate(vec![result]).unwrap_err() {
                    DecidrError::IntegrationEvidence {
                        field: actual_field,
                        reason,
                        ..
                    } => {
                        assert_eq!(actual_field, "status");
                        assert_eq!(reason, "must be PASS, FAIL or SKIP");
                    }
                    error => panic!("expected status integration error, got {error}"),
                }
                continue;
            }
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
