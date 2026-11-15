//! `DecisionError` follows the workspace error convention (AC "REQ-SAG-0002").

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod tests {
    use std::error::Error;

    use sc_agent_http::{BusyReason, HttpError, HttpResponse, NetworkKind, ProviderReply};
    use sc_agent_types::{ErrorCategory, ErrorDetail, ErrorId};
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
            detail: Box::new(llm_chain()),
        },
        DecisionError::SchemaMismatch {
            backend: "systemone",
            detail: Box::new(DecisionError::ResponseParse { status: 200, expected: "response", message: "answer missing".to_owned() }.detail()),
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
        sc_agent_testkit::assert_catalog_rows(&errors);
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
                    DecisionError::ApiBadRequest { .. } => Some(400),
                    DecisionError::ApiValidation { .. } => Some(422),
                    DecisionError::ApiStatus { status, .. }
                    | DecisionError::ResponseParse { status, .. } => Some(*status),
                    _ => None,
                }
            );
            assert_eq!(
                detail.attempts,
                match error {
                    DecisionError::Http(_) | DecisionError::Cancelled { .. } => Some(1),
                    _ => None,
                }
            );
            let expected_category = match error {
                DecisionError::RequestParse { .. }
                | DecisionError::DuplicateKey { .. }
                | DecisionError::ImageParts { .. }
                | DecisionError::Limits { .. }
                | DecisionError::ApiBadRequest { .. }
                | DecisionError::ApiValidation { .. } => ErrorCategory::Validation,
                DecisionError::ResponseParse { .. } | DecisionError::SchemaMismatch { .. } => {
                    ErrorCategory::ResponseInvalid
                }
                DecisionError::ApiStatus { .. } => ErrorCategory::Busy,
                DecisionError::Http(_) => ErrorCategory::Network,
                DecisionError::Cancelled { .. } => ErrorCategory::Cancelled,
                _ => ErrorCategory::Other,
            };
            let expected_category = if matches!(error, DecisionError::Backend { .. }) {
                None
            } else {
                Some(expected_category)
            };
            assert_eq!(detail.category, expected_category, "{}", error.id());
        }
    }

    #[test]
    fn http_wraps_unchanged() {
        let reply = ProviderReply::read(
            &HttpResponse::new(
                429,
                vec![("retry-after".to_owned(), "1.1".to_owned())],
                b"busy".to_vec(),
            ),
            None,
        );
        let http = HttpError::ProviderBusy {
            reason: BusyReason::Sends,
            attempts: 5,
            retry_after_seconds: Some(99.0),
            last: Box::new(reply),
        };
        let expected = http.detail();
        let source_text = http.to_string();
        let error = DecisionError::Http(http);
        let detail = error.detail();
        assert_eq!(
            (
                detail.status,
                detail.attempts,
                detail.retry_after_ms,
                detail.category
            ),
            (Some(429), Some(5), Some(1100), Some(ErrorCategory::Busy))
        );
        let cause = error.cause_detail().expect("cause");
        assert_eq!(cause.id, "http.provider_busy");
        assert_eq!(cause, expected);
        assert_eq!(error.source().expect("source").to_string(), source_text);
        assert!(matches!(
            DecisionError::from(network()),
            DecisionError::Http(_)
        ));
    }

    #[test]
    fn backend_chain_kept() {
        let mut cause = llm_chain();
        cause.status = Some(401);
        cause.category = Some(ErrorCategory::Auth);
        let error = DecisionError::Backend {
            backend: "llm",
            detail: Box::new(cause),
        };
        let detail = error.detail();
        assert_eq!(
            (detail.status, detail.category),
            (Some(401), Some(ErrorCategory::Auth))
        );
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

    #[test]
    fn schema_mismatch_keeps_cause_without_inheriting_fields() {
        let cause = DecisionError::ResponseParse {
            status: 200,
            expected: "response",
            message: "invalid body".to_owned(),
        }
        .detail();
        let error = DecisionError::SchemaMismatch {
            backend: "systemone",
            detail: Box::new(cause.clone()),
        };
        let detail = error.detail();
        assert_eq!(
            (
                detail.status,
                detail.attempts,
                detail.retry_after_ms,
                detail.category
            ),
            (None, None, None, Some(ErrorCategory::ResponseInvalid))
        );
        assert_eq!(detail.cause.as_deref(), Some(&cause));
        assert_eq!(
            (cause.id.as_str(), cause.status),
            ("decision.response_parse", Some(200))
        );
    }

    #[test]
    fn cancelled_carries_send_count() {
        let detail = DecisionError::Cancelled { sends: 3 }.detail();
        assert_eq!(
            (detail.attempts, detail.category),
            (Some(3), Some(ErrorCategory::Cancelled))
        );
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
