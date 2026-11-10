//! Cooperative cancellation shared by a run and its backends.
//!
//! Std-only and always on, so transports can honour it without any
//! feature.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A flag a caller sets to ask in-flight work to stop.
///
/// Clones share one flag. [`Default`] is a token that is never cancelled
/// unless [`Self::cancel`] is called on it or a clone. Two tokens are equal
/// when they share the same flag.
///
/// # Examples
///
/// ```
/// use sc_agent_types::CancelToken;
///
/// let token = CancelToken::default();
/// let seen_by_backend = token.clone();
/// assert!(!seen_by_backend.is_cancelled());
/// token.cancel();
/// assert!(seen_by_backend.is_cancelled());
/// ```
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A fresh token that is not cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Idempotent.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl PartialEq for CancelToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for CancelToken {}
