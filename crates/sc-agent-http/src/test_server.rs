//! A loopback HTTP server that answers from a script, for deterministic tests. Each
//! connection gets the next scripted reply; every request is recorded.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

#[derive(Clone, Debug)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap()
    }
}

pub struct Server {
    pub base: String,
    seen: Arc<Mutex<Vec<Seen>>>,
    thread: Option<JoinHandle<()>>,
}

/// One reply: status, extra headers, body.
pub type Reply = (u16, Vec<(&'static str, String)>, String);

impl Server {
    /// Serves exactly `replies.len()` connections, in order.
    pub fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let thread = std::thread::spawn(move || {
            for (status, headers, body) in replies {
                let (stream, _) = listener.accept().unwrap();
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
                let len: usize = got.iter().find(|(n, _)| n.eq_ignore_ascii_case("content-length")).map_or(0, |(_, v)| v.parse().unwrap());
                let mut buf = vec![0; len];
                reader.read_exact(&mut buf).unwrap();
                log.lock().unwrap().push(Seen { method, path, headers: got, body: String::from_utf8(buf).unwrap() });
                let mut out = stream;
                let mut head = format!("HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n", body.len());
                for (n, v) in headers {
                    head.push_str(&format!("{n}: {v}\r\n"));
                }
                out.write_all(format!("{head}\r\n{body}").as_bytes()).unwrap();
            }
        });
        Self { base, seen, thread: Some(thread) }
    }

    /// Waits for every scripted reply to be served, then returns what was received.
    pub fn finish(mut self) -> Vec<Seen> {
        self.thread.take().unwrap().join().unwrap();
        self.seen.lock().unwrap().clone()
    }
}

pub fn ok(body: serde_json::Value) -> Reply {
    (200, vec![], body.to_string())
}
