//! LocalBackend discovery, construction, Backend use and errors, over ScriptedTransport.
//!
//! AC n = the nth acceptance line of the a-6 sprint doc.

#[cfg(test)]
#[allow(
    dead_code,
    reason = "shared with log.rs and load_wait.rs; this binary uses every helper today, but a helper added for another binary alone must not fail this build"
)]
mod support;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use sc_agent_config::RunnerKind;
    use sc_agent_http::testing::ScriptedTransport;
    use sc_agent_http::{HttpError, LogContext, Method, NetworkKind};
    use sc_agent_types::ErrorId;
    use sc_decision_local::{
        Availability, DaemonKind, LocalBackend, LocalError, LocalTimeouts, ModelState, ParseCause,
    };
    use sc_decision_schema::{Backend, DecisionError};
    use sc_decision_systemone::SystemOneBackend;

    use crate::support::{
        MODEL, SECRET, decided, decision_request, host, json, key, loopback, model, port,
    };

    fn discover(
        kind: DaemonKind,
        key: Option<sc_agent_secrets::ApiKey>,
        reply: Result<sc_agent_http::HttpResponse, HttpError>,
    ) -> (Result<Availability, LocalError>, ScriptedTransport) {
        let transport = ScriptedTransport::new(vec![reply]);
        let result = loopback(kind, key).discover(&transport, LogContext::NONE);
        (result, transport)
    }

    fn up(kind: DaemonKind, status: u16, body: &str) -> Result<Availability, LocalError> {
        discover(kind, None, Ok(json(status, body))).0
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    /// AC 7: Strands Decider: GET /health, served name or checkpoint is Loaded, else
    /// local.model_not_served.
    #[test]
    fn test_strands_rows() {
        let (result, transport) = discover(
            DaemonKind::Strands,
            Some(key()),
            Ok(json(
                200,
                r#"{"status":"ok","model":"jev-latest","checkpoint":"/ckpt/jev"}"#,
            )),
        );
        assert_eq!(
            result.expect("up"),
            Availability {
                daemon: DaemonKind::Strands,
                loaded: strings(&["jev-latest", "/ckpt/jev"]),
                model: ModelState::Loaded,
            }
        );
        let probe = transport.requests().pop().expect("one probe");
        assert_eq!(probe.method(), Method::Get);
        assert_eq!(probe.endpoint().as_str(), "http://127.0.0.1:8000/health");
        assert!(probe.secret().is_none(), "Strands Decider has no auth");

        let by_checkpoint = up(
            DaemonKind::Strands,
            200,
            r#"{"status":"ok","model":"served","checkpoint":"jev-latest"}"#,
        );
        assert_eq!(by_checkpoint.expect("up").model, ModelState::Loaded);

        let error = up(
            DaemonKind::Strands,
            200,
            r#"{"status":"ok","model":"other","checkpoint":"/ckpt/other"}"#,
        )
        .expect_err("not served");
        assert_eq!(error.id(), "local.model_not_served");
        assert!(
            matches!(error, LocalError::ModelNotServed { ref served, .. } if *served == strings(&["other", "/ckpt/other"]))
        );
    }

    /// AC 7: Laya: loaded -> Loaded, absent from loaded -> LoadsOnDemand, no list -> Unlisted;
    /// Bearer only with a key.
    #[test]
    fn test_laya_rows() {
        let (result, transport) = discover(
            DaemonKind::Laya,
            Some(key()),
            Ok(json(200, r#"{"status":"ok","loaded":["jev-latest"]}"#)),
        );
        assert_eq!(result.expect("up").model, ModelState::Loaded);
        let probe = transport.requests().pop().expect("probe");
        assert_eq!(probe.endpoint().as_str(), "http://127.0.0.1:8000/health");
        let secret = probe.secret().expect("bearer with a key");
        assert_eq!(secret.redacted(), "Authorization: Bearer [redacted]");
        assert!(!format!("{probe:?}").contains(SECRET));

        let (result, transport) = discover(
            DaemonKind::Laya,
            None,
            Ok(json(200, r#"{"status":"ok","loaded":[]}"#)),
        );
        assert_eq!(
            result.expect("up"),
            Availability {
                daemon: DaemonKind::Laya,
                loaded: Vec::new(),
                model: ModelState::LoadsOnDemand,
            }
        );
        assert!(
            transport
                .requests()
                .pop()
                .expect("probe")
                .secret()
                .is_none()
        );

        let unlisted = up(DaemonKind::Laya, 200, r#"{"status":"ok"}"#).expect("up");
        assert_eq!(unlisted.model, ModelState::Unlisted);
    }

    /// AC 7: Mapika/decider: ok false -> NotReady; ok and equal -> Loaded; else not served.
    #[test]
    fn test_mapika_rows() {
        let not_ready = up(
            DaemonKind::Mapika,
            200,
            r#"{"ok":false,"model":"jev-latest"}"#,
        );
        assert_eq!(not_ready.expect("up").model, ModelState::NotReady);
        let loaded = up(
            DaemonKind::Mapika,
            200,
            r#"{"ok":true,"model":"jev-latest","device":"cpu"}"#,
        )
        .expect("up");
        assert_eq!(loaded.model, ModelState::Loaded);
        assert_eq!(loaded.loaded, strings(&["jev-latest"]));
        let error =
            up(DaemonKind::Mapika, 200, r#"{"ok":true,"model":"other"}"#).expect_err("not served");
        assert_eq!(error.id(), "local.model_not_served");
    }

    /// AC 7: generic: listed -> Loaded; unlisted name -> not served; 404/405 -> Unlisted;
    /// 500 -> local.probe_status; Bearer only with a key.
    #[test]
    fn test_generic_rows() {
        let models =
            r#"{"models":[{"name":"jev-latest","description":"d","release_date":"2026-09-15"}]}"#;
        let (result, transport) = discover(DaemonKind::Generic, Some(key()), Ok(json(200, models)));
        assert_eq!(result.expect("up").model, ModelState::Loaded);
        let probe = transport.requests().pop().expect("probe");
        assert_eq!(probe.endpoint().as_str(), "http://127.0.0.1:8000/v1/models");
        assert!(probe.secret().is_some());

        let (_, transport) = discover(DaemonKind::Generic, None, Ok(json(200, models)));
        assert!(
            transport
                .requests()
                .pop()
                .expect("probe")
                .secret()
                .is_none()
        );

        let other =
            r#"{"models":[{"name":"other","description":"d","release_date":"2026-09-15"}]}"#;
        let error = up(DaemonKind::Generic, 200, other).expect_err("not served");
        assert!(matches!(
            error,
            LocalError::ModelNotServed {
                daemon: DaemonKind::Generic,
                ..
            }
        ));

        for status in [404, 405] {
            let unlisted =
                up(DaemonKind::Generic, status, r#"{"detail":"Not Found"}"#).expect("up");
            assert_eq!(unlisted.model, ModelState::Unlisted);
        }
        let error = up(DaemonKind::Generic, 500, "boom").expect_err("status");
        assert_eq!(error.id(), "local.probe_status");
        assert!(
            matches!(error, LocalError::ProbeStatus { status: 500, path: "/v1/models", ref body, .. } if body == b"boom")
        );

        let error = up(DaemonKind::Generic, 200, "not json").expect_err("parse");
        assert_eq!(error.id(), "local.decision");
        assert_eq!(
            error.detail().cause.map(|cause| cause.id),
            Some("decision.response_parse".to_owned())
        );
    }

    /// AC 7: a health reply that is not JSON is local.probe_parse, keeping the serde_json
    /// error as its source; a non-200 is local.probe_status.
    #[test]
    fn test_health_errors() {
        for kind in [DaemonKind::Strands, DaemonKind::Laya, DaemonKind::Mapika] {
            let error = up(kind, 200, "<html>").expect_err("parse");
            assert_eq!(error.id(), "local.probe_parse");
            assert!(
                matches!(error, LocalError::ProbeParse { daemon, path: "/health", .. } if daemon == kind)
            );
            let lower = serde_json::from_str::<serde_json::Value>("<html>").expect_err("not json");
            let source = std::error::Error::source(&error).expect("source");
            assert_eq!(source.to_string(), lower.to_string());
            let cause = error.detail().cause.expect("cause");
            assert_eq!(cause.id, "serde_json.syntax");
            assert_eq!(cause.message, lower.to_string());
            assert!(error.recovery().is_some());
            let error = up(kind, 500, "boom").expect_err("status");
            assert_eq!(error.id(), "local.probe_status");
        }
    }

    /// REQ-SAG-0002, REQ-SAG-0401: a ProbeStatus body is kept whole, with the attached key
    /// redacted wherever the daemon echoes it.
    #[test]
    fn test_probe_status_body_whole_and_key_redacted() {
        let echo = format!(r#"{{"detail":"bad token Bearer {SECRET}","again":"{SECRET}"}}"#);
        let (result, transport) = discover(DaemonKind::Laya, Some(key()), Ok(json(401, &echo)));
        assert!(
            transport
                .requests()
                .pop()
                .expect("probe")
                .secret()
                .is_some()
        );
        let error = result.expect_err("status");
        let LocalError::ProbeStatus {
            status, ref body, ..
        } = error
        else {
            panic!("expected ProbeStatus, got {error:?}");
        };
        assert_eq!(status, 401);
        assert_eq!(
            String::from_utf8(body.clone()).expect("utf8"),
            r#"{"detail":"bad token Bearer [redacted]","again":"[redacted]"}"#
        );
        for text in [
            error.to_string(),
            format!("{error:?}"),
            format!("{:?}", error.detail()),
        ] {
            assert!(!text.contains(SECRET), "{text}");
        }

        // A long body is kept whole, not cut.
        let long = "x".repeat(64 * 1024);
        let (result, _) = discover(DaemonKind::Generic, Some(key()), Ok(json(500, &long)));
        assert!(
            matches!(result, Err(LocalError::ProbeStatus { ref body, .. }) if body.len() == long.len())
        );

        // A body that is not UTF-8 and holds the key: only the key is rewritten; every
        // invalid byte stays as it was (REQ-SAG-0002).
        let mut raw = vec![0xFF, b'a', 0xC3];
        raw.extend_from_slice(SECRET.as_bytes());
        raw.extend_from_slice(&[0xE2, 0x82, b'z', 0x80]);
        raw.extend_from_slice(SECRET.as_bytes());
        raw.push(0xFE);
        let (result, _) = discover(
            DaemonKind::Laya,
            Some(key()),
            Ok(sc_agent_http::HttpResponse::new(401, vec![], raw)),
        );
        let mut expected = vec![0xFF, b'a', 0xC3];
        expected.extend_from_slice(b"[redacted]");
        expected.extend_from_slice(&[0xE2, 0x82, b'z', 0x80]);
        expected.extend_from_slice(b"[redacted]");
        expected.push(0xFE);
        assert!(
            matches!(result, Err(LocalError::ProbeStatus { ref body, .. }) if *body == expected),
            "{result:?}"
        );

        // Without an attached key the body is unchanged byte for byte (not even made UTF-8).
        let bytes = vec![0xFF, 0xFE, b'x'];
        let (result, _) = discover(
            DaemonKind::Strands,
            Some(key()),
            Ok(sc_agent_http::HttpResponse::new(500, vec![], bytes.clone())),
        );
        assert!(matches!(result, Err(LocalError::ProbeStatus { ref body, .. }) if *body == bytes));
    }

    /// REQ-SAG-0401: a health field that echoes the attached key reaches no error text; the
    /// ProbeParse cause keeps its id and the redacted serde_json message (REQ-SAG-0002).
    #[test]
    fn test_probe_parse_cause_redacts_attached_key() {
        let echo = format!(r#"{{"status":"ok","loaded":"{SECRET}"}}"#);
        let (result, _) = discover(DaemonKind::Laya, Some(key()), Ok(json(200, &echo)));
        let error = result.expect_err("loaded is not a list");
        assert!(matches!(
            error,
            LocalError::ProbeParse {
                source: ParseCause::Redacted(_),
                ..
            }
        ));
        let cause = error.detail().cause.expect("cause");
        assert_eq!(cause.id, "serde_json.data");
        assert!(
            cause
                .message
                .contains("invalid type: string \"[redacted]\""),
            "{}",
            cause.message
        );
        let source = std::error::Error::source(&error).expect("source");
        let mut texts = vec![
            error.to_string(),
            format!("{error:?}"),
            format!("{:?}", error.detail()),
            source.to_string(),
        ];
        texts.push(format!("{source:?}"));
        for text in texts {
            assert!(!text.contains(SECRET), "{text}");
        }

        // A generic daemon's model list goes through the same rule.
        let models = format!(r#"{{"models":"{SECRET}"}}"#);
        let (result, _) = discover(DaemonKind::Generic, Some(key()), Ok(json(200, &models)));
        let error = result.expect_err("models is not a list");
        assert_eq!(error.id(), "local.decision");
        let text = format!("{error:?} {:?}", error.detail());
        assert!(!text.contains(SECRET), "{text}");
        assert!(text.contains("[redacted]"), "{text}");

        // Loaded and served names echoed back are redacted too (they reach log records).
        let loaded = format!(r#"{{"status":"ok","loaded":["{SECRET}"]}}"#);
        let (result, _) = discover(DaemonKind::Laya, Some(key()), Ok(json(200, &loaded)));
        assert_eq!(result.expect("up").loaded, vec!["[redacted]".to_owned()]);
        let served =
            format!(r#"{{"models":[{{"name":"{SECRET}","description":"d","release_date":"r"}}]}}"#);
        let (result, _) = discover(DaemonKind::Generic, Some(key()), Ok(json(200, &served)));
        assert!(matches!(
            result,
            Err(LocalError::ModelNotServed { ref served, .. }) if *served == vec!["[redacted]".to_owned()]
        ));
    }

    /// Without an attached key the ProbeParse cause is the serde_json error, unchanged.
    #[test]
    fn test_probe_parse_keeps_raw_source_without_key() {
        // Strands Decider has no auth: a configured key is not attached.
        let (result, transport) = discover(DaemonKind::Strands, Some(key()), Ok(json(200, "{")));
        assert!(
            transport
                .requests()
                .pop()
                .expect("probe")
                .secret()
                .is_none()
        );
        let error = result.expect_err("parse");
        let LocalError::ProbeParse {
            source: ParseCause::Json(ref lower),
            ..
        } = error
        else {
            panic!("expected the raw serde_json source, got {error:?}");
        };
        assert_eq!(
            error.detail().cause.map(|cause| cause.message),
            Some(lower.to_string())
        );
        assert!(
            std::error::Error::source(&error)
                .is_some_and(|source| source.downcast_ref::<serde_json::Error>().is_some())
        );
    }

    /// AC 7, AC 14: a daemon that is not listening is local.http with cause http.network and
    /// the lower message unchanged.
    #[test]
    fn test_down_is_local_http_network() {
        let lower = HttpError::network(NetworkKind::Connect, 1, "connection refused (os error 61)");
        for kind in [
            DaemonKind::Strands,
            DaemonKind::Laya,
            DaemonKind::Mapika,
            DaemonKind::Generic,
        ] {
            let (result, _) = discover(kind, None, Err(lower.clone()));
            let error = result.expect_err("down");
            assert_eq!(error.id(), "local.http");
            let cause = error.detail().cause.expect("cause");
            assert_eq!(cause.id, "http.network");
            assert_eq!(cause.message, lower.message());
            assert!(matches!(error, LocalError::Http(ref inner) if *inner == lower));
            let source = std::error::Error::source(&error).expect("source");
            assert_eq!(source.to_string(), lower.to_string());
        }
    }

    /// AC 14: the SystemOne DecisionError chain through LocalBackend keeps the lower
    /// HttpError's id and message (a refused base).
    #[test]
    fn test_construction_error_chain() {
        let error = SystemOneBackend::local("not a url", None).expect_err("refused");
        let wrapped = LocalError::from(error);
        assert_eq!(wrapped.id(), "local.decision");
        let decision = wrapped.detail().cause.expect("decision cause");
        assert_eq!(decision.id, "decision.http");
        let http = decision.cause.expect("http cause");
        assert_eq!(http.id, "http.invalid_endpoint");
        assert!(http.message.contains("not a url"));
    }

    /// AC 12: each RunnerKind maps to its DaemonKind and daemon implementation.
    #[test]
    fn test_runner_kind_mapping() {
        for (runner, kind, name) in [
            (RunnerKind::Strands, DaemonKind::Strands, "strands"),
            (RunnerKind::Laya, DaemonKind::Laya, "laya"),
            (RunnerKind::Mapika, DaemonKind::Mapika, "mapika"),
            (RunnerKind::Generic, DaemonKind::Generic, "generic"),
        ] {
            assert_eq!(DaemonKind::from(runner), kind);
            assert_eq!(kind.daemon().kind(), kind);
            assert_eq!(kind.as_str(), name);
        }
    }

    /// AC 12: LocalError has 6 distinct local.* ids; recovery for not served and timeout;
    /// detail() equals the value built from id, message, recovery and cause.
    #[test]
    fn test_local_error_ids() {
        let errors = [
            LocalError::Http(HttpError::network(NetworkKind::Connect, 1, "down")),
            LocalError::Decision(DecisionError::Cancelled { sends: 0 }),
            LocalError::ProbeStatus {
                daemon: DaemonKind::Laya,
                path: "/health",
                status: 500,
                body: Vec::new(),
            },
            LocalError::ProbeParse {
                daemon: DaemonKind::Strands,
                path: "/health",
                source: ParseCause::Json(
                    serde_json::from_str::<serde_json::Value>("{").expect_err("eof"),
                ),
            },
            LocalError::ModelNotServed {
                daemon: DaemonKind::Mapika,
                model: model(),
                served: Vec::new(),
            },
            LocalError::LoadTimeout {
                runner: "http://10.0.0.5:9000".to_owned(),
                waited: std::time::Duration::from_secs(2),
            },
        ];
        let ids: BTreeSet<&str> = errors.iter().map(ErrorId::id).collect();
        assert_eq!(ids.len(), 6);
        assert!(ids.iter().all(|id| id.starts_with("local.")));
        for error in &errors {
            assert_eq!(
                error.to_string(),
                format!("{}: {}", error.id(), error.message())
            );
            let detail = error.detail();
            assert_eq!(detail.id, error.id());
            assert_eq!(detail.message, error.message());
            assert_eq!(detail.recovery.as_deref(), error.recovery());
            assert_eq!(detail.cause, error.cause_detail().map(Box::new));
        }
        let [http, decision, status, parse, not_served, timeout] = &errors;
        for error in [status, parse, not_served, timeout] {
            assert!(
                error.recovery().is_some_and(|text| !text.is_empty()),
                "{error}"
            );
        }
        assert!(http.recovery().is_none());
        assert!(decision.recovery().is_none());
    }

    /// AC 8, AC 9: a Laya runner at 127.0.0.1:8000 encodes POST {base}/v1/systemone and
    /// decodes exactly like SystemOneBackend.
    #[test]
    fn test_local_backend_encodes_and_decodes_like_systemone() {
        let backend = LocalBackend::new(
            DaemonKind::Laya,
            &host("127.0.0.1"),
            port(8000),
            model(),
            None,
        )
        .expect("backend");
        let systemone = SystemOneBackend::local("http://127.0.0.1:8000", None).expect("systemone");
        let request = decision_request();
        let local = backend.encode(&request).expect("encode");
        assert_eq!(local.method(), Method::Post);
        assert_eq!(
            local.endpoint().as_str(),
            "http://127.0.0.1:8000/v1/systemone"
        );
        let direct = systemone.encode(&request).expect("encode");
        assert_eq!(local.body(), direct.body());
        assert_eq!(local.headers(), direct.headers());
        assert!(local.deadline().is_none(), "no timeouts set");

        let decoded = backend
            .decode(decided(), &request, &LogContext::NONE)
            .expect("decoded");
        assert_eq!(
            decoded,
            systemone
                .decode(decided(), &request, &LogContext::NONE)
                .expect("decoded")
        );
        let refused = backend
            .decode(
                json(503, r#"{"detail":"server busy, try again later"}"#),
                &request,
                &LogContext::NONE,
            )
            .expect_err("final");
        assert_eq!(refused.id(), "decision.api_status");
    }

    /// AC 8: hostnames and IPv6 hosts from config form the base; no host is built in.
    #[test]
    fn test_base_from_config_values() {
        let named = LocalBackend::new(
            DaemonKind::Strands,
            &host("decider.lan"),
            port(9001),
            model(),
            None,
        )
        .expect("backend");
        assert_eq!(named.base(), "http://decider.lan:9001");
        let v6 = LocalBackend::new(
            DaemonKind::Generic,
            &host("::1"),
            port(8080),
            model(),
            Some(key()),
        )
        .expect("backend");
        assert_eq!(v6.base(), "http://[::1]:8080");
        let http = v6
            .encode(&decision_request())
            .expect("loopback may carry a key");
        assert_eq!(http.endpoint().as_str(), "http://[::1]:8080/v1/systemone");
        assert!(http.secret().is_some());
    }

    /// AC 20: a key for plain-http host 10.0.0.5 fails in encode with http.key_on_plain_http
    /// and nothing is sent.
    #[test]
    fn test_key_on_plain_http_refused_before_send() {
        let backend = LocalBackend::new(
            DaemonKind::Laya,
            &host("10.0.0.5"),
            port(8000),
            model(),
            Some(key()),
        )
        .expect("backend");
        let transport = ScriptedTransport::new(Vec::new());
        let error = backend.encode(&decision_request()).expect_err("refused");
        assert!(matches!(
            error,
            DecisionError::Http(HttpError::KeyOnPlainHttp { .. })
        ));
        assert_eq!(
            error.detail().cause.map(|cause| cause.id),
            Some("http.key_on_plain_http".to_owned())
        );
        let error = backend
            .discover(&transport, LogContext::NONE)
            .expect_err("probe refused");
        assert_eq!(
            error.detail().cause.map(|cause| cause.id),
            Some("http.key_on_plain_http".to_owned())
        );
        assert_eq!(transport.sends(), 0);
    }

    /// AC 10: Debug of a backend with a key prints `<set>`, never the key.
    #[test]
    fn test_debug_hides_key() {
        let backend = loopback(DaemonKind::Laya, Some(key())).with_timeouts(LocalTimeouts {
            load: std::time::Duration::from_secs(600),
            request: std::time::Duration::from_secs(300),
        });
        let debug = format!("{backend:?}");
        assert!(!debug.contains(SECRET));
        assert!(debug.contains("key: \"<set>\""));
        assert!(debug.contains("log: \"<unset>\""));
        assert!(debug.contains(MODEL));
    }

    /// AC 9, AC 15: both backends are usable as Arc<dyn sc_decision_schema::Backend>.
    #[test]
    fn test_arc_dyn_backend() {
        let backends: Vec<Arc<dyn sc_decision_schema::Backend>> = vec![
            Arc::new(
                SystemOneBackend::new(sc_decision_systemone::TYPESAFE_BASE_URL, None)
                    .expect("backend"),
            ),
            Arc::new(loopback(DaemonKind::Mapika, None)),
        ];
        for backend in &backends {
            assert_eq!(
                backend
                    .encode(&decision_request())
                    .expect("encode")
                    .method(),
                Method::Post
            );
        }
    }
}
