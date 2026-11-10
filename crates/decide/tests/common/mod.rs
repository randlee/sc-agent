//! Shared by decide's integration tests: tempdir configs, an explicit environment, a map
//! keyring, injected transports and `MemoryLog`, all through the real `DecisionRunner`.
//!
//! No test reads or writes the process environment, HOME or the platform keyring.
#![cfg(test)]
#![allow(dead_code, reason = "each test binary uses a subset of these helpers")]

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use decide::{Context, ModelSelector, SendOptions, load_config};
use sc_agent_http::{HttpError, HttpRequest, HttpResponse, Transport};
use sc_agent_log::testing::MemoryLog;
use sc_agent_log::{Repo, SessionIdentity, Slug};
use sc_agent_secrets::{KeyringSource, SecretsError};
use sc_agent_types::Provider;
use sc_decision_schema::DecisionRequest;
use serde_json::{Value, json};

/// Test keys; no record or output may contain one.
pub const TYPESAFE_KEY: &str = "ts-decide-test-secret-0001";
pub const ANTHROPIC_KEY: &str = "sk-ant-decide-test-secret-0002";
pub const OPENROUTER_KEY: &str = "sk-or-decide-test-secret-0003";
pub const OPENAI_KEY: &str = "sk-oai-decide-test-secret-0004";

/// A noul request on question `q`.
pub const NOUL: &str = r#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}"#;

/// The decided TypeSafe response to [`NOUL`], with an unrounded probability.
pub const NOUL_RESPONSE: &str = r#"{"model":"jev-latest","answers":{"q":{"type":"noul","noul":0.123456789}},"usage":{"input_tokens":1,"output_tokens":1}}"#;

/// A keyring backed by a map (service -> password).
#[derive(Default)]
pub struct MapKeyring(pub HashMap<String, String>);

impl KeyringSource for MapKeyring {
    fn password(&self, service: &str, _account: &str) -> Result<Option<String>, SecretsError> {
        Ok(self.0.get(service).cloned())
    }
}

/// A config dir holding `default.json` = `json`.
pub fn config_dir(json: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("default.json"), json).expect("default.json");
    dir
}

/// A config dir with no `default.json` (the default configuration).
pub fn empty_config_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

pub fn environment(vars: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    vars.iter()
        .map(|(name, value)| (OsString::from(name), OsString::from(value)))
        .collect()
}

/// A context with every seam injected: config dir, environment, empty map keyring,
/// `transport`, and `log`'s general logger.
pub fn context(
    dir: &Path,
    vars: &[(&str, &str)],
    transport: Arc<dyn Transport>,
    log: &MemoryLog,
) -> Context {
    Context {
        service: "decide",
        config_dir: Some(dir.to_path_buf()),
        // No real `.env` is read (QA-011): the repo root is the tempdir config dir.
        repo_root: Some(dir.to_path_buf()),
        environment: Some(environment(vars)),
        keyring: Some(Arc::new(MapKeyring::default())),
        transport: Some(transport),
        general: Some(log.general()),
    }
}

/// The session identity the tests inject.
pub fn identity() -> SessionIdentity {
    SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "proj", "role")
        .expect("identity")
}

/// Send options for `model` over `context`, with `log`'s session logger.
pub fn options(
    model: &str,
    provider: Option<Provider>,
    context: Context,
    log: &MemoryLog,
) -> SendOptions {
    let config = load_config(&context).expect("config");
    SendOptions {
        model: ModelSelector::parse(model, &config).expect("model"),
        provider,
        project: Slug::new("project", "proj").expect("slug"),
        agent_role: Slug::new("agent_role", "role").expect("slug"),
        context,
        session: Some(log.session(identity())),
    }
}

pub fn request(json: &str) -> DecisionRequest {
    decide::input::build_request(json.as_bytes(), &[], false, None).expect("request")
}

pub fn json_response(status: u16, body: &str) -> HttpResponse {
    HttpResponse::new(
        status,
        vec![("content-type".to_owned(), "application/json".to_owned())],
        body.as_bytes().to_vec(),
    )
}

pub fn ok_noul() -> HttpResponse {
    json_response(200, NOUL_RESPONSE)
}

/// An Anthropic Messages reply answering [`NOUL`]'s question.
pub fn anthropic_noul() -> HttpResponse {
    let text = json!({ "answers": { "q": { "noul": 0.25 } } }).to_string();
    json_response(
        200,
        &json!({
            "model": "claude-haiku-5-5-20261007",
            "stop_reason": "end_turn",
            "content": [{ "type": "text", "text": text }],
            "usage": { "input_tokens": 12, "output_tokens": 4 }
        })
        .to_string(),
    )
}

/// The TypeSafe fixtures of sc-decision-schema (openapi.json v0.2.0 examples).
pub fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sc-decision-schema/tests/fixtures/typesafe")
        .join(format!("{name}.json"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{path:?}: {error}"))
        .trim_end()
        .to_owned()
}

/// The body of a request as JSON.
pub fn body_json(request: &HttpRequest) -> Value {
    serde_json::from_slice(request.body()).expect("json body")
}

pub fn header<'r>(request: &'r HttpRequest, name: &str) -> Option<&'r str> {
    request
        .headers()
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// A transport that records when each send started, then answers from `replies` in order
/// (the last reply repeats).
#[derive(Clone)]
pub struct TimedTransport {
    inner: Arc<Mutex<Timed>>,
}

struct Timed {
    replies: Vec<Result<HttpResponse, HttpError>>,
    sends: Vec<(Instant, HttpRequest)>,
}

impl TimedTransport {
    pub fn new(replies: Vec<Result<HttpResponse, HttpError>>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Timed {
                replies,
                sends: Vec::new(),
            })),
        }
    }

    pub fn sends(&self) -> Vec<(Instant, HttpRequest)> {
        self.inner.lock().expect("lock").sends.clone()
    }
}

impl Transport for TimedTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        let mut timed = self.inner.lock().expect("lock");
        timed.sends.push((Instant::now(), request.clone()));
        let index = timed.sends.len() - 1;
        timed
            .replies
            .get(index)
            .or_else(|| timed.replies.last())
            .cloned()
            .expect("a scripted reply")
    }
}

/// Every `MemoryLog` record's fields as text, for "no key value" checks.
pub fn records_text(log: &MemoryLog) -> String {
    log.records()
        .expect("records")
        .iter()
        .map(|record| Value::Object(record.fields.clone()).to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `lines` lines of `width` spaces each, generated on demand (a batch of any size, never held).
pub struct BlankLines {
    width: usize,
    remaining: u64,
    column: usize,
}

impl BlankLines {
    pub fn new(width: usize, lines: u64) -> std::io::BufReader<Self> {
        std::io::BufReader::with_capacity(
            64 * 1024,
            Self {
                width,
                remaining: lines,
                column: 0,
            },
        )
    }
}

impl std::io::Read for BlankLines {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 || out.is_empty() {
            return Ok(0);
        }
        if self.column == self.width {
            self.column = 0;
            self.remaining -= 1;
            out[0] = b'\n';
            return Ok(1);
        }
        let spaces = (self.width - self.column).min(out.len());
        out[..spaces].fill(b' ');
        self.column += spaces;
        Ok(spaces)
    }
}

/// `decide::send` with the result returned: the one result it hands out, or the command
/// error. A flush error after the result is returned as the error (tests that need the
/// result after a flush failure call `decide::send` directly).
pub fn send(
    options: &SendOptions,
    request: DecisionRequest,
    cancel: &sc_agent_types::CancelToken,
) -> Result<sc_decision_schema::DecisionResult, decide::DecideError> {
    let mut delivered = None;
    decide::send(options, request, cancel, &mut |result| {
        delivered = Some(result);
        Ok(())
    })?;
    Ok(delivered.expect("decide::send hands out one result"))
}

/// How long a test waits for a forced ordering event before it fails instead of hanging.
/// A guard only: no assertion depends on how long anything took.
pub const HANG_GUARD: std::time::Duration = std::time::Duration::from_secs(60);

/// A transport whose sends block until the test releases them, with bounded waits.
///
/// Like `sc_agent_http::testing::BlockingTransport`, but `wait_started` gives up after
/// [`HANG_GUARD`] (and releases every send), so a broken ordering fails the test instead of
/// hanging it. Clones share one gate.
#[derive(Clone)]
pub struct Gate {
    inner: Arc<(Mutex<GateState>, std::sync::Condvar)>,
}

struct GateState {
    reply: HttpResponse,
    started: usize,
    released: bool,
}

impl Gate {
    pub fn new(reply: HttpResponse) -> Self {
        Self {
            inner: Arc::new((
                Mutex::new(GateState {
                    reply,
                    started: 0,
                    released: false,
                }),
                std::sync::Condvar::new(),
            )),
        }
    }

    /// Waits until `sends` sends have entered `send`; panics (after releasing) past the guard.
    pub fn wait_started(&self, sends: usize) {
        let (lock, condvar) = &*self.inner;
        let state = lock.lock().expect("lock");
        let (mut state, timeout) = condvar
            .wait_timeout_while(state, HANG_GUARD, |state| state.started < sends)
            .expect("wait");
        if timeout.timed_out() {
            state.released = true;
            condvar.notify_all();
            panic!("only {} of {sends} sends started", state.started);
        }
    }

    /// Lets every blocked and later send return.
    pub fn release(&self) {
        let (lock, condvar) = &*self.inner;
        lock.lock().expect("lock").released = true;
        condvar.notify_all();
    }

    /// Sends that have entered `send`.
    pub fn sends(&self) -> usize {
        self.inner.0.lock().expect("lock").started
    }
}

impl Transport for Gate {
    fn send(&self, _request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        let (lock, condvar) = &*self.inner;
        let mut state = lock.lock().expect("lock");
        state.started += 1;
        condvar.notify_all();
        let (state, _) = condvar
            .wait_timeout_while(state, HANG_GUARD, |state| !state.released)
            .expect("wait");
        Ok(state.reply.clone())
    }
}

/// Waits for `child` to exit, at most [`HANG_GUARD`]; past it the test fails (the waiting
/// thread is left behind, so the test never hangs).
pub fn finish(child: std::process::Child) -> std::process::Output {
    let pid = child.id();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(child.wait_with_output());
    });
    receiver
        .recv_timeout(HANG_GUARD)
        .unwrap_or_else(|_| panic!("child {pid} did not exit within the hang guard"))
        .expect("wait")
}

/// Runs `command` (stdin closed, output captured), bounded by [`HANG_GUARD`].
pub fn run_bounded(command: &mut std::process::Command) -> std::process::Output {
    use std::process::Stdio;
    finish(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn"),
    )
}
