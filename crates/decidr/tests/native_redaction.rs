//! Real composition-root coverage of non-HTTP credential and decision error paths.
//! Config/credential refusals precede an attached resolved key; decision failures
//! exercise the real runner with controlled .env/keyring sources. These tests prove
//! this repository revision only; agt-r17 owns the final published-wheel recheck.
mod common;

#[cfg(test)]
mod tests {
    use crate::common::*;
    use sc_agent_http::testing::ScriptedTransport;
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_secrets::{KeyringSource, SecretsError};
    use sc_agent_types::{CancelToken, ErrorId, Provider};
    use serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    const KEY: &str = "sk-native-redaction-controlled-77";
    struct Keyring {
        value: String,
        calls: Arc<AtomicUsize>,
    }
    impl KeyringSource for Keyring {
        fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Some(self.value.clone()))
        }
    }

    #[test]
    fn config_failure_precedes_any_credential_resolution() {
        let dir = empty_config_dir();
        let transport = ScriptedTransport::default();
        let log = MemoryLog::new().expect("log");
        let calls = Arc::new(AtomicUsize::new(0));
        let mut ctx = context(dir.path(), &[], Arc::new(transport.clone()), &log);
        ctx.keyring = Some(Arc::new(Keyring {
            value: KEY.into(),
            calls: calls.clone(),
        }));
        let opts = options("jev", Some(Provider::TypeSafe), ctx, &log);
        std::fs::write(dir.path().join("default.json"), r#"{"max_sends":0}"#).expect("config");
        let error = send(&opts, request(NOUL), &CancelToken::new()).expect_err("config refused");
        assert_eq!(error.id(), "decidr.config");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "no resolved key exists on this path"
        );
        assert_eq!(transport.sends(), 0);
        assert!(
            !serde_json::to_string(&error.detail())
                .expect("detail")
                .contains(KEY)
        );
    }

    #[test]
    fn malformed_dotenv_and_keyring_credentials_never_enter_error_detail() {
        for dotenv in [false, true] {
            let dir = empty_config_dir();
            let transport = ScriptedTransport::default();
            let log = MemoryLog::new().expect("log");
            let calls = Arc::new(AtomicUsize::new(0));
            let bad = format!("{KEY} trailing");
            let mut ctx = context(dir.path(), &[], Arc::new(transport.clone()), &log);
            ctx.keyring = Some(Arc::new(Keyring {
                value: bad.clone(),
                calls: calls.clone(),
            }));
            if dotenv {
                std::fs::write(
                    dir.path().join(".env"),
                    format!("{}='{bad}'\n", sc_agent_secrets::TYPESAFE_API_KEY),
                )
                .expect("dotenv");
            }
            let opts = options("jev", Some(Provider::TypeSafe), ctx, &log);
            let error =
                send(&opts, request(NOUL), &CancelToken::new()).expect_err("credential refused");
            let detail = error.detail();
            assert_eq!(detail.id, "decidr.secrets");
            let cause = detail.cause.as_ref().expect("native cause");
            assert_eq!(cause.id, "secrets.key_malformed");
            assert!(cause.message.contains("whitespace"));
            assert!(
                !serde_json::to_string(&detail)
                    .expect("detail")
                    .contains(KEY)
            );
            assert!(!format!("{:?}", log.records().expect("records")).contains(KEY));
            assert_eq!(calls.load(Ordering::SeqCst), usize::from(!dotenv));
            assert_eq!(transport.sends(), 0);
        }
    }

    struct RefusingKeyring(SecretsError);
    impl KeyringSource for RefusingKeyring {
        fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
            Err(self.0.clone()) // no credential value is returned on this path
        }
    }

    #[test]
    fn unavailable_credentials_keep_their_native_source_and_never_send() {
        for failure in [
            SecretsError::KeyMissing {
                var: sc_agent_secrets::TYPESAFE_API_KEY.into(),
            },
            SecretsError::KeyringTimeout,
            SecretsError::KeyringRead {
                kind: "controlled",
                cause: "read refused",
                source: Some(sc_agent_types::ErrorDetail::new(
                    "keyring.controlled",
                    "vault unavailable",
                    None,
                )),
            },
        ] {
            let dir = empty_config_dir();
            let transport = ScriptedTransport::default();
            let log = MemoryLog::new().expect("log");
            let mut ctx = context(dir.path(), &[], Arc::new(transport.clone()), &log);
            ctx.keyring = Some(Arc::new(RefusingKeyring(failure.clone())));
            let opts = options("jev", Some(Provider::TypeSafe), ctx, &log);
            let error =
                send(&opts, request(NOUL), &CancelToken::new()).expect_err("no resolved key");
            assert_eq!(error.id(), "decidr.secrets");
            assert_eq!(
                error.cause_detail().expect("credential error"),
                failure.detail()
            );
            assert_eq!(transport.sends(), 0);
        }
    }

    #[test]
    fn dotenv_io_refusal_precedes_keyring_resolution() {
        let dir = empty_config_dir();
        std::fs::create_dir(dir.path().join(".env")).expect("not a regular dotenv file");
        let transport = ScriptedTransport::default();
        let log = MemoryLog::new().expect("log");
        let calls = Arc::new(AtomicUsize::new(0));
        let mut ctx = context(dir.path(), &[], Arc::new(transport.clone()), &log);
        ctx.keyring = Some(Arc::new(Keyring {
            value: KEY.into(),
            calls: calls.clone(),
        }));
        let opts = options("jev", Some(Provider::TypeSafe), ctx, &log);
        let error = send(&opts, request(NOUL), &CancelToken::new()).expect_err("dotenv refused");
        assert_eq!(
            error.cause_detail().expect("cause").id,
            "secrets.dotenv_read"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(transport.sends(), 0);
    }

    #[test]
    fn echoed_keys_in_decision_failures_are_redacted_through_result_and_logs() {
        for key in [KEY.to_owned(), KEY.repeat(5)] {
            for dotenv in [false, true] {
                for llm in [false, true] {
                    let dir = config_dir(r#"{"max_sends":1}"#);
                    let (provider, model) = if llm {
                        (Provider::Anthropic, "haiku")
                    } else {
                        (Provider::TypeSafe, "jev")
                    };
                    let body = if llm {
                        json!({"model":key.as_str(), "stop_reason":"end_turn", "content":[{"type":"text", "text":json!({"answers":{key.as_str():{"noul":0.5}}}).to_string()}], "usage":{"input_tokens":1,"output_tokens":1}}).to_string()
                    } else {
                        json!({"model":"jev-latest", "answers":{"q":{"type":"noul","noul":key.as_str()}}, "usage":{"input_tokens":1,"output_tokens":1}}).to_string()
                    };
                    let transport = ScriptedTransport::responses(vec![json_response(200, &body)]);
                    let log = MemoryLog::new().expect("log");
                    let calls = Arc::new(AtomicUsize::new(0));
                    let mut ctx = context(dir.path(), &[], Arc::new(transport.clone()), &log);
                    ctx.keyring = Some(Arc::new(Keyring {
                        value: key.clone(),
                        calls: calls.clone(),
                    }));
                    if dotenv {
                        std::fs::write(
                            dir.path().join(".env"),
                            format!("{}={key}\n", sc_agent_secrets::env_var(provider)),
                        )
                        .expect("dotenv");
                    }
                    let opts = options(model, Some(provider), ctx, &log);
                    let result =
                        send(&opts, request(NOUL), &CancelToken::new()).expect("native result");
                    let text = decidr::output::result_text(&result).expect("result JSON");
                    assert!(!text.contains(KEY), "dotenv={dotenv}, llm={llm}: {text}");
                    assert!(text.contains("[redacted]"), "{text}");
                    let error = result.outcome.expect_err("decision failed");
                    assert_eq!(
                        error.id(),
                        if llm {
                            "decision.schema_mismatch"
                        } else {
                            "decision.response_parse"
                        }
                    );
                    if llm {
                        let cause = error.cause_detail().expect("cause");
                        assert_eq!(cause.id, "decision_llm.answer_ids");
                        assert_eq!(
                            cause.message,
                            "the answer ids differ from the question ids (missing: [q]; extra: [[redacted]])"
                        );
                    }
                    assert!(!format!("{:?}", log.records().expect("records")).contains(KEY));
                    assert_eq!(calls.load(Ordering::SeqCst), usize::from(!dotenv));
                    assert_eq!(transport.sends(), 1);
                }
            }
        }
    }
}
