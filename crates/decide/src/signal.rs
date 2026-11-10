//! The interrupt hook (REQ-SAG-0011): SIGINT and SIGTERM (Ctrl-C and Ctrl-Break on Windows)
//! cancel the run's token.
//!
//! The `ctrlc` crate (feature `termination`) installs the handler and starts and owns the one
//! handler thread; decide's source starts no thread, uses no tokio and no `unsafe`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use sc_agent_types::CancelToken;

use crate::DecideError;

/// Whether an interrupt arrived; returned by [`install`].
#[derive(Debug, Clone)]
pub struct SignalWatch {
    interrupted: Arc<AtomicBool>,
}

impl SignalWatch {
    /// True once any interrupt was received; `decide` then exits 130.
    ///
    /// ctrlc does not report which signal fired, so SIGINT, SIGTERM, Ctrl-C and Ctrl-Break all
    /// exit 130 (no 143).
    #[must_use]
    pub fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::Acquire)
    }
}

/// Registers the process interrupt handler: each interrupt cancels `cancel`.
///
/// # Errors
///
/// [`DecideError::Signal`] when a handler is already set or the handler cannot be installed.
pub fn install(cancel: &CancelToken) -> Result<SignalWatch, DecideError> {
    let interrupted = Arc::new(AtomicBool::new(false));
    let watch = SignalWatch {
        interrupted: Arc::clone(&interrupted),
    };
    let cancel = cancel.clone();
    ctrlc::set_handler(move || on_interrupt(&cancel, &interrupted)).map_err(|error| {
        DecideError::Signal {
            detail: error.to_string(),
        }
    })?;
    Ok(watch)
}

/// What the handler does: record the interrupt, then cancel the token.
fn on_interrupt(cancel: &CancelToken, interrupted: &AtomicBool) {
    interrupted.store(true, Ordering::Release);
    cancel.cancel();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_interrupt_cancels_the_token_and_is_recorded() {
        let cancel = CancelToken::new();
        let watch = SignalWatch {
            interrupted: Arc::new(AtomicBool::new(false)),
        };
        assert!(!watch.interrupted());
        on_interrupt(&cancel, &watch.interrupted);
        assert!(cancel.is_cancelled());
        assert!(watch.interrupted());
    }
}
