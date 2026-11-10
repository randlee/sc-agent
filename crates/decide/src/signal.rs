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

    /// AC: REQ-SAG-0011 decide, NFR-SAG-0001. `on_interrupt` during a `send_batch` over a
    /// blocked transport cancels the token; every unfinished request ends `decision.cancelled`
    /// and the session log holds one row per started request.
    #[test]
    fn an_interrupt_during_a_batch_cancels_every_unfinished_request() {
        use sc_agent_http::HttpResponse;
        use sc_agent_http::testing::BlockingTransport;
        use sc_agent_log::testing::MemoryLog;
        use sc_agent_log::{Repo, SessionIdentity, Slug};
        use sc_decision_schema::DecisionError;

        let dir = tempfile::tempdir().expect("dir");
        let transport = BlockingTransport::new(Ok(HttpResponse::new(200, Vec::new(), Vec::new())));
        let log = MemoryLog::new().expect("log");
        let context = crate::Context {
            service: "decide",
            config_dir: Some(dir.path().to_path_buf()),
            environment: Some(vec![("TYPESAFE_API_KEY".into(), "ts-signal-test".into())]),
            keyring: None,
            transport: Some(Arc::new(transport.clone())),
            general: Some(log.general()),
        };
        let identity =
            SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "proj", "role")
                .expect("identity");
        let options = crate::SendOptions {
            model: crate::ModelSelector::Short(sc_agent_types::ShortName::Jev),
            provider: None,
            project: Slug::new("project", "proj").expect("slug"),
            agent_role: Slug::new("agent_role", "role").expect("slug"),
            context,
            session: Some(log.session(identity)),
        };
        let json = br#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}"#;
        let requests = (0..12)
            .map(|_| crate::input::build_request(json, &[], false, None).expect("request"))
            .collect();
        let cancel = CancelToken::new();
        let interrupted = Arc::new(AtomicBool::new(false));
        let handler = {
            let (cancel, interrupted, transport) =
                (cancel.clone(), Arc::clone(&interrupted), transport.clone());
            std::thread::spawn(move || {
                transport.wait_started(8);
                on_interrupt(&cancel, &interrupted);
            })
        };
        let mut results = Vec::new();
        crate::send_batch(&options, requests, &cancel, &mut |result| {
            results.push(result);
            Ok(())
        })
        .expect("batch");
        handler.join().expect("handler");
        transport.release();
        assert!(interrupted.load(Ordering::Acquire));
        assert_eq!(results.len(), 12);
        assert!(
            results
                .iter()
                .all(|result| matches!(result.outcome, Err(DecisionError::Cancelled { .. })))
        );
        let rows = log
            .records()
            .expect("records")
            .into_iter()
            .filter(|record| record.action == "session.row")
            .count();
        assert_eq!(rows, 12, "one row per started request");
    }
}
