//! Busy-reply wait.
//!
//! [`RetryDelay`] is intentionally open so tests can record waits without
//! sleeping. Production uses [`ThreadRetryDelay`].

use std::thread;
use std::time::Duration;

/// Waits between busy retries.
///
/// Object-safe. The client calls this only after HTTP 429, 502, or 529, and
/// only when another attempt remains.
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

/// `Retry-After` seconds clamped to `[0, 30]`.
///
/// Missing or empty means 1 second. Unparsable text means 30 seconds.
pub(crate) fn retry_delay(header: Option<&str>) -> Duration {
    let seconds = match header {
        None | Some("") => 1.0,
        Some(text) => match text.trim().parse::<f64>() {
            Ok(value) if value.is_finite() => value.clamp(0.0, 30.0),
            _ => 30.0,
        },
    };
    Duration::from_secs_f64(seconds)
}
