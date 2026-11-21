//! [`recv_within`] and [`join_within`]: a receive and a join that cannot hang a test.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use sc_agent_types::{CancelToken, Clock, Wait};

/// How long any wait of a test lasts before the test fails instead of hanging.
///
/// A guard only: reaching it is a failure, and no assertion depends on how long anything took.
pub const HANG_GUARD: Duration = Duration::from_secs(60);

/// The next value on `receiver`, waiting at most `guard`.
///
/// # Panics
///
/// When nothing arrives within `guard`, or every sender is gone with nothing left to receive.
#[track_caller]
pub fn recv_within<T>(receiver: &Receiver<T>, guard: Duration) -> T {
    match receiver.recv_timeout(guard) {
        Ok(value) => value,
        Err(RecvTimeoutError::Timeout) => panic!("nothing was received within {guard:?}"),
        Err(RecvTimeoutError::Disconnected) => {
            panic!("every sender was dropped before a value was sent")
        }
    }
}

/// Joins `handle`, waiting at most `guard`; the thread's own result (`Err` when it panicked).
///
/// The join runs on a helper thread that hands the result back over a channel, so a thread
/// that never ends fails the test after `guard` (that thread and the helper are left behind).
///
/// # Panics
///
/// When the thread has not ended within `guard`, or the helper thread cannot be started.
#[track_caller]
pub fn join_within<T: Send + 'static>(handle: JoinHandle<T>, guard: Duration) -> thread::Result<T> {
    let name = handle.thread().name().unwrap_or("unnamed").to_owned();
    let (sender, receiver) = mpsc::channel();
    let helper = thread::Builder::new()
        .name(format!("join-within-{name}"))
        .spawn(move || {
            // The receiver may be gone after a timeout; the result is then dropped.
            let _ = sender.send(handle.join());
        });
    if let Err(error) = helper {
        panic!("could not start the join helper for thread {name}: {error}");
    }
    match receiver.recv_timeout(guard) {
        Ok(result) => result,
        Err(_) => panic!("thread {name} did not end within {guard:?}"),
    }
}

/// Waits until `token` is cancelled (used as a one-shot "done" latch), at most `guard`.
///
/// # Panics
///
/// When the token is still not cancelled after `guard`.
#[track_caller]
pub fn cancelled_within(token: &CancelToken, guard: Duration) {
    let clock = Clock::system();
    assert_eq!(
        clock.wait_until(token, clock.now() + guard),
        Wait::Cancelled,
        "the token was not cancelled within {guard:?}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_received_value_and_a_joined_result_come_back() {
        let (sender, receiver) = mpsc::channel();
        sender.send(7_u8).expect("send");
        assert_eq!(recv_within(&receiver, HANG_GUARD), 7);
        let worker = thread::spawn(|| 3_u8);
        assert_eq!(join_within(worker, HANG_GUARD).ok(), Some(3));
        let panicked = thread::spawn(|| panic!("expected in this test"));
        assert!(join_within(panicked, HANG_GUARD).is_err());
    }

    #[test]
    #[should_panic(expected = "every sender was dropped")]
    fn a_receive_from_a_closed_channel_panics_at_once() {
        let (sender, receiver) = mpsc::channel::<u8>();
        drop(sender);
        let _ = recv_within(&receiver, HANG_GUARD);
    }

    #[test]
    #[should_panic(expected = "nothing was received")]
    fn a_receive_with_nothing_sent_panics_at_the_guard() {
        let (_sender, receiver) = mpsc::channel::<u8>();
        let _ = recv_within(&receiver, Duration::ZERO);
    }
}
