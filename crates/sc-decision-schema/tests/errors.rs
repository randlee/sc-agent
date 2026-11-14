//! `DecisionError` follows the workspace error convention (AC "REQ-SAG-0002").

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod tests {
    use std::error::Error;

    use sc_agent_http::{HttpError, NetworkKind};
    use sc_agent_types::{ErrorDetail, ErrorId};
    use sc_decision_schema::{DecisionError, HttpBadRequestError, HttpValidationError, LimitKind};

    fn network() -> HttpError {
        HttpError::network(NetworkKind::Connect, 1, "connection refused")
    }

    fn llm_chain() -> ErrorDetail {
        let lower = ErrorDetail::new("http.provider_busy", "the provider is busy", None);
        ErrorDetail::new(
            "llm.provider_failed",
            "the LLM provider failed",
            Some(lower),
        )
    }

    fn every_variant() -> Vec<DecisionError> {
        vec![
        DecisionError::RequestParse {
            path: "questions.q".to_owned(),
            message: "missing field `criteria`".to_owned(),
        },
        DecisionError::ResponseParse {
            status: 200,
            expected: "SystemOneResponse",
            message: "expected value".to_owned(),
        },
        DecisionError::DuplicateKey {
            key: "q".to_owned(),
        },
        DecisionError::Encode {
            message: "no".to_owned(),
        },
        DecisionError::ImageParts {
            detail: "image part `p` is referenced by no asset".to_owned(),
        },
        DecisionError::Limits {
            what: LimitKind::Images,
            max: 20,
        },
        DecisionError::ApiBadRequest {
            body: HttpBadRequestError {
                detail: serde_json::json!("bad"),
            },
        },
        DecisionError::ApiValidation {
            body: serde_json::from_str::<HttpValidationError>(
                r#"{"detail":[{"loc":["body","state"],"msg":"Field required","type":"missing"}]}"#,
            )
            .expect("422"),
        },
        DecisionError::ApiStatus {
            status: 503,
            body: b"down".to_vec(),
        },
        DecisionError::Http(network()),
        DecisionError::Backend {
            backend: "llm",
            detail: llm_chain(),
        },
        DecisionError::SchemaMismatch {
            backend: "systemone",
            detail: ErrorDetail::new("decision.schema", "answer `q` is missing", None),
        },
        DecisionError::Cancelled { sends: 1 },
        DecisionError::Shutdown {
            reason: "worker stopped".to_owned(),
        },
    ]
    }

    #[test]
    fn error_ids() {
        let errors = every_variant();
        let mut ids: Vec<&str> = errors.iter().map(ErrorId::id).collect();
        for id in &ids {
            let rest = id
                .strip_prefix("decision.")
                .unwrap_or_else(|| panic!("{id}"));
            assert!(
                !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
                "{id}"
            );
        }
        assert!(ids.contains(&"decision.image_parts") && ids.contains(&"decision.limits"));
        assert!(!ids.contains(&"decision.digest"));
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 14);

        for error in &errors {
            assert_eq!(
                error.to_string(),
                format!("{}: {}", error.id(), error.message())
            );
            // detail() is the provided one: id, message, recovery and cause.
            let detail = error.detail();
            assert_eq!(detail.id, error.id());
            assert_eq!(detail.message, error.message());
            assert_eq!(detail.recovery.as_deref(), error.recovery());
            assert_eq!(detail.cause.map(|cause| *cause), error.cause_detail());
            assert_eq!(
                detail.status,
                match error {
                    DecisionError::ApiStatus { status, .. } => Some(*status),
                    _ => None,
                }
            );
        }
    }

    #[test]
    fn http_wraps_unchanged() {
        let error = DecisionError::Http(network());
        let cause = error.cause_detail().expect("cause");
        assert_eq!(cause.id, "http.network");
        assert_eq!(cause, network().detail());
        assert!(matches!(&error, DecisionError::Http(inner) if *inner == network()));
        let source = error.source().expect("source");
        assert_eq!(source.to_string(), network().to_string());
        assert!(matches!(
            DecisionError::from(network()),
            DecisionError::Http(_)
        ));
    }

    #[test]
    fn backend_chain_kept() {
        let error = DecisionError::Backend {
            backend: "llm",
            detail: llm_chain(),
        };
        let detail = error.detail();
        let first = detail.cause.expect("first level");
        assert_eq!(
            (first.id.as_str(), first.message.as_str()),
            ("llm.provider_failed", "the LLM provider failed")
        );
        let second = first.cause.expect("second level");
        assert_eq!(
            (second.id.as_str(), second.message.as_str()),
            ("http.provider_busy", "the provider is busy")
        );
        assert_eq!(
            error.source().expect("source").to_string(),
            "llm.provider_failed: the LLM provider failed"
        );
        // Display is this level only.
        assert!(!error.to_string().contains("the provider is busy"));
    }

    /// RBP-F004: these variants carry a static recovery hint, kept out of the message.
    #[test]
    fn recovery_hints() {
        for error in every_variant() {
            if let DecisionError::Http(inner) = &error {
                assert_eq!(
                    error.recovery(),
                    inner.recovery(),
                    "decision.http forwards a-5's"
                );
                continue;
            }
            let expected = matches!(
                error.id(),
                "decision.request_parse"
                    | "decision.duplicate_key"
                    | "decision.encode"
                    | "decision.image_parts"
                    | "decision.limits"
                    | "decision.api_bad_request"
                    | "decision.api_validation"
                    | "decision.schema_mismatch"
                    | "decision.shutdown"
            );
            match error.recovery() {
                Some(recovery) => {
                    assert!(expected, "{}: unexpected recovery", error.id());
                    assert!(!recovery.is_empty());
                    assert!(!error.message().contains(recovery), "{}", error.id());
                    assert!(!error.to_string().contains(recovery), "{}", error.id());
                }
                None => assert!(!expected, "{}: no recovery", error.id()),
            }
        }
        assert_eq!(LimitKind::Images.as_str(), "images");
        assert_eq!(LimitKind::ImageBytes.to_string(), "image_bytes");
    }

    #[test]
    fn request_parse_recovery() {
        let errors = every_variant();
        let recovery = errors[0].recovery().expect("request_parse recovery");
        assert!(recovery.contains("path"));
    }
}
