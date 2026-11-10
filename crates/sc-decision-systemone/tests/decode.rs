//! SystemOneBackend decode: success, extras, refusals, 503, models, numbers, error chains.
//!
//! AC n = the nth acceptance line of the a-6 sprint doc.

// Each test binary uses part of the shared helpers.
#[cfg(test)]
#[allow(dead_code)]
mod support;

#[cfg(test)]
mod tests {
    use sc_agent_http::testing::ScriptedTransport;
    use sc_agent_http::{HttpError, HttpResponse, LogContext, NetworkKind, Transport};
    use sc_agent_types::ErrorId;
    use sc_decision_schema::{
        Answer, Backend, DecisionError, DecisionRequest, ModelMetadataList, SystemOneResponse,
    };
    use sc_decision_systemone::{SystemOneBackend, TYPESAFE_BASE_URL};

    use crate::support::request;

    const NOUL: &str = r#"{"model":"jev-latest","state":"I was charged twice.","questions":{"billing":{"type":"noul"}}}"#;

    fn backend() -> SystemOneBackend {
        SystemOneBackend::new(TYPESAFE_BASE_URL, None).expect("backend")
    }

    fn noul() -> DecisionRequest {
        request(NOUL, false)
    }

    fn decode(status: u16, body: &[u8]) -> Result<SystemOneResponse, DecisionError> {
        backend().decode(
            HttpResponse::new(status, vec![], body.to_vec()),
            &noul(),
            &LogContext::NONE,
        )
    }

    /// AC 2: the TypeSafe 200 example decodes and round-trips.
    #[test]
    fn test_decode_typesafe_200() {
        let body = br#"{"model":"jev-latest","answers":{"billing":{"type":"noul","noul":0.98}},"usage":{"input_tokens":120,"output_tokens":12}}"#;
        let response = decode(200, body).expect("decoded");
        assert_eq!(response.model, "jev-latest");
        assert_eq!(
            serde_json::to_vec(&response).expect("encode"),
            body.to_vec()
        );
    }

    /// AC 2: Strands Decider `latency_ms` and Laya `routing`, `x_jev_confidence` and usage
    /// extras are kept verbatim.
    #[test]
    fn test_decode_keeps_daemon_extras() {
        let strands = br#"{"model":"jev-latest","answers":{"billing":{"type":"noul","noul":0.5}},"usage":{"input_tokens":1,"output_tokens":1},"latency_ms":42.5}"#;
        let response = decode(200, strands).expect("strands");
        assert_eq!(
            response.extra.get("latency_ms"),
            Some(&serde_json::json!(42.5))
        );
        assert_eq!(
            serde_json::to_vec(&response).expect("encode"),
            strands.to_vec()
        );

        let laya = br#"{"model":"convaiinnovations/laya","answers":{"billing":{"type":"noul","noul":0.25}},"usage":{"input_tokens":3,"output_tokens":1,"truncated":false},"routing":{"checkpoint":"laya-base"},"x_jev_confidence":0.9}"#;
        let response = decode(200, laya).expect("laya");
        assert_eq!(
            response.usage.extra.get("truncated"),
            Some(&serde_json::json!(false))
        );
        assert!(response.extra.contains_key("routing"));
        assert!(response.extra.contains_key("x_jev_confidence"));
        assert_eq!(
            serde_json::to_vec(&response).expect("encode"),
            laya.to_vec()
        );
    }

    /// AC 18: a probability of 0.123456789 decodes unchanged.
    #[test]
    fn test_decode_probability_unrounded() {
        let body = br#"{"model":"jev-latest","answers":{"billing":{"type":"noul","noul":0.123456789}},"usage":{"input_tokens":1,"output_tokens":1}}"#;
        let response = decode(200, body).expect("decoded");
        let (_, answer) = response.answers.iter().next().expect("one answer");
        assert!(matches!(answer, Answer::Noul { noul } if *noul == 0.123456789));
        assert!(
            String::from_utf8(serde_json::to_vec(&response).expect("encode"))
                .expect("utf8")
                .contains("0.123456789")
        );
    }

    /// AC 2: a 200 that is not JSON is decision.response_parse.
    #[test]
    fn test_decode_200_not_json() {
        let error = decode(200, b"<html>oops</html>").expect_err("refused");
        assert_eq!(error.id(), "decision.response_parse");
        assert!(matches!(
            error,
            DecisionError::ResponseParse { status: 200, .. }
        ));
    }

    /// AC 2: 400 with a string or object detail is ApiBadRequest; another shape is ApiStatus.
    #[test]
    fn test_decode_400() {
        for body in [
            br#"{"detail":"Every asset must be referenced."}"#.as_slice(),
            br#"{"detail":{"message":"Every asset must be referenced.","asset":"photo"}}"#,
        ] {
            let error = decode(400, body).expect_err("refused");
            assert_eq!(error.id(), "decision.api_bad_request");
        }
        let error = decode(400, b"bad request").expect_err("refused");
        assert!(
            matches!(error, DecisionError::ApiStatus { status: 400, ref body } if body == b"bad request")
        );
    }

    /// AC 2: 422 with the documented array is ApiValidation; Strands/Laya string detail is
    /// ApiStatus with the body.
    #[test]
    fn test_decode_422() {
        let body =
            br#"{"detail":[{"loc":["body","state"],"msg":"Field required","type":"missing"}]}"#;
        let error = decode(422, body).expect_err("refused");
        let DecisionError::ApiValidation { body } = &error else {
            panic!("expected ApiValidation, got {error:?}");
        };
        assert_eq!(body.detail.len(), 1);

        let string = br#"{"detail":"model 'x' is not published"}"#;
        let error = decode(422, string).expect_err("refused");
        assert!(
            matches!(error, DecisionError::ApiStatus { status: 422, ref body } if body == string)
        );
    }

    /// AC 2: 401 and 402 are ApiStatus with the body whole.
    #[test]
    fn test_decode_401_402() {
        for status in [401, 402] {
            let body = br#"{"detail":"no"}"#;
            let error = decode(status, body).expect_err("refused");
            assert_eq!(error.id(), "decision.api_status");
            assert!(
                matches!(error, DecisionError::ApiStatus { status: s, ref body } if s == status && body == br#"{"detail":"no"}"#)
            );
        }
    }

    /// AC 11: a 503 from Laya or Mapika/decider is a final reply, decision.api_status with its
    /// body, not a busy signal.
    #[test]
    fn test_decode_503_is_final() {
        let laya = br#"{"detail":"server busy, try again later"}"#;
        let error = decode(503, laya).expect_err("final reply");
        assert_eq!(error.id(), "decision.api_status");
        assert!(
            matches!(error, DecisionError::ApiStatus { status: 503, ref body } if body == laya)
        );
        assert!(error.recovery().is_none());
    }

    /// AC 4: decode_models decodes the openapi example; non-2xx and non-JSON are errors.
    #[test]
    fn test_decode_models() {
        let body = br#"{"models":[{"name":"jev-latest","description":"General-purpose system one model.","release_date":"2026-09-15"}]}"#;
        let list = backend()
            .decode_models(HttpResponse::new(200, vec![], body.to_vec()))
            .expect("models");
        assert_eq!(
            list,
            ModelMetadataList::from_json(200, body).expect("schema parse")
        );
        assert_eq!(
            list.models.first().map(|model| model.name.as_str()),
            Some("jev-latest")
        );

        let error = backend()
            .decode_models(HttpResponse::new(200, vec![], b"[]".to_vec()))
            .expect_err("not a list");
        assert_eq!(error.id(), "decision.response_parse");
        let error = backend()
            .decode_models(HttpResponse::new(401, vec![], b"unauthorized".to_vec()))
            .expect_err("refused");
        assert!(matches!(
            error,
            DecisionError::ApiStatus { status: 401, .. }
        ));
    }

    /// AC 14: a failed send becomes DecisionError::Http with the lower HttpError's id and
    /// message unchanged.
    #[test]
    fn test_send_failure_chain_keeps_http_error() {
        let lower = HttpError::network(NetworkKind::Connect, 1, "connection refused");
        let transport = ScriptedTransport::new(vec![Err(lower.clone())]);
        let http = backend().encode(&noul()).expect("encode");
        let error: DecisionError = transport.send(&http).expect_err("send fails").into();
        assert_eq!(error.id(), "decision.http");
        let cause = error.detail().cause.expect("cause");
        assert_eq!(cause.id, "http.network");
        assert_eq!(cause.message, lower.message());
        assert!(matches!(error, DecisionError::Http(ref inner) if *inner == lower));
        let source = std::error::Error::source(&error).expect("source");
        assert_eq!(source.to_string(), lower.to_string());
    }
}
