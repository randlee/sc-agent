//! [`serve_once`]: a loopback server for exactly one connection, every wait bounded.

use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use sc_agent_types::{CancelToken, Clock};

use crate::wait::{HANG_GUARD, join_within};

/// How often the accept loop looks for the connection. A deadline wait between two
/// non-blocking accepts, never a sleep.
const ACCEPT_POLL: Duration = Duration::from_millis(5);

/// A server started by [`serve_once`].
#[derive(Debug)]
pub struct Server {
    port: u16,
    handle: JoinHandle<()>,
}

impl Server {
    /// The loopback port it listens on.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Waits for the server thread to end, at most [`HANG_GUARD`].
    ///
    /// # Panics
    ///
    /// When the server thread panicked (its handler failed, or no client connected within the
    /// accept deadline), or did not end within the guard.
    #[track_caller]
    pub fn finish(self) {
        if let Err(panic) = join_within(self.handle, HANG_GUARD) {
            std::panic::resume_unwind(panic);
        }
    }
}

/// Binds `127.0.0.1:0` and runs `serve` on the first connection, on its own thread.
///
/// The accept has a deadline of [`HANG_GUARD`] (a non-blocking accept polled between deadline
/// waits), and the accepted stream has read and write timeouts of [`HANG_GUARD`], so neither
/// a client that never connects nor one that stops mid-request can hang the server. The
/// listener is held until the thread ends, so the port is never freed and reused while the
/// test runs.
///
/// # Panics
///
/// When the listener cannot be bound or set up, or the thread cannot be started.
#[track_caller]
pub fn serve_once(serve: impl FnOnce(TcpStream) + Send + 'static) -> Server {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind a loopback listener");
    let port = listener.local_addr().expect("listener address").port();
    listener
        .set_nonblocking(true)
        .expect("a non-blocking listener");
    let handle = thread::Builder::new()
        .name(format!("serve-once-{port}"))
        .spawn(move || {
            let stream = accept_within(&listener, HANG_GUARD);
            stream
                .set_nonblocking(false)
                .expect("a blocking accepted stream");
            stream
                .set_read_timeout(Some(HANG_GUARD))
                .expect("a read timeout");
            stream
                .set_write_timeout(Some(HANG_GUARD))
                .expect("a write timeout");
            serve(stream);
        })
        .expect("start the server thread");
    Server { port, handle }
}

/// The first connection on `listener` (non-blocking), within `guard`.
fn accept_within(listener: &TcpListener, guard: Duration) -> TcpStream {
    let clock = Clock::system();
    let never = CancelToken::new();
    let deadline = clock.now() + guard;
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                let now = clock.now();
                assert!(now < deadline, "no client connected within {guard:?}");
                let _ = clock.wait_until(&never, (now + ACCEPT_POLL).min(deadline));
            }
            Err(error) => panic!("accept failed: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;

    #[test]
    fn one_connection_is_served_and_the_thread_ends() {
        let server = serve_once(|mut stream| {
            let mut byte = [0_u8; 1];
            stream.read_exact(&mut byte).expect("read");
            stream.write_all(&byte).expect("echo");
        });
        let mut client = TcpStream::connect(("127.0.0.1", server.port())).expect("connect");
        client.write_all(b"x").expect("write");
        let mut echo = [0_u8; 1];
        client.read_exact(&mut echo).expect("read");
        assert_eq!(&echo, b"x");
        server.finish();
    }
}
