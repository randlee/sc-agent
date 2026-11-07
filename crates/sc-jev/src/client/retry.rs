//! Busy-reply wait.
//!
//! [`RetryDelay`] is intentionally open so tests can record waits without
//! sleeping. Production uses [`ThreadRetryDelay`].

use std::thread;
use std::time::Duration;

/// Waits between busy retries.
///
/// Object-safe. The client calls this only after HTTP 429, 502, or 529, and
/// only when another attempt and retry budget remain. `delay` already
/// includes the clamp and jitter from the busy-retry policy.
pub trait RetryDelay: Send + Sync {
    /// Waits for `delay` before the next send of the same body.
    fn wait(&self, delay: Duration);
}

/// Sleeps the calling thread.
#[derive(Debug, Default, Clone, Copy)]
pub struct ThreadRetryDelay;

impl RetryDelay for ThreadRetryDelay {
    fn wait(&self, delay: Duration) {
        thread::sleep(delay);
    }
}
