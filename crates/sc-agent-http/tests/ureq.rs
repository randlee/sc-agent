const KEY: &str = "sk-loopback-secret";

/// The response cap of `UreqTransport` (8 MiB).
const CAP: usize = 8 * 1024 * 1024;

fn key() -> ResolvedApiKey {
    ResolvedApiKey::try_new(BackendKeyId::Anthropic, KEY.to_owned()).expect("key")
}

fn loopback() -> UreqTransport {
    UreqTransport::loopback_for_tests(Timeouts::new(
        Duration::from_secs(2),
        Duration::from_secs(10),
    ))
}

fn post(endpoint: Endpoint, secret: Option<SecretHeader>) -> HttpRequest {
    let request = HttpRequest::new(Method::Post, endpoint, Arc::from(r#"{"q":1}"#))
        .with_header(HeaderName::new("content-type"), "application/json");
    match secret {
        Some(secret) => request.with_secret(secret),
        None => request,
    }
}

fn api_key_header() -> SecretHeader {
    SecretHeader::new(HeaderName::new("x-api-key"), key())
}

fn assert_no_key(error: &JevError) {
    let rendered = format!("{error} {error:?} {}", error.message());
    assert!(!rendered.contains(KEY), "{rendered}");
}
/// The kind and retryability of a real transport failure, key never shown.
fn assert_network(error: &JevError, kind: NetworkErrorKind, retryable: bool) {
    assert!(
        matches!(error, JevError::NetworkError { kind: got, .. } if *got == kind),
        "expected {kind:?}: {error:?}"
    );
    assert_eq!(error.retryable(), retryable, "{error:?}");
    assert_no_key(error);
}

fn send_once(transport: &UreqTransport, endpoint: Endpoint) -> JevError {
    transport
        .send(&post(endpoint, Some(api_key_header())))
        .expect_err("transport failure")
}

/// Binds a loopback listener and runs `serve` on its first connection.
fn serve_once(
    serve: impl FnOnce(std::net::TcpStream) + Send + 'static,
) -> (u16, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        serve(stream);
    });
    (port, handle)
}

/// A real refused connection (a port that was bound, then freed) is
/// `connect`, retryable. Windows retries the SYN of a closed port until the
/// connect deadline, so there it may surface as `timeout` (also retryable).
#[test]
fn test_real_connection_refused_is_connect() {
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("addr").port()
    };
    let error = send_once(&loopback(), Endpoint::loopback_http(port, "/closed"));
    if cfg!(windows) {
        assert!(error.retryable(), "{error:?}");
    } else {
        assert_network(&error, NetworkErrorKind::Connect, true);
    }
}
