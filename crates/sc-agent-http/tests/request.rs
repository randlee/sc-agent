//! REQ-SAG-0013 at the request level (A5-24): no request to a plain-http host
//! that is not loopback carries a key.

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod cases {

    use std::sync::Arc;
    use std::time::Instant;

    use sc_agent_http::testing::ScriptedTransport;
    use sc_agent_http::{
        Attempts, Endpoint, HttpClient, HttpError, HttpRequest, HttpResponse, LogContext, Method,
        Next, ResendPolicy, SecretHeader, Start,
    };
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_secrets::{
        ApiKey, EnvSource, KeySources, KeyringSource, SecretsError, resolve_var,
    };
    use sc_agent_types::ErrorId;

    struct OneVar;
    impl EnvSource for OneVar {
        fn var(&self, name: &str) -> Option<String> {
            (name == "HTTP_TEST_KEY").then(|| KEY.to_owned())
        }
    }
    struct NoKeyring;
    impl KeyringSource for NoKeyring {
        fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
            Ok(None)
        }
    }

    fn key() -> ApiKey {
        let sources = KeySources {
            env: &OneVar,
            repo_root: None,
            keyring: &NoKeyring,
        };
        resolve_var(&"HTTP_TEST_KEY".parse().expect("var"), &sources)
            .expect("key")
            .key
    }

    const KEY: &str = "sk-plain-http-secret-42";

    fn local(url: &str) -> HttpRequest {
        HttpRequest::new(
            Method::Post,
            Endpoint::parse_local(url).expect("local endpoint"),
            b"{}".as_slice(),
        )
    }

    /// Sends `request` once through a scripted 200 and returns the sends made.
    fn send(request: &HttpRequest) -> usize {
        let transport =
            ScriptedTransport::responses(vec![HttpResponse::new(200, vec![], Vec::new())]);
        let memory = MemoryLog::new().expect("memory log");
        let client = HttpClient::new(
            Arc::new(transport.clone()),
            ResendPolicy::default(),
            memory.general(),
        );
        let Ok(Start::Go(permit)) = client.may_start(request, Attempts::new(), Instant::now())
        else {
            panic!("fresh");
        };
        let next = client.send_once(request, permit, LogContext::NONE, &Instant::now);
        assert!(matches!(next, Ok(Next::Done { .. })), "{next:?}");
        transport.sends()
    }

    #[test]
    fn key_refused_on_plain_http_remote() {
        let transport =
            ScriptedTransport::responses(vec![HttpResponse::new(200, vec![], Vec::new())]);
        let memory = MemoryLog::new().expect("memory log");
        let client = HttpClient::new(
            Arc::new(transport.clone()),
            ResendPolicy::default(),
            memory.general(),
        );
        for url in [
            "http://10.0.0.5/v1",
            "http://10.0.0.5:8080/v1",
            "http://gpu.lan/v1",
            "http://[::2]/",
        ] {
            // The keyed request is never built, so nothing can be sent.
            let sent = local(url)
                .with_secret(SecretHeader::bearer(key()))
                .map(|request| {
                    let Ok(Start::Go(permit)) =
                        client.may_start(&request, Attempts::new(), Instant::now())
                    else {
                        panic!("fresh");
                    };
                    client.send_once(&request, permit, LogContext::NONE, &Instant::now)
                });
            let Err(error) = sent else {
                panic!("{url}: a key was attached to plain http");
            };
            assert_eq!(error.id(), "http.key_on_plain_http", "{url}");
            assert!(
                matches!(&error, HttpError::KeyOnPlainHttp { host } if url.contains(host.as_str()))
            );
            let recovery = error.recovery().expect("recovery text");
            assert!(
                recovery.contains("https") && recovery.contains("loopback"),
                "{recovery}"
            );
            let rendered = format!("{error} {error:?} {:?}", error.detail());
            assert!(!rendered.contains(KEY), "{rendered}");
        }
        // Refused before any send: nothing reached a transport.
        assert_eq!(transport.sends(), 0);
    }

    #[test]
    fn key_allowed_on_loopback() {
        for url in [
            "http://127.0.0.1:11434/v1/chat/completions",
            "http://127.1.2.3/x",
            "http://localhost:8080/v1",
            "http://[::1]:8080/v1",
        ] {
            let request = local(url)
                .with_secret(SecretHeader::new("x-api-key", key()))
                .expect(url);
            assert_eq!(request.secret().map(SecretHeader::name), Some("x-api-key"));
            assert_eq!(send(&request), 1, "{url}");
        }
    }

    #[test]
    fn key_allowed_on_https() {
        for request in [
            HttpRequest::new(
                Method::Post,
                Endpoint::parse("https://api.example.test/v1").expect("https"),
                Vec::new(),
            ),
            local("https://10.0.0.5/v1"),
        ] {
            let request = request
                .with_secret(SecretHeader::bearer(key()))
                .expect("https carries a key");
            assert_eq!(send(&request), 1);
        }
    }

    #[test]
    fn plain_http_remote_without_key_allowed() {
        let request = local("http://10.0.0.5/v1")
            .with_header("content-type", "application/json")
            .expect("not a credential header");
        assert!(request.secret().is_none());
        assert_eq!(send(&request), 1);
    }
}
