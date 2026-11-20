use std::{
    net::TcpListener,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[test]
fn network_retry() {
    if std::env::var_os("RETRY_CHILD").is_some() {
        let delays = Arc::new(Mutex::new(Vec::<Duration>::new()));
        let observed = Arc::clone(&delays);
        let result = sc_agent::send(
            "{}",
            "jev",
            None,
            Some("test"),
            None,
            &move |delay| {
                observed.lock().expect("delay lock").push(delay);
            },
            None,
        );
        assert_eq!(result["error"]["id"], "network_error");
        assert_eq!(delays.lock().expect("delay lock").len(), 2);
        return;
    }
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local server");
    let address = listener.local_addr().expect("local address");
    let server = thread::spawn(move || {
        for _ in 0..3 {
            let (stream, _) = listener.accept().expect("accept connection");
            drop(stream);
        }
    });
    let output = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", "network_retry", "--nocapture"])
        .env("RETRY_CHILD", "1")
        .env("TYPESAFE_API_KEY", "test-key")
        .env("SC_AGENT_TEST_BASE_URL", format!("http://{address}"))
        .output()
        .expect("run child");
    for _ in 0..3 {
        let _ = std::net::TcpStream::connect(address);
    }
    server.join().expect("server thread");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cancelled_batch_starts_no_requests() {
    let flag = std::sync::atomic::AtomicBool::new(true);
    let result = sc_agent::send_batch(
        &["{}".into()],
        "jev",
        None,
        Some("test"),
        None,
        1,
        &|_| {},
        Some(&flag),
    );
    assert_eq!(result["value"], serde_json::json!([]));
}
