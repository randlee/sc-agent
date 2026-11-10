//! UreqTransport over real loopback sockets (no live network, no sleep):
//! the refused connection, byte bodies with the secret header, and the
//! per-transport and per-request deadlines (REQ-SAG-0015).

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod cases {

    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Condvar, Mutex, PoisonError, mpsc};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use sc_agent_http::{
        Endpoint, HttpError, HttpRequest, Method, NetworkKind, SecretHeader, Timeouts, Transport,
        UreqTransport,
    };
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

    fn send_once(transport: &UreqTransport, endpoint: Endpoint) -> HttpError {
        transport
            .send(&post(endpoint, Some(api_key_header())))
            .expect_err("transport failure")
    }

    /// Binds a loopback listener and runs `serve` on its first connection.
    fn serve_once(serve: impl FnOnce(TcpStream) + Send + 'static) -> (u16, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            serve(stream);
        });
        (port, handle)
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

    /// A gate the server waits on until the test opens it. No timeout.
    #[derive(Clone, Default)]
    struct Gate(Arc<(Mutex<bool>, Condvar)>);

    impl Gate {
        fn wait(&self) {
            let (open, condvar) = &*self.0;
            let mut open = open.lock().unwrap_or_else(PoisonError::into_inner);
            while !*open {
                open = condvar.wait(open).unwrap_or_else(PoisonError::into_inner);
            }
        }

        fn open(&self) {
            let (open, condvar) = &*self.0;
            *open.lock().unwrap_or_else(PoisonError::into_inner) = true;
            condvar.notify_all();
        }
    }

    /// A server that reads one request, tells the test, waits for `gate`, then
    /// answers 200.
    fn gated_server(gate: &Gate) -> (u16, mpsc::Receiver<()>, JoinHandle<()>) {
        let (received_tx, received) = mpsc::channel();
        let gate = gate.clone();
        let (port, handle) = serve_once(move |mut stream| {
            let _request = read_request(&mut stream);
            received_tx.send(()).expect("tell the test");
            gate.wait();
            write_ok(&mut stream);
        });
        (port, received, handle)
    }

    /// A real refused connection (a port that was bound, then freed) is
    /// `connect`, after one send. Windows retries the SYN of a closed port until
    /// the connect deadline, so there it may surface as `timeout`.
    #[test]
    fn refused_connection_is_connect() {
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            listener.local_addr().expect("addr").port()
        };
        let error = send_once(&loopback(), endpoint(port, "/closed"));
        if cfg!(windows) {
            assert!(
                matches!(
                    error,
                    HttpError::Network {
                        kind: NetworkKind::Connect | NetworkKind::Timeout,
                        ..
                    }
                ),
                "{error:?}"
            );
        } else {
            assert_network(&error, NetworkKind::Connect);
        }
        assert_eq!(error.id(), "http.network");
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
        let (port, server) = serve_once(move |mut stream| {
            let request = read_request(&mut stream);
            received_tx.send(request).expect("hand over");
            write_ok(&mut stream);
        });
        let request = HttpRequest::new(Method::Post, endpoint(port, "/upload"), body)
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
        let (head, got) = received.recv().expect("request");
        server.join().expect("server");
        assert_eq!(got, expected, "the body bytes are sent unchanged");
        assert!(head.starts_with("post /upload http/1.1\r\n"), "{head}");
        assert!(head.contains(&format!("x-api-key: {}", KEY.to_ascii_lowercase())));
        assert!(head.contains(&format!(
            "content-type: multipart/form-data; boundary={boundary}"
        )));
        assert!(head.contains("user-agent: sc-agent-http/"));
    }

    /// A5-25: a transport built with `with_request(1 s)` gives up after its own
    /// deadline; the server answers only when the test opens the gate.
    #[test]
    fn per_runner_transport_deadline() {
        let transport =
            UreqTransport::new(Timeouts::default().with_request(Duration::from_secs(1)));
        let gate = Gate::default();
        let (port, received, server) = gated_server(&gate);
        let started = Instant::now();
        let error = transport
            .send(&post(endpoint(port, "/slow"), None))
            .expect_err("deadline");
        let waited = started.elapsed();
        received.recv().expect("the server got the request");
        gate.open();
        server.join().expect("server");
        assert_network(&error, NetworkKind::Timeout);
        assert!(waited >= Duration::from_secs(1), "{waited:?}");
        assert!(
            waited < Duration::from_secs(10),
            "the 120 s default was not used: {waited:?}"
        );
    }

    /// A5-26: `with_deadline(30 s)` replaces the transport's 1 s request
    /// timeout for that request only; the next request without one times out.
    #[test]
    fn request_deadline_overrides_transport() {
        let transport =
            UreqTransport::new(Timeouts::default().with_request(Duration::from_secs(1)));
        let long_gate = Gate::default();
        let (long_port, long_received, long_server) = gated_server(&long_gate);
        let long_request =
            post(endpoint(long_port, "/load"), None).with_deadline(Duration::from_secs(30));
        let sender = transport.clone();
        let started = Instant::now();
        let long = thread::spawn(move || sender.send(&long_request).map(|r| r.status()));
        long_received.recv().expect("the long request is in flight");

        // A request without a deadline on the same transport times out after 1 s,
        // while the long one is still waiting: the 1 s mark has passed for it too.
        let short_gate = Gate::default();
        let (short_port, short_received, short_server) = gated_server(&short_gate);
        let error = transport
            .send(&post(endpoint(short_port, "/plain"), None))
            .expect_err("1 s transport deadline");
        assert_network(&error, NetworkKind::Timeout);
        short_received.recv().expect("received");
        short_gate.open();
        short_server.join().expect("server");
        assert!(started.elapsed() >= Duration::from_secs(1));

        long_gate.open();
        assert_eq!(long.join().expect("sender"), Ok(200));
        long_server.join().expect("server");
    }

    /// RSH-004: UreqTransport refuses zero or over-long timeouts and
    /// deadlines without opening a connection.
    #[test]
    fn invalid_timeouts_refused_without_connecting() {
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            listener.local_addr().expect("addr").port()
        };
        let zero = UreqTransport::new(Timeouts::default().with_request(Duration::ZERO));
        let error = zero
            .send(&post(endpoint(port, "/x"), None))
            .expect_err("zero request timeout");
        assert_eq!(error.id(), "http.invalid_request", "{error:?}");
        let long = post(endpoint(port, "/x"), None)
            .with_deadline(Timeouts::DEADLINE_MAX + Duration::from_secs(1));
        let error = loopback().send(&long).expect_err("over-long deadline");
        assert_eq!(error.id(), "http.invalid_request", "{error:?}");
    }
}
