//! UreqTransport over real loopback sockets and over the refusing stub (no
//! live network, no sleep, no real-time assertion): the refused connection,
//! byte bodies with the secret header, and the per-transport and per-request
//! deadlines (REQ-SAG-0015), asserted as the Duration ureq applies.

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod cases {

    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::mpsc;
    use std::time::Duration;

    use sc_agent_http::testing::RefusingTransport;
    use sc_agent_http::{
        Endpoint, HttpError, HttpRequest, Method, NetworkKind, SecretHeader, Timeouts, Transport,
        UreqTransport,
    };
    use sc_agent_secrets::{
        ApiKey, EnvSource, KeySources, KeyringSource, SecretsError, resolve_var,
    };
    use sc_agent_testkit::{Gate, HANG_GUARD, Server, recv_within, serve_once};
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

    const KEY: &str = "sk-loopback-secret";

    fn loopback() -> UreqTransport {
        UreqTransport::new(Timeouts::probe())
    }

    fn endpoint(port: u16, path: &str) -> Endpoint {
        Endpoint::parse_local(&format!("http://127.0.0.1:{port}{path}")).expect("loopback")
    }

    fn post(endpoint: Endpoint, secret: Option<SecretHeader>) -> HttpRequest {
        let request = HttpRequest::new(Method::Post, endpoint, br#"{"q":1}"#.as_slice())
            .with_header("content-type", "application/json")
            .expect("not a credential header");
        match secret {
            Some(secret) => request.with_secret(secret).expect("loopback carries a key"),
            None => request,
        }
    }

    fn api_key_header() -> SecretHeader {
        SecretHeader::new("x-api-key", key())
    }

    fn assert_no_key(error: &HttpError) {
        let rendered = format!("{error} {error:?} {}", error.message());
        assert!(!rendered.contains(KEY), "{rendered}");
    }

    /// The kind of a real transport failure, key never shown.
    fn assert_network(error: &HttpError, kind: NetworkKind) {
        assert!(
            matches!(error, HttpError::Network { kind: got, .. } if *got == kind),
            "expected {kind:?}: {error:?}"
        );
        assert_no_key(error);
    }

    /// Reads one HTTP/1.1 request: the head (lower-cased) and the body bytes.
    fn read_request(stream: &mut TcpStream) -> (String, Vec<u8>) {
        let mut data = Vec::new();
        let mut chunk = [0_u8; 4096];
        let head_end = loop {
            if let Some(at) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                break at + 4;
            }
            let read = stream.read(&mut chunk).expect("read head");
            assert!(read > 0, "client closed before the head ended");
            data.extend_from_slice(&chunk[..read]);
        };
        let head = String::from_utf8_lossy(&data[..head_end]).to_ascii_lowercase();
        let length: usize = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .map_or(0, |value| value.trim().parse().expect("content-length"));
        let mut body = data[head_end..].to_vec();
        while body.len() < length {
            let read = stream.read(&mut chunk).expect("read body");
            assert!(read > 0, "client closed before the body ended");
            body.extend_from_slice(&chunk[..read]);
        }
        (head, body)
    }

    fn write_ok(stream: &mut TcpStream) {
        // The client may have given up already; a failed write is not this test's concern.
        let _ignored = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
    }

    /// A server that reads one request, tells the test, waits for `gate` (bounded), then
    /// answers 200.
    fn gated_server(gate: &Gate) -> (Server, mpsc::Receiver<()>) {
        let (received_tx, received) = mpsc::channel();
        let gate = gate.clone();
        let server = serve_once(move |mut stream| {
            let _request = read_request(&mut stream);
            received_tx.send(()).expect("tell the test");
            gate.wait_open();
            write_ok(&mut stream);
        });
        (server, received)
    }

    /// A refused connection is `connect`, after one send, with the key never shown. The
    /// refusal is by construction (the stub's connector refuses every connection), so no
    /// freed port can be reused by another test.
    #[test]
    fn refused_connection_is_connect() {
        let transport = RefusingTransport::new(Timeouts::probe());
        let error = transport
            .send(&post(endpoint(9, "/closed"), Some(api_key_header())))
            .expect_err("refused");
        assert_network(&error, NetworkKind::Connect);
        assert_eq!(error.id(), "http.network");
        assert_eq!(
            error.cause_detail().map(|cause| cause.id),
            Some("io.ConnectionRefused".to_owned())
        );
        assert_eq!(transport.deadlines().len(), 1, "one connection attempt");
    }

    /// A5-10: headers, the secret header and a byte body arrive unchanged.
    #[test]
    fn multipart_bytes_sent() {
        let boundary = "sc-agent-boundary-42";
        let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n"
    )
    .into_bytes();
        body.extend_from_slice(&[0, 159, 146, 150, 255, 13, 10, 0]);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let expected = body.clone();
        let (received_tx, received) = mpsc::channel();
        let server = serve_once(move |mut stream| {
            let request = read_request(&mut stream);
            received_tx.send(request).expect("hand over");
            write_ok(&mut stream);
        });
        let request = HttpRequest::new(Method::Post, endpoint(server.port(), "/upload"), body)
            .with_header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .expect("not a credential header")
            .with_secret(api_key_header())
            .expect("loopback");
        let response = loopback().send(&request).expect("200");
        assert_eq!(
            (response.status(), response.body()),
            (200, b"{}".as_slice())
        );
        let (head, got) = recv_within(&received, HANG_GUARD);
        server.finish();
        assert_eq!(got, expected, "the body bytes are sent unchanged");
        assert!(head.starts_with("post /upload http/1.1\r\n"), "{head}");
        assert!(head.contains(&format!("x-api-key: {}", KEY.to_ascii_lowercase())));
        assert!(head.contains(&format!(
            "content-type: multipart/form-data; boundary={boundary}"
        )));
        assert!(head.contains("user-agent: sc-agent-http/"));
    }

    /// A5-25 / A5-26: the whole-send deadline ureq applies is the transport's own request
    /// timeout (`with_request(1 s)`, or the 120 s default), and a request's
    /// `with_deadline(30 s)` replaces it for that request only; the next request without one
    /// is back on the transport's. Asserted as the Duration ureq is configured with for each
    /// connection attempt, not as elapsed time.
    #[test]
    fn the_applied_deadline_is_the_transport_or_the_request_one() {
        let one_second =
            RefusingTransport::new(Timeouts::default().with_request(Duration::from_secs(1)));
        let _ = one_second.send(&post(endpoint(9, "/plain"), None));
        let _ = one_second
            .send(&post(endpoint(9, "/load"), None).with_deadline(Duration::from_secs(30)));
        let _ = one_second.send(&post(endpoint(9, "/plain"), None));
        assert_eq!(
            one_second.deadlines(),
            [
                Some(Duration::from_secs(1)),
                Some(Duration::from_secs(30)),
                Some(Duration::from_secs(1))
            ]
        );
        let default = RefusingTransport::new(Timeouts::default());
        let _ = default.send(&post(endpoint(9, "/plain"), None));
        assert_eq!(default.deadlines(), [Some(Duration::from_secs(120))]);
    }

    /// A5-25 over a real socket: a transport built with `with_request(1 s)` gives up with a
    /// timeout while the server holds its reply; the server answers only after the test saw
    /// the error and opened the gate, so the outcome does not depend on how long anything took
    /// (the applied 1 s is asserted by the_applied_deadline_is_the_transport_or_the_request_one).
    #[test]
    fn per_runner_transport_deadline() {
        let transport =
            UreqTransport::new(Timeouts::default().with_request(Duration::from_secs(1)));
        let gate = Gate::new();
        let opener = gate.opener();
        let (server, received) = gated_server(&gate);
        let error = transport
            .send(&post(endpoint(server.port(), "/slow"), None))
            .expect_err("deadline");
        recv_within(&received, HANG_GUARD);
        opener.open();
        server.finish();
        assert_network(&error, NetworkKind::Timeout);
    }

    /// RSH-004: UreqTransport refuses zero or over-long timeouts and deadlines without opening
    /// a connection: the refusing stub saw no connection attempt.
    #[test]
    fn invalid_timeouts_refused_without_connecting() {
        let zero = RefusingTransport::new(Timeouts::default().with_request(Duration::ZERO));
        let error = zero
            .send(&post(endpoint(9, "/x"), None))
            .expect_err("zero request timeout");
        assert_eq!(error.id(), "http.invalid_request", "{error:?}");
        assert!(zero.deadlines().is_empty(), "no connection was attempted");
        let refusing = RefusingTransport::new(Timeouts::probe());
        let long = post(endpoint(9, "/x"), None)
            .with_deadline(Timeouts::DEADLINE_MAX + Duration::from_secs(1));
        let error = refusing.send(&long).expect_err("over-long deadline");
        assert_eq!(error.id(), "http.invalid_request", "{error:?}");
        assert!(
            refusing.deadlines().is_empty(),
            "no connection was attempted"
        );
    }

    /// QA3-001 / REQ-SAG-0015: UreqTransport accepts a per-request deadline
    /// up to Timeouts::DEADLINE_MAX (load + request timeout), above the
    /// 3600 s cap of the transport's own timeouts.
    #[test]
    fn deadline_up_to_deadline_max_is_sent() {
        let transport = loopback();
        for deadline in [Duration::from_secs(4200), Timeouts::DEADLINE_MAX] {
            let server = serve_once(|mut stream| {
                let _request = read_request(&mut stream);
                write_ok(&mut stream);
            });
            let request = post(endpoint(server.port(), "/first"), None).with_deadline(deadline);
            let response = transport
                .send(&request)
                .unwrap_or_else(|error| panic!("{deadline:?} refused: {error:?}"));
            assert_eq!(response.status(), 200, "{deadline:?}");
            server.finish();
        }
    }
}
