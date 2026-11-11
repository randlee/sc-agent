<!-- Generated from bead agt-4p2.6 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-6: sc-decision-systemone and sc-decision-local
- id: `a-6` (bead `agt-4p2.6`)
- branch: `sprint/a-6-decision-backends`
- depends on: a-2, a-4, a-5, a-8
- copy script: `docs/plans/phase-a/copy/a-6.sh`

## Description

sc-decision-systemone and sc-decision-local are sc-agent library crates; each provides a decision backend that implements the Backend trait from sc-decision-schema. Jev backend (encode/decode POST /v1/systemone, GET /v1/models). sc-decision-local: stub reusing the Jev encode/decode for any Jev-compatible local daemon from config by name (host and port in config): Strands Decider, Laya, Mapika/decider or generic; LocalDaemon trait whose main task is discovering whether a model is available/loaded. Records: REQ-SAG-0201, REQ-SAG-0801, ADR-SAG-0001.

## Design

```yaml
sprint:
  id: a-6
  closure_type: boundary
  target_boundary: sc-decision-systemone
  vertical_rationale: sc-decision-local is a thin stub that builds a SystemOneBackend and adds discovery over the same Jev encode/decode and canned replies, so a separate sprint would add an edge and a hand-off without shortening the critical path.
  owned_paths: [crates/sc-decision-systemone/**, crates/sc-decision-local/**]
  must_follow:
    - sprint: a-2
      artifact: sc_decision_schema::{Backend, DecisionRequest (private fields, DecisionRequest::new and accessors), ImagePart, SystemOneRequest, SystemOneResponse, ModelMetadataList, DecisionError} (crates/sc-decision-schema/src/)
      trigger: dev-push
    - sprint: a-5
      artifact: sc_agent_http::{HttpRequest, HttpResponse, HttpError, Endpoint, Transport, Timeouts::probe, HttpRequest::with_secret, HttpRequest::with_deadline} and sc_agent_http::testing::ScriptedTransport (feature `testing`)
      trigger: dev-push
    - sprint: a-4
      artifact: sc_agent_config::{RunnerKind (closed enum), RunnerHost} and sc_agent_secrets::ApiKey
      trigger: dev-push
    - sprint: a-8
      artifact: sc_agent_log::{GeneralLogger, Level, LogContext} and sc_agent_log::testing::MemoryLog (feature `testing`)
      trigger: dev-push
  requirements: [REQ-SAG-0201, REQ-SAG-0801, REQ-SAG-0001, REQ-SAG-0002, REQ-SAG-0007, REQ-SAG-0008, REQ-SAG-0009, REQ-SAG-0010, REQ-SAG-0013, REQ-SAG-0015, REQ-SAG-0401, REQ-SAG-0601, REQ-SAG-0011, REQ-SAG-0014]  # 0011: cancel ends a load wait (decision.cancelled); 0014: limits enforced in a-2 DecisionRequest::new, consumed by multipart encode
  adrs: [ADR-SAG-0001, ADR-SAG-0004]
  does_not_close:
    - "Discovery is a library capability of sc-decision-local; no decidr or Python operation calls it in this phase (REQ-SAG-0801 requires only the library report)."
    - "REQ-SAG-0201 end to end (a decision sent through decidr send over SystemOneBackend, or over LocalBackend for a local runner from config): a-10 acceptance."
    - "decidr models over SystemOneBackend::models_request/decode_models: a-10 acceptance."
    - "Busy resend and holdoff for these backends (REQ-SAG-0601): a-5 (sc-agent-http) and a-9 (runner); this sprint only shows 503 is a final reply."
    - "Rejecting an unknown local runner kind, bad host, port 0 or bad key_env (REQ-SAG-0013): a-4 at config load (RunnerKind is closed, so this crate has no unknown-kind error)."
    - "Per-runner load and request timeouts in config with longer defaults (REQ-SAG-0015), and calling LocalProbe::wait_until_loaded before a local runner's first decision: a-4 (config) and a-10 (decidr)."
    - "Building the probe transport with UreqTransport::new(Timeouts::probe()) for discover and decidr models: a-10 (the caller owns the transport)."
```

# Sprint A-6 design: sc-decision-systemone and sc-decision-local (bead agt-4p2.6)

Governing: REQ-SAG-0201, ADR-SAG-0001, REQ-SAG-0801 (local runners from config, discovery, no hard-coded host/port). Also binding: ADR-SAG-0004 (layer: above sc-decision-schema, below sc-decision-runner), REQ-SAG-0002, REQ-SAG-0001, REQ-SAG-0007, REQ-SAG-0401, REQ-SAG-0601 (busy handled by sc-agent-http and the runner, not here), owner-architecture, owner-system-one-backends, owner-local-decision-crate, owner-local-discovery, owner-no-sleep, owner-lowlevel-no-tokio.

- sc-decision-systemone: one sans-IO `Backend` for any System One server: Jev (`POST {base}/v1/systemone`, Bearer key), Strands Decider, Laya and Mapika/decider (same path, key optional); plus GET /v1/models encode/decode (used by `decidr models`, A-10).
- sc-decision-local: minimal stub. Builds a `SystemOneBackend` for `http://{host}:{port}` from a config `local_runners` entry (A-4: kind, host, port, model, key_env) and adds discovery: is the daemon up, is its configured model loaded. LocalBackend is sans-IO and reads no clock (ADR-SAG-0001): it builds the probe (`probe_request`) and reads the reply (`read_probe`); the `LocalProbe` driver sends it on the caller's `Transport` and `Clock` (`discover`, `wait_until_loaded`). One `LocalDaemon` implementation per daemon kind: `strands` (Strands Decider), `laya`, `mapika` (Mapika/decider), `generic`; a later daemon (e.g. 'ollaya') adds one. Runners come from config by name (REQ-SAG-0801). It runs through sc-decision-runner like any backend (as `Arc<dyn Backend>`, wired by decidr, A-10).

## 1. Copy script

`docs/plans/phase-a/copy/a-6.sh` (dry run 2026-11-09, reconciliation: 2 copy steps, 2 files). Copies only crates/sc-jev/LICENSE to both crates. No source is copied: the old Jev client is built on JudgeBackend, JevError, the canonical payload, TOML pins and the sleeping busy retry; its reusable part, the 6-line request build at client/mod.rs:234-240, is rewritten against the A-5 HttpRequest. sc-agent-ref has no discovery code.

## 2. Discovery research (read-only, 2026-11-09)

Strands Decider, github.com/strands-labs/strands-decider @846b69e, src/strands_decider/server.py:
- `GET /health` and `GET /ping`: 200 `{"status":"ok","model":<served name>,"checkpoint":<path or HF id>,...}`. No `/v1/models`. One model per process. No auth (binds 127.0.0.1, default port 8000).
- Binds only after model load, so while loading every request is a connect failure (http.network).
- `POST /v1/systemone` ignores the request `model`; errors are FastAPI `{"detail": ...}` (422 with a string detail). Adds top-level `latency_ms`.

Laya, github.com/NandhaKishorM/laya @b8a0f09, laya/serve.py and docs/http-api.md:
- `GET /health` always 200. With `LAYA_API_KEY` set and no/wrong bearer: `{"status":"ok"}` only; else `{"status":"ok","loaded":[<checkpoint names>],...}`. `loaded` is empty until a request builds a checkpoint; checkpoints load on demand.
- No `/v1/models`. A Jev id or `convaiinnovations/laya` auto-routes; an unpublished id is a 422.
- Bearer on every route except liveness when `LAYA_API_KEY` is set. `503 {"detail":"server busy, try again later"}` past `LAYA_MAX_CONCURRENT`. Extra response fields: `routing`, `x_jev_confidence`, `usage.{truncated,...}`.

Mapika/decider, github.com/Mapika/decider @91b6fa9, decider/serve.py:
- `POST /v1/systemone` (Jev wire format), `GET /v1/models` (`{"models":[{name,description,release_date}]}`), `GET /health` 200 `{"ok": bool, "model": <name>, "device", ...}`; default port 8000; one model per process; no auth.
- Errors: 422 bad question, 413 too large, 503 queue full.

Generic Jev-compatible daemon: the only documented discovery endpoint is TypeSafe `GET /v1/models`.

Busy: Laya and Mapika/decider answer 503 when full. REQ-SAG-0601 defines busy as 429/502/529, so a 503 is a final reply, decoded as decision.api_status with its body and not resent (assistant decision under the record).

## 3. Discovery design

| `kind` | Probe | Up | Loaded models | Configured model check |
|---|---|---|---|---|
| `strands` | `GET {base}/health` | 200 | `[model, checkpoint]` | equal to either -> `Loaded`; else `local.model_not_served` |
| `laya` | `GET {base}/health`, Bearer if a key is configured | 200 | `loaded` | in `loaded` -> `Loaded`; else `LoadsOnDemand`; `loaded` absent -> `Unlisted` |
| `mapika` | `GET {base}/health` | 200 | `[model]` | `ok` true and equal -> `Loaded`; `ok` false -> `NotReady`; else `local.model_not_served` |
| `generic` | `GET {base}/v1/models`, Bearer if a key is configured | any 2xx | `models[].name` | listed -> `Loaded`; else `local.model_not_served`; 404/405 -> `Unlisted`; other status -> `local.probe_status`; a 2xx that is not a ModelMetadataList -> `local.decision` (ResponseParse) |

Down = the probe's transport error, returned as `LocalError::Http(HttpError)` (id `local.http`, cause `http.network`). Any other status -> `ProbeStatus`. Discovery is one GET through `Transport::send`, sent once (not a decision, no busy resend, nothing to wait on).

## 4. Exact Rust surface (bodies elided)

### sc-decision-systemone

```rust
pub use backend::{SystemOneBackend, TYPESAFE_BASE_URL, SYSTEMONE_PATH, MODELS_PATH};
pub use base::BaseUrl;

pub const TYPESAFE_BASE_URL: &str = "https://api.typesafe.ai";

/// Backend for any System One server: POST {base}/v1/systemone, GET {base}/v1/models.
#[derive(Debug, Clone)]
pub struct SystemOneBackend { systemone: Endpoint, models: Endpoint, key: Option<ApiKey> }
impl SystemOneBackend {
    /// `base` has no trailing path, e.g. TYPESAFE_BASE_URL. https only (Endpoint::parse).
    pub fn new(base: &str, key: Option<ApiKey>) -> Result<Self, DecisionError>;   // Http(InvalidEndpoint); also refuses a query or fragment
    pub fn at(base: &BaseUrl, key: Option<ApiKey>) -> Result<Self, DecisionError>;
    /// A configured local runner's base, e.g. "http://127.0.0.1:8000"; http or https (Endpoint::parse_local).
    pub fn local(base: &str, key: Option<ApiKey>) -> Result<Self, DecisionError>;
    /// GET /v1/models; Bearer if key, attached with `HttpRequest::with_secret(..)?` (a-5; http.key_on_plain_http -> DecisionError::Http).
    pub fn models_request(&self) -> Result<HttpRequest, DecisionError>;
    pub fn decode_models(&self, response: HttpResponse) -> Result<ModelMetadataList, DecisionError>;
}
impl Backend for SystemOneBackend {
    /// JSON when request.image_parts() is empty, else multipart/form-data (part "request" application/json
    /// first, then one part per image: name = ImagePart.name, image/png or image/jpeg; boundary "sc-agent-<ULID>").
    /// Headers: Content-Type; Bearer secret header when key is Some, attached with `HttpRequest::with_secret(..)?`
    /// (a-5 refuses a key on a plain-http non-loopback endpoint: DecisionError::Http(http.key_on_plain_http)); TypeSafe-Danger-Zone-Images: true when
    /// needs_images_header(); TypeSafe-Danger-Zone-Long-Context: true when long_context.
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError>;
    /// 2xx -> SystemOneResponse::from_json; 400 -> ApiBadRequest (ApiStatus if another shape);
    /// 422 -> ApiValidation (ApiStatus for Strands/Laya string details); other (incl. 402, 503) -> ApiStatus, body whole. `request` unused.
    /// `ctx` (a-2 trait parameter) is unused: sc-decision-systemone writes no records.
    fn decode(&self, response: HttpResponse, request: &DecisionRequest, ctx: &LogContext) -> Result<SystemOneResponse, DecisionError>;
}
/// Writes the parts in `request.image_parts()` order. DecisionRequest is valid by construction (a-2
/// DecisionRequest::new), so neither crate checks part names or calls a validator.
fn body(json: &str, parts: &[ImagePart], boundary: &str) -> Vec<u8>;   // private, infallible; part names escaped (" -> %22, CR -> %0D, LF -> %0A) and also sent as filename
```

No error enum of its own, under the ADR-SAG-0004 exemption (amended 2026-11-09): a crate whose public functions all return the error type fixed by a trait it implements needs no union; every sc-decision-systemone failure is a `DecisionError` (the Backend trait fixes it); an invalid base URL is the A-5 `HttpError::InvalidEndpoint` wrapped as `DecisionError::Http`.

### sc-decision-local

```rust
pub use backend::{LocalBackend, LocalTimeouts, LOAD_POLL_INTERVAL, MAX_LOAD_WAIT};
pub use probe::LocalProbe;
pub use daemon::{Availability, DaemonKind, LocalDaemon, ModelState};
pub use error::{LocalError, ParseCause};
pub use generic::Generic;
pub use laya::Laya;
pub use mapika::Mapika;
pub use strands::Strands;

mod sealed { pub trait Sealed {} }   // private module: only this crate implements Sealed

/// One daemon kind's discovery, sans-IO. Sealed (private supertrait): the implementors are the four
/// kinds below; a new daemon is added inside this crate as a DaemonKind variant (DaemonKind::daemon is the extension path).
pub trait LocalDaemon: sealed::Sealed + Send + Sync + fmt::Debug {
    fn kind(&self) -> DaemonKind;
    /// Bearer (Laya, Generic, when a key is set) via `HttpRequest::with_secret(..)?` -> LocalError::Http.
    fn probe(&self, base: &BaseUrl, key: Option<&ApiKey>) -> Result<HttpRequest, LocalError>;
    fn availability(&self, response: &HttpResponse, model: &ModelId) -> Result<Availability, LocalError>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DaemonKind { Strands, Laya, Mapika, Generic }
/// Total: RunnerKind is a closed enum in sc-agent-config (a-4, not #[non_exhaustive]), parsed and checked at
/// config load (REQ-SAG-0013), so a new runner kind is a compile error here, not a runtime error.
impl From<RunnerKind> for DaemonKind {}
impl DaemonKind {
    pub fn daemon(self) -> Box<dyn LocalDaemon>;
    pub const fn as_str(self) -> &'static str;                       // "strands" | "laya" | "mapika" | "generic" (log field)
}

/// The daemon is up (the probe got an HTTP answer it understood).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Availability { pub daemon: DaemonKind, pub loaded: Vec<String>, pub model: ModelState }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelState { Loaded, LoadsOnDemand, NotReady, Unlisted }

#[derive(Debug, Clone, Copy, Default)] pub struct Strands;
#[derive(Debug, Clone, Copy, Default)] pub struct Laya;
#[derive(Debug, Clone, Copy, Default)] pub struct Mapika;
#[derive(Debug, Clone, Copy, Default)] pub struct Generic;
// each: impl sealed::Sealed for X {} and impl LocalDaemon for X {}
// private serde shapes (not Jev API types): StrandsHealth { model, checkpoint }, LayaHealth { loaded: Option<Vec<String>> }, MapikaHealth { ok: bool, model: Option<String> };
// Generic decodes sc_decision_schema::ModelMetadataList.

/// A System One backend at http://{host}:{port} with discovery of its daemon; arguments come from a config
/// `local_runners` entry (A-4). This crate reads no config and hard-codes no host or port. Sans-IO (ADR-SAG-0001):
/// no I/O and no clock; the daemon probe is built by `probe_request`, read by `read_probe` and sent by `LocalProbe`.
/// Debug is written by hand: it prints daemon, base, model, `key: <set|unset>`, timeouts and served_once (no logger: the backend holds none).
pub struct LocalBackend { daemon: Box<dyn LocalDaemon>, base: BaseUrl, model: ModelId, key: Option<ApiKey>, backend: SystemOneBackend,
    timeouts: Option<LocalTimeouts>, served_once: AtomicBool }
/// REQ-SAG-0015: local runners get their own, longer timeouts, and model loading does not count toward the request timeout.
/// Values come from the caller (a-10 from config, a-4); this crate has no default and reads no config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTimeouts { pub load: Duration, pub request: Duration }
pub const LOAD_POLL_INTERVAL: Duration = Duration::from_secs(1);
impl fmt::Debug for LocalBackend { /* as above */ }
impl LocalBackend {
    /// base = http://{host}:{port}; the backend is SystemOneBackend::local(base, key). No timeouts until with_timeouts.
    /// A key on a plain-http non-loopback host (e.g. 10.0.0.5) is refused by `HttpRequest::with_secret` (a-5,
    /// `http.key_on_plain_http`) inside encode, surfaced as DecisionError::Http before any send (REQ-SAG-0013).
    /// decidr passes `DaemonKind::from(runner.kind)`, `&runner.host`, `runner.port` and `runner.model` from a LocalRunner.
    /// host and port are already validated at config load (a-4 RunnerHost: hostname or IP only; NonZeroU16), so new checks neither.
    pub fn new(kind: DaemonKind, host: &RunnerHost, port: NonZeroU16, model: ModelId, key: Option<ApiKey>) -> Result<Self, LocalError>;
    pub fn base(&self) -> &str;
    pub fn kind(&self) -> DaemonKind;
    /// The configured model.
    #[must_use] pub fn model(&self) -> &ModelId;
    /// REQ-SAG-0015 timeouts for this runner, used for the load-on-first-request deadline (encode). Without them no
    /// deadline is set. LocalProbe::wait_until_loaded takes its load_timeout as an argument (the caller passes timeouts.load).
    pub fn with_timeouts(self, timeouts: LocalTimeouts) -> Self;
    /// The daemon kind's probe request (sans-IO): the one GET that tells whether the daemon is up and what it has
    /// loaded, with the key attached as Bearer by kinds that check it.
    /// Err: LocalError::Http when the probe URL is invalid, or a key would go to a plain-http host that is not loopback
    /// (`http.key_on_plain_http`, REQ-SAG-0013).
    pub fn probe_request(&self) -> Result<HttpRequest, LocalError>;
    /// What the transport's `reply` to `request` (from probe_request) says about the daemon and the configured model
    /// (sans-IO). A transport error is redacted of the probe's key exactly as HttpClient::send_once redacts its errors
    /// (`HttpError::redacted_for(request)`); with the key attached, every copy of it in the reply text the result carries
    /// (error bodies and messages, loaded and served model names) is redacted too.
    /// Err: LocalError::Http for a transport error (a daemon that is not listening is cause `http.network`); otherwise
    /// as LocalDaemon::availability.
    pub fn read_probe(&self, request: &HttpRequest, reply: Result<HttpResponse, HttpError>) -> Result<Availability, LocalError>;
}

/// Sends a LocalBackend's daemon probe: once (`discover`) or until the model is loaded (`wait_until_loaded`). The I/O
/// driver of the sans-IO backend (ADR-SAG-0001): it owns the caller's transport, clock and optional logger, and builds
/// and reads the probe through `LocalBackend::probe_request` / `read_probe`, so a transport error is redacted of the
/// key by `HttpError::redacted_for`. `transport` is the caller's probe transport, built with
/// `UreqTransport::new(Timeouts::probe())` (connect 5 s, request 10 s; a-5), so a /health answer is never waited on for
/// the 120 s decision timeout; `clock` is Clock::system() in production and a ManualClock in tests (a-1), so no load wait
/// depends on real time in a test. Debug prints `log: <bool>` and never a key.
pub struct LocalProbe<'a> { backend: &'a LocalBackend, transport: &'a dyn Transport, clock: Clock, log: Option<GeneralLogger> }
impl<'a> LocalProbe<'a> {
    /// A probe of `backend`'s daemon, sent on `transport`, timed by `clock`; no logger until with_log.
    #[must_use] pub fn new(backend: &'a LocalBackend, transport: &'a dyn Transport, clock: Clock) -> Self;
    /// The caller's GeneralLogger (default or injected, REQ-SAG-0008); the probe writes its local.* records to it.
    #[must_use] pub fn with_log(self, log: GeneralLogger) -> Self;
    /// Sends the probe once (no busy resend) and, when a logger is set, writes one `local.discover` record
    /// carrying `ctx`.
    pub fn discover(&self, ctx: LogContext) -> Result<Availability, LocalError>;
    /// REQ-SAG-0015: wait for the configured model to load before the first decision, so loading never counts toward
    /// the request timeout. Probes the daemon every LOAD_POLL_INTERVAL of the probe's clock, waiting between probes with
    /// `Clock::wait_until(cancel, next_poll)` (a-1; no sleep).
    /// Ok when a probe reports ModelState::Loaded, or at once for Generic Unlisted (cannot tell) and for kinds that load
    /// on the first request (Laya: ModelState::LoadsOnDemand, see encode). A connect failure (Strands binds only
    /// after load) or Mapika NotReady means "not loaded yet": keep polling.
    /// `load_timeout` is clamped to MAX_LOAD_WAIT; the deadline is checked after each probe and the last probe is sent at
    /// the deadline, so the wait can end later than `load_timeout` by at most one probe's duration.
    /// Err: DecisionError::Backend { backend: "local", detail: LocalError::LoadTimeout{..}.detail() } once `load_timeout`
    /// has passed; DecisionError::Cancelled { sends: 0 } when cancel fires (`Wait::Cancelled`); ModelNotServed,
    /// ProbeStatus and ProbeParse end the wait at once as DecisionError::Backend with that detail.
    /// Writes one `local.load_wait` record per probe (state or error id, waited_ms) and one at the end (outcome);
    /// its probes do not also write local.discover.
    pub fn wait_until_loaded(&self, cancel: &CancelToken, load_timeout: Duration, ctx: LogContext) -> Result<(), DecisionError>;
}
impl Backend for LocalBackend {
    /// Delegates to SystemOneBackend::encode. For a load-on-first-request kind (Laya) with timeouts set, every request
    /// encoded before the first 2xx reply is marked `.with_deadline(timeouts.load + timeouts.request)` (a-5), so the
    /// on-demand checkpoint load fits inside that request's deadline; after the first 2xx (`served_once`) requests carry
    /// no deadline override. Parallel first requests are all marked (none can be cut off by another's load).
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError>;
    /// Delegates to SystemOneBackend::decode with ctx; a 2xx sets `served_once`. Writes no local.* record.
    fn decode(&self, response: HttpResponse, request: &DecisionRequest, ctx: &LogContext) -> Result<SystemOneResponse, DecisionError>;
}
```

Load on first request, per kind (REQ-SAG-0015): Laya loads checkpoints on demand (section 2), so LocalProbe::wait_until_loaded returns Ok at once on LoadsOnDemand and the first request gets the load + request deadline. Strands Decider and Mapika/decider load at start (Strands is unreachable until loaded; Mapika reports `ok: false`), so LocalProbe::wait_until_loaded polls. Generic: a listed model is Loaded; Unlisted returns Ok at once.

Open or sealed, per public trait this sprint touches: `LocalDaemon` sealed (above). `Backend` (a-2) is open, implemented here by SystemOneBackend and LocalBackend. `ErrorId` (a-1) is open to workspace crates, implemented by LocalError; `detail()` is not overridden.

## 5. Error enum (full, sc-decision-local; ErrorId, Display prints `<id>: <message>` only (chain via source()/detail()), no thiserror)

```rust
#[derive(Debug)]
#[non_exhaustive]
pub enum LocalError {
    /// The probe could not be sent: daemon down (http.network) or bad endpoint; wrapped unchanged.
    Http(HttpError),                                                                    // "local.http"
    /// SystemOneBackend::at or decode_models failed; wrapped unchanged (a bad base is LocalError::Http(http.invalid_endpoint) from BaseUrl).
    Decision(DecisionError),                                                            // "local.decision"
    ProbeStatus { daemon: DaemonKind, path: &'static str, status: u16, body: Vec<u8> },  // "local.probe_status"
    ProbeParse { daemon: DaemonKind, path: &'static str, source: ParseCause },        // "local.probe_parse"; ParseCause::{Json(serde_json::Error) (no key attached), Redacted(ErrorDetail) (key attached; serde_json.<category> id, message with the key redacted)}
    ModelNotServed { daemon: DaemonKind, model: ModelId, served: Vec<String> },        // "local.model_not_served"
    /// REQ-SAG-0015: the model was not loaded within the load timeout. runner = the base (http://host:port).
    LoadTimeout { runner: String, waited: Duration },                                  // "local.load_timeout"
}
impl ErrorId for LocalError {}   // 6 ids; cause_detail(): Http -> HttpError detail, Decision -> DecisionError detail; recovery(): ModelNotServed -> Some("load the configured model on the daemon, or set the runner's model to one it serves"), LoadTimeout -> Some("raise the runner's load timeout or check that the daemon is loading the model")
```

Log record (ANC-007): every `LocalProbe::discover` call with a logger set writes exactly one record, `GeneralLogger::write(Level::Info, "sc_decision_local", "local.discover", ctx, fields)` (the caller's `LogContext`, `LogContext::NONE` when there is no session; a-8 carries session_id and position on the event); fields `daemon` (`DaemonKind::as_str`), `model` (configured), and either `loaded` (the daemon's loaded-model list) and `state` (ModelState) on Ok, or `error` (the LocalError id, e.g. `local.http`) on Err. Never a key, header value or body. A log write error is ignored (counted by sc-agent-log) and never changes the discover result. Sending the probe is logged by sc-agent-http only when a caller sends through HttpClient (`http.send`, a-5); LocalBackend is sans-IO and logs no send, and LocalProbe writes only the local.* records. sc-decision-systemone writes no records: it only builds and reads HTTP values, and its sends are logged by sc-agent-http.

`LocalBackend` as a `Backend` returns `DecisionError`; `LocalError` is only for construction and discovery, so `DecisionError` needs no local variant.

## 6. Dependencies

sc-decision-systemone: sc-decision-schema (A-2), sc-agent-http (A-5), sc-agent-secrets (A-4; ApiKey), sc-agent-types (ErrorId); serde_json = { version = "1", features = ["preserve_order", "float_roundtrip"] }; ulid = "1" (multipart boundary).
sc-decision-local: sc-decision-systemone, sc-decision-schema, sc-agent-http, sc-agent-secrets, sc-agent-types (ModelId, ErrorId), sc-agent-config (RunnerKind, RunnerHost), sc-agent-types also for CancelToken, Clock and Wait, sc-agent-log (GeneralLogger, Level) (all path); serde = { version = "1", features = ["derive"] }, serde_json (as above).
Dev-dependencies: sc-agent-http with feature `testing` (ScriptedTransport, a-5) in both crates; sc-agent-log with feature `testing` (MemoryLog, a-8) in sc-decision-local; sc-agent-testkit (dev-only: bounded waits, `Started` latch) in sc-decision-local. `ManualClock` is sc_agent_types::testing::ManualClock (a-1), enabled through sc-agent-http's `testing` feature. No hand-rolled transport or logger fakes. Neither crate needs a fake Backend, so a-2's ScriptedBackend is not used.
Both: workspace package and lints; no tokio, no thiserror.

## 7. Required validation

- `cargo build -p sc-decision-systemone -p sc-decision-local`
- `cargo clippy -p sc-decision-systemone -p sc-decision-local --all-targets -- -D warnings` with the workspace denies (a-1)
- `cargo test -p sc-decision-systemone -p sc-decision-local`
- `cargo tree -p sc-decision-systemone -e normal --depth 1` and `cargo tree -p sc-decision-local -e normal --depth 1`: the direct sc-agent dependencies are exactly those in section 6; the full `cargo tree -p sc-decision-systemone -p sc-decision-local` has no sc-llm-provider, sc-decision-llm, sc-decision-runner, decidr or tokio (AC 13, AC 16)
- `grep -rn "sleep" crates/sc-decision-systemone/src crates/sc-decision-local/src` finds nothing (AC 12)
- `grep -rnE "127\.0\.0\.1|localhost|:8000" crates/sc-decision-local/src crates/sc-decision-systemone/src` finds no runner host or port (AC 8)

## 8. Tests (no live calls; ScriptedTransport and canned HttpResponse values; no key printed). AC n = the nth acceptance line.

sc-decision-systemone (tests/encode.rs, tests/decode.rs):
- JSON encode, Bearer only with a key, request Debug hides the key -> AC 2, AC 10.
- Images header for assets and for a box question, unset otherwise; long-context header only when flagged -> AC 2.
- Multipart encode: part order, names, content types, boundary "sc-agent-" + 26-char ULID -> AC 2.
- A request built with `DecisionRequest::new(body, image_parts, false)?` whose asset names a part encodes multipart with that part; encode has no part check (grep: no `validate` call) -> AC 3.
- Decode: TypeSafe 200 example; Strands `latency_ms` and Laya `routing`/usage extras kept; 200 non-JSON; 400 string/object; 422 array/string; 401, 402 -> AC 2. 503 -> decision.api_status with the body -> AC 11.
- models_request / decode_models (openapi example) -> AC 4; base "not a url" -> decision.http (http.invalid_endpoint); `new("http://...")` refused, `local("http://127.0.0.1:8000")` accepted -> AC 8.
- The same SystemOneRequest encodes to the same body for a TypeSafe and a local base -> AC 5.

sc-decision-local (tests/discovery.rs, over ScriptedTransport; discovery is `LocalProbe::new(&backend, &transport, ManualClock::new().clock()).discover(LogContext::NONE)`, and the clock never moves because one probe reads none):
- `test_strands_rows`, `test_laya_rows`, `test_mapika_rows`, `test_generic_rows`, `test_health_errors`: Strands, Mapika, Laya, Generic availability rows of section 3 (incl. Laya bearer only with a key, Generic 404 -> Unlisted, 500 -> local.probe_status, health not JSON -> local.probe_parse) -> AC 7.
- `test_down_is_local_http_network`: ScriptedTransport returns HttpError::Network -> local.http with cause http.network and the lower message; `test_construction_error_chain`: SystemOneBackend's DecisionError::Http chain likewise -> AC 7, AC 14.
- `test_probe_status_body_whole_and_key_redacted`, `test_probe_parse_cause_redacts_attached_key`, `test_probe_parse_keeps_raw_source_without_key`: with a key attached the probe status body, parse cause, loaded and served names carry `[redacted]` for every copy, everything else whole; without a key the raw source is kept -> AC 10.
- `test_a_transport_error_echoing_the_key_is_redacted` (RSH-006): a transport error whose text contains the probe's key (a proxy echoing Authorization) comes back with every copy `[redacted]` in its message and its cause, exactly as HttpClient::send_once redacts, because the probe reads the reply through `HttpError::redacted_for` -> AC 10, AC 14.
- `test_runner_kind_mapping`, `test_local_error_ids`: each RunnerKind maps to its DaemonKind; LocalError has 6 distinct ids; ModelNotServed has a non-empty recovery() -> AC 12.
- `test_base_from_config_values`, `test_local_backend_encodes_and_decodes_like_systemone`: `LocalBackend::new(DaemonKind::Laya, &RunnerHost::try_from("127.0.0.1".to_owned())?, NonZeroU16::new(8000).unwrap(), model, None)` (test code) encodes POST http://127.0.0.1:8000/v1/systemone and decodes like SystemOneBackend -> AC 8, AC 9.
- `test_arc_dyn_backend`: compile test: `Arc<dyn sc_decision_schema::Backend>` from SystemOneBackend and LocalBackend -> AC 9, AC 15.
- `test_debug_hides_key`: Debug prints `key: "<set>"` and never the key -> AC 10.
- tests/log.rs: `let log = MemoryLog::new()?; let backend = LocalBackend::new(..)?; let probe = LocalProbe::new(&backend, &transport, ManualClock::new().clock()).with_log(log.general());` `test_discover_writes_one_record_per_call` runs one discover that succeeds (Laya loaded) and one that fails (Network), one with `LogContext::request(session_id, Some(0))` and one with `LogContext::NONE`; `log.records()?` (MemoryRecord, write order) holds exactly two records with `target == "sc_decision_local"` and `action == "local.discover"`, whose `fields` carry daemon, model, the first record's session_id (the second has none) and loaded/state or error, and no key text (not in `{backend:?}` or `{probe:?}` either; the probe's Debug prints `log: true`); encode and decode write no local.* record -> AC 17. `test_discover_result_does_not_depend_on_logger`: a probe without a logger returns the same Availability as one with a logger, and only the logged probe wrote a record -> AC 17.
- `test_key_on_plain_http_refused_before_send`: LocalBackend for host 10.0.0.5 with `Some(key)`: encode fails at `HttpRequest::with_secret` with DecisionError::Http, cause id `http.key_on_plain_http`, `LocalProbe::discover` refuses it the same way, and the ScriptedTransport records no send -> AC 20.
- tests/load_wait.rs (deterministic: the wait runs on a `ManualClock` and a transport that stamps each probe with the clock, so no poll interval elapses on its own; the test waits until probe n has been sent and the waiter is parked on the clock (`ManualClock::wait_for_waiters`), then advances the clock by LOAD_POLL_INTERVAL; probe times and waited durations are exact clock readings, and every wait for the waiter thread is bounded by the sc-agent-testkit hang guard):
  - `test_mapika_loaded_on_third_poll`: Mapika `{"ok":false}`, `{"ok":false}`, then `{"ok":true,"model":m}` -> Ok after 3 probes at exactly 0, 1 and 2 s of the clock (loaded on the 3rd, after 2 not-loaded polls); local.load_wait records in order (three `poll` records, then the `end` record, waited_ms 2000) -> AC 21. `test_strands_connect_failure_keeps_polling`: a connect failure keeps polling, the next probe one poll later -> AC 21.
  - `test_mapika_never_ready_times_out`: Mapika never ready, load_timeout 2 s -> DecisionError::Backend with cause id local.load_timeout after probes at exactly 0, 1 and 2 s, waited exactly 2000 ms -> AC 22. `test_timeout_between_polls_probes_at_deadline`: a timeout that is not a whole number of polls probes at exactly 0, 1 and 1.5 s and ends at 1.5 s -> AC 22. `test_huge_load_timeout_is_clamped`: Duration::MAX does not overflow the deadline -> AC 22.
  - `test_cancel_after_first_poll`: cancel while the waiter is parked after the first poll -> decision.cancelled, no further probe (the clock never moves, so no poll can fire) -> AC 23. `test_cancelled_before_wait_sends_nothing`: a token cancelled before the wait sends no probe -> AC 23.
  - `test_model_not_served_ends_wait`, `test_generic_unlisted_is_ok_at_once`: local.model_not_served ends the wait at once as decision.backend; a generic daemon without /v1/models cannot tell, Ok at once -> AC 21.
  - `test_laya_loads_on_demand_first_request_deadline`: Laya `loaded: []` -> Ok with one probe; the next encode carries deadline load + request, and after a 2xx decode the following encode carries none -> AC 24. `test_no_deadline_without_load_on_first_request`: kinds that load at start, and Laya without timeouts, set no deadline -> AC 24.
- Decode keeps a probability of 0.123456789 unchanged (no rounding) -> AC 18.
- Two multipart encodes use different boundaries, each "sc-agent-" + a ULID that parses with `ulid::Ulid::from_string` -> AC 19.
// sc-decision-systemone::BaseUrl (QA-1 RBP-F004): https(&str) / local(&str) -> Result<Self, HttpError> (no query or fragment), as_str(), join(path) -> Result<Endpoint, HttpError>, Display; pub const SYSTEMONE_PATH, MODELS_PATH.
// sc-decision-local: pub const MAX_LOAD_WAIT: Duration = 86 400 s (load_timeout clamp; the wait overshoots by at most one probe timeout); local.load_wait records carry phase "poll" | "end".// Key redaction (REQ-SAG-0002 0.2.0): with a key attached, Availability.loaded and ModelNotServed.served names are redacted as well.

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-6.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] REQ-SAG-0201: sc-decision-systemone: SystemOneBackend::encode turns every DecisionRequest (JSON and multipart with image parts, images and long-context headers, Bearer when a key is set) into POST {base}/v1/systemone, and SystemOneBackend::decode turns 200/400/422/other final replies into SystemOneResponse or the matching DecisionError, covering 100% of the request and response.
- [ ] REQ-SAG-0201: sc-decision-systemone: SystemOneBackend::encode (and LocalBackend::encode through it) takes a DecisionRequest that is valid by construction (a-2 DecisionRequest::new rejects a part mismatch); a request with an image part encodes multipart with that part, and neither crate calls a validator or has a part check of its own.
- [ ] REQ-SAG-0201: sc-decision-systemone: SystemOneBackend::models_request encodes GET /v1/models and decode_models decodes the reply into ModelMetadataList.
- [ ] ADR-SAG-0001: sc-decision-systemone encodes the same SystemOneRequest unchanged for Jev, Strands Decider, Laya and Mapika/decider bases; no backend-specific request type exists.
- [ ] REQ-SAG-0201: sc-decision-systemone and sc-decision-local import SystemOneRequest, SystemOneResponse, ModelMetadataList and the Jev error types from sc-decision-schema and redefine none of them; daemon health shapes are private.
- [ ] REQ-SAG-0801: sc-decision-local: LocalProbe::discover (over the backend's sans-IO probe_request and read_probe) reports Loaded / LoadsOnDemand / NotReady, Unlisted or local.model_not_served for Strands Decider (GET /health), Laya (GET /health, optional Bearer), Mapika/decider (GET /health) and a generic daemon (GET /v1/models), each selected by its config kind; a daemon that is not listening is reported down as local.http with cause http.network; a runner up without its model is reported up with the model not loaded.
- [ ] REQ-SAG-0801: sc-decision-local: LocalBackend::new takes a config local_runners entry's values from the caller (kind as DaemonKind::from(sc_agent_config::RunnerKind), host as RunnerHost, port as NonZeroU16, model as ModelId, key), over http or https (SystemOneBackend::local); no host or port literal for a runner outside tests (grep).
- [ ] LocalBackend and SystemOneBackend implement the Backend trait imported from sc-decision-schema (no crate defines its own Backend trait) (encode(&DecisionRequest), decode(HttpResponse, &DecisionRequest, &LogContext)) and are usable as Arc<dyn Backend> (owner-local-decision-crate).
- [ ] REQ-SAG-0401: sc-decision-systemone and sc-decision-local take keys as sc_agent_secrets::ApiKey (credential variable names TYPESAFE_API_KEY, LAYA_API_KEY come from sc-agent-secrets); no test, Debug output or log record prints a key; an attached key echoed by a daemon is replaced by [redacted] in error text, causes and log fields (a probe's transport error through HttpError::redacted_for, as HttpClient::send_once does), everything else kept whole (REQ-SAG-0002 0.2.0 exception).
- [ ] REQ-SAG-0601: sc-decision-systemone: SystemOneBackend::decode returns a 503 from Laya or Mapika/decider as a final reply (decision.api_status with its body), not a busy signal.
- [ ] REQ-SAG-0002: sc-decision-local: LocalError implements ErrorId with 6 distinct local.* ids (no unknown-kind error: RunnerKind is closed and checked at config load, a-4); wrapped HttpError/DecisionError are kept unchanged; clippy with the workspace denies passes; no sleep.
- [ ] ADR-SAG-0004: `cargo tree -p sc-decision-systemone -e normal --depth 1` lists exactly sc-decision-schema, sc-agent-http, sc-agent-secrets and sc-agent-types as sc-agent dependencies (sc-decision-systemone has no error union, under the ADR-SAG-0004 exemption for crates whose functions all return the trait-fixed DecisionError), and `cargo tree -p sc-decision-local -e normal --depth 1` lists exactly sc-decision-systemone, sc-decision-schema, sc-agent-http, sc-agent-secrets, sc-agent-types, sc-agent-config and sc-agent-log; the full `cargo tree -p sc-decision-systemone -p sc-decision-local` contains no sc-llm-provider, sc-decision-llm, sc-decision-runner or decidr. No live API calls in tests. Neither crate's Cargo.toml depends on a crate in a higher layer: per ADR-SAG-0004 the layers from the bottom are types; config, secrets and log; http; decision schema; decision backends (sc-decision-llm over sc-llm-provider); runner, and decidr sits above the runner.
- [ ] REQ-SAG-0002: a test makes the ScriptedTransport fail and asserts the returned LocalError and SystemOne DecisionError chains carry the lower HttpError's id and message.
- [ ] ADR-SAG-0001: every decision backend this sprint adds implements the Backend trait defined in the sc-decision-schema crate: SystemOneBackend (sc-decision-systemone) and LocalBackend (sc-decision-local) are each `impl sc_decision_schema::Backend`, and a compile-time test builds `Arc<dyn sc_decision_schema::Backend>` from both.
- [ ] REQ-SAG-0009: neither sc-decision-systemone nor sc-decision-local depends on tokio; `cargo tree -p sc-decision-systemone -p sc-decision-local` lists no tokio crate.
- [ ] ANC-007: the existing sc-agent library code in sc-decision-local is updated to write log records: LocalProbe::with_log takes the caller's GeneralLogger (LocalBackend has no with_log: it does no I/O) and each LocalProbe::discover(ctx) call writes one `local.discover` record (daemon, model, the caller's LogContext session_id and position, loaded models and state, or the error id; never a key) (REQ-SAG-0008: the logger is the caller's, default or injected); a test builds `MemoryLog::new()?` (a-8's sc_agent_log::testing), passes `log.general()` to LocalProbe::with_log, runs one successful and one failed discover, and finds both records in `log.records()?` by `action == "local.discover"`.
- [ ] REQ-SAG-0007: sc-decision-systemone: SystemOneBackend::decode returns a probability of 0.123456789 as 0.123456789; no rounding, normalizing or clamping.
- [ ] REQ-SAG-0001: sc-decision-systemone: the multipart boundary is "sc-agent-" + a fresh ULID (ulid crate) per encode; a test parses it and sees two encodes differ.
- [ ] REQ-SAG-0013: sc-decision-local: LocalBackend::new takes the validated RunnerHost and NonZeroU16 port (a-4); a LocalBackend for plain-http host 10.0.0.5 with a key fails in encode at HttpRequest::with_secret (a-5) with DecisionError::Http carrying `http.key_on_plain_http`, before any send.
- [ ] REQ-SAG-0015: sc-decision-local: LocalProbe::wait_until_loaded polls the probe every 1 s of its Clock (Clock::wait_until, no sleep) and returns Ok once the daemon reports the configured model loaded; a test on a ManualClock with a daemon not loaded for 2 polls returns Ok on the 3rd (probes at exactly 0, 1 and 2 s), so model loading is waited out before any decision request and does not count toward its timeout.
- [ ] REQ-SAG-0015: sc-decision-local: when load_timeout passes without the model loaded, LocalProbe::wait_until_loaded returns decision.backend with cause local.load_timeout (runner, waited); a test on a ManualClock checks it (waited exactly the timeout).
- [ ] REQ-SAG-0015: sc-decision-local: cancelling the CancelToken during LocalProbe::wait_until_loaded returns decision.cancelled without another probe; a test checks it.
- [ ] REQ-SAG-0015: sc-decision-local: for Laya (loads on the first request) LocalProbe::wait_until_loaded returns Ok at once and requests encoded before the first 2xx carry HttpRequest::with_deadline(load + request) (a-5), using the runner's own LocalTimeouts; a test checks the deadline is set and then cleared.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7):
- Q1 a 503 is a final reply (REQ-SAG-0601 busy = 429/502/529; owner: only timeouts and busy are resent).
- Q2 Strands Decider with a configured model that is neither its served name nor its checkpoint -> local.model_not_served.
- Mapika/decider added as discovery kind `mapika` (GET /health); kind `decider` renamed `strands` to keep the two apart.
No open questions.
Plan hardening round 1 (2026-11-09): PLAN-CRIT-005 LocalBackend::with_log + local.discover record, sc-agent-log added to sc-decision-local; PLAN-CRIT-014 encode calls DecisionRequest::validate (a-2), no own part checks; PLAN-CRIT-015 discovery scope in does_not_close; PLAN-CRIT-010 tests use a-5 ScriptedTransport and a-8 MemoryLog (no fake Backend is needed, so a-2 ScriptedBackend is unused); front matter, Required validation, tests cite acceptance lines.

Plan review round 2 (2026-11-09): ARCH-003 sc-decision-systemone cites the ADR-SAG-0004 exemption (no union, returns DecisionError); REQ-QA-006 adds REQ-SAG-0001 (ULID boundary AC), 0007 (unrounded decode AC), 0008 (ANC-007 line); RBP-F002 validate calls dropped (DecisionRequest valid by construction); RBP-F014 LocalDaemon sealed, DaemonKind from a-4 RunnerKind (UnknownDaemon/local.unknown_daemon removed, unknown kinds rejected at config load by a-4, 5 ids); RBP-F010/F011 ModelId and DaemonKind in signatures and errors; RSH-002 discover runs on the caller's Timeouts::probe() transport (a-10 builds it); RSH-009 discover takes &LogContext and local.discover carries it; ErrorId::cause -> cause_detail; recovery() for model_not_served.
a-1/a-4 contract folded in: LocalBackend::new takes &RunnerHost and NonZeroU16 (validated at load); RunnerKind -> DaemonKind; no unknown-kind check; Display prints `<id>: <message>` only; recovery() provided by ErrorId.

Backend::decode takes ctx: &LogContext (a-2); discover takes ctx: LogContext by value, written via GeneralLogger::write(.., ctx, fields) (a-8).
Round 3: REQ-SAG-0013 added (validated RunnerHost/NonZeroU16; http.key_on_plain_http surfaced as DecisionError::Http, test AC 20); REQ-SAG-0201 end to end -> a-10.
Round 4: with_secret(..)? at every secret attach (encode, models_request now Result, probe); REQ-SAG-0015 wait_until_loaded + LocalTimeouts + Laya first-request deadline + LoadTimeout (local.load_timeout, 6 ids), AC 21-24. wait_until_loaded returns DecisionError (LoadTimeout as Backend detail, cancel as Cancelled{sends:0}). Needs from others: a-5 HttpRequest::with_deadline(Duration), a-4/a-10 the per-runner load/request timeouts in config.
- QA-1 (2026-11-10) surface additions accepted: ModelState::as_str (log field), LocalBackend::base(), LocalBackend::kind(), Display for DaemonKind. sc-agent-types is a normal dependency of sc-decision-systemone per AC 13. Health kinds (Strands, Laya, Mapika) are up only on 200.
- QA-1 fix round (code at 56776e4): ProbeStatus/ProbeParse recovery text; ProbeStatus body kept whole (REQ-SAG-0002) with the key redacted when one was attached; with a key attached, the Generic model-list parse message, ModelNotServed.served and Availability.loaded are redacted too (REQ-SAG-0401); escape_name returns Cow; LocalBackend::new returns LocalError::Http(http.invalid_endpoint) for a bad base.
