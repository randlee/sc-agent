//! Minimal in-process HTTP/1.1 server for transport tests.
//!
//! Copied verbatim into `sc-jev` and `sc-jev-backends`. A unit test in
//! `sc-jev` (`lib.rs`, `shared_copies`) compares the two files byte for byte,
//! so they cannot drift. The production transports are HTTPS-only; tests
//! build them with a loopback-only constructor that allows
//! `http://127.0.0.1`, so everything except the TLS handshake runs through
//! the real ureq agent.
//!
//! Every wait is bounded (FTQ-006). The accept loop is nonblocking and gives
//! up after [`SERVER_DEADLINE`] or when [`LoopbackServer::finish`] signals
//! shutdown. Each accepted stream has read and write timeouts. `finish`
//! waits at most [`FINISH_DEADLINE`] and panics with "server expected N
//! connections, saw M" when the client made fewer connections than scripted.

use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Longest the server thread waits for all scripted connections.
const SERVER_DEADLINE: Duration = Duration::from_secs(30);

/// Longest [`LoopbackServer::finish`] waits for the server thread.
const FINISH_DEADLINE: Duration = Duration::from_secs(10);

/// Read and write timeout on each accepted stream.
const STREAM_TIMEOUT: Duration = Duration::from_secs(10);

/// Poll interval of the nonblocking accept loop.
const ACCEPT_POLL: Duration = Duration::from_millis(5);

/// What the server does with one accepted connection.
pub(crate) enum Reply {
    /// Writes these bytes after reading the request, then closes.
    Raw(Vec<u8>),
    /// Reads the request, then holds the connection open without answering
    /// until [`LoopbackServer::finish`] releases it (FTQ-010). The hold is
    /// bounded by [`SERVER_DEADLINE`].
    Stall,
}

impl Reply {
    /// A complete response with `Content-Length` and `Connection: close`.
    pub(crate) fn status(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Self {
        let mut head = format!("HTTP/1.1 {status} Test\r\n");
        for (name, value) in headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str(&format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        ));
        let mut bytes = head.into_bytes();
        bytes.extend_from_slice(body);
        Self::Raw(bytes)
    }
}

/// One request as the server saw it.
pub(crate) struct Seen {
    /// Request line and headers as received, `\r\n` separated.
    pub(crate) head: String,
    /// Request body.
    pub(crate) body: Vec<u8>,
}

impl Seen {
    /// Value of the first header named `name` (case-insensitive).
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim().to_ascii_lowercase() == name).then(|| value.trim())
        })
    }
}

/// Server that answers one connection per scripted [`Reply`], in order.
pub(crate) struct LoopbackServer {
    addr: SocketAddr,
    expected: usize,
    shutdown: Arc<AtomicBool>,
    release: Option<Sender<()>>,
    done: Receiver<Vec<Seen>>,
}

impl LoopbackServer {
    /// Binds `127.0.0.1:0` and serves `replies` on a background thread.
    pub(crate) fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let addr = listener.local_addr().expect("addr");
        let expected = replies.len();
        let shutdown = Arc::new(AtomicBool::new(false));
        let (release, released) = mpsc::channel::<()>();
        let (finished, done) = mpsc::channel();
        let stop = Arc::clone(&shutdown);
        thread::spawn(move || {
            let seen = serve(&listener, replies, &stop, &released);
            let _ = finished.send(seen);
        });
        Self {
            addr,
            expected,
            shutdown,
            release: Some(release),
            done,
        }
    }

    /// `http://127.0.0.1:{port}{path}`.
    pub(crate) fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    /// Releases any stalled reply, stops accepting, and returns the requests
    /// seen.
    ///
    /// # Panics
    ///
    /// When the server thread does not finish within [`FINISH_DEADLINE`], or
    /// when it saw fewer connections than scripted replies.
    pub(crate) fn finish(mut self) -> Vec<Seen> {
        self.shutdown.store(true, Ordering::SeqCst);
        drop(self.release.take());
        let seen = match self.done.recv_timeout(FINISH_DEADLINE) {
            Ok(seen) => seen,
            Err(RecvTimeoutError::Timeout) => {
                panic!("server thread did not finish within {FINISH_DEADLINE:?}")
            }
            Err(RecvTimeoutError::Disconnected) => panic!("server thread panicked"),
        };
        assert_eq!(
            seen.len(),
            self.expected,
            "server expected {} connections, saw {}",
            self.expected,
            seen.len()
        );
        seen
    }
}

/// A bound loopback port that never accepts (FTQ-009).
///
/// The listener stays bound for the guard's lifetime, so the OS cannot hand
/// the port to another test's server. A connection is queued by the kernel
/// but never answered, so a client fails with a network error at its own
/// timeout. Use a short client timeout.
pub(crate) struct ClosedPort {
    _listener: TcpListener,
    addr: SocketAddr,
}

impl ClosedPort {
    /// `http://127.0.0.1:{port}/closed`.
    pub(crate) fn url(&self) -> String {
        format!("http://{}/closed", self.addr)
    }
}

/// Binds a loopback port that is held but never accepted on.
pub(crate) fn closed_port() -> ClosedPort {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("addr");
    ClosedPort {
        _listener: listener,
        addr,
    }
}

/// A redirect target that is never contacted (reserved `.test` name).
pub(crate) const UNREACHED_URL: &str = "http://invalid.test/";

fn serve(
    listener: &TcpListener,
    replies: Vec<Reply>,
    shutdown: &AtomicBool,
    released: &Receiver<()>,
) -> Vec<Seen> {
    let started = Instant::now();
    let mut seen = Vec::new();
    for reply in replies {
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break Some(stream),
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if shutdown.load(Ordering::SeqCst) || started.elapsed() >= SERVER_DEADLINE {
                        break None;
                    }
                    thread::sleep(ACCEPT_POLL);
                }
                Err(_) => break None,
            }
        };
        let Some(stream) = stream else { break };
        seen.push(serve_one(stream, reply, released));
    }
    seen
}

fn serve_one(stream: TcpStream, reply: Reply, released: &Receiver<()>) -> Seen {
    // Accepted sockets inherit nonblocking mode on some platforms.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(STREAM_TIMEOUT));
    let _ = stream.set_write_timeout(Some(STREAM_TIMEOUT));
    let mut reader = BufReader::new(stream);
    let mut head = String::new();
    let mut content_length = 0_usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
        head.push_str(&line);
    }
    let mut body = vec![0_u8; content_length];
    let _ = reader.read_exact(&mut body);
    let mut stream = reader.into_inner();
    match reply {
        Reply::Raw(bytes) => {
            let _ = stream.write_all(&bytes);
            let _ = stream.flush();
        }
        // Returns on release (the sender is dropped) or at the deadline.
        Reply::Stall => {
            let _ = released.recv_timeout(SERVER_DEADLINE);
        }
    }
    Seen { head, body }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "server expected 2 connections, saw 0")]
    fn finish_fails_fast_when_the_client_makes_fewer_connections() {
        let server = LoopbackServer::start(vec![
            Reply::status(200, &[], b""),
            Reply::status(200, &[], b""),
        ]);
        server.finish();
    }

    #[test]
    fn closed_port_holds_its_address() {
        let closed = closed_port();
        let url = closed.url();
        let port = url
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches("/closed");
        assert!(
            TcpListener::bind(format!("127.0.0.1:{port}")).is_err(),
            "the guard must keep the port bound"
        );
    }
}
