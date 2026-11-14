//! Observe timeout delivery through public send entry points without timing or sockets.

use std::cell::RefCell;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use sc_agent_http::testing::ScriptedTransport;
use sc_agent_http::{HttpResponse, Timeouts, Transport};
use sc_agent_log::testing::MemoryLog;
use sc_agent_log::{Repo, SessionIdentity, Slug};
use sc_agent_types::CancelToken;

use super::{Context, ModelSelector, SendOptions};
use crate::{RequestTimeout, send_batch_with_timeout, send_with_timeout};

const REQUEST: &[u8] = br#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}"#;
const RESPONSE: &[u8] = br#"{"model":"jev-latest","answers":{"q":{"type":"noul","noul":0.25}},"usage":{"input_tokens":1,"output_tokens":1}}"#;

thread_local! {
    // Scoped to the calling test thread. Production has no constructor substitution.
    static CONSTRUCTION: RefCell<Option<(ScriptedTransport, Vec<Timeouts>)>> = const { RefCell::new(None) };
}

/// The default-transport constructor's controlled test substitute. The public path
/// selects the timeouts; only construction is substituted, leaving backend/runner intact.
pub(super) fn record_construction(timeouts: Timeouts) -> Option<Arc<dyn Transport>> {
    CONSTRUCTION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let (transport, seen) = slot.as_mut()?;
        seen.push(timeouts);
        Some(Arc::new(transport.clone()) as Arc<dyn Transport>)
    })
}

struct Recording;

impl Recording {
    fn install(transport: ScriptedTransport) -> Self {
        CONSTRUCTION.with(|slot| {
            assert!(
                slot.borrow().is_none(),
                "no nested constructor substitutions"
            );
            *slot.borrow_mut() = Some((transport, Vec::new()));
        });
        Self
    }

    fn timeouts(&self) -> Vec<Timeouts> {
        CONSTRUCTION.with(|slot| slot.borrow().as_ref().expect("installed").1.clone())
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        CONSTRUCTION.with(|slot| *slot.borrow_mut() = None);
    }
}

fn options(
    local: bool,
    transport: &ScriptedTransport,
    log: &MemoryLog,
) -> (tempfile::TempDir, SendOptions) {
    let dir = tempfile::tempdir().expect("config dir");
    std::fs::write(
        dir.path().join("default.json"),
        if local {
            r#"{"local_runners":{"laya":{"kind":"laya","host":"127.0.0.1","port":9000,"model":"jev-latest","request_timeout_secs":900,"load_timeout_secs":17}}}"#
        } else {
            "{}"
        },
    ).expect("write config");
    let mut context = Context::new("decidr");
    context.config_dir = Some(dir.path().to_owned());
    context.repo_root = Some(dir.path().to_owned());
    context.environment = Some(vec![(
        OsString::from(sc_agent_secrets::TYPESAFE_API_KEY),
        OsString::from("timeout-test-key"),
    )]);
    context.general = Some(log.general());
    context.clock = Some(sc_agent_http::testing::ManualClock::new().clock());
    if local {
        context.transport = Some(Arc::new(transport.clone()));
    }
    let config = super::load_config(&context).expect("config");
    let model = ModelSelector::parse(if local { "laya" } else { "jev" }, &config).expect("model");
    let identity =
        SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "proj", "role")
            .expect("identity");
    (
        dir,
        SendOptions {
            model,
            provider: None,
            project: Slug::new("project", "proj").expect("project"),
            agent_role: Slug::new("agent_role", "role").expect("role"),
            context,
            session: Some(log.session(identity)),
        },
    )
}

fn run(batch: bool, options: &SendOptions, timeout: Option<RequestTimeout>) {
    let request = crate::input::build_request(REQUEST, &[], false, None).expect("request");
    let mut count = 0;
    let mut delivered = |result: sc_decision_schema::DecisionResult| {
        assert!(result.outcome.is_ok(), "{:?}", result.outcome);
        count += 1;
        Ok(())
    };
    if batch {
        send_batch_with_timeout(
            options,
            vec![request],
            &CancelToken::new(),
            timeout,
            &mut delivered,
        )
    } else {
        send_with_timeout(
            options,
            request,
            &CancelToken::new(),
            timeout,
            &mut delivered,
        )
    }
    .expect("public send");
    assert_eq!(
        count, 1,
        "one successful decision reached the public callback"
    );
}

fn hosted(batch: bool) {
    for seconds in [None, Some(30.5)] {
        let transport =
            ScriptedTransport::responses(vec![HttpResponse::new(200, vec![], RESPONSE.to_vec())]);
        let recording = Recording::install(transport.clone());
        let log = MemoryLog::new().expect("log");
        let (_dir, options) = options(false, &transport, &log);
        let timeout = seconds.map(|value| RequestTimeout::from_secs(value).expect("timeout"));
        run(batch, &options, timeout);
        assert_eq!(
            recording.timeouts(),
            [Timeouts {
                connect: Duration::from_secs(10),
                request: Duration::from_secs_f64(seconds.unwrap_or(120.0)),
            }],
            "the hosted default transport constructor received the per-call value"
        );
        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].endpoint().host(), "api.typesafe.ai");
    }
}

fn local(batch: bool) {
    for seconds in [None, Some(30.5)] {
        let transport = ScriptedTransport::responses(vec![
            HttpResponse::new(200, vec![], br#"{"status":"ok","loaded":[]}"#.to_vec()),
            HttpResponse::new(200, vec![], RESPONSE.to_vec()),
        ]);
        let log = MemoryLog::new().expect("log");
        let (_dir, options) = options(true, &transport, &log);
        let timeout = seconds.map(|value| RequestTimeout::from_secs(value).expect("timeout"));
        run(batch, &options, timeout);
        let requests = transport.requests();
        assert_eq!(requests.len(), 2, "one health probe, then one decision");
        assert_eq!(requests[0].deadline(), None, "the probe was not overridden");
        assert!(requests[1].endpoint().as_str().ends_with("/v1/systemone"));
        assert_eq!(
            requests[1].deadline(),
            Some(Duration::from_secs(17) + Duration::from_secs_f64(seconds.unwrap_or(900.0))),
            "the decision transport received configured load allowance + per-call request timeout"
        );
    }
}

#[test]
fn public_send_delivers_hosted_timeout_to_transport() {
    hosted(false);
}

#[test]
fn public_batch_delivers_hosted_timeout_to_transport() {
    hosted(true);
}

#[test]
fn public_send_delivers_local_timeout_to_transport() {
    local(false);
}

#[test]
fn public_batch_delivers_local_timeout_to_transport() {
    local(true);
}
