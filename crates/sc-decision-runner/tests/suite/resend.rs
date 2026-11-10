//! REQ-SAG-0601 / 0602 resend, REQ-SAG-0601 AC4 cancel during holdoff, REQ-SAG-0012
//! (runner side) provider pause.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

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
    let harness = Harness::new(backend.clone(), Arc::new(transport.clone()), config(1), 5);
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
    let harness = Harness::new(backend.clone(), Arc::new(transport.clone()), config(1), 5);
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
/// worker, a second request is answered during the first one's holdoff. Forced order
/// (QA-R2-008): request 0's busy reply is held until the input iterator returned `None`, so
/// request 1 is certainly queued when request 0 is held off.
#[test]
fn holdoff_frees_the_worker() {
    let (input, input_ended) = EndSignal::new(requests(2));
    let input_ended = Mutex::new(input_ended);
    let order = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&order);
    let (transport, _dropped) = FnTransport::new(move |request| {
        let index = index_of(request);
        let first = {
            let mut order = lock(&seen);
            let first = !order.contains(&index);
            order.push(index);
            first
        };
        Ok(if index == 0 && first {
            lock(&input_ended)
                .recv_timeout(FAILSAFE)
                .expect("the input ended before request 0 is held off");
            busy("1")
        } else {
            ok()
        })
    });
    let harness = Harness::new(
        Arc::new(host_per_request()),
        Arc::new(transport),
        config(1),
        5,
    );
    let runner = Arc::clone(&harness.runner);
    let (results_to, results) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for result in runner.decide_stream(Box::new(input), &CancelToken::new()) {
            let _ = results_to.send(result);
        }
    });
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
        config(1),
        5,
    );
    let cancel = CancelToken::new();
    let results = harness.stream(requests(2), &cancel);
    gate.wait_inside(1);
    cancel.cancel();
    // Both results arrive while the gate is still closed: request 1's send is blocked in
    // the transport and request 0's 30 s holdoff has not passed, so only the cancel can
    // have answered them (QA-R2-007; the clock-free holdoff case is a JobQueue unit test).
    let all = take(&results, 2);
    assert_eq!(gate.inside(), 1, "the gate is still closed");
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

/// REQ-SAG-0601 AC4 (decide): cancel after a 30 s busy reply ends the decide at once. The
/// runner's clock is a ManualClock the test never moves, so the 30 s holdoff cannot elapse:
/// the Cancelled result can only come from the cancel (QA-007). The wait is bounded by a hang
/// guard only; no assertion depends on elapsed time.
#[test]
fn decide_cancel_during_holdoff() {
    let (busy_sent, on_busy) = mpsc::channel();
    let busy_sent = Mutex::new(busy_sent);
    let (transport, dropped) = FnTransport::new(move |_| {
        let _ = lock(&busy_sent).send(());
        Ok(busy("30"))
    });
    let clock = sc_agent_http::testing::ManualClock::new(std::time::Instant::now());
    let frozen = clock.now();
    let harness = Harness::with_clock(
        Arc::new(ScriptedBackend::new()),
        Arc::new(transport),
        sc_decision_runner::RunnerConfig::default(),
        5,
        clock.clone(),
    );
    let cancel = CancelToken::new();
    let result = harness.decide(request("m0"), &cancel);
    on_busy
        .recv_timeout(FAILSAFE)
        .expect("the busy reply was sent");
    cancel.cancel();
    let result = result
        .recv_timeout(FAILSAFE * 30)
        .expect("the cancelled result (hang guard)");
    assert_eq!(
        clock.now(),
        frozen,
        "the clock never moved: the holdoff cannot have ended"
    );
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
        config(3),
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

// REQ-SAG-0012 (runner side), formerly `provider_pause_holds_other_jobs` here, raced on when
// A's pause was set. It is now `provider_pause_holds_other_jobs_until_exactly_its_end` in
// src/runner/tests.rs: a ManualClock, one worker, and a crate-private seam (QA-R2-002).
