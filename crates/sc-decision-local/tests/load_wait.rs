//! REQ-SAG-0015: LocalBackend::wait_until_loaded and the first-request load deadline.
//!
//! Real 1 s poll interval (CancelToken::wait_until, no sleep); each test finishes in under
//! 5 s, and outcomes that must arrive in time are read through a channel bounded at 5 s.
//! AC n = the nth acceptance line of the a-6 sprint doc.

#[cfg(test)]
#[expect(
    dead_code,
    reason = "each test binary uses only part of the shared helpers"
)]
mod support;

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use sc_agent_http::testing::{BlockingTransport, ScriptedTransport};
    use sc_agent_http::{HttpError, LogContext, NetworkKind, Transport};
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_types::{CancelToken, ErrorId};
    use sc_decision_local::{DaemonKind, LOAD_POLL_INTERVAL, LocalBackend, LocalTimeouts};
    use sc_decision_schema::{Backend, DecisionError};
    use serde_json::json;

    use crate::support::{decided, decision_request, json, loopback};

    const BOUND: Duration = Duration::from_secs(5);
    const NOT_READY: &str = r#"{"ok":false}"#;
    const READY: &str = r#"{"ok":true,"model":"jev-latest"}"#;

    /// Runs `wait_until_loaded` on another thread; the result arrives on the channel.
    fn spawn_wait(
        backend: LocalBackend,
        transport: Arc<dyn Transport>,
        cancel: CancelToken,
        load_timeout: Duration,
    ) -> mpsc::Receiver<(Result<(), DecisionError>, Duration)> {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let start = Instant::now();
            let result =
                backend.wait_until_loaded(&*transport, &cancel, load_timeout, LogContext::NONE);
            let _ = sender.send((result, start.elapsed()));
        });
        receiver
    }

    /// AC 21: not loaded for 2 polls, loaded on the 3rd -> Ok after 3 probes, 1 s apart;
    /// local.load_wait records in order.
    #[test]
    fn test_mapika_loaded_on_third_poll() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(LOAD_POLL_INTERVAL, Duration::from_secs(1));
        let log = MemoryLog::new()?;
        let backend = loopback(DaemonKind::Mapika, None).with_log(log.general());
        let transport = ScriptedTransport::responses(vec![
            json(200, NOT_READY),
            json(200, NOT_READY),
            json(200, READY),
        ]);
        let receiver = spawn_wait(
            backend,
            Arc::new(transport.clone()),
            CancelToken::new(),
            Duration::from_secs(30),
        );
        let (result, elapsed) = receiver.recv_timeout(BOUND)?;
        result?;
        assert_eq!(transport.sends(), 3);
        assert!(elapsed >= 2 * LOAD_POLL_INTERVAL, "{elapsed:?}");

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
                    None
                ),
                (
                    Some(json!("poll")),
                    Some(json!(2)),
                    Some(json!("not_ready")),
                    None
                ),
                (
                    Some(json!("poll")),
                    Some(json!(3)),
                    Some(json!("loaded")),
                    None
                ),
                (Some(json!("end")), None, None, Some(json!("loaded"))),
            ]
        );
        let last = records.last().expect("end record");
        assert_eq!(last.fields.get("polls"), Some(&json!(3)));
        assert!(
            last.fields
                .get("waited_ms")
                .and_then(serde_json::Value::as_u64)
                >= Some(2000)
        );
        Ok(())
    }

    /// A connect failure (Strands Decider listens only after loading) means not loaded yet.
    #[test]
    fn test_strands_connect_failure_keeps_polling() -> Result<(), Box<dyn std::error::Error>> {
        let transport = ScriptedTransport::new(vec![
            Err(HttpError::network(
                NetworkKind::Connect,
                1,
                "connection refused",
            )),
            Ok(json(200, r#"{"status":"ok","model":"jev-latest"}"#)),
        ]);
        let receiver = spawn_wait(
            loopback(DaemonKind::Strands, None),
            Arc::new(transport.clone()),
            CancelToken::new(),
            Duration::from_secs(30),
        );
        let (result, _) = receiver.recv_timeout(BOUND)?;
        result?;
        assert_eq!(transport.sends(), 2);
        Ok(())
    }

    /// AC 22: never ready with load_timeout 2 s -> decision.backend with cause
    /// local.load_timeout, waited >= 2 s.
    #[test]
    fn test_mapika_never_ready_times_out() -> Result<(), Box<dyn std::error::Error>> {
        let transport = ScriptedTransport::responses(vec![json(200, NOT_READY); 5]);
        let backend = loopback(DaemonKind::Mapika, None);
        let receiver = spawn_wait(
            backend,
            Arc::new(transport.clone()),
            CancelToken::new(),
            Duration::from_secs(2),
        );
        let (result, elapsed) = receiver.recv_timeout(BOUND)?;
        let error = result.expect_err("timed out");
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
        let waited_ms: u64 = cause
            .message
            .split_whitespace()
            .rev()
            .nth(1)
            .and_then(|ms| ms.parse().ok())
            .expect("waited ms in message");
        assert!(waited_ms >= 2000, "{waited_ms}");
        assert!(elapsed >= Duration::from_secs(2));
        assert!(cause.recovery.is_some());
        assert_eq!(transport.sends(), 3, "probes at 0, 1 and 2 s");
        Ok(())
    }

    /// A timeout that is not a whole number of polls: the last probe is at the deadline
    /// (0, 1 and 1.5 s), so the wait ends before 2 s after 3 probes.
    #[test]
    fn test_timeout_between_polls_probes_at_deadline() -> Result<(), Box<dyn std::error::Error>> {
        let transport = ScriptedTransport::responses(vec![json(200, NOT_READY); 5]);
        let receiver = spawn_wait(
            loopback(DaemonKind::Mapika, None),
            Arc::new(transport.clone()),
            CancelToken::new(),
            Duration::from_millis(1500),
        );
        let (result, elapsed) = receiver.recv_timeout(BOUND)?;
        let error = result.expect_err("timed out");
        assert_eq!(
            error.detail().cause.map(|cause| cause.id),
            Some("local.load_timeout".to_owned())
        );
        assert_eq!(transport.sends(), 3);
        assert!(elapsed >= Duration::from_millis(1500), "{elapsed:?}");
        assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
        Ok(())
    }

    /// A load timeout of Duration::MAX does not overflow the deadline: the wait still ends
    /// on a loaded model.
    #[test]
    fn test_huge_load_timeout_is_clamped() -> Result<(), Box<dyn std::error::Error>> {
        let transport = ScriptedTransport::responses(vec![json(200, READY)]);
        loopback(DaemonKind::Mapika, None).wait_until_loaded(
            &transport,
            &CancelToken::new(),
            Duration::MAX,
            LogContext::NONE,
        )?;
        assert_eq!(transport.sends(), 1);
        assert_eq!(
            sc_decision_local::MAX_LOAD_WAIT,
            Duration::from_secs(86_400)
        );
        Ok(())
    }

    /// AC 23: cancel from another thread after the first poll -> decision.cancelled at once,
    /// no further probe.
    #[test]
    fn test_cancel_after_first_poll() -> Result<(), Box<dyn std::error::Error>> {
        let transport = BlockingTransport::new(Ok(json(200, NOT_READY)));
        let cancel = CancelToken::new();
        let receiver = spawn_wait(
            loopback(DaemonKind::Mapika, None),
            Arc::new(transport.clone()),
            cancel.clone(),
            Duration::from_secs(30),
        );
        assert!(transport.wait_started(1), "sends started");
        transport.release();
        cancel.cancel();
        let (result, elapsed) = receiver.recv_timeout(BOUND)?;
        assert!(
            matches!(result, Err(DecisionError::Cancelled { sends: 0 })),
            "{result:?}"
        );
        assert_eq!(transport.sends(), 1);
        assert!(elapsed < LOAD_POLL_INTERVAL * 2, "{elapsed:?}");
        Ok(())
    }

    /// A token cancelled before the wait sends no probe.
    #[test]
    fn test_cancelled_before_wait_sends_nothing() {
        let transport = ScriptedTransport::new(Vec::new());
        let cancel = CancelToken::new();
        cancel.cancel();
        let result = loopback(DaemonKind::Mapika, None).wait_until_loaded(
            &transport,
            &cancel,
            Duration::from_secs(30),
            LogContext::NONE,
        );
        assert!(matches!(result, Err(DecisionError::Cancelled { sends: 0 })));
        assert_eq!(transport.sends(), 0);
    }

    /// local.model_not_served ends the wait at once as decision.backend.
    #[test]
    fn test_model_not_served_ends_wait() {
        let transport =
            ScriptedTransport::responses(vec![json(200, r#"{"ok":true,"model":"other"}"#)]);
        let error = loopback(DaemonKind::Mapika, None)
            .wait_until_loaded(
                &transport,
                &CancelToken::new(),
                Duration::from_secs(30),
                LogContext::NONE,
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
        loopback(DaemonKind::Generic, None)
            .wait_until_loaded(
                &transport,
                &CancelToken::new(),
                Duration::from_secs(30),
                LogContext::NONE,
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
        backend.wait_until_loaded(
            &transport,
            &CancelToken::new(),
            timeouts.load,
            LogContext::NONE,
        )?;
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
