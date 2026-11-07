//! Minimal in-process HTTP/1.1 server for the `ureq` loopback tests.
//!
//! Moved here from the former `sc-jev` / `sc-jev-backends` copies
//! (`src/test_server.rs`). The production transport is HTTPS-only; tests
//! build `UreqTransport::loopback_for_tests` and `Endpoint::loopback_http`,
//! so everything except the TLS handshake runs through the real ureq agent
//! against a std `TcpListener` on 127.0.0.1.
//!
//! Every wait is bounded. The accept loop is nonblocking and gives up after
//! [`SERVER_DEADLINE`] or when [`LoopbackServer::finish`] signals shutdown.
//! Each accepted stream has read and write timeouts. `finish` waits at most
//! [`FINISH_DEADLINE`] and panics with "server expected N requests, saw M"
//! when the client sent fewer requests than scripted.

use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use sc_agent_http::Endpoint;

/// Longest the server thread waits for all scripted requests.
const SERVER_DEADLINE: Duration = Duration::from_secs(30);

/// Longest [`LoopbackServer::finish`] waits for the server thread.
const FINISH_DEADLINE: Duration = Duration::from_secs(10);

/// Read and write timeout on each accepted stream.
const STREAM_TIMEOUT: Duration = Duration::from_secs(10);

/// Poll interval of the nonblocking accept loop.
const ACCEPT_POLL: Duration = Duration::from_millis(5);

/// What the server does with one request.
pub enum Reply {
    /// Writes these bytes after reading the request.
    Raw(Vec<u8>),
    /// Reads the request, then holds the connection open without answering
    /// until [`LoopbackServer::finish`] releases it. Bounded by
    /// [`SERVER_DEADLINE`].
    Stall,
}

impl Reply {
    /// A complete response with `Content-Length` and `Connection: close`.
    pub fn status(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Self {
        Self::build(status, headers, body, true)
    }

    /// A complete response that keeps the connection open.
    pub fn keep_alive(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Self {
        Self::build(status, headers, body, false)
    }

    fn build(status: u16, headers: &[(&str, &str)], body: &[u8], close: bool) -> Self {
        let mut head = format!("HTTP/1.1 {status} Test\r\n");
        for (name, value) in headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
        if close {
            head.push_str("Connection: close\r\n");
        }
        head.push_str("\r\n");
        let mut bytes = head.into_bytes();
        bytes.extend_from_slice(body);
        Self::Raw(bytes)
    }
}

/// One request as the server saw it.
pub struct Seen {
    /// Index of the accepted connection that carried it (0-based).
    pub connection: usize,
    /// Request line and headers as received, `\r\n` separated.
    pub head: String,
    /// Request body.
    pub body: Vec<u8>,
}

impl Seen {
    /// Value of the first header named `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim().to_ascii_lowercase() == name).then(|| value.trim())
        })
    }
}

/// How replies map onto connections.
#[derive(Clone, Copy)]
enum Mode {
    /// One connection per reply.
    PerReply,
    /// Every reply on the first connection (keep-alive).
    OneConnection,
}

/// Server that answers scripted [`Reply`]s in order.
pub struct LoopbackServer {
    addr: SocketAddr,
    expected: usize,
    shutdown: Arc<AtomicBool>,
    release: Option<Sender<()>>,
    done: Receiver<Vec<Seen>>,
}

impl LoopbackServer {
    /// One accepted connection per reply.
    pub fn start(replies: Vec<Reply>) -> Self {
        Self::spawn(replies, Mode::PerReply)
    }

    /// Every reply on one accepted connection. A client that opens a second
    /// connection is never answered, so it fails at its own timeout.
    pub fn start_one_connection(replies: Vec<Reply>) -> Self {
        Self::spawn(replies, Mode::OneConnection)
    }

    fn spawn(replies: Vec<Reply>, mode: Mode) -> Self {
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
            let seen = serve(&listener, replies, mode, &stop, &released);
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
    pub fn endpoint(&self, path: &str) -> Endpoint {
        Endpoint::loopback_http(self.addr.port(), path)
    }

    /// Releases any stalled reply, stops accepting, and returns the requests
    /// seen.
    ///
    /// # Panics
    ///
    /// When the server thread does not finish within [`FINISH_DEADLINE`], or
    /// when it saw fewer requests than scripted replies.
    pub fn finish(mut self) -> Vec<Seen> {
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
            "server expected {} requests, saw {}",
            self.expected,
            seen.len()
        );
        seen
    }
}

/// A loopback port with nothing listening: a connect is refused at once.
pub fn refused_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    port
}

fn accept(listener: &TcpListener, shutdown: &AtomicBool, started: Instant) -> Option<TcpStream> {
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // Accepted sockets inherit nonblocking mode on some platforms.
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(STREAM_TIMEOUT));
                let _ = stream.set_write_timeout(Some(STREAM_TIMEOUT));
                return Some(stream);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if shutdown.load(Ordering::SeqCst) || started.elapsed() >= SERVER_DEADLINE {
                    return None;
                }
                thread::sleep(ACCEPT_POLL);
            }
            Err(_) => return None,
        }
    }
}

fn serve(
    listener: &TcpListener,
    replies: Vec<Reply>,
    mode: Mode,
    shutdown: &AtomicBool,
    released: &Receiver<()>,
) -> Vec<Seen> {
    let started = Instant::now();
    let mut seen = Vec::new();
    let mut connection: Option<BufReader<TcpStream>> = None;
    let mut connections = 0_usize;
    for reply in replies {
        let reuse = matches!(mode, Mode::OneConnection) && connection.is_some();
        if !reuse {
            let Some(stream) = accept(listener, shutdown, started) else {
                break;
            };
            connection = Some(BufReader::new(stream));
            connections += 1;
        }
        let Some(reader) = connection.as_mut() else {
            break;
        };
        let Some((head, body)) = read_request(reader) else {
            break;
        };
        seen.push(Seen {
            connection: connections - 1,
            head,
            body,
        });
        match reply {
            Reply::Raw(bytes) => {
                let stream = reader.get_mut();
                let _ = stream.write_all(&bytes);
                let _ = stream.flush();
            }
            // Returns on release (the sender is dropped) or at the deadline.
            Reply::Stall => {
                let _ = released.recv_timeout(SERVER_DEADLINE);
            }
        }
        if matches!(mode, Mode::PerReply) {
            connection = None;
        }
    }
    seen
}

fn read_request(reader: &mut BufReader<TcpStream>) -> Option<(String, Vec<u8>)> {
    let mut head = String::new();
    let mut content_length = 0_usize;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).unwrap_or(0);
        if read == 0 {
            return (!head.is_empty()).then(|| (head, Vec::new()));
        }
        if line == "\r\n" {
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
    Some((head, body))
}
