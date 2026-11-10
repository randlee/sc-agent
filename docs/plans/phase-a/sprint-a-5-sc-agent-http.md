<!-- Generated from bead agt-4p2.5 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-5: sc-agent-http
- id: `a-5` (bead `agt-4p2.5`)
- branch: `sprint/a-5-sc-agent-http`
- depends on: a-4, a-8
- copy script: `docs/plans/phase-a/copy/a-5.sh`

## Description

Sync ureq transport, HttpRequest/HttpResponse, busy resend and holdoff schedule (no sleep), quota detection, secret redaction, HttpError. Records: REQ-SAG-0601, REQ-SAG-0602, REQ-SAG-0002.

## Design

```yaml
sprint:
  id: a-5
  closure_type: boundary
  target_boundary: sc-agent-http
  owned_paths: [crates/sc-agent-http/**]
  must_follow:                        # a-1 (sc-agent-types, workspace root) is reached through a-4 and a-8
    - sprint: a-4
      artifact: sc_agent_secrets::ApiKey (crates/sc-agent-secrets)
      trigger: dev-push
    - sprint: a-8
      artifact: sc_agent_log::{GeneralLogger, Level, LogContext} (crates/sc-agent-log)
      trigger: dev-push
  requirements: [REQ-SAG-0601, REQ-SAG-0602, REQ-SAG-0012, REQ-SAG-0013, REQ-SAG-0015, REQ-SAG-0002, REQ-SAG-0007, REQ-SAG-0008, REQ-SAG-0009, REQ-SAG-0010, REQ-SAG-0401]
  adrs: [ADR-SAG-0001, ADR-SAG-0004]
  does_not_close:
    - "REQ-SAG-0601 end to end (a decide send against a busy or timing-out provider, held off and resent through the real DecisionRunner): a-10 acceptance; the holdoff wait and cancel are a-9's"
    - "REQ-SAG-0602 end to end (decide reports http.quota_exhausted from a provider reply): a-10 acceptance"
    - "REQ-SAG-0012 end to end (one busy reply pauses the other requests of a batch through the runner's one HttpClient): a-9 and a-10 acceptance"
    - "REQ-SAG-0015 end to end (local runner timeouts 300 s / load 600 s read from config, one UreqTransport per runner, the load-on-first-request deadline chosen by the daemon): a-4 (config keys), a-6 (which request gets with_deadline), a-10 (builds the transports). Closed here: the per-transport Timeouts and the per-request deadline (A5-25, A5-26)"
    - "REQ-SAG-0013 max_sends 1..=10 bound and key_env refusal for plain-http non-loopback runners: checked at config load, a-4; ResendPolicy accepts any NonZeroU32. The request-level key guard (A5-24) is closed here"
```

# Sprint A-5: sc-agent-http (bead agt-4p2.5)

Governing: REQ-SAG-0601 (busy resend and holdoff), REQ-SAG-0602 (quota is not busy), REQ-SAG-0012 (provider-wide busy pause), REQ-SAG-0002. Also binding: ADR-SAG-0004 (layer: above sc-agent-types, sc-agent-secrets and sc-agent-log; below sc-decision-schema and sc-llm-provider), REQ-SAG-0009, REQ-SAG-0010, ADR-SAG-0001 (sans-IO backends, separate transport), REQ-SAG-0007 (`retry_after_seconds` unrounded), REQ-SAG-0401, REQ-SAG-0008 (logs through the injected GeneralLogger), owner-busy-resend, owner-no-sleep, owner-error-contract, owner-lowlevel-no-tokio, owner-architecture.

Copy script: `docs/plans/phase-a/copy/a-5.sh` (source sc-agent-ref@dce239a; dry run 2026-11-09, reconciliation: 26 copy steps, 13 files, 2075 lines; no `sleep(` in any copied line).

What the crate is: a sync HTTP layer with no queue. It owns `HttpRequest`/`HttpResponse` (byte bodies, so multipart works), sends one request once (`Transport`), classifies the reply (busy / quota / final), and for a busy reply returns a not-before `Instant` for the resend under `ResendPolicy`; a busy reply also pauses every later send from the same `HttpClient` to that host (`ProviderPause`, REQ-SAG-0012). It never waits and takes no `CancelToken`: the holdoff wait and cancellation live in the caller's queue (the runner, A-9, uses `CancelToken::wait_until` and `on_cancel`). Every final reply that is neither busy nor quota is returned to the caller as `Next::Done { response, .. }`, whatever its status, so the backend decodes the API's own error bodies (Jev 400/422 are sc-decision-schema types; LLM providers' errors are read by sc-llm-provider with `ProviderReply::read`).

Main departures from the old crate:
- Old `retry.rs:247-268` (`Hooks::wait`, `thread::sleep` slices) and `retry_loop` (507-626) are not copied; `send_once` returns `Next::ResendAt { at, .. }`.
- Old `MAX_SENDS = 5` cap and `clamp(1, MAX_SENDS)` removed; `max_sends` is any `NonZeroU32`, default 5.
- A valid `Retry-After` is clamped to 250 ms-30 s and gets 0-250 ms jitter (REQ-SAG-0012: no `Retry-After: 0` storm, no herd waking at one instant); the fallback holdoffs keep their jitter (REQ-SAG-0601).
- Old `RetryAfter::report_seconds` rounded; the error now carries the parsed `f64`.
- Old `api_failure` status mapping is not copied: non-busy statuses go back to the backend.
- `HeaderName::new` and `Endpoint::loopback_http` panicked through `assert!`; both go.
- The old `test-util` feature is not copied. Instead the crate has a cargo feature `testing` (not default) exposing `sc_agent_http::testing::{ScriptedTransport, BlockingTransport}`. This overrides the earlier "no test-util feature" decision (plan hardening, PLAN-CRIT-010): five sprints (a-2, a-6, a-7, a-9, a-10) need the same transport fakes and would otherwise each re-implement them. Consumers take `sc-agent-http = { path = .., features = ["testing"] }` as a dev-dependency only. Tests still drive `send_once` with an explicit clock.
- A timed-out send is resent unchanged after 0-250 ms jitter (no host pause), counting toward `max_sends`, while the 90 s start budget allows; with the real default `Timeouts` that means a connect timeout (10 s) is resent and a request timeout (120 s, past the budget) ends with the timeout error. A resent POST after a timeout may run twice at the provider (and be billed twice): accepted by the owner (resend on timeout, REQ-SAG-0601); connect, DNS, TLS and other transport failures are returned after one send; every other remote error (402, Anthropic's usage-limit 400, 503, ...) is a final reply returned to the backend with the remote id and message (REQ-SAG-0601).
- Provider text is kept whole after secret redaction: `PROVIDER_TEXT_MAX_BYTES` and `bound_provider_text` are deleted after copying (REQ-SAG-0002).

Decisions (assistant, 2026-11-09):
- Scheme: `Endpoint::parse` accepts only https; `Endpoint::parse_local` also accepts http and is called only by sc-decision-local with a host and port from a `local_runners` entry (runners on the user's machine or network often have no TLS).
- One-off calls with no runner queue (decide models and doctor, a non-decision LLM call, REQ-SAG-0502) hold off on `CancelToken::wait_until(t)` on the calling thread: a queue of one, ended at once by cancel; no sleep.

Push order (PLAN-SCOPE-002): the first pushes after the copy commit are `src/request.rs` + `src/error.rs` building (`HttpRequest`, `HttpResponse`, `HttpError` with the section 3/4 surface), so a-2 can start on its dev-push edge; `resend.rs`, `ureq_transport.rs` and the rest follow.

## 1. Copy list (source `path:lines` -> target)

| # | Source (sc-agent-ref@dce239a) | Target | What it becomes |
|---|---|---|---|
| C1 | `crates/sc-agent-http/LICENSE` | `LICENSE` | unchanged |
| C2 | `crates/sc-agent-http/src/request.rs:1-37` | `src/request.rs` | `Method` |
| C3 | `crates/sc-agent-http/src/request.rs:119-484` | `src/request.rs` | `SecretHeader`, `Endpoint`, `HttpRequest`, `HttpResponse` |
| C4 | `crates/sc-agent-http/src/transport.rs:1-128` | `src/transport.rs` | `Transport`, `Timeouts` |
| C5 | `crates/sc-agent-http/src/ureq_transport.rs` (all) | `src/ureq_transport.rs` | `UreqTransport`, `LookupResolver`, error classification, unit tests |
| C6 | `crates/sc-agent-http/src/retry_after.rs` (all) | `src/retry_after.rs` | the one `Retry-After` parser |
| C7 | `crates/sc-agent-http/src/redact.rs:1-46` | `src/redact.rs` | `redact(error, request)` |
| C8 | `crates/sc-agent-http/src/failure.rs:1-53, 61-177, 390-422` | `src/quota.rs` | quota reader `Body`, `QUOTA_MARKERS`, `QUOTA_MESSAGES`, `is_quota_reply`, documented-shape test |
| C9 | `crates/sc-agent-http/src/failure.rs:304-352, 627-652` | `src/reply.rs` | `read_failure` (-> `ProviderReply::read`), key-scrub test |
| C10 | `crates/sc-agent-types/src/error.rs:67-73, 108-247` | `src/reply.rs` | `ProviderFailure` (-> `ProviderReply`) + Display; the copied text bound is deleted |
| C11 | `crates/sc-agent-types/src/code.rs:268-325, 344-364` | `src/error.rs` | `NetworkErrorKind` (-> `NetworkKind`), `BusyCause` (-> `BusyReason`) |
| C12 | `crates/sc-agent-http/src/retry.rs:54-74, 231-245, 628-653, 663-713, 726-742` | `src/resend.rs` | constants, jitter, `is_busy`, `backoff`, unit tests |
| C13 | `crates/sc-agent-http/src/test_util.rs:18-30, 139-223` | `tests/support/mod.rs` | `ScriptedTransport`, `RecordingTransport`, `lock` |
| C14 | `crates/sc-agent-http/tests/ureq.rs:22-54, 240-284` | `tests/ureq.rs` | request helpers, `assert_network`, `send_once`, `serve_once`, refused-connection test |
| C15 | `crates/sc-agent-http/tests/retry.rs:22-45` | `tests/resend.rs` | `secs`, `request`, `busy`, `ok` builders |

## 2. Required edits after copying

- All files: `JevError` -> `HttpError`; `ResolvedApiKey` -> `sc_agent_secrets::ApiKey`; `BackendKeyId` -> nothing (tests build keys with `sc_agent_secrets::resolve_var` over `KeySources` with a map env). Remove every `cfg(feature = "test-util")` item.
- `request.rs`: delete `HeaderName`; headers are `(String, String)`, `SecretHeader.name` is `&'static str`; an invalid header name/value becomes `HttpError::InvalidRequest` when the ureq request is built. `Endpoint`: drop `https`, `is_https`, `loopback_http`; `parse` (https only) and `parse_local` (http or https) return `InvalidEndpoint`. Rewrite `log_form` (286-308) with `get(..)`/`split_once` (no indexing). Body `Arc<[u8]>`. `Debug` prints `body_len` and the redacted secret.
- `transport.rs`: drop `MIN`, `MAX_CONNECT`, `MAX_REQUEST`, `clamp`; public fields, `Default` 10 s / 120 s, add `Timeouts::probe()` 5 s / 10 s. Drop ADR-SAG-0007/RSH-002 references.
- `ureq_transport.rs`: keep `AgentConfig::build` (no redirects, `http_status_as_error(false)`, idle pool 64, versioned User-Agent), `LookupResolver` (DNS failure -> `Connect`), `secret_header_value`, `expose` (the single `ApiKey::as_str` site, `#[expect(clippy::disallowed_methods, reason = ..)]`), `read_response` (8 MiB cap), `ureq_error_kind`, `io_error_kind`. `AgentConfig::build` sets `timeout_connect(Some(timeouts.connect))` and `timeout_global(Some(timeouts.request))`: `request` is one deadline over connect, headers and the whole body read (a slow-drip body cannot exceed it). When `request.deadline()` is Some, the send sets it per request (`ureq::RequestBuilder::config().timeout_global(Some(deadline)).build()`), replacing the agent's value for that send only. Delete `loopback_for_tests`, `is_valid_url`, `SecretHeader::matches`, the `https_only` refusal (164-169). `BodyExceedsLimit` -> `ResponseTooLarge`; other errors and body read failures -> `Network { kind, .. }`. Port the unit tests (380-561), keep `test_endpoint_parse_refuses_http`, add a `parse_local` accepts-http test.
- `retry_after.rs`: delete `REPORT_MAX_SECONDS`/`report_seconds`; add `pub(crate) fn seconds(self) -> f64` (validated text parsed, unrounded). Keep `wait()` (0-30 s) and the integer `nanos` path.
- `redact.rs`: `redact(error: HttpError, request: &HttpRequest) -> HttpError` = `error.map_text(|t| key.redact_text(t))` when the request has a secret.
- `quota.rs`: keep the "Documented body shapes" doc (21-53), drop the mapping table (7-19). `is_quota_reply(status, body)` is consulted only for 429/502/529 (REQ-SAG-0602); the 402 shortcut and the Anthropic usage-limit 400 message start are dropped: those replies are final, returned after one send with the remote id and message (REQ-SAG-0601). Port the test to `HttpError::QuotaExhausted`.
- `reply.rs`: `ProviderFailure` -> `ProviderReply`: drop the `provider` label, `retry_after_seconds: Option<f64>`, drop `Eq`. `read_failure` -> `pub fn ProviderReply::read(response: &HttpResponse, secret: Option<&ApiKey>) -> Self` (scrub before bound, kept); public because sc-llm-provider reads non-2xx provider replies with it. Delete `PROVIDER_TEXT_MAX_BYTES` and `bound_provider_text`: text is redacted, never cut (REQ-SAG-0002).
- `error.rs`: rename the two enums; drop serde derives, `FromStr`, `NetworkErrorKind::Cancelled` and the old `retryable()`; add `HttpError` (section 4).
- `resend.rs`: drop `MAX_SENDS`, `WAIT_SLICE`; `Hooks::jitter` -> `fn jitter() -> Duration` (std `RandomState` hash `% 251` ms); remove `sends_are_bounded_to_one_through_five`; keep `backoff_doubles_to_the_cap`, `retry_after_parser`, `system_jitter_stays_in_range`, `default_delays_are_one_two_four_eight`, `busy_set_is_exactly_three_statuses`. Write `ResendPolicy`, `Attempts`, `Next`, `HttpClient` (section 3).
- `tests/support/mod.rs` (C13, copied there unchanged in the copy commit): in a later commit move it to `src/testing.rs` behind `#[cfg(feature = "testing")]` and delete `tests/support/`; drop `FakeClock` uses; script exhaustion returns `Network { kind: Connect, .. }`; `lock` recovers a poisoned mutex (`into_inner`), no unwrap; add `BlockingTransport` (section 3). The crate's own tests enable the feature through a self dev-dependency (`sc-agent-http = { path = ".", features = ["testing"] }`).
- `tests/ureq.rs`: `Endpoint::loopback_http(port, p)` -> `Endpoint::parse_local(&format!("http://127.0.0.1:{port}{p}"))`; `loopback_for_tests` -> `UreqTransport::new`.
- `tests/resend.rs`: drop the `Hooks` struct; keep the four builders.

## 3. New code (exact public surface; bodies elided)

```rust
// src/lib.rs
#![warn(missing_docs)]
mod error; mod quota; mod redact; mod reply; mod request; mod resend; mod retry_after; mod transport; mod ureq_transport;
pub use error::{BusyReason, HttpError, NetworkKind};
pub use reply::ProviderReply;
pub use request::{Endpoint, HttpRequest, HttpResponse, Method, SecretHeader};
pub use resend::{Attempts, HttpClient, Next, Pending, ProviderPause, ResendPolicy, SendPermit, Start};
pub use sc_agent_log::LogContext;   // re-exported: send_once takes it
pub use transport::{Timeouts, Transport};
pub use ureq_transport::UreqTransport;
#[cfg(feature = "testing")]
pub mod testing;   // ScriptedTransport, RecordingTransport, BlockingTransport (test doubles for consumers)

// src/request.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Method { Get, Post }
impl Method { pub const fn as_str(self) -> &'static str; }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Endpoint { url: String }
impl Endpoint {
    pub fn parse(url: &str) -> Result<Self, HttpError>;   // https only; InvalidEndpoint
    pub fn parse_local(url: &str) -> Result<Self, HttpError>; // http or https; only for a configured local runner
    pub fn as_str(&self) -> &str;
    pub fn log_form(&self) -> String;                     // scheme://host/path only
    pub fn host(&self) -> &str;                           // the ProviderPause key (host without port)
    /// Scheme is http (only `parse_local` can produce one).
    pub fn is_plain_http(&self) -> bool;
    /// Host is `localhost`, an IPv4 address in 127.0.0.0/8, or `::1` (parsed with std::net, no DNS lookup).
    pub fn is_loopback(&self) -> bool;
}

#[derive(Clone)]                                          // Debug hand-written: never the key
pub struct SecretHeader { name: &'static str, prefix: &'static str, key: ApiKey }
impl SecretHeader {
    pub fn new(name: &'static str, key: ApiKey) -> Self;  // e.g. "x-api-key"
    pub fn bearer(key: ApiKey) -> Self;                   // Authorization: Bearer <key>
    pub fn name(&self) -> &'static str;
    pub fn key(&self) -> &ApiKey;                         // for redaction only
    pub fn redacted(&self) -> String;
}

#[derive(Clone)]                                          // Debug: method, log_form, header names, body_len, redacted secret
pub struct HttpRequest { method: Method, endpoint: Endpoint, headers: Vec<(String, String)>, secret: Option<SecretHeader>, body: Arc<[u8]>, deadline: Option<Duration> }
impl HttpRequest {
    pub fn new(method: Method, endpoint: Endpoint, body: impl Into<Arc<[u8]>>) -> Self;
    pub fn with_header(self, name: impl Into<String>, value: impl Into<String>) -> Result<Self, HttpError>; // refuses authorization, proxy-authorization, x-api-key, api-key, x-goog-api-key, cookie (case-insensitive) -> InvalidRequest (RSH-006)
    /// REQ-SAG-0013: refused with `HttpError::KeyOnPlainHttp` when `endpoint.is_plain_http() && !endpoint.is_loopback()`,
    /// so no request to a plain-http non-loopback host carries a key. This is the only way a secret enters a request
    /// (fields are private), so the check runs before any send.
    pub fn with_secret(self, secret: SecretHeader) -> Result<Self, HttpError>;
    /// REQ-SAG-0015: replaces the transport's `Timeouts::request` for every send of this request only (a load-on-first-
    /// request daemon's first request: load timeout + request timeout). Resends of the request keep it unchanged.
    pub fn with_deadline(self, deadline: Duration) -> Self;
    pub fn deadline(&self) -> Option<Duration>;
    pub fn method(&self) -> Method;
    pub fn endpoint(&self) -> &Endpoint;
    pub fn headers(&self) -> &[(String, String)];
    pub fn secret(&self) -> Option<&SecretHeader>;
    pub fn body(&self) -> &[u8];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse { status: u16, headers: Vec<(String, String)>, body: Vec<u8> }
impl HttpResponse {
    pub fn new(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Self;
    pub fn status(&self) -> u16;
    pub fn header(&self, name: &str) -> Option<&str>;     // ASCII case-insensitive
    pub fn body(&self) -> &[u8];
    pub fn into_body(self) -> Vec<u8>;
}

// src/transport.rs
pub trait Transport: Send + Sync {
    /// One send. Every HTTP status is Ok; only transport failures are Err. Open trait: test doubles and
/// callers' own transports implement it (ADR-SAG-0001).
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError>;
}
impl<T: Transport + ?Sized> Transport for &T {}
impl<T: Transport + ?Sized> Transport for Arc<T> {}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// `connect`: TCP + TLS connect (ureq timeout_connect). `request`: one global deadline for the whole send,
/// connect, headers and body read included (ureq timeout_global).
pub struct Timeouts { pub connect: Duration, pub request: Duration }
impl Timeouts { pub const MAX: Duration /* 3600 s, connect and request */; pub const DEADLINE_MAX: Duration; /* 10800 s = load max 7200 s + request max 3600 s (REQ-SAG-0015); with_deadline values over it or zero are refused as http.invalid_request at send */ pub fn validate(self) -> Result<Self, HttpError>; }
impl Default for Timeouts { /* connect 10 s, request 120 s: decision and LLM sends */ }
impl Timeouts {
    /// connect 5 s, request 10 s: discovery, `decide models`, `decide doctor` and health probes (a-6, a-7, a-10).
    pub const fn probe() -> Self;
    /// Same connect timeout, this request deadline (REQ-SAG-0015: a local runner's own request timeout, default 300 s;
    /// a-10 builds one UreqTransport per local runner with `Timeouts::default().with_request(runner.request_timeout)`).
    pub const fn with_request(self, request: Duration) -> Self;
}

// src/ureq_transport.rs
#[derive(Debug, Clone)]
pub struct UreqTransport { agent: ureq::Agent }
impl UreqTransport { pub fn new(timeouts: Timeouts) -> Self; }
impl Default for UreqTransport {}
impl Transport for UreqTransport {}

// src/resend.rs
const BUSY_STATUSES: [u16; 3] = [429, 502, 529];
const BACKOFF_FIRST: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(30);
const JITTER_MAX: Duration = Duration::from_millis(250);
const BUDGET: Duration = Duration::from_secs(90);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResendPolicy { max_sends: NonZeroU32 }
impl ResendPolicy {
    pub const fn new(max_sends: NonZeroU32) -> Self;      // AgentConfig.max_sends (A-4)
    pub const fn max_sends(self) -> u32;
}
impl Default for ResendPolicy { /* sc_agent_config::DEFAULT_MAX_SENDS is the same 5; this crate does not depend on config */ }

/// Resend state of one request: sends made and when the first started. A fresh request uses
/// `Attempts::new()`; `Next::Done` hands it back for a caller resend after a final reply (a-9 schema mismatch).
#[derive(Debug, Clone, Default)]
pub struct Attempts { first_send: Option<Instant>, sends: u32 }
impl Attempts { pub fn new() -> Self; pub fn sends(&self) -> u32; }

/// A request held for resend after a busy reply or a timeout. Only `send_once` makes one, so the
/// reason it is held (and the error it ends with) is always known.
#[derive(Debug, Clone)]
pub struct Pending { attempts: Attempts, last: Last }
#[derive(Debug, Clone)]
enum Last { Busy(ProviderReply), Timeout { message: String, cause: Option<Box<ErrorDetail>> } }   // private
impl Pending { pub fn attempts(&self) -> &Attempts; }

/// Leave to make one send now. Only `HttpClient::may_start` / `may_resend` make one; `send_once` consumes it,
/// so every send passed the max_sends, budget and provider-pause checks. Not Clone.
#[derive(Debug)]
#[must_use]
pub struct SendPermit { attempts: Attempts, last: Option<Last> }

/// Outcome of an admission check that did not refuse.
#[derive(Debug)]
#[must_use]
pub enum Start<S> {
    Go(SendPermit),
    /// The host is paused (REQ-SAG-0012): hold `state` and ask again no earlier than `at`.
    Wait { at: Instant, state: S },
}

#[derive(Debug)]
pub enum Next {
    /// A final reply (any status that is neither busy nor quota). The backend decodes it.
    Done { response: HttpResponse, attempts: Attempts },
    /// Busy or timed out: hold `pending`, call `may_resend` no earlier than `at`, send it unchanged.
    ResendAt { at: Instant, pending: Pending },
}

/// Per-host "not before" shared by every clone of one HttpClient (REQ-SAG-0012). Keys are
/// `Endpoint::host()`; a busy reply moves the host's time later, never earlier. Entries in the past are ignored
/// (bounded by the number of hosts). Poison: every access is one insert or one read with no call out while
/// locked, so the map is consistent after a panic elsewhere and `into_inner` is safe.
#[derive(Debug, Clone, Default)]
pub struct ProviderPause { hosts: Arc<Mutex<HashMap<String, Instant>>> }
impl ProviderPause {
    pub fn not_before(&self, host: &str) -> Option<Instant>;
    pub(crate) fn hold_until(&self, host: &str, until: Instant);       // keeps the later instant
}

#[derive(Clone)]
pub struct HttpClient { transport: Arc<dyn Transport>, policy: ResendPolicy, pause: ProviderPause, log: GeneralLogger }
impl HttpClient {
    /// `log` is the caller's GeneralLogger, default or injected (REQ-SAG-0008; sc-agent-log is below this crate).
    pub fn new(transport: Arc<dyn Transport>, policy: ResendPolicy, log: GeneralLogger) -> Self;
    pub fn policy(&self) -> ResendPolicy;
    pub fn pause(&self) -> &ProviderPause;
    pub fn transport(&self) -> &Arc<dyn Transport>;      // one-shot probes (A-6 discovery)
    /// First send, or a caller resend after `Done`. Err(Sends) when `sends >= max_sends`; Err(Budget) when
    /// `first_send` is set and `now - first_send >= BUDGET` (90 s): no HttpError exists for these, the caller
    /// finishes with the result it has (a-9: decision.schema_mismatch). Host paused -> `Wait`.
    pub fn may_start(&self, request: &HttpRequest, attempts: Attempts, now: Instant) -> Result<Start<Attempts>, BusyReason>;
    /// Resend of a held request. Refused -> the error it ends with: last reply busy ->
    /// `ProviderBusy { reason, attempts, retry_after_seconds, last }`; last send timed out ->
    /// `Network { kind: Timeout, attempts, message }` (never ProviderBusy). Host paused -> `Wait`.
    pub fn may_resend(&self, request: &HttpRequest, pending: Pending, now: Instant) -> Result<Start<Pending>, HttpError>;
    /// One send under `permit`. `now` is read after the send for the holdoff start. Never waits. Every Err is
    /// redacted. Every http.* event carries `ctx` (session_id, position).
    pub fn send_once(&self, request: &HttpRequest, permit: SendPermit, ctx: LogContext, now: &dyn Fn() -> Instant) -> Result<Next, HttpError>;
}
```

Admission (`may_start` / `may_resend`, each line one REQ rule): sends left and inside the 90 s start budget (REQ-SAG-0601), else refused as above; then `pause.not_before(request.endpoint().host())` later than `now` -> `Start::Wait { at, state }` (REQ-SAG-0012); else `Start::Go(permit)`.

`send_once` order (each step is one REQ line):
1. `transport.send(request)`; `sends += 1` (and `first_send` set on the first). `Err(Network { kind: Timeout })`: with `sends < max_sends` -> `Ok(ResendAt { at: now() + jitter(), pending: Last::Timeout })` (resent unchanged after 0-250 ms jitter, no host pause; `may_resend` re-checks the budget); else the timeout error. Every other `Err` (connect, DNS, TLS, other) is returned after this send (REQ-SAG-0601).
2. Status not in `BUSY_STATUSES` -> `Ok(Next::Done { response, attempts })`.
3. Busy and `is_quota_reply` -> `Err(QuotaExhausted { attempts, reply })` at once (REQ-SAG-0602).
4. Busy: `reply = ProviderReply::read(&response, request.secret().map(SecretHeader::key))`. Holdoff = valid `Retry-After` -> `RetryAfter::wait()` clamped to 250 ms-30 s, plus `jitter()` (REQ-SAG-0012); else `backoff(sends) + jitter()` (1, 2, 4, 8, 16, 30 ... s + 0-250 ms). `pause.hold_until(host, now() + holdoff)` FIRST, for every non-quota busy reply (REQ-SAG-0012). Then: if `sends >= max_sends` -> `Err(ProviderBusy { reason: Sends, attempts, retry_after_seconds, last: Box::new(reply) })`. `remaining = BUDGET - (now() - first_send)`; zero -> `ProviderBusy { reason: Budget, .. }`; else `Ok(ResendAt { at: now() + min(holdoff, remaining), pending: Last::Busy(reply) })`.

The 90 s start budget starts at the first send (`Attempts.first_send`); a local daemon's model load waits happen before it (a-6), so load time never counts toward the budget, and a per-request deadline covers only that send (REQ-SAG-0015).

Worst-case wall clock per request: starts happen only in the first 90 s after the first send, and the last start's send ends within its own request deadline, so a request finishes within 90 s + that deadline plus the caller's queue wait (held holdoffs are capped by the remaining budget). Remote providers (default 120 s): 210 s. A local runner: 90 s + its request timeout (default 300 s → 390 s). The first request to a load-on-first-request daemon (Laya): 90 s + load + request timeout (defaults 90 + 600 + 300 = 990 s); a load waited out before the first send (wait_until_loaded) adds up to the load timeout before this clock starts. With `Timeouts::probe()`: 90 s + 10 s.

Logging (ANC-007, REQ-SAG-0008) through `log.write(level, "sc_agent_http", action, ctx, fields)` (never a body, header value or key; a log Err never changes the result, the logger counts it); every event carries `ctx` (session_id as the event correlation id, position; a-8): `http.send` (method, endpoint log_form, attempt, max_sends; this is the "request sent" record for every backend: sans-IO crates send through `HttpClient` and do not log sends themselves, plan-hardening decision 4), `http.response` (status, attempt, elapsed_ms), `http.busy` (status, attempt, holdoff_ms, retry_after_honoured, host_paused_until_ms), `http.busy_exhausted`, `http.budget_exhausted`, `http.quota_exhausted`, `http.timeout_resend` (attempt), `http.transport_failed` (error id, elapsed_ms). `elapsed_ms` is measured with `Instant` around the transport call.

How the runner (A-9) uses it, no sleep (one-off calls: see Decisions): take job -> `may_start`/`may_resend` -> `Go(permit)` -> `send_once` -> `ResendAt { at, pending }` or `Wait { at, state }` puts the job back in its queue keyed by `at`; idle workers wait with `Condvar::wait_timeout` to the earliest `at`, and a `CancelToken::on_cancel` hook wakes them at once; a cancelled job is never sent again. The runner's workers share one HttpClient, so they share its ProviderPause.

```rust
// src/testing.rs (feature "testing"; copied C13 + new BlockingTransport)
/// Answers from a script, first entry first; records every request; exhausted -> HttpError::network(Connect, 1, "script exhausted"). Clones share state.
#[derive(Clone, Default)]
pub struct ScriptedTransport { inner: Arc<Mutex<Script>> }
impl ScriptedTransport {
    pub fn new(replies: Vec<Result<HttpResponse, HttpError>>) -> Self;
    pub fn responses(responses: Vec<HttpResponse>) -> Self;
    pub fn requests(&self) -> Vec<HttpRequest>;
    pub fn sends(&self) -> usize;
}
impl Transport for ScriptedTransport {}
/// Records every request, then forwards it to `T`.
pub struct RecordingTransport<T> { inner: T, requests: Arc<Mutex<Vec<HttpRequest>>> }
impl<T: Transport> RecordingTransport<T> { pub fn new(inner: T) -> Self; pub fn requests(&self) -> Vec<HttpRequest>; }
impl<T: Transport> Transport for RecordingTransport<T> {}
/// Explicit test clock shared with a transport: `now()` for send_once, advanced by sends.
#[derive(Debug, Clone)]
pub struct ManualClock { now: Arc<Mutex<Instant>> }
impl ManualClock { pub fn new(start: Instant) -> Self; pub fn now(&self) -> Instant; pub fn advance(&self, by: Duration); }
impl ScriptedTransport {
    /// Every send advances `clock` by `per_send` before answering (a send that takes the full timeout).
    pub fn with_send_duration(self, clock: ManualClock, per_send: Duration) -> Self;
}
/// A send that blocks on a Condvar until the test releases it (PLAN-CRIT-002 cancel tests). No sleep, no timeout.
#[derive(Clone)]
pub struct BlockingTransport { gate: Arc<(Mutex<Gate>, Condvar)> }
impl BlockingTransport {
    pub fn new(reply: Result<HttpResponse, HttpError>) -> Self;   // every released send returns a clone of `reply`
    pub fn wait_started(&self, sends: usize);                     // blocks until `sends` sends have entered send()
    pub fn release(&self);                                        // lets every blocked and later send return
    pub fn sends(&self) -> usize;
}
impl Transport for BlockingTransport {}
```

Corrections after QA-1 (2026-11-10; code at 2671dc5 follows these):
- Step 4 order (REQ-SAG-0012, REQ-QA-001): every busy reply that is not quota computes the holdoff and calls pause.hold_until(host, ended + holdoff) BEFORE the sends/budget checks, so a busy reply on the last allowed send still pauses the host.
- A timed-out send is resent after 0-250 ms jitter (no host pause) (RSH-003).
- Timeouts::MAX = 3600 s (connect, request) and Timeouts::validate(self) -> Result<Self, HttpError>; Timeouts::DEADLINE_MAX = 10800 s for with_deadline (REQ-SAG-0015 load + request); a zero or over-max timeout or deadline is refused as http.invalid_request before any send; with_deadline stays infallible (RSH-004).
- InvalidEndpoint.url drops userinfo, query and fragment and is cut to 256 bytes (RSH-005).
- http.response, http.busy, http.busy_exhausted, http.budget_exhausted and http.quota_exhausted carry a redacted provider_request_id when present; busy events carry host and endpoint (log_form) (RSH-007).
- CI runs cargo-deny (advisories, licenses) with a repo-root deny.toml; ureq stays pinned =3.5.1, with the reason in Cargo.toml (RSH-002).

## 4. Error list (`src/error.rs`, full; ErrorId from A-1, Display = `id: message` only, the chain stays in source()/detail())

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BusyReason { Sends, Budget }
impl BusyReason { pub const fn as_str(self) -> &'static str; }       // "sends" | "budget"

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NetworkKind { Timeout, Connect, Tls, Other }
impl NetworkKind { pub const fn as_str(self) -> &'static str; }

/// What a provider said in a reply. Text fields are redacted and otherwise kept whole (REQ-SAG-0002).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ProviderReply {
    pub status: u16,
    pub provider_type: Option<String>,
    pub provider_code: Option<String>,
    pub message: String,
    pub param: Option<String>,
    pub request_id: Option<String>,
    pub retry_after_seconds: Option<f64>,   // valid Retry-After as sent, unrounded
}
impl ProviderReply { pub fn read(response: &HttpResponse, secret: Option<&ApiKey>) -> Self; }

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum HttpError {
    /// Sends or budget ran out on busy replies (REQ-SAG-0601).
    /// `last` is the latest busy reply (always present: only a held busy request can end this way).
    ProviderBusy { reason: BusyReason, attempts: u32, retry_after_seconds: Option<f64>, last: Box<ProviderReply> }, // "http.provider_busy" (boxed: clippy result_large_err on every Result<_, HttpError>)
    /// Busy status whose body marks quota, credit or spend exhaustion (REQ-SAG-0602).
    QuotaExhausted { attempts: u32, reply: Box<ProviderReply> },        // "http.quota_exhausted"
    /// Transport failure. Timeout: resent under max_sends and the start budget, returned when either runs out; Connect (incl. DNS), Tls, Other: returned after one send.
    #[non_exhaustive] Network { kind: NetworkKind, attempts: u32, message: String, cause: Option<Box<ErrorDetail>> }, // "http.network"
    /// The body exceeded the 8 MiB read cap.
    ResponseTooLarge { status: u16, limit_bytes: u64 },            // "http.response_too_large"
    /// The URL does not parse, has no host, or its scheme is refused (`parse`: https; `parse_local`: http or https).
    InvalidEndpoint { url: String, reason: String },               // "http.invalid_endpoint"
    /// A header name or value the HTTP builder refuses.
    #[non_exhaustive] InvalidRequest { message: String, cause: Option<Box<ErrorDetail>> }, // "http.invalid_request"
    /// A key was attached to a plain-http endpoint whose host is not loopback (REQ-SAG-0013). `host` only, never the key.
    KeyOnPlainHttp { host: String },                               // "http.key_on_plain_http"
}
impl HttpError {
    pub fn network(kind: NetworkKind, attempts: u32, message: impl Into<String>) -> Self;
    pub fn invalid_request(message: impl Into<String>) -> Self;
    pub fn with_cause(self, cause: ErrorDetail) -> Self;
    pub(crate) fn map_text(self, map: &impl Fn(String) -> String) -> Self; // every text field and every cause level
}
impl ErrorId for HttpError {}   // cause_detail() (a-1 rename, RBP-F001): Network/InvalidRequest carry an explicit `cause: Option<Box<ErrorDetail>>` (built with ErrorDetail::foreign), are #[non_exhaustive], and are built with HttpError::network / invalid_request / with_cause; message() is this layer's text only; cause_detail() and source() return the cause (RBP-F001)
                                // recovery() (RBP-F005): provider_busy -> "the provider is busy; try again later or lower workers"; quota_exhausted -> "the provider account is out of quota or credit; add credit or use another key"; key_on_plain_http -> "use an https endpoint or a loopback host (localhost, 127.0.0.0/8, ::1) for a runner that needs a key"; others None
impl std::error::Error for HttpError { /* source() returns the cause */ }
```

7 variants. Cancellation is not an HTTP error: the queue that waits owns it (DecisionError::Cancelled, A-2).

## 5. Config keys

`max_sends` (A-4 `AgentConfig.max_sends`, one value, default 5) -> `ResendPolicy::new`. `Timeouts` stays code-level: `Timeouts::default()` for sends, `Timeouts::probe()` for probes.

## 6. Tests

Each test names the acceptance line (A5-nn) it proves; the assertion itself is in that line.

| Test (file) | Proves |
|---|---|
| `busy_every_time_default`, `busy_every_time_max_two` (tests/resend.rs) | A5-02 |
| `retry_after_jittered`, `retry_after_capped`, `retry_after_zero_floored`, `retry_after_invalid_falls_back` (tests/resend.rs; `retry_after_parser` unit) | A5-03 |
| `budget_stops_next_start`, `may_start_after_done_refused` (Err(Sends) / Err(Budget) with no HttpError), `retry_after_unrounded`, `resend_same_bytes` (tests/resend.rs) | A5-04 |
| `holdoff_is_resend_at` (tests/resend.rs) | A5-05 |
| `timeout_resent_under_max_sends`, `timeout_then_budget_ends_with_timeout`, `default_timeouts_which_resend`, `connect_not_resent` (tests/resend.rs); `refused_connection_is_connect` (tests/ureq.rs, real socket) | A5-06 |
| `final_402_and_usage_400` (tests/resend.rs) | A5-07 |
| `quota_429_is_final` (Anthropic body), `openai_quota_429_is_final` (OpenAI `insufficient_quota` body), `rate_limit_429_resends` (tests/resend.rs; documented-shape unit, C8) | A5-08 |
| `final_replies_unchanged` (400, 402, 422, 503) | A5-09 |
| `multipart_bytes_sent` (tests/ureq.rs over `serve_once`: headers, secret, byte body) | A5-10 |
| `http_error_ids` | A5-11 |
| `key_never_leaks` (key in a transport error and a busy reply message), `long_provider_text_kept` | A5-12 |
| `test_endpoint_parse_refuses_http`, `parse_local_accepts_http` (unit) | A5-13 |
| `events_logged` (injects `sc_agent_log::testing::MemoryLog`; also a failing logger that leaves the result unchanged; elapsed_ms present on http.response and http.transport_failed) | A5-14, A5-15 |
| `concurrent_busy_carry_session_ids` (two threads, two LogContexts, one HttpClient; each http.busy record carries its own session_id) | A5-20 |
| `busy_pauses_host`, `pause_is_per_host`, `pause_shared_by_clones` (tests/resend.rs) | A5-21 |
| `permit_consumed` (compile-fail doc test: a SendPermit cannot be used twice; no public constructor) | A5-22 |
| `probe_timeouts`, `request_timeout_is_global` (unit: AgentConfig::build sets timeout_connect and timeout_global) | A5-23 |
| `timeouts_with_request` (unit: connect kept, request replaced), `per_runner_transport_deadline` (tests/ureq.rs: UreqTransport built with `with_request(1 s)` against a `serve_once` that answers after the client gives up -> Network Timeout; the server waits on a Condvar the test releases, no sleep) | A5-25 |
| `request_deadline_overrides_transport` (tests/ureq.rs: transport request 1 s, `with_deadline(30 s)` on one request -> that send succeeds when the server answers after the 1 s mark gated by the test; the next request without a deadline times out), `budget_starts_at_first_send` (tests/resend.rs: ManualClock advanced 600 s before the first send; may_start still Go) | A5-26 |
| `key_refused_on_plain_http_remote` (http://10.0.0.5 + key -> http.key_on_plain_http, ScriptedTransport sends 0), `key_allowed_on_loopback` (http://127.0.0.1 and http://localhost + key), `key_allowed_on_https` (https://host + key), `plain_http_remote_without_key_allowed` (http://10.0.0.5, no key) (tests/request.rs) | A5-24 |
| `lower_layer_cause_kept` (ureq error through `UreqTransport` to a closed port) | A5-17 |
| `scripted_and_blocking_doubles` (feature `testing`: script order, exhaustion error, BlockingTransport released only by the test) | A5-19 |
| Unit tests copied in C5, C6, C8, C9, C12, ported | A5-03, A5-06, A5-08, A5-12 |

Not proved by a test here: A5-01 (commit history), A5-16/A5-18 (Required validation commands below).

`timeout_then_budget_ends_with_timeout`: the scripted transport returns `Network { Timeout }` and advances the `ManualClock` by 120 s (the default request timeout) on that send; `may_resend` refuses the second send (budget) and returns the timeout error, not `http.provider_busy`.

`default_timeouts_which_resend` (RSH-001, real `Timeouts::default()` values, ManualClock): a connect timeout taking `connect` (10 s) every time is sent 5 times and ends with the timeout error; a request timeout taking `request` (120 s) is sent once and ends with the timeout error.

## Required validation

- `cargo build -p sc-agent-http` and `cargo build -p sc-agent-http --features testing`
- `cargo clippy -p sc-agent-http --all-targets --features testing -- -D warnings` (workspace denies: panic, unwrap_used, expect_used, indexing_slicing; disallowed-methods incl. sleep and `ApiKey::as_str` outside ureq_transport)
- `cargo test -p sc-agent-http --features testing`
- `cargo tree -p sc-agent-http -e normal`: no tokio, no sc-decision-*, sc-llm-provider or runner crate; only sc-agent crates are sc-agent-types, sc-agent-secrets, sc-agent-log
- `cargo tree -p sc-agent-http -e normal --features testing`: same result
- `grep -rnE "sleep\(" crates/sc-agent-http` finds nothing

## 7. Dependencies

- `ureq = "=3.5.1"` (as pinned in ref-src), default features (rustls).
- `serde_json = "1"` (quota body reader, ProviderReply).
- `sc-agent-types` (path; ErrorId, ErrorDetail), `sc-agent-secrets` (path; ApiKey), `sc-agent-log` (path; GeneralLogger, Level).
- No thiserror, no serde derive, no tokio, no sc-observability directly.
- `clippy.toml` is not edited here (a-1 owns it, PLAN-SCOPE-009); a-1's file carries the `sc_agent_secrets::ApiKey::as_str` disallowed-methods rule that A5-12 relies on.
- Feature `testing` (not default): enables `src/testing.rs`; no extra dependencies.

Estimated size: about 620 lines of code, 350 of tests.

## Acceptance criteria

- [ ] A5-01 Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-5.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] A5-02 REQ-SAG-0601: sc-agent-http: with the default ResendPolicy, HttpClient::send_once over a ScriptedTransport answering busy (429/502/529) every time sends the request 5 times and ends with http.provider_busy, reason sends, 5 attempts; with max_sends = 2 it sends 2 times (no hard cap).
- [ ] A5-03 REQ-SAG-0601: sc-agent-http: HttpClient::send_once returns ResendAt at t + 3 s to t + 3.25 s for Retry-After: 3 (REQ-SAG-0012 jitter), t + 30 s to 30.25 s for Retry-After: 99, and t + 250 ms to 500 ms for Retry-After: 0; for Retry-After: abc (and absent) the holdoffs are 1, 2, 4, 8 s (then doubling, capped 30 s) each with at most 250 ms jitter; Retry-After is valid only as 1-32 ASCII digits with an optional fraction.
- [ ] A5-04 REQ-SAG-0601: sc-agent-http: HttpClient::may_start / may_resend refuse a send once 90 s have passed since the first send (reason budget) or max_sends sends were made, so send_once never runs without a SendPermit; ProviderBusy carries reason, attempts and the last retry_after_seconds unrounded (REQ-SAG-0007); a busy request is resent unchanged (same body bytes).
- [ ] A5-05 REQ-SAG-0601: sc-agent-http: the holdoff is returned as a not-before Instant (Next::ResendAt); sc-agent-http never waits and takes no CancelToken (the queue that waits owns cancellation, a-9).
- [ ] A5-06 REQ-SAG-0601: sc-agent-http: a send that times out is resent unchanged by HttpClient::send_once and counts toward max_sends (a scripted transport timing out instantly every time is sent 5 times by default); no attempt starts after the 90-second budget, and a request whose last attempt timed out ends with the timeout error (http.network, kind timeout), never http.provider_busy (scripted clock: timeout taking 120 s -> no second send -> timeout error); a connect failure returns http.network after one send (real refused-connection test over UreqTransport); DNS and TLS failures are not resent.
- [ ] A5-07 REQ-SAG-0601: sc-agent-http: a 402 response (and Anthropic's usage-limit 400) is returned by send_once after one send as Next::Done { response, .. } with the remote error id and message unchanged; no remote error other than a busy response is resent.
- [ ] A5-08 REQ-SAG-0602: sc-agent-http: send_once returns http.quota_exhausted after one send for a 429 with the documented Anthropic quota-exhausted body and for one with the OpenAI insufficient_quota body; a 429 with an unrecognized body returns ResendAt.
- [ ] A5-09 sc-agent-http: every final reply that is neither busy nor quota (e.g. 400, 402, 422, 503) is returned by send_once as Next::Done { response, .. } with the response unchanged, so the backend decodes it.
- [ ] A5-10 sc-agent-http: HttpRequest carries a byte body (Arc<[u8]>) that UreqTransport sends unchanged, so multipart bodies can be sent; HttpRequest/HttpResponse/ResendPolicy/Attempts are public here for sc-decision-schema, the backends and the runner.
- [ ] A5-11 REQ-SAG-0002: sc-agent-http: HttpError implements ErrorId with 7 distinct http.* ids; no assert!/unreachable!/panic in non-test code; clippy with the workspace denies passes.
- [ ] A5-12 Secrets: sc-agent-http: no error text, Debug output or log event contains the API key (test with the key in a transport error and a reply message); ApiKey::as_str is called only in ureq_transport (clippy disallowed-methods rule in a-1's clippy.toml); provider text is redacted, never truncated (REQ-SAG-0002).
- [ ] A5-13 sc-agent-http: Endpoint::parse refuses http://; Endpoint::parse_local accepts it (tests).
- [ ] A5-14 REQ-SAG-0008: sc-agent-http: send, response, busy, exhaustion, quota and transport-failure events go to the GeneralLogger passed to HttpClient::new; no body, header value or key is logged; a log failure never changes a send result.
- [ ] A5-15 ANC-007: sc-agent-http's HttpClient::send_once writes http.send (the "request sent" record for every backend), http.response (with elapsed_ms), http.busy, http.busy_exhausted, http.budget_exhausted, http.quota_exhausted, http.timeout_resend and http.transport_failed to the GeneralLogger passed to HttpClient::new; a test injects sc_agent_log::testing::MemoryLog, runs busy, quota, timeout and connect-failure sends and finds each record with no key or body.
- [ ] A5-16 ADR-SAG-0004 / REQ-SAG-0009 / REQ-SAG-0010: sc-agent-http: no ureq type in a public signature; cargo tree -p sc-agent-http lists no tokio and no sc-decision-* or sc-llm-provider crate; no sleep call.
- [ ] A5-17 REQ-SAG-0002: sc-agent-http: a test makes the transport fail with a lower-layer (ureq) error and asserts the returned HttpError's detail().cause (and source()) carries that cause's id and message, held in the explicit cause field.
- [ ] A5-18 ADR-SAG-0004: sc-agent-http sits in the http layer; its only sc-agent crate dependencies are the lower-layer crates sc-agent-types, sc-agent-secrets and sc-agent-log, and it depends on no decision-schema, decision-backend, sc-llm-provider or runner crate, checked by `cargo tree -p sc-agent-http` (with and without `--features testing`).
- [ ] A5-19 sc-agent-http: feature `testing` (not default) exposes sc_agent_http::testing::{ScriptedTransport, RecordingTransport, BlockingTransport}; tests/support/ is gone; BlockingTransport returns only after the test calls release() (no sleep).
- [ ] A5-20 sc-agent-http: every http.* event carries the LogContext passed to send_once (session_id as correlation id, position); two concurrent requests on one HttpClient log http.busy records with different session_ids.
- [ ] A5-21 REQ-SAG-0012: sc-agent-http: every non-quota busy reply, including one on the last allowed send, sets HttpClient's ProviderPause for that host, so may_start / may_resend for any request to the same host return Start::Wait until then; another host is not paused; clones of the HttpClient share the pause.
- [ ] A5-22 sc-agent-http: SendPermit has no public constructor and send_once consumes it; a held request is a Pending made only by send_once, so ProviderBusy always carries the last busy ProviderReply.
- [ ] A5-23 sc-agent-http: Timeouts::request is one global deadline over connect, headers and body (ureq timeout_global); Timeouts::probe() is connect 5 s, request 10 s.
- [ ] A5-24 REQ-SAG-0013: sc-agent-http: no request to a plain-http host that is not loopback carries a key: HttpRequest::with_secret on http://10.0.0.5 returns http.key_on_plain_http (with recovery text) before any send (ScriptedTransport records 0 sends); http://127.0.0.1 and http://localhost with a key, https://host with a key, and http://10.0.0.5 without a key are accepted (tests key_refused_on_plain_http_remote, key_allowed_on_loopback, key_allowed_on_https, plain_http_remote_without_key_allowed).
- [ ] A5-25 REQ-SAG-0015: sc-agent-http: Timeouts::with_request(d) keeps the connect timeout and sets the request deadline, so one UreqTransport per local runner carries that runner's own request timeout (tests timeouts_with_request, per_runner_transport_deadline).
- [ ] A5-26 REQ-SAG-0015: sc-agent-http: HttpRequest::with_deadline(d) replaces the transport's request timeout for that request's sends only (accepted up to Timeouts::DEADLINE_MAX = 10800 s, refused above it), and the 90 s start budget starts at the first send, so a load wait before it never counts toward the budget (tests request_deadline_overrides_transport, budget_starts_at_first_send).

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); decisions are in the design:
- Q1 http://: only through Endpoint::parse_local for configured local runners; providers use https (assistant).
- Q2 one-off calls hold off on CancelToken::wait_until on their own thread (assistant).
- Q3 402 and Anthropic's usage-limit 400 are final replies returned with the remote id and message after one send (owner: only timeouts are retried).
- Q4 timeouts are resent under max_sends; connect/DNS/TLS/other are returned after one send (owner).
- Q5 provider text is kept whole (REQ-SAG-0002).

Plan hardening round 1 (2026-11-10):
- Owner answer (2026-11-10): "The 5 retries are for busy which is much faster than 90 sec." The 90 s start budget and 120 s request timeout stay; a timed-out request ends with the timeout error when the budget or max_sends refuses the next attempt (A5-06).
- Round 2 (RBP-F009): admission moved to HttpClient::may_start / may_resend returning Start::{Go(SendPermit), Wait}; send_once consumes the SendPermit and returns Next::{Done { response, attempts }, ResendAt { at, pending }}. ProviderBusy.last is ProviderReply again (the last: None fallback is gone).
- Round 2: ProviderPause (REQ-SAG-0012), Retry-After floor 250 ms + 0-250 ms jitter, Timeouts::probe(), LogContext on every http.* event (RSH-009), elapsed_ms (RSH-010), OpenAI quota test (REQ-QA-017), cause -> cause_detail, recovery text, Display = id: message.
- Accepted risk (owner: resend on timeout): a POST resent after a timeout may run twice at the provider and be billed twice.
- REQ-SAG-0601 and REQ-SAG-0012 agree: a valid Retry-After is clamped to 0.25-30 s plus up to 250 ms jitter (A5-03).
- Feature `testing` overrides the earlier no-test-util decision (PLAN-CRIT-010; reason: five sprints would otherwise re-implement the same fakes).
- clippy.toml edit dropped: a-1 owns the file (PLAN-SCOPE-009).
- Round 3 (REQ-QA-R2-001): request-level REQ-SAG-0013 guard; HttpRequest::with_secret now returns Result<Self, HttpError> (callers in a-6/a-7 add `?`); new variant HttpError::KeyOnPlainHttp { host } id http.key_on_plain_http; 7 variants.
- REQ-SAG-0015: Timeouts::with_request(Duration), HttpRequest::with_deadline(Duration) / deadline(); budget starts at first send.
