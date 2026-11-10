//! ErrorId and ErrorDetail (REQ-SAG-0002).

use std::collections::BTreeSet;
// `Error` in scope on purpose: `cause_detail()` must not be ambiguous with `Error::cause` (E0034).
use std::error::Error;
use std::fmt;
use std::io;

use sc_agent_types::{
    ErrorDetail, ErrorId, ModelId, Provider, ReleaseDate, SessionId, ShortName, TypesError, VarName,
};

#[cfg(test)]
fn every_types_error() -> Vec<TypesError> {
    let model: ModelId = "claude-haiku-5-5".parse().expect("model");
    vec![
        "".parse::<ModelId>().expect_err("model"),
        "nope".parse::<ShortName>().expect_err("short name"),
        "nope".parse::<Provider>().expect_err("provider"),
        "soon".parse::<ReleaseDate>().expect_err("date"),
        TypesError::ModelOnTwoLines {
            model,
            line: ShortName::Sonnet,
            other_line: ShortName::Haiku,
        },
        "lower".parse::<VarName>().expect_err("var name"),
        "nope".parse::<SessionId>().expect_err("session id"),
    ]
}

/// Every TypesError variant has a distinct id `types.<variant_snake>`; Display is
/// `"<id>: <message>"` only; bottom crate: no cause.
#[test]
fn test_types_error_ids_display_and_detail() {
    let errors = every_types_error();
    let ids: Vec<&str> = errors.iter().map(ErrorId::id).collect();
    assert_eq!(
        ids,
        [
            "types.invalid_model_id",
            "types.unknown_short_name",
            "types.unknown_provider",
            "types.invalid_release_date",
            "types.model_on_two_lines",
            "types.invalid_var_name",
            "types.invalid_session_id",
        ]
    );
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
    for error in &errors {
        assert!(!error.message().is_empty());
        assert_eq!(
            error.to_string(),
            format!("{}: {}", error.id(), error.message())
        );
        assert!(error.cause_detail().is_none());
        assert!(error.source().is_none());
    }
}

/// Every TypesError variant has a non-empty static recovery hint; the short-name and provider
/// hints list exactly the spellings `FromStr` accepts.
#[test]
fn test_every_types_error_has_a_recovery_hint() {
    for error in every_types_error() {
        let recovery = error
            .recovery()
            .unwrap_or_else(|| panic!("{} has no recovery", error.id()));
        assert!(!recovery.is_empty(), "{}", error.id());
    }
    let listed = |names: Vec<&str>| format!("use one of: {}", names.join(", "));
    let short_names = listed(ShortName::ALL.iter().map(|name| name.as_str()).collect());
    let providers = listed(
        Provider::ALL
            .iter()
            .map(|provider| provider.as_str())
            .collect(),
    );
    let error = "nope".parse::<ShortName>().expect_err("short name");
    assert_eq!(error.recovery(), Some(short_names.as_str()));
    let error = "nope".parse::<Provider>().expect_err("provider");
    assert_eq!(error.recovery(), Some(providers.as_str()));
    assert!(
        "lower"
            .parse::<VarName>()
            .expect_err("var name")
            .recovery()
            .is_some_and(|hint| hint.contains("^[A-Z_][A-Z0-9_]{0,127}$"))
    );
}

/// RBP-F002: for every TypesError variant, the provided detail() equals the value built from
/// id(), message(), recovery() and cause_detail(): no implementation overrides it.
#[test]
fn test_detail_is_built_from_the_four_methods() {
    for error in every_types_error() {
        let expected = ErrorDetail {
            id: error.id().to_owned(),
            message: error.message(),
            recovery: error.recovery().map(std::borrow::Cow::Borrowed),
            cause: error.cause_detail().map(Box::new),
        };
        assert_eq!(error.detail(), expected, "{}", error.id());
        assert!(error.detail().recovery.is_some(), "{}", error.id());
    }
}

/// ErrorDetail round-trips through serde with its cause chain; recovery is omitted when None.
#[test]
fn test_error_detail_serde_roundtrip_and_recovery_omitted() {
    let bottom = ErrorDetail::new("types.invalid_model_id", "bad id", None);
    let detail = ErrorDetail::new("decision.backend_failed", "failed", Some(bottom))
        .with_recovery("check the model id");
    let json = serde_json::to_value(&detail).expect("serialize");
    assert_eq!(
        json,
        serde_json::json!({
            "id": "decision.backend_failed",
            "message": "failed",
            "recovery": "check the model id",
            "cause": { "id": "types.invalid_model_id", "message": "bad id" }
        })
    );
    assert_eq!(
        serde_json::from_value::<ErrorDetail>(json).expect("deserialize"),
        detail
    );
    let plain = serde_json::to_value(ErrorDetail::new("x.y", "z", None)).expect("serialize");
    assert_eq!(plain, serde_json::json!({ "id": "x.y", "message": "z" }));
    assert_eq!(detail.to_string(), "decision.backend_failed: failed");
}

/// A lower-layer error wrapped two levels deep: the outermost chain carries the id and message
/// of every level, and `source()` walks the same chain.
#[test]
fn test_two_level_chain_carries_every_cause() {
    let lowest = ErrorDetail::new("http.provider_busy", "the provider is busy", None);
    let middle = ErrorDetail::new("llm.request_failed", "the request failed", Some(lowest));
    let outer = ErrorDetail::new("decision.backend_failed", "no decision", Some(middle));

    let mut chain = Vec::new();
    let mut level = Some(&outer);
    while let Some(detail) = level {
        chain.push((detail.id.as_str(), detail.message.as_str()));
        level = detail.cause.as_deref();
    }
    assert_eq!(
        chain,
        [
            ("decision.backend_failed", "no decision"),
            ("llm.request_failed", "the request failed"),
            ("http.provider_busy", "the provider is busy"),
        ]
    );

    let mut displayed = Vec::new();
    let mut source: Option<&(dyn Error + 'static)> = Some(&outer);
    while let Some(error) = source {
        displayed.push(error.to_string());
        source = error.source();
    }
    assert_eq!(
        displayed,
        [
            "decision.backend_failed: no decision",
            "llm.request_failed: the request failed",
            "http.provider_busy: the provider is busy",
        ]
    );
}

/// A wrapping error the way a higher crate writes one: it keeps the lower value, overrides
/// recovery and uses the provided detail().
#[derive(Debug)]
struct Wrapping(TypesError);

impl fmt::Display for Wrapping {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl Error for Wrapping {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl ErrorId for Wrapping {
    fn id(&self) -> &'static str {
        "config.invalid_model"
    }
    fn message(&self) -> String {
        "the configured model is invalid".to_owned()
    }
    fn cause_detail(&self) -> Option<ErrorDetail> {
        Some(self.0.detail())
    }
    fn recovery(&self) -> Option<&'static str> {
        Some("fix the model id in the config")
    }
}

/// `cause_detail()` and `recovery()` called through `&dyn ErrorId` with `Error` in scope
/// compile (the E0034 guard), and the provided detail() carries the chain and recovery.
#[test]
fn test_provided_detail_through_dyn_error_id() {
    let lower = "".parse::<ModelId>().expect_err("model");
    let wrapping = Wrapping(lower.clone());
    let error: &dyn ErrorId = &wrapping;
    assert_eq!(error.cause_detail(), Some(lower.detail()));
    assert_eq!(error.recovery(), Some("fix the model id in the config"));
    let detail = error.detail();
    assert_eq!(detail.id, "config.invalid_model");
    assert_eq!(
        detail.recovery.as_deref(),
        Some("fix the model id in the config")
    );
    assert_eq!(detail.cause.as_deref(), Some(&lower.detail()));
    assert_eq!(
        error.to_string(),
        "config.invalid_model: the configured model is invalid"
    );
    assert!(!error.to_string().contains("types.invalid_model_id"));
    let as_error: &dyn Error = error;
    assert!(as_error.source().is_some());
}

/// A foreign error with a source chain: `ErrorDetail::foreign` keeps every level.
#[derive(Debug)]
struct ReadFailed(io::Error);

impl fmt::Display for ReadFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("could not read the session file")
    }
}

impl Error for ReadFailed {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

#[test]
fn test_foreign_keeps_an_io_error_source_chain() {
    let io_error = io::Error::new(io::ErrorKind::NotFound, "no such file");
    let io_text = io_error.to_string();
    let error = ReadFailed(io_error);
    let detail = ErrorDetail::foreign("io.not_found", &error);
    assert_eq!(detail.id, "io.not_found");
    assert_eq!(detail.message, "could not read the session file");
    assert!(detail.recovery.is_none());
    let cause = detail.cause.as_deref().expect("io cause");
    assert_eq!(cause.id, "foreign.source");
    assert_eq!(cause.message, io_text);
    assert!(cause.cause.is_none());
}
