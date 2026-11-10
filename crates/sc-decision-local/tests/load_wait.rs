//! REQ-SAG-0015: LocalProbe::wait_until_loaded and the first-request load deadline.
//!
//! Deterministic by design: the wait runs on a ManualClock, so no poll interval elapses on
//! its own. A test steps it: it waits until the probe of step n has been sent and the waiter is
//! parked on the clock, then advances the clock to the next poll. Probe times and waited
//! durations are then exact clock readings, and every wait for the waiter thread is bounded by
//! the testkit's hang guard. AC n = the nth acceptance line of the a-6 sprint doc.

#[cfg(test)]
#[expect(
    dead_code,
    reason = "each test binary uses only part of the shared helpers"
)]
mod support;

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use sc_agent_http::testing::ScriptedTransport;
    use sc_agent_http::{HttpError, HttpRequest, HttpResponse, LogContext, NetworkKind, Transport};
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_testkit::{HANG_GUARD, Started, join_within, recv_within};
    use sc_agent_types::testing::ManualClock;
    use sc_agent_types::{CancelToken, ErrorId};
    use sc_decision_local::{
        DaemonKind, LOAD_POLL_INTERVAL, LocalBackend, LocalProbe, LocalTimeouts,
    };
    use sc_decision_schema::{Backend, DecisionError};
    use serde_json::json;

    use crate::support::{decided, decision_request, json, loopback};

    const NOT_READY: &str = r#"{"ok":false}"#;
    const READY: &str = r#"{"ok":true,"model":"jev-latest"}"#;

    /// Answers each probe with the next scripted reply (the last repeats), stamps it with the
    /// manual clock, and signals each send.
    #[derive(Clone)]
    struct Stamped {
        clock: ManualClock,
        replies: Arc<Vec<Result<HttpResponse, HttpError>>>,
        stamps: Arc<Mutex<Vec<Instant>>>,
        sent: Started,
    }

    impl Stamped {
        fn new(clock: &ManualClock, replies: Vec<Result<HttpResponse, HttpError>>) -> Self {
            Self {
                clock: clock.clone(),
                replies: Arc::new(replies),
                stamps: Arc::default(),
                sent: Started::new(),
            }
        }

        /// Each probe's clock reading, as milliseconds after `start`.
        fn sent_at_ms(&self, start: Instant) -> Vec<u128> {
            self.stamps
                .lock()
                .expect("stamps")
                .iter()
                .map(|at| at.duration_since(start).as_millis())
                .collect()
        }
    }

    impl Transport for Stamped {
        fn send(&self, _request: &HttpRequest) -> Result<HttpResponse, HttpError> {
            let index = {
                let mut stamps = self.stamps.lock().expect("stamps");
                stamps.push(self.clock.now());
                stamps.len() - 1
            };
            self.sent.signal();
            self.replies
                .get(index)
                .or_else(|| self.replies.last())
                .cloned()
                .expect("a scripted reply")
        }
    }

    /// Runs the load wait of `backend` on its own thread, on `clock`; the result arrives on
    /// the returned channel.
    fn spawn_wait(
        backend: LocalBackend,
        transport: Stamped,
        log: Option<&MemoryLog>,
        cancel: CancelToken,
        load_timeout: Duration,
    ) -> (
        mpsc::Receiver<Result<(), DecisionError>>,
        thread::JoinHandle<()>,
    ) {
        let (sender, receiver) = mpsc::channel();
        let general = log.map(MemoryLog::general);
        let handle = thread::spawn(move || {
            let mut probe = LocalProbe::new(&backend, &transport, transport.clock.clock());
            if let Some(general) = general {
                probe = probe.with_log(general);
            }
            let _ = sender.send(probe.wait_until_loaded(&cancel, load_timeout, LogContext::NONE));
        });
        (receiver, handle)
    }

    /// Once probe `sends` has been sent and the waiter is parked on the clock, moves the clock
    /// by `by`. The old registration is gone before the next probe is sent, so the parked
    /// waiter seen here is the one waiting for the poll after probe `sends`.
    fn step(clock: &ManualClock, transport: &Stamped, sends: usize, by: Duration) {
        transport.sent.wait_for(sends);
        assert!(
            clock.wait_for_waiters(1),
            "the waiter parked after probe {sends}"
        );
        clock.advance(by);
    }

    /// AC 21: not loaded for 2 polls, loaded on the 3rd -> Ok after 3 probes at exactly 0, 1
    /// and 2 s of the clock; local.load_wait records in order, the end record waited 2000 ms.
    #[test]
    fn test_mapika_loaded_on_third_poll() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(LOAD_POLL_INTERVAL, Duration::from_secs(1));
        let clock = ManualClock::new();
        let start = clock.now();
        let log = MemoryLog::new()?;
        let transport = Stamped::new(
            &clock,
            vec![
                Ok(json(200, NOT_READY)),
                Ok(json(200, NOT_READY)),
                Ok(json(200, READY)),
            ],
        );
        let (receiver, handle) = spawn_wait(
            loopback(DaemonKind::Mapika, None),
            transport.clone(),
            Some(&log),
            CancelToken::new(),
            Duration::from_secs(30),
        );
        step(&clock, &transport, 1, LOAD_POLL_INTERVAL);
        step(&clock, &transport, 2, LOAD_POLL_INTERVAL);
        recv_within(&receiver, HANG_GUARD)?;
        join_within(handle, HANG_GUARD).expect("waiter");
        assert_eq!(transport.sent_at_ms(start), [0, 1000, 2000]);

        let records = log.records()?;
        assert!(
            records
                .iter()
                .all(|record| record.action == "local.load_wait")
        );
        let summary: Vec<_> = records
            .iter()
            .map(|record| {
                (
                    record.fields.get("phase").cloned(),
                    record.fields.get("poll").cloned(),
                    record.fields.get("state").cloned(),
                    record.fields.get("outcome").cloned(),
                    record.fields.get("waited_ms").cloned(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                (
                    Some(json!("poll")),
                    Some(json!(1)),
                    Some(json!("not_ready")),
                    None,
                    Some(json!(0))
                ),
                (
                    Some(json!("poll")),
                    Some(json!(2)),
                    Some(json!("not_ready")),
                    None,
                    Some(json!(1000))
                ),
                (
                    Some(json!("poll")),
                    Some(json!(3)),
                    Some(json!("loaded")),
                    None,
                    Some(json!(2000))
                ),
                (
                    Some(json!("end")),
                    None,
                    None,
                    Some(json!("loaded")),
                    Some(json!(2000))
                ),
            ]
        );
        let last = records.last().expect("end record");
        assert_eq!(last.fields.get("polls"), Some(&json!(3)));
        Ok(())
    }

    /// A connect failure (Strands Decider listens only after loading) means not loaded yet:
    /// the next probe follows one poll later.
    #[test]
    fn test_strands_connect_failure_keeps_polling() -> Result<(), Box<dyn std::error::Error>> {
        let clock = ManualClock::new();
        let start = clock.now();
        let transport = Stamped::new(
            &clock,
            vec![
                Err(HttpError::network(
                    NetworkKind::Connect,
                    1,
                    "connection refused",
                )),
                Ok(json(200, r#"{"status":"ok","model":"jev-latest"}"#)),
            ],
        );
        let (receiver, handle) = spawn_wait(
            loopback(DaemonKind::Strands, None),
            transport.clone(),
            None,
            CancelToken::new(),
            Duration::from_secs(30),
        );
        step(&clock, &transport, 1, LOAD_POLL_INTERVAL);
        recv_within(&receiver, HANG_GUARD)?;
        join_within(handle, HANG_GUARD).expect("waiter");
        assert_eq!(transport.sent_at_ms(start), [0, 1000]);
        Ok(())
    }

    /// The waited milliseconds a local.load_timeout message reports.
    fn waited_ms(message: &str) -> u64 {
        message
            .split_whitespace()
            .rev()
            .nth(1)
            .and_then(|ms| ms.parse().ok())
            .expect("waited ms in message")
    }

    /// AC 22: never ready with load_timeout 2 s -> decision.backend with cause
    /// local.load_timeout after probes at exactly 0, 1 and 2 s; waited exactly 2000 ms.
    #[test]
    fn test_mapika_never_ready_times_out() -> Result<(), Box<dyn std::error::Error>> {
        let clock = ManualClock::new();
        let start = clock.now();
        let transport = Stamped::new(&clock, vec![Ok(json(200, NOT_READY))]);
        let (receiver, handle) = spawn_wait(
            loopback(DaemonKind::Mapika, None),
            transport.clone(),
            None,
            CancelToken::new(),
            Duration::from_secs(2),
        );
        step(&clock, &transport, 1, LOAD_POLL_INTERVAL);
        step(&clock, &transport, 2, LOAD_POLL_INTERVAL);
        let error = recv_within(&receiver, HANG_GUARD).expect_err("timed out");
        join_within(handle, HANG_GUARD).expect("waiter");
        assert_eq!(error.id(), "decision.backend");
        assert!(matches!(
            error,
            DecisionError::Backend {
                backend: "local",
                ..
            }
        ));
        let cause = error.detail().cause.expect("cause");
        assert_eq!(cause.id, "local.load_timeout");
        assert!(
            cause.message.contains("http://127.0.0.1:8000"),
            "{}",
            cause.message
        );
        assert_eq!(waited_ms(&cause.message), 2000);
        assert!(cause.recovery.is_some());
        assert_eq!(transport.sent_at_ms(start), [0, 1000, 2000]);
        Ok(())
    }

    /// A timeout that is not a whole number of polls: the last probe is at the deadline, so
    /// probes go at exactly 0, 1 and 1.5 s and the wait ends at 1.5 s.
    #[test]
    fn test_timeout_between_polls_probes_at_deadline() -> Result<(), Box<dyn std::error::Error>> {
        let clock = ManualClock::new();
        let start = clock.now();
        let transport = Stamped::new(&clock, vec![Ok(json(200, NOT_READY))]);
        let (receiver, handle) = spawn_wait(
            loopback(DaemonKind::Mapika, None),
            transport.clone(),
            None,
            CancelToken::new(),
            Duration::from_millis(1500),
        );
        step(&clock, &transport, 1, LOAD_POLL_INTERVAL);
        step(&clock, &transport, 2, Duration::from_millis(500));
        let error = recv_within(&receiver, HANG_GUARD).expect_err("timed out");
        join_within(handle, HANG_GUARD).expect("waiter");
        let cause = error.detail().cause.expect("cause");
        assert_eq!(cause.id, "local.load_timeout");
        assert_eq!(waited_ms(&cause.message), 1500);
        assert_eq!(transport.sent_at_ms(start), [0, 1000, 1500]);
        Ok(())
    }

    /// AC 23: a cancel while the waiter is parked after the first poll ends the wait with
    /// decision.cancelled and no further probe. The clock never moves, so no poll can fire
    /// between the first probe and the cancel.
    #[test]
    fn test_cancel_after_first_poll() {
        let clock = ManualClock::new();
        let transport = Stamped::new(&clock, vec![Ok(json(200, NOT_READY))]);
        let cancel = CancelToken::new();
        let (receiver, handle) = spawn_wait(
            loopback(DaemonKind::Mapika, None),
            transport.clone(),
            None,
            cancel.clone(),
            Duration::from_secs(30),
        );
        transport.sent.wait_for(1);
        assert!(clock.wait_for_waiters(1), "the waiter parked after probe 1");
        cancel.cancel();
        let result = recv_within(&receiver, HANG_GUARD);
        join_within(handle, HANG_GUARD).expect("waiter");
        assert!(
            matches!(result, Err(DecisionError::Cancelled { sends: 0 })),
            "{result:?}"
        );
        assert_eq!(transport.sent.count(), 1);
    }

    /// A probe of `backend` on `transport`, on a manual clock that never moves (each of the
    /// tests below ends after its first probe).
    fn wait_once(
        backend: &LocalBackend,
        transport: &ScriptedTransport,
        cancel: &CancelToken,
        load_timeout: Duration,
    ) -> Result<(), DecisionError> {
        LocalProbe::new(backend, transport, ManualClock::new().clock()).wait_until_loaded(
            cancel,
            load_timeout,
            LogContext::NONE,
        )
    }

    /// A load timeout of Duration::MAX does not overflow the deadline: the wait still ends
    /// on a loaded model.
    #[test]
    fn test_huge_load_timeout_is_clamped() -> Result<(), Box<dyn std::error::Error>> {
        let transport = ScriptedTransport::responses(vec![json(200, READY)]);
        wait_once(
            &loopback(DaemonKind::Mapika, None),
            &transport,
            &CancelToken::new(),
            Duration::MAX,
        )?;
        assert_eq!(transport.sends(), 1);
        assert_eq!(
            sc_decision_local::MAX_LOAD_WAIT,
            Duration::from_secs(86_400)
        );
        Ok(())
    }

    /// A token cancelled before the wait sends no probe.
    #[test]
    fn test_cancelled_before_wait_sends_nothing() {
        let transport = ScriptedTransport::new(Vec::new());
        let cancel = CancelToken::new();
        cancel.cancel();
        let result = wait_once(
            &loopback(DaemonKind::Mapika, None),
            &transport,
            &cancel,
            Duration::from_secs(30),
        );
        assert!(matches!(result, Err(DecisionError::Cancelled { sends: 0 })));
        assert_eq!(transport.sends(), 0);
    }

    /// local.model_not_served ends the wait at once as decision.backend.
    #[test]
    fn test_model_not_served_ends_wait() {
        let transport =
            ScriptedTransport::responses(vec![json(200, r#"{"ok":true,"model":"other"}"#)]);
        let error = wait_once(
            &loopback(DaemonKind::Mapika, None),
            &transport,
            &CancelToken::new(),
            Duration::from_secs(30),
        )
        .expect_err("not served");
        assert_eq!(error.id(), "decision.backend");
        assert_eq!(
            error.detail().cause.map(|cause| cause.id),
            Some("local.model_not_served".to_owned())
        );
        assert_eq!(transport.sends(), 1);
    }

    /// A generic daemon without /v1/models cannot tell: Ok at once.
    #[test]
    fn test_generic_unlisted_is_ok_at_once() {
        let transport = ScriptedTransport::responses(vec![json(404, "{}")]);
        wait_once(
            &loopback(DaemonKind::Generic, None),
            &transport,
            &CancelToken::new(),
            Duration::from_secs(30),
        )
        .expect("unlisted is ok");
        assert_eq!(transport.sends(), 1);
    }

    /// AC 24: Laya `loaded: []` -> Ok with one probe; requests before the first 2xx carry
    /// deadline load + request, after it none.
    #[test]
    fn test_laya_loads_on_demand_first_request_deadline() -> Result<(), Box<dyn std::error::Error>>
    {
        let timeouts = LocalTimeouts {
            load: Duration::from_secs(600),
            request: Duration::from_secs(300),
        };
        let backend = loopback(DaemonKind::Laya, None).with_timeouts(timeouts);
        let transport =
            ScriptedTransport::responses(vec![json(200, r#"{"status":"ok","loaded":[]}"#)]);
        wait_once(&backend, &transport, &CancelToken::new(), timeouts.load)?;
        assert_eq!(transport.sends(), 1);

        let request = decision_request();
        let first = backend.encode(&request)?;
        let parallel = backend.encode(&request)?;
        assert_eq!(first.deadline(), Some(Duration::from_secs(900)));
        assert_eq!(parallel.deadline(), Some(Duration::from_secs(900)));

        // A refused reply does not count as served.
        let _ = backend.decode(
            json(503, r#"{"detail":"busy"}"#),
            &request,
            &LogContext::NONE,
        );
        assert_eq!(
            backend.encode(&request)?.deadline(),
            Some(Duration::from_secs(900))
        );

        backend.decode(decided(), &request, &LogContext::NONE)?;
        assert_eq!(backend.encode(&request)?.deadline(), None);
        Ok(())
    }

    /// Kinds that load at start, and Laya without timeouts, set no deadline.
    #[test]
    fn test_no_deadline_without_load_on_first_request() -> Result<(), Box<dyn std::error::Error>> {
        let timeouts = LocalTimeouts {
            load: Duration::from_secs(600),
            request: Duration::from_secs(300),
        };
        let request = decision_request();
        for kind in [DaemonKind::Strands, DaemonKind::Mapika, DaemonKind::Generic] {
            let backend = loopback(kind, None).with_timeouts(timeouts);
            assert_eq!(backend.encode(&request)?.deadline(), None, "{kind}");
        }
        assert_eq!(
            loopback(DaemonKind::Laya, None)
                .encode(&request)?
                .deadline(),
            None
        );
        Ok(())
    }
}
