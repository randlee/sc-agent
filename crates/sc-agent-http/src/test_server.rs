//! A loopback HTTP server that answers from a script, for deterministic tests. Each
//! connection gets the next scripted reply; every request is recorded.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Failure-only bound: a passing test never waits on it. A connection that stalls, or a
/// scripted reply the client never asks for, fails the test after this long instead of hanging.
const DEADLINE: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap()
    }
}

pub struct Server {
    pub base: String,
    addr: SocketAddr,
    scripted: usize,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    done: Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

/// One reply: status, extra headers, body.
pub type Reply = (u16, Vec<(&'static str, String)>, String);

impl Server {
    /// Serves exactly `replies.len()` connections, in order.
    pub fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let scripted = replies.len();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        // Dropped when the thread ends, normally or by panic, which ends `finish`'s wait.
        let (done_tx, done) = channel::<()>();
        let thread = std::thread::spawn(move || {
            let _done = done_tx;
            for (status, headers, body) in replies {
                let (stream, _) = listener.accept().unwrap();
                if stopped.load(Ordering::SeqCst) {
                    return;
                }
                stream.set_read_timeout(Some(DEADLINE)).unwrap();
                stream.set_write_timeout(Some(DEADLINE)).unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let mut parts = line.split_whitespace();
                let method = parts.next().unwrap_or_default().to_owned();
                let path = parts.next().unwrap_or_default().to_owned();
                let mut got = Vec::new();
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    let h = h.trim_end();
                    if h.is_empty() {
                        break;
                    }
                    if let Some((n, v)) = h.split_once(':') {
                        got.push((n.trim().to_owned(), v.trim().to_owned()));
                    }
                }
                let len: usize = got
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
                    .map_or(0, |(_, v)| v.parse().unwrap());
                let mut buf = vec![0; len];
                reader.read_exact(&mut buf).unwrap();
                log.lock().unwrap().push(Seen {
                    method,
                    path,
                    headers: got,
                    body: String::from_utf8(buf).unwrap(),
                });
                let mut out = stream;
                let mut head = format!(
                    "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n",
                    body.len()
                );
                for (n, v) in headers {
                    head.push_str(&format!("{n}: {v}\r\n"));
                }
                out.write_all(format!("{head}\r\n{body}").as_bytes())
                    .unwrap();
            }
        });
        Self {
            base,
            addr,
            scripted,
            seen,
            stop,
            done,
            thread: Some(thread),
        }
    }

    /// Waits for every scripted reply to be served, then returns what was received. Panics,
    /// naming how many of the scripted replies were served, when they are not all served
    /// within [`DEADLINE`] (the server thread is woken and joined first, so nothing leaks).
    pub fn finish(mut self) -> Vec<Seen> {
        let thread = self.thread.take().unwrap();
        if let Err(RecvTimeoutError::Timeout) = self.done.recv_timeout(DEADLINE) {
            self.stop.store(true, Ordering::SeqCst);
            let _wake = TcpStream::connect(self.addr);
            let _ = thread.join();
            let served = self.seen.lock().unwrap().len();
            panic!(
                "test server: {served} of {} scripted replies served within {DEADLINE:?}",
                self.scripted
            );
        }
        thread.join().unwrap();
        self.seen.lock().unwrap().clone()
    }
}

pub fn ok(body: serde_json::Value) -> Reply {
    (200, vec![], body.to_string())
}
