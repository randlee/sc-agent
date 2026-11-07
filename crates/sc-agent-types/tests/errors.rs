//! Error taxonomy contract (REQ-SAG-0101 amended, REQ-SAG-0106, REQ-SAG-0701,
//! NFR-SAG-0001, ADR-SAG-0027).
//!
//! An integration target cannot build `#[non_exhaustive]` variants with
//! literals, so every error here comes from its public constructor; the
//! golden test therefore also proves each constructor exists.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;
use std::str::FromStr;

use sc_agent_types::{
    BackendKeyId, ErrorCode, JevError, NetworkErrorKind, ScAgentErrorResponse, SessionId,
};

fn repo_fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn crate_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// One value per variant (and per `Operational` area), keyed by a stable name.
fn every_variant() -> Vec<(&'static str, JevError)> {
    let session = SessionId::from_uuid(uuid_nil());
    let json_error = serde_json::from_str::<serde_json::Value>("{").expect_err("eof");
    vec![
        ("ConfigInvalid", JevError::config_invalid("missing model")),
        (
            "CredentialStoreInvalid",
            JevError::credential_store_invalid(
                "the repo-root .env file",
                "permission denied",
                "Make the file readable and retry.",
            ),
        ),
        (
            "RegistryInvalid",
            JevError::registry_invalid(
                "Backend jev is not registered",
                "Register it before the run.",
            ),
        ),
        (
            "SessionStoreIo",
            JevError::session_store_io(
                "Create session directory sessions",
                io::ErrorKind::PermissionDenied,
                "Choose a writable directory, then retry.",
            ),
        ),
        (
            "Operational.Config",
            JevError::operational_config("Pass --repo-root with the repository root."),
        ),
        (
            "Operational.Io",
            JevError::operational_io("stdout write failed. Check that stdout is writable."),
        ),
        (
            "SessionDocumentInvalid",
            JevError::session_document_invalid(
                "Session file s is not a session document",
                "Delete it and start a new session.",
            ),
        ),
        (
            "EncodeFailed",
            JevError::encode_failed("response JSON", &json_error),
        ),
        (
            "IoFailed",
            JevError::io_failed("stdout", &io::Error::from(io::ErrorKind::BrokenPipe)),
        ),
        ("KeyMissing", JevError::key_missing(BackendKeyId::TypeSafe)),
        ("InputTooLarge", JevError::input_too_large(10, 11)),
        ("InputEmpty", JevError::InputEmpty),
        ("InputInvalid", JevError::input_invalid("bad json")),
        ("ExtractionEmpty", JevError::ExtractionEmpty),
        (
            "ExtractionParseError",
            JevError::extraction_parse_error("a.py", 3),
        ),
        ("ExtractionInvalid", JevError::extraction_invalid("schema")),
        ("ExtractionFailed", JevError::extraction_failed("exit 1")),
        ("ExtractionTimeout", JevError::extraction_timeout(30)),
        (
            "LanguageUnavailable",
            JevError::language_unavailable("sc-treesitter", "cobol"),
        ),
        (
            "QueryInvalid",
            JevError::query_invalid("sc-treesitter", "syntax"),
        ),
        (
            "QueryNotFound",
            JevError::query_not_found("sc-treesitter", "missing"),
        ),
        (
            "ProducerError",
            JevError::producer_error("sc-treesitter", ErrorCode::QueryNotFound, "Use branches."),
        ),
        ("InputMissing", JevError::input_missing("gone.toml")),
        ("ResponseInvalid", JevError::response_invalid("truncated")),
        (
            "NetworkError",
            JevError::network(NetworkErrorKind::Status, "Jev HTTP 500"),
        ),
        (
            "PromptFileMissing",
            JevError::prompt_file_missing("prompt.md"),
        ),
        ("SessionNotFound", JevError::session_not_found(session)),
        ("SessionExpired", JevError::session_expired(session)),
    ]
}

fn uuid_nil() -> uuid::Uuid {
    uuid::Uuid::nil()
}

#[test]
fn test_every_variant_code_golden() {
    let text = std::fs::read_to_string(crate_fixture("error-codes.json")).expect("golden");
    let golden: BTreeMap<String, BTreeMap<String, String>> =
        serde_json::from_str(&text).expect("golden json");
    let mut actual = BTreeMap::new();
    for (name, error) in every_variant() {
        let hint = error.fix_hint();
        let first_line = hint.lines().next().unwrap_or_default().to_owned();
        actual.insert(
            name.to_owned(),
            BTreeMap::from([
                ("code".to_owned(), error.code().as_str().to_owned()),
                ("fix_hint".to_owned(), first_line),
            ]),
        );
    }
    assert_eq!(actual, golden);
    // Several variants share one id (REQ-SAG-0101 as amended by r-1).
    assert_eq!(golden["Operational.Config"]["code"], "CONFIG_INVALID");
    assert_eq!(golden["RegistryInvalid"]["code"], "CONFIG_INVALID");
}

#[test]
fn test_error_codes_match_errors_json() {
    let text = std::fs::read_to_string(repo_fixture(
        "tests/fixtures/jev/README-contract/errors.json",
    ))
    .expect("errors.json");
    let document: serde_json::Value = serde_json::from_str(&text).expect("json");
    let expected: BTreeSet<String> = document["error_ids"]
        .as_array()
        .expect("error_ids")
        .iter()
        .map(|id| id.as_str().expect("id").to_owned())
        .collect();
    let actual: BTreeSet<String> = ErrorCode::ALL
        .iter()
        .map(|code| code.as_str().to_owned())
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), ErrorCode::ALL.len(), "ids are unique");
    for code in ErrorCode::ALL {
        assert_eq!(ErrorCode::from_str(code.as_str()).expect("parse"), *code);
        assert_eq!(code.to_string(), code.as_str());
    }
    let produced: BTreeSet<&str> = every_variant()
        .iter()
        .map(|(_, error)| error.code().as_str())
        .collect();
    assert!(produced.iter().all(|code| expected.contains(*code)));
}

#[test]
fn test_error_response_stderr_golden() {
    let golden = std::fs::read_to_string(crate_fixture("error-stderr.jsonl")).expect("golden");
    let errors = [
        JevError::InputEmpty,
        JevError::network(NetworkErrorKind::Busy, "Jev busy: HTTP 429 after 4 sends"),
        JevError::producer_error(
            "sc-treesitter",
            ErrorCode::LanguageUnavailable,
            "Use a language built into sc-treesitter instead of cobol.",
        ),
    ];
    let mut actual = String::new();
    for error in &errors {
        let line =
            serde_json::to_string(&ScAgentErrorResponse::from_jev_error(error)).expect("json");
        actual.push_str(&line);
        actual.push('\n');
        let back: ScAgentErrorResponse = serde_json::from_str(&line).expect("reads back");
        assert_eq!(back, ScAgentErrorResponse::from_jev_error(error));
        assert_eq!(back.schema_version(), 1);
    }
    assert_eq!(actual, golden);
}

#[test]
fn test_retryable_only_network_transient() {
    for kind in NetworkErrorKind::ALL.iter().copied() {
        let error = JevError::network(kind, "detail");
        let expected = matches!(
            kind,
            NetworkErrorKind::Busy | NetworkErrorKind::Timeout | NetworkErrorKind::Connect
        );
        assert_eq!(error.retryable(), expected, "{kind}");
        let response = ScAgentErrorResponse::from_jev_error(&error);
        assert_eq!(response.retryable(), expected);
        let value = serde_json::to_value(&response).expect("json");
        assert_eq!(value.get("retryable").is_some(), expected, "{kind}");
    }
    for (name, error) in every_variant() {
        if !matches!(error, JevError::NetworkError { .. }) {
            assert!(!error.retryable(), "{name}");
        }
    }
}

#[test]
fn test_from_error_response_round_trip() {
    for code in ErrorCode::ALL.iter().copied() {
        let line = format!(r#"{{"error":"{code}","message":"Do the thing.","schema_version":1}}"#);
        let response: ScAgentErrorResponse = serde_json::from_str(&line).expect("response");
        let error = JevError::from_error_response("sc-treesitter", &response);
        assert!(
            matches!(error, JevError::ProducerError { .. }),
            "{code}: {error:?}"
        );
        assert_eq!(error.code(), code);
        assert_eq!(error.fix_hint(), "sc-treesitter: Do the thing.");
        let again = ScAgentErrorResponse::from_jev_error(&error);
        assert_eq!(again.error(), code.as_str());
        assert_eq!(again.producer(), Some("sc-treesitter"));
    }
    let unknown: ScAgentErrorResponse =
        serde_json::from_str(r#"{"error":"NOT_A_CODE","message":"m"}"#).expect("phase a shape");
    let error = JevError::from_error_response("tool", &unknown);
    assert_eq!(error.code(), "EXTRACTION_FAILED");
    assert!(error.fix_hint().contains("NOT_A_CODE"));
}

#[test]
fn test_network_error_kind_spelling() {
    let expected = [
        (NetworkErrorKind::Busy, "busy"),
        (NetworkErrorKind::Timeout, "timeout"),
        (NetworkErrorKind::Connect, "connect"),
        (NetworkErrorKind::Tls, "tls"),
        (NetworkErrorKind::Status, "status"),
        (NetworkErrorKind::BodyTooLarge, "body_too_large"),
        (NetworkErrorKind::Cancelled, "cancelled"),
    ];
    assert_eq!(expected.len(), NetworkErrorKind::ALL.len());
    for (kind, text) in expected {
        assert_eq!(kind.to_string(), text);
        assert_eq!(serde_json::to_value(kind).expect("json"), text);
        assert_eq!(
            serde_json::from_value::<NetworkErrorKind>(serde_json::json!(text)).expect("serde"),
            kind
        );
        assert_eq!(text.parse::<NetworkErrorKind>().expect("from_str"), kind);
    }
    let error = "Busy".parse::<NetworkErrorKind>().expect_err("unknown");
    assert_eq!(error.code(), "INPUT_INVALID");
}

#[test]
fn test_error_source_is_none_for_redacted_variants() {
    use std::error::Error as _;
    for (name, error) in every_variant() {
        assert!(error.source().is_none(), "{name}");
    }
}

#[test]
fn test_explicit_recovery_variants_do_not_blame_check_toml() {
    for (name, error) in every_variant() {
        let hint = error.fix_hint();
        assert!(!hint.trim().is_empty(), "{name}");
        if !matches!(
            name,
            "ConfigInvalid" | "InputInvalid" | "InputMissing" | "PromptFileMissing"
        ) {
            assert!(!hint.contains("check.toml"), "{name}: {hint}");
            assert!(!hint.contains("expected schema"), "{name}: {hint}");
        }
    }
}
