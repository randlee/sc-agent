//! Trait shape, object safety and the public surface.
//!
//! AC "Backend/Decision shape", AC "ADR-SAG-0001", AC "ANC-001", AC "REQ-SAG-0006 AC1".

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use sc_agent_http::{Endpoint, HttpRequest, HttpResponse, LogContext, Method};
    use sc_agent_types::{CancelToken, SessionId};
    use sc_decision_schema::{
        Backend, Decision, DecisionError, DecisionRequest, DecisionResult, SystemOneRequest,
        SystemOneResponse,
    };

    /// An in-test backend: encodes the body as JSON, decodes the reply body.
    #[derive(Debug)]
    struct EchoBackend;

    impl Backend for EchoBackend {
        fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError> {
            let endpoint = Endpoint::parse("https://api.example.test/v1/systemone")?;
            Ok(HttpRequest::new(
                Method::Post,
                endpoint,
                request.body().to_json()?.into_bytes(),
            ))
        }

        fn decode(
            &self,
            response: HttpResponse,
            _request: &DecisionRequest,
            _ctx: &LogContext,
        ) -> Result<SystemOneResponse, DecisionError> {
            SystemOneResponse::from_json(response.status(), response.body())
        }
    }

    /// An in-test decision: answers every request with `Cancelled` when `cancel` is set.
    struct CancelledDecision;

    impl Decision for CancelledDecision {
        fn decide(&self, request: DecisionRequest, _cancel: &CancelToken) -> DecisionResult {
            DecisionResult::new(
                SessionId::new(),
                0,
                request.digests().clone(),
                Err(DecisionError::Cancelled { sends: 0 }),
            )
        }

        fn decide_stream<'a>(
            &'a self,
            requests: Box<dyn Iterator<Item = DecisionRequest> + Send + 'static>,
            _cancel: &CancelToken,
        ) -> Box<dyn Iterator<Item = DecisionResult> + Send + 'a> {
            Box::new(requests.zip(0..).map(|(request, position)| {
                DecisionResult::new(
                    SessionId::new(),
                    position,
                    request.digests().clone(),
                    Err(DecisionError::Cancelled { sends: 0 }),
                )
            }))
        }
    }

    fn request(id: &str) -> DecisionRequest {
        let text =
            format!(r#"{{"model":"m","state":"s","questions":{{"{id}":{{"type":"noul"}}}}}}"#);
        DecisionRequest::try_from(SystemOneRequest::from_json(&text).expect("body"))
            .expect("request")
    }

    #[test]
    fn object_safety() {
        let backend: Arc<dyn Backend> = Arc::new(EchoBackend);
        let http = backend.encode(&request("q")).expect("encode");
        assert_eq!(http.method(), Method::Post);
        let reply = HttpResponse::new(
        200,
        Vec::new(),
        br#"{"model":"m","answers":{"q":{"type":"noul","noul":0.5}},"usage":{"input_tokens":1,"output_tokens":1}}"#.to_vec(),
    );
        let ctx = LogContext::default();
        let decided = backend.decode(reply, &request("q"), &ctx).expect("decode");
        assert_eq!(decided.model, "m");

        let decision: Box<dyn Decision> = Box::new(CancelledDecision);
        let cancel = CancelToken::new();
        let one = decision.decide(request("q"), &cancel);
        assert_eq!(one.position, 0);
        assert_eq!(one.digests, request("q").digests().clone());
        assert!(matches!(
            one.outcome,
            Err(DecisionError::Cancelled { sends: 0 })
        ));

        let results: Vec<DecisionResult> = decision
            .decide_stream(
                Box::new(vec![request("a"), request("b")].into_iter()),
                &cancel,
            )
            .collect();
        assert_eq!(
            results
                .iter()
                .map(|result| result.position)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_ne!(results[0].session_id, results[1].session_id);

        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<dyn Backend>();
        assert_send_sync::<dyn Decision>();
    }

    /// Every public item, by name. The crate's modules are private, so its public surface is
    /// exactly the `pub use` lines of lib.rs.
    const PUBLIC_ITEMS: [&str; 28] = [
        // TypeSafe request types (POST /v1/systemone).
        "Asset",
        "AssetKind",
        "ImageFormat",
        "NoulCriteria",
        "Question",
        "State",
        "SystemOneRequest",
        "OrderedMap",
        // TypeSafe response and answer types.
        "Answer",
        "BoxCoordinates",
        "SystemOneResponse",
        "Usage",
        // GET /v1/models.
        "ModelMetadata",
        "ModelMetadataList",
        // TypeSafe error bodies (400, 422).
        "HttpBadRequestError",
        "HttpValidationError",
        "LocItem",
        "ValidationError",
        // The traits and their request/result types.
        "Backend",
        "Decision",
        "DecisionRequest",
        "DecisionResult",
        "ImagePart",
        "MAX_IMAGES",
        "MAX_IMAGE_BYTES",
        // The question digest and the error type.
        "QuestionDigest",
        "DecisionError",
        "LimitKind",
    ];

    const SOURCES: [(&str, &str); 11] = [
        ("lib.rs", include_str!("../src/lib.rs")),
        ("api_error.rs", include_str!("../src/api_error.rs")),
        ("backend.rs", include_str!("../src/backend.rs")),
        ("decision.rs", include_str!("../src/decision.rs")),
        ("digest.rs", include_str!("../src/digest.rs")),
        ("error.rs", include_str!("../src/error.rs")),
        ("key_order.rs", include_str!("../src/key_order.rs")),
        ("map.rs", include_str!("../src/map.rs")),
        ("models.rs", include_str!("../src/models.rs")),
        ("request.rs", include_str!("../src/request.rs")),
        ("response.rs", include_str!("../src/response.rs")),
    ];

    /// The lines of `source` that are code: trimmed, comments and doc comments dropped.
    fn code_lines(source: &str) -> impl Iterator<Item = &str> {
        source
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with("//"))
    }

    /// The names re-exported by lib.rs's `pub use` statements.
    fn reexported_names(lib: &str) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        let mut rest = lib;
        while let Some(start) = rest.find("pub use ") {
            let statement = &rest[start..];
            let end = statement.find(';').expect("statement end");
            let path = &statement["pub use ".len()..end];
            let list = match (path.find('{'), path.rfind('}')) {
                (Some(open), Some(close)) => &path[open + 1..close],
                _ => path.rsplit("::").next().expect("name"),
            };
            for name in list
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
            {
                assert!(!name.contains('*') && !name.contains(" as "), "{name}");
                names.insert(name.to_owned());
            }
            rest = &statement[end..];
        }
        names
    }

    #[test]
    fn public_items() {
        let lib = SOURCES[0].1;
        let expected: BTreeSet<String> =
            PUBLIC_ITEMS.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(reexported_names(lib), expected);
        assert_eq!(expected.len(), 28);

        // Every module is private and lib.rs defines no item of its own.
        for line in lib.lines().map(str::trim) {
            if line.starts_with("mod ") || line.starts_with("pub ") {
                assert!(
                    line.starts_with("mod ") || line.starts_with("pub use "),
                    "lib.rs: {line}"
                );
            }
        }
        for (file, source) in SOURCES {
            assert!(!source.contains("#[macro_export]"), "{file}: macro_export");
            assert!(
                !code_lines(source).any(|line| line.starts_with("pub mod ")),
                "{file}: pub mod"
            );
        }

        // Each name is a real item of the crate (compile-time check).
        let _ = (
            std::any::type_name::<sc_decision_schema::Asset>(),
            std::any::type_name::<sc_decision_schema::AssetKind>(),
            std::any::type_name::<sc_decision_schema::ImageFormat>(),
            std::any::type_name::<sc_decision_schema::NoulCriteria>(),
            std::any::type_name::<sc_decision_schema::Question>(),
            std::any::type_name::<sc_decision_schema::State>(),
            std::any::type_name::<sc_decision_schema::OrderedMap<String, u8>>(),
            std::any::type_name::<sc_decision_schema::Answer>(),
            std::any::type_name::<sc_decision_schema::BoxCoordinates>(),
            std::any::type_name::<sc_decision_schema::Usage>(),
            std::any::type_name::<sc_decision_schema::ModelMetadata>(),
            std::any::type_name::<sc_decision_schema::ModelMetadataList>(),
            std::any::type_name::<sc_decision_schema::HttpBadRequestError>(),
            std::any::type_name::<sc_decision_schema::HttpValidationError>(),
            std::any::type_name::<sc_decision_schema::LocItem>(),
            std::any::type_name::<sc_decision_schema::ValidationError>(),
            std::any::type_name::<sc_decision_schema::ImagePart>(),
            std::any::type_name::<sc_decision_schema::QuestionDigest>(),
            sc_decision_schema::MAX_IMAGES,
            sc_decision_schema::MAX_IMAGE_BYTES,
        );
    }

    /// REQ-SAG-0006 AC1: nothing is named for a consumer application or a mock backend; ANC-001:
    /// no transport, retry, cache, logging or CLI code.
    #[test]
    fn no_consumer_or_infrastructure_code() {
        for (file, source) in SOURCES {
            let lower = source.to_lowercase();
            for word in ["haiku", "luna", "mock", "fake", "stub"] {
                assert!(!lower.contains(word), "{file}: names `{word}`");
            }
            for code in [
                "ureq",
                "tokio",
                "std::net",
                "std::fs",
                "std::thread",
                "std::time",
                "std::env",
                "println!",
                "eprintln!",
                "sc_agent_log",
                "clap",
            ] {
                assert!(
                    !code_lines(source).any(|line| line.contains(code)),
                    "{file}: uses `{code}`"
                );
            }
        }
    }
}
