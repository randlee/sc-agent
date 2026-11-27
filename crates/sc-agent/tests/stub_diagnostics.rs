pub mod support;

use std::io::{Read, Write};
use std::net::TcpStream;
use support::{Reply, Stub, Test, http};

fn send_raw(stub: &Stub, body: &str) -> Test {
    let address = stub
        .url
        .trim_start_matches("http://")
        .trim_end_matches("/v1");
    let mut stream = TcpStream::connect(address)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    write!(
        stream,
        "POST /v1/test HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    Ok(())
}

#[test]
fn stub_error_reports_reply_stage_and_captured_requests() -> Test {
    let stub = Stub::new(vec![http(200, serde_json::json!({})), Reply::Disconnect])?;
    send_raw(&stub, "{}")?;
    send_raw(&stub, "{")?;
    let error = stub
        .finish()
        .err()
        .ok_or("expected worker failure")?
        .to_string();
    assert!(error.contains("reply 2 at read request"), "{error}");
    assert!(error.contains("/v1/test"), "{error}");
    Ok(())
}

#[test]
fn dropping_unfinished_stub_surfaces_worker_error() -> Test {
    let stub = Stub::new(vec![Reply::Disconnect])?;
    send_raw(&stub, "{")?;
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(stub)));
    let panic = failure.err().ok_or("expected worker failure on drop")?;
    let message = panic
        .downcast_ref::<String>()
        .ok_or("expected panic message")?;
    assert!(message.contains("reply 1 at read request"), "{message}");
    Ok(())
}
