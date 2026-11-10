//! REQ-SAG-0601 / 0602 resend, REQ-SAG-0601 AC4 cancel during holdoff, REQ-SAG-0012
//! (runner side) provider pause.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sc_agent_http::testing::ScriptedTransport;
use sc_agent_http::{HttpError, HttpResponse, NetworkKind};
use sc_agent_types::{CancelToken, ErrorId};
use sc_decision_schema::{Decision, DecisionError};

use crate::support::scripted_backend::encode_to;
use crate::support::*;

/// A backend sending `m<i>` to host `h<i>.test`, so holdoffs of one request do not pause
/// the others (the provider pause is per host).
fn host_per_request() -> ScriptedBackend {
    ScriptedBackend::new().with_encode(|request| {
        let model = &request.body().model;
        encode_to(&format!("https://h{model}.test/v1/systemone"), request)
    })
}

/// REQ-SAG-0601: 529 four times then 200 yields the answer after 5 sends; encode once.
#[test]
fn busy_four_then_ok() {
    let transport =
        ScriptedTransport::responses(vec![busy("0"), busy("0"), busy("0"), busy("0"), ok()]);
    let backend = Arc::new(ScriptedBackend::new());
    let harness = Harness::new(
        backend.clone(),
        Arc::new(transport.clone()),
        config(1, 4),
        5,
    );
    let result = harness.runner.decide(request("m0"), &CancelToken::new());
    assert!(result.outcome.is_ok(), "{result:?}");
    assert_eq!(transport.sends(), 5);
    assert_eq!(backend.encodes(), 1, "encoded once");
    let rows = harness.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].fields["sends"], 5);
    assert_eq!(rows[0].fields["outcome"], "ok");
}

/// REQ-SAG-0601: every send carries the same encoded bytes.
#[test]
fn same_bytes_each_send() {
    let transport = ScriptedTransport::responses(vec![busy("0"), busy("0"), ok()]);
    let backend = Arc::new(ScriptedBackend::new());
    let harness = Harness::new(
        backend.clone(),
        Arc::new(transport.clone()),
        config(1, 4),
        5,
    );
    let result = harness.runner.decide(request("m0"), &CancelToken::new());
    assert!(result.outcome.is_ok(), "{result:?}");
    let sent = transport.requests();
    assert_eq!(sent.len(), 3);
    for request in &sent {
        assert_eq!(request.body(), sent[0].body());
        assert_eq!(request.endpoint(), sent[0].endpoint());
        assert_eq!(request.headers(), sent[0].headers());
    }
    assert_eq!(backend.encodes(), 1);
}

/// REQ-SAG-0601: 529 every time stops after max_sends (5) sends with http.provider_busy.
#[test]
fn busy_every_time_stops_at_max_sends() {
    let transport = ScriptedTransport::responses((0..8).map(|_| busy("0")).collect());
    let harness = Harness::of(Arc::new(transport.clone()));
    let result = harness.runner.decide(request("m0"), &CancelToken::new());
    match &result.outcome {
        Err(DecisionError::Http(HttpError::ProviderBusy { attempts, .. })) => {
            assert_eq!(*attempts, 5)
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(transport.sends(), 5);
    assert_eq!(harness.rows()[0].fields["sends"], 5);
}

/// REQ-SAG-0601: a held-off request waits in the queue, not in a worker: with one
/// worker, a second request is answered during the first one's holdoff.
#[test]
fn holdoff_frees_the_worker() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&order);
    let (transport, _dropped) = FnTransport::new(move |request| {
        let index = index_of(request);
        let mut order = lock(&seen);
        let first = !order.contains(&index);
        order.push(index);
        Ok(if index == 0 && first { busy("1") } else { ok() })
    });
    let harness = Harness::new(
        Arc::new(host_per_request()),
        Arc::new(transport),
        config(1, 4),
        5,
    );
    let results = harness.stream(requests(2), &CancelToken::new());
    let all = take(&results, 2);
    assert_eq!(all[0].position, 1, "answered during the holdoff");
    assert_eq!(all[1].position, 0);
    assert!(all.iter().all(|result| result.outcome.is_ok()));
    assert_eq!(*lock(&order), vec![0, 1, 0]);
}

/// REQ-SAG-0601 AC4 (decide_stream): cancelling a held-off request ends its 30 s wait at
/// once. One worker: request 1's send starts only once request 0 is held.
#[test]
fn cancel_ends_holdoff_at_once() {
    let gate = Gate::default();
    let held = gate.clone();
    let (transport, dropped) = FnTransport::new(move |request| {
        if index_of(request) == 0 {
            Ok(busy("30"))
        } else {
            held.pass();
            Ok(ok())
        }
    });
    let harness = Harness::new(
        Arc::new(host_per_request()),
        Arc::new(transport),
        config(1, 4),
        5,
    );
    let cancel = CancelToken::new();
    let results = harness.stream(requests(2), &cancel);
    gate.wait_inside(1);
    let cancelled_at = Instant::now();
    cancel.cancel();
    let all = take(&results, 2);
    assert!(cancelled_at.elapsed() < FAILSAFE);
    for result in &all {
        assert!(
            matches!(result.outcome, Err(DecisionError::Cancelled { sends: 1 })),
            "{result:?}"
        );
    }
    gate.open();
    let log = harness.finish(&dropped);
    assert_eq!(rows_of(&log).len(), 2);
}

/// REQ-SAG-0601 AC4 (decide): cancel after a 30 s busy reply ends the decide at once.
#[test]
fn decide_cancel_during_holdoff() {
    let (busy_sent, on_busy) = mpsc::channel();
    let busy_sent = Mutex::new(busy_sent);
    let (transport, dropped) = FnTransport::new(move |_| {
        let _ = lock(&busy_sent).send(());
        Ok(busy("30"))
    });
    let harness = Harness::of(Arc::new(transport));
    let cancel = CancelToken::new();
    let result = harness.decide(request("m0"), &cancel);
    on_busy
        .recv_timeout(FAILSAFE)
        .expect("the busy reply was sent");
    cancel.cancel();
    let result = result
        .recv_timeout(FAILSAFE)
        .expect("cancelled within 2 s, not 30 s");
    assert!(
        matches!(result.outcome, Err(DecisionError::Cancelled { sends: 1 })),
        "{result:?}"
    );
    let log = harness.finish(&dropped);
    assert_eq!(rows_of(&log).len(), 1);
}

/// The ids of an error and every cause below it.
fn chain(error: &DecisionError) -> Vec<String> {
    let mut ids = Vec::new();
    let mut detail = Some(error.detail());
    while let Some(level) = detail {
        ids.push(level.id.clone());
        detail = level.cause.map(|cause| *cause);
    }
    ids
}

/// REQ-SAG-0601/0602: quota, provider busy and network errors come back unchanged inside
/// decision.http; only ResendAt is resent.
#[test]
fn busy_quota_network_passed_through() {
    let sends = Arc::new(Mutex::new(HashMap::<usize, usize>::new()));
    let counted = Arc::clone(&sends);
    let (transport, _dropped) = FnTransport::new(move |request| {
        let index = index_of(request);
        *lock(&counted).entry(index).or_default() += 1;
        match index {
            0 => Ok(HttpResponse::new(
                429,
                vec![],
                br#"{"error":{"message":"out of quota","type":"insufficient_quota","code":"insufficient_quota"}}"#.to_vec(),
            )),
            1 => Ok(busy("0")),
            _ => Err(HttpError::network(NetworkKind::Connect, 1, "connection refused")),
        }
    });
    let harness = Harness::new(
        Arc::new(host_per_request()),
        Arc::new(transport),
        config(3, 12),
        2,
    );
    let mut results: Vec<_> = harness
        .runner
        .decide_stream(Box::new(requests(3).into_iter()), &CancelToken::new())
        .collect();
    results.sort_by_key(|result| result.position);
    let ids: Vec<Vec<String>> = results
        .iter()
        .map(|result| chain(result.outcome.as_ref().expect_err("failed")))
        .collect();
    assert_eq!(ids[0][..2], ["decision.http", "http.quota_exhausted"]);
    assert_eq!(ids[1][..2], ["decision.http", "http.provider_busy"]);
    assert_eq!(ids[2][..2], ["decision.http", "http.network"]);
    let sends = lock(&sends);
    assert_eq!(sends[&0], 1, "quota is never resent");
    assert_eq!(sends[&1], 2, "busy is resent up to max_sends (2)");
    assert_eq!(sends[&2], 1, "a connect failure is not resent");
}

/// REQ-SAG-0012 (runner side): A's 529 Retry-After: 1 pauses the host for every job of
/// the one HttpClient; B's next send reaches the transport no earlier than A's pause end.
#[test]
fn provider_pause_holds_other_jobs() {
    #[derive(Default)]
    struct Calls {
        /// (index, entered, returned) per call.
        log: Vec<(usize, Instant, Instant)>,
        a_entered: bool,
    }
    let calls = Arc::new((Mutex::new(Calls::default()), std::sync::Condvar::new()));
    let recorded = Arc::clone(&calls);
    let (transport, _dropped) = FnTransport::new(move |request| {
        let index = index_of(request);
        let entered = Instant::now();
        let (mutex, condvar) = &*recorded;
        let mut state = lock(mutex);
        let first = !state.log.iter().any(|(seen, _, _)| *seen == index);
        let reply = match (index, first) {
            (0, true) => {
                state.a_entered = true;
                condvar.notify_all();
                busy("1")
            }
            (1, true) => {
                // B's first send returns only after A's has started.
                let (waited, _) = condvar
                    .wait_timeout_while(state, FAILSAFE, |calls| !calls.a_entered)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state = waited;
                busy("0")
            }
            _ => ok(),
        };
        state.log.push((index, entered, Instant::now()));
        Ok(reply)
    });
    let harness = Harness::new(
        Arc::new(ScriptedBackend::new()),
        Arc::new(transport),
        config(2, 8),
        5,
    );
    let results: Vec<_> = harness
        .runner
        .decide_stream(Box::new(requests(2).into_iter()), &CancelToken::new())
        .collect();
    assert!(
        results.iter().all(|result| result.outcome.is_ok()),
        "{results:?}"
    );
    let state = lock(&calls.0);
    let a_busy_returned = state
        .log
        .iter()
        .find(|(index, _, _)| *index == 0)
        .map(|(_, _, returned)| *returned)
        .expect("A's first call");
    let b_second = state
        .log
        .iter()
        .filter(|(index, _, _)| *index == 1)
        .nth(1)
        .map(|(_, entered, _)| *entered)
        .expect("B's second call");
    assert!(
        b_second >= a_busy_returned + Duration::from_secs(1),
        "B resent {:?} after A's busy reply, before A's 1 s pause ended",
        b_second.saturating_duration_since(a_busy_returned)
    );
}
