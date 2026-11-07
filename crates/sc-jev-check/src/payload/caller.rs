//! Caller-built request documents (`sc-ai jev --request-json`).
//!
//! Validation and [`PayloadBundle`] construction stay in this crate
//! (ADR-SAG-0005); `crate::run::CallerRequest::from_json` is the public
//! entry. The document has no system-prompt field. Prompt text is resolved
//! by `crate::run::prepare` from the override file or the packaged asset
//! (ADR-SAG-0019).

use sc_agent_types::{BackendId, ChoiceAnswer, JevError, PayloadBundle, Question};
use serde::Deserialize;
use serde_json::Value;

use super::encode_payload;
use super::questions::{validate_question_set, validate_state};

/// Canonical payload built from a caller questions document.
#[derive(Debug)]
pub(crate) struct ParsedCallerRequest {
    /// Sorted compact JSON and its digest.
    pub(crate) bundle: PayloadBundle,
    /// Backend ids named in the document. Empty when the document omits them.
    pub(crate) backends: Vec<BackendId>,
}

/// Validates `bytes` and builds one canonical payload.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when the document is empty, not the
/// questions shape, names an unknown backend, or includes `system_prompt`.
/// No byte cap applies here: each backend refuses an oversize request
/// before any send (REQ-SAG-0314).
pub(crate) fn parse_caller_request(bytes: &[u8]) -> Result<ParsedCallerRequest, JevError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(JevError::input_invalid(
            "request JSON is empty. Pass a questions document on --request-json.",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(request_json_error)?;
    reject_inline_prompt(&value)?;
    let document: RequestDocument = serde_json::from_value(value).map_err(request_json_error)?;
    validate_question_set(&document.questions)?;
    validate_state(&document.state, &document.questions)?;
    let bundle = encode_payload(&document.questions, &document.state)?;
    let mut backends = Vec::with_capacity(document.backends.len());
    for name in &document.backends {
        backends.push(name.parse::<BackendId>()?);
    }
    Ok(ParsedCallerRequest { bundle, backends })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestDocument {
    questions: Vec<Question>,
    #[serde(default)]
    state: Vec<ChoiceAnswer>,
    #[serde(default)]
    backends: Vec<String>,
}

fn reject_inline_prompt(value: &Value) -> Result<(), JevError> {
    let present = value
        .as_object()
        .is_some_and(|object| object.contains_key("system_prompt"));
    if present {
        return Err(JevError::input_invalid(
            "request JSON must not include system_prompt. Pass --llm-system-prompt-file, set check.toml [llm_judge] system_prompt_file, or use the packaged default.",
        ));
    }
    Ok(())
}

fn request_json_error(error: serde_json::Error) -> JevError {
    let detail = match error.classify() {
        serde_json::error::Category::Syntax => {
            "request JSON is not valid JSON. Pass a questions document.".to_owned()
        }
        serde_json::error::Category::Eof => "request JSON was empty or truncated.".to_owned(),
        serde_json::error::Category::Io => "request JSON could not be read.".to_owned(),
        serde_json::error::Category::Data => {
            "request JSON must contain a questions array and may contain state and backends."
                .to_owned()
        }
    };
    JevError::input_invalid(detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUESTIONS: &str = r#"{
        "questions": [{
            "id": "relevant",
            "prompt": "Does it apply?",
            "choices": ["no", "yes"]
        }],
        "state": [{"question_id": "relevant", "choice": "yes"}],
        "backends": ["anthropic_haiku"]
    }"#;

    #[test]
    fn caller_request_builds_a_canonical_bundle() {
        let parsed = parse_caller_request(QUESTIONS.as_bytes()).expect("request");
        assert_eq!(parsed.backends, vec![BackendId::AnthropicHaiku]);
        let text = std::str::from_utf8(parsed.bundle.bytes()).expect("utf8");
        assert!(text.contains("\"id\":\"relevant\""));
        assert!(text.contains("\"choice\":\"yes\""));
        assert!(!text.contains("system_prompt"));
        assert_eq!(parsed.bundle.sha256().as_str().len(), 64);
    }

    #[test]
    fn malformed_request_hides_the_raw_document() {
        let error = parse_caller_request(br#"{"invalid":true}"#).expect_err("shape");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(!error.fix_hint().contains("invalid"));
    }

    #[test]
    fn inline_system_prompt_is_rejected() {
        let bytes = br#"{"questions":[{"id":"q","prompt":"p","choices":["a","b"]}],"system_prompt":"INLINE-PROMPT-TOKEN"}"#;
        let error = parse_caller_request(bytes).expect_err("prompt");
        assert_eq!(error.code(), "INPUT_INVALID");
        let hint = error.fix_hint();
        assert!(hint.contains("--llm-system-prompt-file"));
        assert!(hint.contains("check.toml"));
        assert!(hint.contains("packaged default"));
        assert!(!hint.contains("INLINE-PROMPT-TOKEN"));
    }

    #[test]
    fn caller_json_uses_the_check_toml_rules() {
        let repeated_state = br#"{"questions":[{"id":"q","prompt":"p","choices":["a","b"]}],"state":[{"question_id":"q","choice":"a"},{"question_id":"q","choice":"b"}]}"#;
        let error = parse_caller_request(repeated_state).expect_err("repeat");
        assert!(error.fix_hint().contains("repeats question q"));
        let duplicate_id = br#"{"questions":[{"id":"q","prompt":"p","choices":["a"]},{"id":"q","prompt":"p","choices":["a"]}]}"#;
        let error = parse_caller_request(duplicate_id).expect_err("duplicate");
        assert!(error.fix_hint().contains("repeated"));
        let empty_prompt = br#"{"questions":[{"id":"q","prompt":"  ","choices":["a"]}]}"#;
        let error = parse_caller_request(empty_prompt).expect_err("empty");
        assert!(error.fix_hint().contains("instructions are empty"));
        let empty_id = br#"{"questions":[{"id":"","prompt":"p","choices":["a"]}]}"#;
        assert_eq!(
            parse_caller_request(empty_id).expect_err("id").code(),
            "INPUT_INVALID"
        );
    }

    #[test]
    fn state_choice_must_belong_to_the_question() {
        let bytes = br#"{"questions":[{"id":"q","prompt":"p","choices":["a","b"]}],"state":[{"question_id":"q","choice":"nope"}]}"#;
        let error = parse_caller_request(bytes).expect_err("state");
        assert!(error.fix_hint().contains("not in the question"));
    }
}
