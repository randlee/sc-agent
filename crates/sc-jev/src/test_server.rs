//! Minimal in-process HTTP/1.1 server for transport tests.
//!
//! Copied verbatim into `sc-jev` and `sc-jev-backends`. The production
//! transports are HTTPS-only; tests build them with a loopback-only
//! constructor that allows `http://127.0.0.1`, so everything except the TLS
//! handshake runs through the real ureq agent.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// What the server does with one accepted connection.
pub(crate) enum Reply {
    /// Writes these bytes after reading the request, then closes.
    Raw(Vec<u8>),
    /// Reads the request, waits this long without answering, then closes.
    Stall(Duration),
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
    handle: JoinHandle<Vec<Seen>>,
}

impl LoopbackServer {
    /// Binds `127.0.0.1:0` and serves `replies` on a background thread.
    pub(crate) fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let mut seen = Vec::new();
            for reply in replies {
                let Ok((stream, _)) = listener.accept() else {
                    break;
                };
                seen.push(serve_one(stream, reply));
            }
            seen
        });
        Self { addr, handle }
    }

    /// `http://127.0.0.1:{port}{path}`.
    pub(crate) fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    /// Waits for every scripted reply and returns the requests seen.
    pub(crate) fn finish(self) -> Vec<Seen> {
        self.handle.join().expect("server thread")
    }
}

/// A loopback URL nothing listens on.
pub(crate) fn closed_port_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("addr");
    drop(listener);
    format!("http://{addr}/closed")
}

fn serve_one(stream: TcpStream, reply: Reply) -> Seen {
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
        Reply::Stall(wait) => thread::sleep(wait),
    }
    Seen { head, body }
}
