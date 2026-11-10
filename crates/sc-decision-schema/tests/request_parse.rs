//! `SystemOneRequest::from_json` names the failing field (AC "REQ-SAG-0504 support").

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod tests {
    use sc_agent_types::ErrorId;
    use sc_decision_schema::{DecisionError, OrderedMap, SystemOneRequest};

    fn parse_error(text: &str) -> (String, String) {
        match SystemOneRequest::from_json(text) {
            Err(DecisionError::RequestParse { path, message }) => (path, message),
            other => panic!("expected RequestParse, got {other:?}"),
        }
    }

    #[test]
    fn request_parse_errors() {
        // Score without criteria: the path names the question, the message the field.
        let (path, message) = parse_error(
            r#"{"model":"m","state":"s","questions":{"urgency":{"type":"score","instructions":"How urgent?"}}}"#,
        );
        assert_eq!(path, "questions.urgency");
        assert!(message.contains("missing field `criteria`"), "{message}");
        assert!(message.contains("line 1 column"), "{message}");

        // Unknown field inside a question.
        let (path, message) = parse_error(
            r#"{"model":"m","state":"s","questions":{"q1":{"type":"noul","instructoins":"typo"}}}"#,
        );
        assert_eq!(path, "questions.q1");
        assert!(
            message.contains("unknown field `instructoins`"),
            "{message}"
        );

        // Unknown field at the root: the path is the unknown field itself.
        let (path, message) =
            parse_error(r#"{"model":"m","state":"s","questions":{"q":{"type":"box"}},"extra":1}"#);
        assert_eq!(path, "extra");

        // Missing root field: the path is the root.
        let (path, message_root) = parse_error(r#"{"model":"m","questions":{"q":{"type":"box"}}}"#);
        assert_eq!(path, ".");
        assert!(
            message_root.contains("missing field `state`"),
            "{message_root}"
        );
        assert!(message.contains("unknown field `extra`"), "{message}");

        // Duplicate question id: refused at the questions object, naming the key.
        let (path, message) = parse_error(
            r#"{"model":"m","state":"s","questions":{"q":{"type":"box"},"q":{"type":"box"}}}"#,
        );
        assert_eq!(path, "questions");
        assert!(
            message.contains("duplicate key `q`; give every key once"),
            "{message}"
        );

        // Trailing text after the body.
        let (path, message) =
            parse_error(r#"{"model":"m","state":"s","questions":{"q":{"type":"box"}}} x"#);
        assert_eq!(path, ".");
        assert!(message.contains("trailing characters"), "{message}");
    }

    #[test]
    fn request_parse_error_contract() {
        let error = SystemOneRequest::from_json(r#"{"model":"m"}"#).expect_err("missing state");
        assert_eq!(error.id(), "decision.request_parse");
        assert!(
            error.message().contains("missing field `state`"),
            "{}",
            error.message()
        );
        assert!(
            error
                .recovery()
                .is_some_and(|recovery| !recovery.is_empty())
        );
    }

    #[test]
    fn try_from_pairs_refuses_duplicates() {
        let error = OrderedMap::try_from_pairs(vec![("a".to_owned(), 1), ("a".to_owned(), 2)])
            .expect_err("duplicate");
        assert!(matches!(&error, DecisionError::DuplicateKey { key } if key == "a"));
        assert_eq!(error.id(), "decision.duplicate_key");
        assert_eq!(
            error.to_string(),
            "decision.duplicate_key: duplicate key `a`; give every key once"
        );

        let map = OrderedMap::try_from_pairs(vec![("z".to_owned(), 1), ("a".to_owned(), 2)])
            .expect("distinct");
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&"a".to_owned()), Some(&2));
        assert_eq!(
            serde_json::to_string(&map).expect("encode"),
            r#"{"z":1,"a":2}"#
        );
    }
}
