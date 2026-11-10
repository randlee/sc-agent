//! The interrupt hook (REQ-SAG-0011): SIGINT and SIGTERM (Ctrl-C and Ctrl-Break on Windows).
//!
//! The `ctrlc` crate (feature `termination`) installs the handler and starts and owns the one
//! handler thread; decide's source starts no thread, uses no tokio and no `unsafe`.
//!
//! Two phases. Before [`SignalWatch::start_run`] decide is reading its input (a file, stdin or
//! a FIFO, where a read can block forever) and nothing has been sent, so an interrupt exits
//! the process at once with [`EXIT_INTERRUPTED`] (RSH-001). After it, an interrupt cancels the
//! run's token: unfinished requests end `decision.cancelled`, are printed, and the session log
//! is flushed before `main` exits 130. A second or later interrupt changes nothing: the cancel
//! is idempotent and the drain is already bounded, so every exit after a run starts goes
//! through the flush (REQ-SAG-0011).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use sc_agent_types::CancelToken;

use crate::DecideError;

/// The exit code after any interrupt. ctrlc does not report which signal fired, so SIGINT,
/// SIGTERM, Ctrl-C and Ctrl-Break all exit 130 (no 143).
pub const EXIT_INTERRUPTED: i32 = 130;

/// What the interrupts so far mean for the process.
#[derive(Debug, Default)]
struct State {
    /// Interrupts received.
    interrupts: AtomicU32,
    /// Whether requests may be in flight (`start_run` was called).
    running: AtomicBool,
}

/// What the handler does with one interrupt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Cancel the token (again, for a later interrupt) and let `main` drain, flush and exit 130.
    Cancel,
    /// Exit the process now with 130: only before the run starts, when nothing was sent.
    Exit,
}

/// Whether an interrupt arrived; returned by [`install`].
#[derive(Debug, Clone)]
pub struct SignalWatch {
    state: Arc<State>,
}

impl SignalWatch {
    /// True once any interrupt was received; `decide` then exits 130.
    #[must_use]
    pub fn interrupted(&self) -> bool {
        self.state.interrupts.load(Ordering::Acquire) > 0
    }

    /// The input is read and the run starts: from now on every interrupt cancels and lets the
    /// run drain and flush; none exits at once.
    pub fn start_run(&self) {
        self.state.running.store(true, Ordering::Release);
    }
}

/// Registers the process interrupt handler for `cancel`.
///
/// # Errors
///
/// [`DecideError::Signal`] when a handler is already set or cannot be installed.
pub fn install(cancel: &CancelToken) -> Result<SignalWatch, DecideError> {
    let state = Arc::new(State::default());
    let watch = SignalWatch {
        state: Arc::clone(&state),
    };
    let cancel = cancel.clone();
    ctrlc::set_handler(move || {
        if on_interrupt(&cancel, &state) == Action::Exit {
            std::process::exit(EXIT_INTERRUPTED);
        }
    })
    .map_err(|source| DecideError::Signal { source })?;
    Ok(watch)
}

/// What the handler does: count the interrupt, cancel the token, and exit now only before the
/// run starts.
fn on_interrupt(cancel: &CancelToken, state: &State) -> Action {
    state.interrupts.fetch_add(1, Ordering::AcqRel);
    cancel.cancel();
    if state.running.load(Ordering::Acquire) {
        Action::Cancel
    } else {
        Action::Exit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RSH-001: while the input is read, an interrupt exits at once.
    #[test]
    fn an_interrupt_while_reading_exits_at_once() {
        let cancel = CancelToken::new();
        let watch = SignalWatch {
            state: Arc::new(State::default()),
        };
        assert!(!watch.interrupted());
        assert_eq!(on_interrupt(&cancel, &watch.state), Action::Exit);
        assert!(cancel.is_cancelled());
        assert!(watch.interrupted());
    }

    /// Runs a 12-request batch whose 8th send (all 8 workers then blocked) delivers
    /// `interrupts` interrupts to the handler before it blocks too; returns the handler's
    /// actions, the results and the session-row count. Ordering is forced by the transport
    /// itself (the interrupts arrive inside the 8th send); no thread of the test's own, no
    /// timing.
    fn interrupted_batch(
        interrupts: usize,
    ) -> (Vec<Action>, Vec<sc_decision_schema::DecisionResult>, usize) {
        use sc_agent_http::HttpResponse;
        use sc_agent_http::testing::BlockingTransport;
        use sc_agent_log::testing::MemoryLog;
        use sc_agent_log::{Repo, SessionIdentity, Slug};

        // The interrupt arrives while 8 sends are blocked: the 8th send runs the handler on
        // its worker thread (no thread of this test's own), then blocks like the others.
        struct InterruptAt {
            sends: std::sync::atomic::AtomicUsize,
            cancel: CancelToken,
            state: Arc<State>,
            interrupts: usize,
            actions: std::sync::Mutex<Vec<Action>>,
            inner: BlockingTransport,
        }
        impl sc_agent_http::Transport for InterruptAt {
            fn send(
                &self,
                request: &sc_agent_http::HttpRequest,
            ) -> Result<HttpResponse, sc_agent_http::HttpError> {
                if self.sends.fetch_add(1, Ordering::SeqCst) + 1 == 8 {
                    for _ in 0..self.interrupts {
                        let action = on_interrupt(&self.cancel, &self.state);
                        self.actions.lock().expect("lock").push(action);
                    }
                }
                self.inner.send(request)
            }
        }

        let dir = tempfile::tempdir().expect("dir");
        let transport = BlockingTransport::new(Ok(HttpResponse::new(200, Vec::new(), Vec::new())));
        let log = MemoryLog::new().expect("log");
        let cancel = CancelToken::new();
        let watch = SignalWatch {
            state: Arc::new(State::default()),
        };
        watch.start_run();
        let interrupting = Arc::new(InterruptAt {
            sends: std::sync::atomic::AtomicUsize::new(0),
            cancel: cancel.clone(),
            state: Arc::clone(&watch.state),
            interrupts,
            actions: std::sync::Mutex::new(Vec::new()),
            inner: transport.clone(),
        });
        let context = crate::Context {
            service: "decide",
            config_dir: Some(dir.path().to_path_buf()),
            repo_root: Some(dir.path().to_path_buf()),
            environment: Some(vec![(
                sc_agent_secrets::TYPESAFE_API_KEY.into(),
                "ts-signal-test".into(),
            )]),
            keyring: None,
            transport: Some(interrupting.clone()),
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
        let mut results = Vec::new();
        crate::send_batch(&options, requests, &cancel, &mut |result| {
            results.push(result);
            Ok(())
        })
        .expect("batch");
        transport.release();
        assert!(watch.interrupted());
        let rows = log
            .records()
            .expect("records")
            .into_iter()
            .filter(|record| record.action == "session.row")
            .count();
        let actions = interrupting.actions.lock().expect("lock").clone();
        (actions, results, rows)
    }

    fn all_cancelled(results: &[sc_decision_schema::DecisionResult]) -> bool {
        results.iter().all(|result| {
            matches!(
                result.outcome,
                Err(sc_decision_schema::DecisionError::Cancelled { .. })
            )
        })
    }

    /// AC: REQ-SAG-0011 decide, NFR-SAG-0001. An interrupt during a `send_batch` over a blocked
    /// transport cancels the token; every unfinished request ends `decision.cancelled` and the
    /// session log holds one row per started request.
    #[test]
    fn an_interrupt_during_a_batch_cancels_every_unfinished_request() {
        let (actions, results, rows) = interrupted_batch(1);
        assert_eq!(actions, [Action::Cancel]);
        assert_eq!(results.len(), 12);
        assert!(all_cancelled(&results));
        assert_eq!(rows, 12, "one row per started request");
    }

    /// REQ-SAG-0011 (REQ-QA-002): a second interrupt during a run does not exit at once; the
    /// run still drains every request as cancelled and flushes one row per started request.
    #[test]
    fn a_second_interrupt_of_a_run_still_drains_and_flushes() {
        let (actions, results, rows) = interrupted_batch(2);
        assert_eq!(
            actions,
            [Action::Cancel, Action::Cancel],
            "no immediate exit"
        );
        assert_eq!(results.len(), 12);
        assert!(all_cancelled(&results));
        assert_eq!(rows, 12, "one row per started request, flushed");
    }
}
