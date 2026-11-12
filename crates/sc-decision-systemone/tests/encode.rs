//! SystemOneBackend encode: JSON, multipart, headers, key, models request, base URLs.
//!
//! AC n = the nth acceptance line of the a-6 sprint doc.

#[cfg(test)]
mod support;

#[cfg(test)]
mod tests {
    use sc_agent_http::{HttpRequest, Method};
    use sc_agent_types::ErrorId;
    use sc_decision_schema::{
        Backend, DecisionError, DecisionRequest, ImageFormat, SystemOneRequest,
    };
    use sc_decision_systemone::{SystemOneBackend, TYPESAFE_BASE_URL};

    use crate::support::{SECRET, header, image_part, key, request};

    const NOUL: &str = r#"{"model":"jev-latest","state":"I was charged twice.","questions":{"billing":{"type":"noul","instructions":"Is this about billing?"}}}"#;
    const BOX: &str = r#"{"model":"jev-preview","state":"photo","questions":{"red":{"type":"box","instructions":"Locate the red rectangle."}}}"#;
    const INLINE_ASSET: &str = r#"{"model":"jev-preview","state":"photo","questions":{"q":{"type":"noul"}},"assets":{"photo":{"type":"image","format":"png","data_base64":"iVBORw0KGgo="}}}"#;
    const TWO_PARTS: &str = r#"{"model":"jev-latest","state":{"door":"a","yard":"b"},"questions":{"person":{"type":"noul","instructions":"Is there a person?"}},"assets":{"a":{"type":"image","part":"door-photo"},"b":{"type":"image","part":"yard-photo"}}}"#;

    fn typesafe(key: bool) -> SystemOneBackend {
        SystemOneBackend::new(TYPESAFE_BASE_URL, key.then(crate::support::key)).expect("backend")
    }

    fn multipart_request() -> DecisionRequest {
        let body = SystemOneRequest::from_json(TWO_PARTS).expect("json");
        let parts = vec![
            image_part("door-photo", ImageFormat::Png, vec![0x89, b'P', b'N', b'G']),
            image_part("yard-photo", ImageFormat::Jpeg, vec![0xFF, 0xD8, 0xFF]),
        ];
        DecisionRequest::new(body, parts, false).expect("valid request")
    }

    /// The boundary of a multipart request's content-type.
    fn boundary(http: &HttpRequest) -> String {
        header(http.headers(), "content-type")
            .and_then(|value| value.strip_prefix("multipart/form-data; boundary="))
            .expect("multipart content-type")
            .to_owned()
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    /// AC 2, AC 10: JSON body unchanged, POST {base}/v1/systemone, no key without one.
    #[test]
    fn test_json_encode_without_key() {
        let request = request(NOUL, false);
        let http = typesafe(false).encode(&request).expect("encode");
        assert_eq!(http.method(), Method::Post);
        assert_eq!(
            http.endpoint().as_str(),
            "https://api.typesafe.ai/v1/systemone"
        );
        assert_eq!(http.body(), NOUL.as_bytes());
        assert_eq!(
            header(http.headers(), "content-type"),
            Some("application/json")
        );
        assert!(http.secret().is_none());
        assert!(http.deadline().is_none());
        assert_eq!(header(http.headers(), "TypeSafe-Danger-Zone-Images"), None);
        assert_eq!(
            header(http.headers(), "TypeSafe-Danger-Zone-Long-Context"),
            None
        );
    }

    /// AC 2, AC 10: Bearer only with a key; Debug of request and backend hides it.
    #[test]
    fn test_json_encode_with_key_is_bearer_and_redacted() {
        let backend = typesafe(true);
        let http = backend.encode(&request(NOUL, false)).expect("encode");
        let secret = http.secret().expect("secret header");
        assert_eq!(secret.name(), "Authorization");
        assert_eq!(secret.redacted(), "Authorization: Bearer [redacted]");
        assert!(
            !http
                .headers()
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        );
        assert!(!format!("{http:?}").contains(SECRET));
        assert!(!format!("{backend:?}").contains(SECRET));
        let models = backend.models_request().expect("models request");
        assert!(models.secret().is_some());
        assert!(!format!("{models:?}").contains(SECRET));
    }

    /// AC 2: images header for inline assets and for a box question.
    #[test]
    fn test_images_header_for_assets_and_box() {
        for json in [INLINE_ASSET, BOX] {
            let http = typesafe(false)
                .encode(&request(json, false))
                .expect("encode");
            assert_eq!(
                header(http.headers(), "TypeSafe-Danger-Zone-Images"),
                Some("true")
            );
            assert_eq!(
                header(http.headers(), "content-type"),
                Some("application/json")
            );
        }
    }

    /// AC 2: long-context header only when flagged.
    #[test]
    fn test_long_context_header_only_when_flagged() {
        let http = typesafe(false)
            .encode(&request(NOUL, true))
            .expect("encode");
        assert_eq!(
            header(http.headers(), "TypeSafe-Danger-Zone-Long-Context"),
            Some("true")
        );
        assert_eq!(header(http.headers(), "TypeSafe-Danger-Zone-Images"), None);
    }

    /// AC 2, AC 3: multipart part order, names, content types and bytes.
    #[test]
    fn test_multipart_encode_parts_in_order() {
        let request = multipart_request();
        let http = typesafe(true).encode(&request).expect("encode");
        let boundary = boundary(&http);
        assert_eq!(
            header(http.headers(), "TypeSafe-Danger-Zone-Images"),
            Some("true")
        );
        assert!(http.secret().is_some());

        let mut expected = Vec::new();
        expected.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"request\"\r\nContent-Type: application/json\r\n\r\n{TWO_PARTS}\r\n"
            )
            .as_bytes(),
        );
        expected.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"door-photo\"; filename=\"door-photo\"\r\nContent-Type: image/png\r\n\r\n"
            )
            .as_bytes(),
        );
        expected.extend_from_slice(&[0x89, b'P', b'N', b'G']);
        expected.extend_from_slice(
            format!(
                "\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"yard-photo\"; filename=\"yard-photo\"\r\nContent-Type: image/jpeg\r\n\r\n"
            )
            .as_bytes(),
        );
        expected.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
        expected.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        assert_eq!(http.body(), expected.as_slice());
        let door = contains(http.body(), b"name=\"door-photo\"").expect("door part");
        let yard = contains(http.body(), b"name=\"yard-photo\"").expect("yard part");
        assert!(contains(http.body(), b"name=\"request\"").expect("request part") < door);
        assert!(door < yard);
    }

    /// AC 3: a request whose asset names a part (built through DecisionRequest::new) encodes
    /// multipart with that part.
    #[test]
    fn test_asset_part_request_encodes_that_part() {
        let body = SystemOneRequest::from_json(
            r#"{"model":"jev-latest","state":{"door":"d"},"questions":{"p":{"type":"noul"}},"assets":{"d":{"type":"image","part":"door-photo"}}}"#,
        )
        .expect("json");
        let request = DecisionRequest::new(
            body,
            vec![image_part("door-photo", ImageFormat::Png, vec![1, 2])],
            false,
        )
        .expect("valid request");
        let http = typesafe(false).encode(&request).expect("encode");
        assert!(boundary(&http).starts_with("sc-agent-"));
        assert!(contains(http.body(), b"name=\"door-photo\"").is_some());
    }

    /// AC 19: the boundary is "sc-agent-" + a fresh ULID per encode.
    #[test]
    fn test_boundary_is_fresh_ulid() {
        let backend = typesafe(false);
        let request = multipart_request();
        let first = boundary(&backend.encode(&request).expect("encode"));
        let second = boundary(&backend.encode(&request).expect("encode"));
        assert_ne!(first, second);
        for boundary in [first, second] {
            let ulid = boundary.strip_prefix("sc-agent-").expect("prefix");
            assert_eq!(ulid.len(), 26);
            ulid::Ulid::from_string(ulid).expect("boundary is a ULID");
        }
    }

    /// AC 4: models_request is GET {base}/v1/models with an empty body.
    #[test]
    fn test_models_request() {
        let http = typesafe(false).models_request().expect("models request");
        assert_eq!(http.method(), Method::Get);
        assert_eq!(
            http.endpoint().as_str(),
            "https://api.typesafe.ai/v1/models"
        );
        assert!(http.body().is_empty());
        assert!(http.secret().is_none());
    }

    /// AC 8: an invalid base is decision.http carrying http.invalid_endpoint.
    #[test]
    fn test_invalid_base_is_invalid_endpoint() {
        let error = SystemOneBackend::new("not a url", None).expect_err("refused");
        assert_eq!(error.id(), "decision.http");
        assert_eq!(
            error.detail().cause.map(|cause| cause.id),
            Some("http.invalid_endpoint".to_owned())
        );
        assert!(matches!(error, DecisionError::Http(_)));
    }

    /// AC 8: new() is https only; local() accepts a configured runner's plain-http base.
    #[test]
    fn test_new_refuses_http_local_accepts_it() {
        let error = SystemOneBackend::new("http://api.typesafe.ai", None).expect_err("refused");
        assert_eq!(
            error.detail().cause.map(|cause| cause.id),
            Some("http.invalid_endpoint".to_owned())
        );
        let local = SystemOneBackend::local("http://127.0.0.1:8000", Some(key())).expect("local");
        let http = local.encode(&request(NOUL, false)).expect("encode");
        assert_eq!(
            http.endpoint().as_str(),
            "http://127.0.0.1:8000/v1/systemone"
        );
        assert!(http.secret().is_some(), "loopback may carry a key");
        let trailing = SystemOneBackend::local("http://127.0.0.1:8000/", None).expect("local");
        assert_eq!(
            trailing
                .models_request()
                .expect("models")
                .endpoint()
                .as_str(),
            "http://127.0.0.1:8000/v1/models"
        );
    }

    /// RBP-F004: the base is validated once (BaseUrl) and every endpoint is joined onto it.
    #[test]
    fn test_base_url_validated_once() {
        let base = sc_decision_systemone::BaseUrl::local("http://decider.lan:9000/").expect("base");
        assert_eq!(base.as_str(), "http://decider.lan:9000");
        let backend = SystemOneBackend::at(&base, None).expect("backend");
        assert_eq!(
            backend
                .encode(&request(NOUL, false))
                .expect("encode")
                .endpoint()
                .as_str(),
            "http://decider.lan:9000/v1/systemone"
        );
        assert_eq!(
            backend
                .models_request()
                .expect("models")
                .endpoint()
                .as_str(),
            "http://decider.lan:9000/v1/models"
        );
        for refused in [
            "https://api.typesafe.ai/?key=x",
            "https://api.typesafe.ai#frag",
        ] {
            let error = SystemOneBackend::new(refused, None).expect_err("refused");
            let cause = error.detail().cause.expect("cause");
            assert_eq!(cause.id, "http.invalid_endpoint");
            assert!(!cause.message.contains("key=x"), "{}", cause.message);
        }
    }

    /// A key to a plain-http host that is not loopback is refused before any send.
    #[test]
    fn test_key_on_plain_http_refused() {
        let backend = SystemOneBackend::local("http://10.0.0.5:8000", Some(key())).expect("local");
        let error = backend.encode(&request(NOUL, false)).expect_err("refused");
        assert_eq!(error.id(), "decision.http");
        assert_eq!(
            error.detail().cause.map(|cause| cause.id),
            Some("http.key_on_plain_http".to_owned())
        );
        let error = backend.models_request().expect_err("refused");
        assert!(matches!(error, DecisionError::Http(_)));
    }

    /// AC 5: the same SystemOneRequest encodes to the same body and headers for TypeSafe Jev
    /// and every local daemon base (Strands Decider, Laya, Mapika/decider).
    #[test]
    fn test_same_body_for_every_base() {
        let request = request(BOX, true);
        let jev = typesafe(false).encode(&request).expect("encode");
        for base in [
            "http://127.0.0.1:8000",
            "http://strands.internal:8001",
            "https://laya.internal",
            "http://[::1]:8002",
        ] {
            let local = SystemOneBackend::local(base, None)
                .expect("local")
                .encode(&request)
                .expect("encode");
            assert_eq!(local.body(), jev.body(), "{base}");
            assert_eq!(local.headers(), jev.headers(), "{base}");
            assert_eq!(local.endpoint().as_str(), format!("{base}/v1/systemone"));
        }
    }
}
