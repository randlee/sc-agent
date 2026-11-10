<!-- Generated from bead agt-4p2.5 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-5: sc-agent-http
- id: `a-5` (bead `agt-4p2.5`)
- branch: `sprint/a-5-sc-agent-http`
- depends on: a-8, a-4
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
      artifact: sc_agent_log::{GeneralLogger, Level} (crates/sc-agent-log)
      trigger: dev-push
  requirements: [REQ-SAG-0601, REQ-SAG-0602, REQ-SAG-0002, REQ-SAG-0007, REQ-SAG-0008, REQ-SAG-0009, REQ-SAG-0010, REQ-SAG-0401]
  adrs: [ADR-SAG-0001, ADR-SAG-0004]
  does_not_close:
    - "REQ-SAG-0601 end to end (a decide send against a busy or timing-out provider, held off and resent through the real DecisionRunner): a-10 acceptance; the holdoff wait and cancel are a-9's"
    - "REQ-SAG-0602 end to end (decide reports http.quota_exhausted from a provider reply): a-10 acceptance"
```

# Sprint A-5: sc-agent-http (bead agt-4p2.5)

Governing: REQ-SAG-0601 (busy resend and holdoff), REQ-SAG-0602 (quota is not busy), REQ-SAG-0002. Also binding: ADR-SAG-0004 (layer: above sc-agent-types, sc-agent-secrets and sc-agent-log; below sc-decision-schema and sc-llm-provider), REQ-SAG-0009, REQ-SAG-0010, ADR-SAG-0001 (sans-IO backends, separate transport), REQ-SAG-0007 (`retry_after_seconds` unrounded), REQ-SAG-0401, REQ-SAG-0008 (logs through the injected GeneralLogger), owner-busy-resend, owner-no-sleep, owner-error-contract, owner-lowlevel-no-tokio, owner-architecture.

Copy script: `docs/plans/phase-a/copy/a-5.sh` (source sc-agent-ref@dce239a; dry run 2026-11-09, reconciliation: 26 copy steps, 13 files, 2075 lines; no `sleep(` in any copied line).

What the crate is: a sync HTTP layer with no queue. It owns `HttpRequest`/`HttpResponse` (byte bodies, so multipart works), sends one request once (`Transport`), classifies the reply (busy / quota / final), and for a busy reply returns a not-before `Instant` for the resend under `ResendPolicy`. It never waits and takes no `CancelToken`: the holdoff wait and cancellation live in the caller's queue (the runner, A-9, uses `CancelToken::wait_until` and `on_cancel`). Every final reply that is neither busy nor quota is returned to the caller as `Next::Done(HttpResponse)`, whatever its status, so the backend decodes the API's own error bodies (Jev 400/422 are sc-decision-schema types; LLM providers' errors are read by sc-llm-provider with `ProviderReply::read`).

Main departures from the old crate:
- Old `retry.rs:247-268` (`Hooks::wait`, `thread::sleep` slices) and `retry_loop` (507-626) are not copied; `send_once` returns `Next::ResendAt(instant)`.
- Old `MAX_SENDS = 5` cap and `clamp(1, MAX_SENDS)` removed; `max_sends` is any `NonZeroU32`, default 5.
- A valid `Retry-After` is used exactly (clamped 0-30 s), without jitter; jitter is only for the fallback holdoffs (REQ-SAG-0601).
- Old `RetryAfter::report_seconds` rounded; the error now carries the parsed `f64`.
- Old `api_failure` status mapping is not copied: non-busy statuses go back to the backend.
- `HeaderName::new` and `Endpoint::loopback_http` panicked through `assert!`; both go.
- The old `test-util` feature is not copied. Instead the crate has a cargo feature `testing` (not default) exposing `sc_agent_http::testing::{ScriptedTransport, BlockingTransport}`. This overrides the earlier "no test-util feature" decision (plan hardening, PLAN-CRIT-010): five sprints (a-2, a-6, a-7, a-9, a-10) need the same transport fakes and would otherwise each re-implement them. Consumers take `sc-agent-http = { path = .., features = ["testing"] }` as a dev-dependency only. Tests still drive `send_once` with an explicit clock.
- A timed-out send is resent unchanged at once, counting toward `max_sends`; connect, DNS, TLS and other transport failures are returned after one send; every other remote error (402, Anthropic's usage-limit 400, 503, ...) is a final reply returned to the backend with the remote id and message (REQ-SAG-0601).
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
- `transport.rs`: drop `MIN`, `MAX_CONNECT`, `MAX_REQUEST`, `clamp`; public fields, `Default` 10 s / 120 s. Drop ADR-SAG-0007/RSH-002 references.
- `ureq_transport.rs`: keep `AgentConfig::build` (no redirects, `http_status_as_error(false)`, idle pool 64, versioned User-Agent), `LookupResolver` (DNS failure -> `Connect`), `secret_header_value`, `expose` (the single `ApiKey::as_str` site, `#[expect(clippy::disallowed_methods, reason = ..)]`), `read_response` (8 MiB cap), `ureq_error_kind`, `io_error_kind`. Delete `loopback_for_tests`, `is_valid_url`, `SecretHeader::matches`, the `https_only` refusal (164-169). `BodyExceedsLimit` -> `ResponseTooLarge`; other errors and body read failures -> `Network { kind, .. }`. Port the unit tests (380-561), keep `test_endpoint_parse_refuses_http`, add a `parse_local` accepts-http test.
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
pub use resend::{Attempts, HttpClient, Next, ResendPolicy};
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
pub struct HttpRequest { method: Method, endpoint: Endpoint, headers: Vec<(String, String)>, secret: Option<SecretHeader>, body: Arc<[u8]> }
impl HttpRequest {
    pub fn new(method: Method, endpoint: Endpoint, body: impl Into<Arc<[u8]>>) -> Self;
    pub fn with_header(self, name: impl Into<String>, value: impl Into<String>) -> Self;
    pub fn with_secret(self, secret: SecretHeader) -> Self;
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
    /// One send. Every HTTP status is Ok; only transport failures are Err.
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError>;
}
impl<T: Transport + ?Sized> Transport for &T {}
impl<T: Transport + ?Sized> Transport for Arc<T> {}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timeouts { pub connect: Duration, pub request: Duration }
impl Default for Timeouts { /* 10 s, 120 s */ }

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

/// Busy-resend state of one request; the queue entry owns it between sends.
#[derive(Debug, Clone, Default)]
pub struct Attempts { first_send: Option<Instant>, sends: u32, last: Option<Last> }
#[derive(Debug, Clone)]
enum Last { Busy(ProviderReply), Timeout(String) }        // private: what the latest unfinished send got
impl Attempts {
    pub fn new() -> Self;
    pub fn sends(&self) -> u32;
    /// May another send of this request start now? Err(Sends) when `sends >= policy.max_sends()`;
    /// Err(Budget) when `first_send` is set and `now - first_send >= BUDGET` (90 s). Pure; send_once step 1
    /// calls it, and the runner (a-9) calls it before requeueing a schema-mismatch resend.
    pub fn may_start(&self, policy: &ResendPolicy, now: Instant) -> Result<(), BusyReason>;
}

#[derive(Debug)]
pub enum Next {
    /// A final reply (any status that is neither busy nor quota). The backend decodes it.
    Done(HttpResponse),
    /// Busy: requeue and send again, unchanged, no earlier than this instant.
    ResendAt(Instant),
}

#[derive(Clone)]
pub struct HttpClient { transport: Arc<dyn Transport>, policy: ResendPolicy, log: GeneralLogger }
impl HttpClient {
    /// `log` is the caller's GeneralLogger, default or injected (REQ-SAG-0008; sc-agent-log is below this crate).
    pub fn new(transport: Arc<dyn Transport>, policy: ResendPolicy, log: GeneralLogger) -> Self;
    pub fn policy(&self) -> ResendPolicy;
    pub fn transport(&self) -> &Arc<dyn Transport>;      // one-shot probes (A-6 discovery)
    /// One send. `now` is read for the budget check before the send and the holdoff start after it.
    /// Never waits. Every Err is redacted.
    pub fn send_once(&self, request: &HttpRequest, attempts: &mut Attempts, now: &dyn Fn() -> Instant) -> Result<Next, HttpError>;
}
```

`send_once` order (each step is one REQ line):
1. `attempts.may_start(&self.policy, now())`; on `Err(reason)` no send starts: latest `Last::Busy(reply)` -> `Err(ProviderBusy { reason, attempts: sends, retry_after_seconds: reply.retry_after_seconds, last: Some(reply) })`; `Last::Timeout(message)` -> `Err(Network { kind: Timeout, attempts: sends, message })` (a request whose last attempt timed out ends with the timeout error, never `ProviderBusy`; `ProviderBusy { reason }` only when the last reply was busy); none (only a caller resending after a `Done`, which a-9 guards with `may_start`) -> `Err(ProviderBusy { reason, attempts: sends, retry_after_seconds: None, last: None })` (attempts start only within 90 s and `max_sends`).
2. `transport.send(request)`; `sends += 1` (and `first_send` set on the first). `Err(Network { kind: Timeout })` -> `last = Some(Last::Timeout(message))`; with `sends < max_sends` -> `Ok(ResendAt(now()))` (resent unchanged at once; a later step-1 budget check still applies); when sends or budget run out the timeout error is returned with `attempts`. Every other `Err` (connect, DNS, TLS, other) is returned after this send (REQ-SAG-0601).
3. Status not in `BUSY_STATUSES` -> `Ok(Next::Done(response))`.
4. Busy and `is_quota_reply` -> `Err(QuotaExhausted { attempts, reply })` at once (REQ-SAG-0602).
5. Busy: `last = Some(Last::Busy(ProviderReply::read(&response, request.secret().map(SecretHeader::key))))`. If `sends >= max_sends` -> `Err(ProviderBusy { reason: Sends, attempts, retry_after_seconds, last: Some(reply) })`. Holdoff = valid `Retry-After` -> `RetryAfter::wait()` (0-30 s, no jitter); else `backoff(sends) + jitter()` (1, 2, 4, 8, 16, 30 ... s + 0-250 ms). `remaining = BUDGET - (now() - first_send)`; zero -> `ProviderBusy { reason: Budget }`; else `Ok(ResendAt(now() + min(holdoff, remaining)))`.

Logging (ANC-007, REQ-SAG-0008) through `log.write(level, "sc_agent_http", action, fields)` (never a body, header value or key; a log Err never changes the result, the logger counts it): `http.send` (method, endpoint log_form, attempt, max_sends; this is the "request sent" record for every backend: sans-IO crates send through `HttpClient` and do not log sends themselves, plan-hardening decision 4), `http.response` (status, attempt), `http.busy` (status, attempt, holdoff_ms, retry_after_honoured), `http.busy_exhausted`, `http.budget_exhausted`, `http.quota_exhausted`, `http.timeout_resend` (attempt), `http.transport_failed` (error id).

How the runner (A-9) uses it, no sleep (one-off calls: see Decisions): take job -> `send_once` -> `ResendAt(t)` puts the job back in its queue keyed by `t`; idle workers wait with `Condvar::wait_timeout` to the earliest `t`, and a `CancelToken::on_cancel` hook wakes them at once; a cancelled job is never sent again.

```rust
// src/testing.rs (feature "testing"; copied C13 + new BlockingTransport)
/// Answers from a script, first entry first; records every request; exhausted -> Network { Connect, "script exhausted" }. Clones share state.
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

## 4. Error list (`src/error.rs`, full; ErrorId from A-1, Display = detail())

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
    /// `last` is the latest busy reply; None only when no busy reply was seen (a resend after a Done that may_start refused).
    ProviderBusy { reason: BusyReason, attempts: u32, retry_after_seconds: Option<f64>, last: Option<ProviderReply> }, // "http.provider_busy"
    /// Busy status whose body marks quota, credit or spend exhaustion (REQ-SAG-0602).
    QuotaExhausted { attempts: u32, reply: ProviderReply },        // "http.quota_exhausted"
    /// Transport failure. Timeout: resent under max_sends, returned when they run out; Connect (incl. DNS), Tls, Other: returned after one send.
    Network { kind: NetworkKind, attempts: u32, message: String }, // "http.network"
    /// The body exceeded the 8 MiB read cap.
    ResponseTooLarge { status: u16, limit_bytes: u64 },            // "http.response_too_large"
    /// The URL does not parse, has no host, or its scheme is refused (`parse`: https; `parse_local`: http or https).
    InvalidEndpoint { url: String, reason: String },               // "http.invalid_endpoint"
    /// A header name or value the HTTP builder refuses.
    InvalidRequest { message: String },                            // "http.invalid_request"
}
impl HttpError {
    pub(crate) fn map_text(self, map: &impl Fn(&str) -> String) -> Self;
}
impl ErrorId for HttpError {}   // cause(): Network/InvalidRequest keep the ureq/io text in `message`; no lower sc-agent error
impl std::error::Error for HttpError {}
```

6 variants. Cancellation is not an HTTP error: the queue that waits owns it (DecisionError::Cancelled, A-2).

## 5. Config keys

`max_sends` (A-4 `AgentConfig.max_sends`, one value, default 5) -> `ResendPolicy::new`. `Timeouts` stays code-level.

## 6. Tests

Each test names the acceptance line (A5-nn) it proves; the assertion itself is in that line.

| Test (file) | Proves |
|---|---|
| `busy_every_time_default`, `busy_every_time_max_two` (tests/resend.rs) | A5-02 |
| `retry_after_exact`, `retry_after_capped`, `retry_after_invalid_falls_back` (tests/resend.rs; `retry_after_parser` unit) | A5-03 |
| `budget_stops_next_start`, `may_start_sends_and_budget`, `retry_after_unrounded`, `resend_same_bytes` (tests/resend.rs) | A5-04 |
| `holdoff_is_resend_at` (tests/resend.rs) | A5-05 |
| `timeout_resent_under_max_sends`, `timeout_then_budget_ends_with_timeout`, `connect_not_resent` (tests/resend.rs); `refused_connection_is_connect` (tests/ureq.rs, real socket) | A5-06 |
| `final_402_and_usage_400` (tests/resend.rs) | A5-07 |
| `quota_429_is_final`, `rate_limit_429_resends` (tests/resend.rs; documented-shape unit, C8) | A5-08 |
| `final_replies_unchanged` (400, 402, 422, 503) | A5-09 |
| `multipart_bytes_sent` (tests/ureq.rs over `serve_once`: headers, secret, byte body) | A5-10 |
| `http_error_ids` | A5-11 |
| `key_never_leaks` (key in a transport error and a busy reply message), `long_provider_text_kept` | A5-12 |
| `test_endpoint_parse_refuses_http`, `parse_local_accepts_http` (unit) | A5-13 |
| `events_logged` (injects `sc_agent_log::testing::MemoryLog`; also a failing logger that leaves the result unchanged) | A5-14, A5-15 |
| `lower_layer_cause_kept` (ureq error through `UreqTransport` to a closed port) | A5-17 |
| `scripted_and_blocking_doubles` (feature `testing`: script order, exhaustion error, BlockingTransport released only by the test) | A5-19 |
| Unit tests copied in C5, C6, C8, C9, C12, ported | A5-03, A5-06, A5-08, A5-12 |

Not proved by a test here: A5-01 (commit history), A5-16/A5-18 (Required validation commands below).

`timeout_then_budget_ends_with_timeout`: the scripted transport returns `Network { Timeout }` and advances the explicit clock by 120 s (the default request timeout) on that send; the next `send_once` makes no second send (`may_start` -> Budget) and returns the timeout error, not `http.provider_busy`.

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
- [ ] A5-03 REQ-SAG-0601: sc-agent-http: HttpClient::send_once returns ResendAt(t + 3 s) exactly for Retry-After: 3 and ResendAt(t + 30 s) for Retry-After: 99; for Retry-After: abc (and absent) the holdoffs are 1, 2, 4, 8 s (then doubling, capped 30 s) each with at most 250 ms jitter; Retry-After is valid only as 1-32 ASCII digits with an optional fraction.
- [ ] A5-04 REQ-SAG-0601: sc-agent-http: Attempts::may_start returns Err(Budget) once 90 s have passed since the first send and Err(Sends) once max_sends sends were made, and send_once starts no send then (reason budget); ProviderBusy carries reason, attempts and the last retry_after_seconds unrounded (REQ-SAG-0007); a busy request is resent unchanged (same body bytes).
- [ ] A5-05 REQ-SAG-0601: sc-agent-http: the holdoff is returned as a not-before Instant (Next::ResendAt); sc-agent-http never waits and takes no CancelToken (the queue that waits owns cancellation, a-9).
- [ ] A5-06 REQ-SAG-0601: sc-agent-http: a send that times out is resent unchanged by HttpClient::send_once and counts toward max_sends (a scripted transport timing out instantly every time is sent 5 times by default); no attempt starts after the 90-second budget, and a request whose last attempt timed out ends with the timeout error (http.network, kind timeout), never http.provider_busy (scripted clock: timeout taking 120 s -> no second send -> timeout error); a connect failure returns http.network after one send (real refused-connection test over UreqTransport); DNS and TLS failures are not resent.
- [ ] A5-07 REQ-SAG-0601: sc-agent-http: a 402 response (and Anthropic's usage-limit 400) is returned by send_once after one send as Next::Done with the remote error id and message unchanged; no remote error other than a busy response is resent.
- [ ] A5-08 REQ-SAG-0602: sc-agent-http: send_once returns http.quota_exhausted after one send for a 429 with the documented Anthropic quota-exhausted body; a 429 with an unrecognized body returns ResendAt.
- [ ] A5-09 sc-agent-http: every final reply that is neither busy nor quota (e.g. 400, 402, 422, 503) is returned by send_once as Next::Done with the response unchanged, so the backend decodes it.
- [ ] A5-10 sc-agent-http: HttpRequest carries a byte body (Arc<[u8]>) that UreqTransport sends unchanged, so multipart bodies can be sent; HttpRequest/HttpResponse/ResendPolicy/Attempts are public here for sc-decision-schema, the backends and the runner.
- [ ] A5-11 REQ-SAG-0002: sc-agent-http: HttpError implements ErrorId with 6 distinct http.* ids; no assert!/unreachable!/panic in non-test code; clippy with the workspace denies passes.
- [ ] A5-12 Secrets: sc-agent-http: no error text, Debug output or log event contains the API key (test with the key in a transport error and a reply message); ApiKey::as_str is called only in ureq_transport (clippy disallowed-methods rule in a-1's clippy.toml); provider text is redacted, never truncated (REQ-SAG-0002).
- [ ] A5-13 sc-agent-http: Endpoint::parse refuses http://; Endpoint::parse_local accepts it (tests).
- [ ] A5-14 REQ-SAG-0008: sc-agent-http: send, response, busy, exhaustion, quota and transport-failure events go to the GeneralLogger passed to HttpClient::new; no body, header value or key is logged; a log failure never changes a send result.
- [ ] A5-15 ANC-007: sc-agent-http's HttpClient::send_once writes http.send (the "request sent" record for every backend), http.response, http.busy, http.busy_exhausted, http.budget_exhausted, http.quota_exhausted, http.timeout_resend and http.transport_failed to the GeneralLogger passed to HttpClient::new; a test injects sc_agent_log::testing::MemoryLog, runs busy, quota, timeout and connect-failure sends and finds each record with no key or body.
- [ ] A5-16 ADR-SAG-0004 / REQ-SAG-0009 / REQ-SAG-0010: sc-agent-http: no ureq type in a public signature; cargo tree -p sc-agent-http lists no tokio and no sc-decision-* or sc-llm-provider crate; no sleep call.
- [ ] A5-17 REQ-SAG-0002: sc-agent-http: a test makes the transport fail with a lower-layer (ureq) error and asserts the returned HttpError's ErrorDetail chain carries that cause's id and message.
- [ ] A5-18 ADR-SAG-0004: sc-agent-http sits in the http layer; its only sc-agent crate dependencies are the lower-layer crates sc-agent-types, sc-agent-secrets and sc-agent-log, and it depends on no decision-schema, decision-backend, sc-llm-provider or runner crate, checked by `cargo tree -p sc-agent-http` (with and without `--features testing`).
- [ ] A5-19 sc-agent-http: feature `testing` (not default) exposes sc_agent_http::testing::{ScriptedTransport, RecordingTransport, BlockingTransport}; tests/support/ is gone; BlockingTransport returns only after the test calls release() (no sleep).

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); decisions are in the design:
- Q1 http://: only through Endpoint::parse_local for configured local runners; providers use https (assistant).
- Q2 one-off calls hold off on CancelToken::wait_until on their own thread (assistant).
- Q3 402 and Anthropic's usage-limit 400 are final replies returned with the remote id and message after one send (owner: only timeouts are retried).
- Q4 timeouts are resent under max_sends; connect/DNS/TLS/other are returned after one send (owner).
- Q5 provider text is kept whole (REQ-SAG-0002).

Plan hardening round 1 (2026-11-10):
- Owner answer (2026-11-10): "The 5 retries are for busy which is much faster than 90 sec." The 90 s start budget and 120 s request timeout stay; a timed-out request ends with the timeout error when the budget or max_sends refuses the next attempt (A5-06).
- Attempts::may_start added (PLAN-CRIT-003); ProviderBusy.last became Option<ProviderReply> (None only when no busy reply was seen).
- Feature `testing` overrides the earlier no-test-util decision (PLAN-CRIT-010; reason: five sprints would otherwise re-implement the same fakes).
- clippy.toml edit dropped: a-1 owns the file (PLAN-SCOPE-009).
