<!-- Generated from bead agt-4p2.6 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-6: sc-decision-systemone and sc-decision-local
- id: `a-6` (bead `agt-4p2.6`)
- branch: `sprint/a-6-decision-backends`
- depends on: a-2
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
      artifact: sc_decision_schema::{Backend, DecisionRequest, DecisionRequest::validate, ImagePart, SystemOneRequest, SystemOneResponse, ModelMetadataList, DecisionError} (crates/sc-decision-schema/src/)
      trigger: dev-push
    - sprint: a-5
      artifact: sc_agent_http::{HttpRequest, HttpResponse, HttpError, Endpoint, Transport} and sc_agent_http::testing::ScriptedTransport (feature `testing`)
      trigger: dev-push
    - sprint: a-8
      artifact: sc_agent_log::{GeneralLogger, Level} and sc_agent_log::testing::MemoryLog (feature `testing`)
      trigger: dev-push
  requirements: [REQ-SAG-0201, REQ-SAG-0801, REQ-SAG-0002, REQ-SAG-0009, REQ-SAG-0010, REQ-SAG-0401, REQ-SAG-0601]
  adrs: [ADR-SAG-0001, ADR-SAG-0004]
  does_not_close:
    - "Discovery is a library capability of sc-decision-local; no decide or Python operation calls it in this phase (REQ-SAG-0801 requires only the library report)."
    - "A decision sent end to end through SystemOneBackend or LocalBackend (REQ-SAG-0201 via decide send, local runners from config via decide): a-10 acceptance."
    - "decide models over SystemOneBackend::models_request/decode_models: a-10 acceptance."
    - "Busy resend and holdoff for these backends (REQ-SAG-0601): a-5 (sc-agent-http) and a-9 (runner); this sprint only shows 503 is a final reply."
```

# Sprint A-6 design: sc-decision-systemone and sc-decision-local (bead agt-4p2.6)

Governing: REQ-SAG-0201, ADR-SAG-0001, REQ-SAG-0801 (local runners from config, discovery, no hard-coded host/port). Also binding: ADR-SAG-0004 (layer: above sc-decision-schema, below sc-decision-runner), REQ-SAG-0002, REQ-SAG-0001, REQ-SAG-0007, REQ-SAG-0401, REQ-SAG-0601 (busy handled by sc-agent-http and the runner, not here), owner-architecture, owner-system-one-backends, owner-local-decision-crate, owner-local-discovery, owner-no-sleep, owner-lowlevel-no-tokio.

- sc-decision-systemone: one sans-IO `Backend` for any System One server: Jev (`POST {base}/v1/systemone`, Bearer key), Strands Decider, Laya and Mapika/decider (same path, key optional); plus GET /v1/models encode/decode (used by `decide models`, A-10).
- sc-decision-local: minimal stub. Builds a `SystemOneBackend` for `http://{host}:{port}` from a config `local_runners` entry (A-4: kind, host, port, model, key_env) and adds discovery: is the daemon up, is its configured model loaded. One `LocalDaemon` implementation per daemon kind: `strands` (Strands Decider), `laya`, `mapika` (Mapika/decider), `generic`; a later daemon (e.g. 'ollaya') adds one. Runners come from config by name (REQ-SAG-0801). It runs through sc-decision-runner like any backend (as `Arc<dyn Backend>`, wired by decide, A-10).

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
| `generic` | `GET {base}/v1/models`, Bearer if a key is configured | any HTTP answer | `models[].name` | listed -> `Loaded`; else `local.model_not_served`; 404/405 -> `Unlisted` |

Down = the probe's transport error, returned as `LocalError::Http(HttpError)` (id `local.http`, cause `http.network`). Any other status -> `ProbeStatus`. Discovery is one GET through `Transport::send`, sent once (not a decision, no busy resend, nothing to wait on).

## 4. Exact Rust surface (bodies elided)

### sc-decision-systemone

```rust
pub use backend::{SystemOneBackend, TYPESAFE_BASE_URL};

pub const TYPESAFE_BASE_URL: &str = "https://api.typesafe.ai";

/// Backend for any System One server: POST {base}/v1/systemone, GET {base}/v1/models.
#[derive(Debug, Clone)]
pub struct SystemOneBackend { systemone: Endpoint, models: Endpoint, key: Option<ApiKey> }
impl SystemOneBackend {
    /// `base` has no trailing path, e.g. TYPESAFE_BASE_URL. https only (Endpoint::parse).
    pub fn new(base: &str, key: Option<ApiKey>) -> Result<Self, DecisionError>;   // Http(InvalidEndpoint)
    /// A configured local runner's base, e.g. "http://127.0.0.1:8000"; http or https (Endpoint::parse_local).
    pub fn local(base: &str, key: Option<ApiKey>) -> Result<Self, DecisionError>;
    pub fn models_request(&self) -> HttpRequest;                                    // GET /v1/models, Bearer if key
    pub fn decode_models(&self, response: HttpResponse) -> Result<ModelMetadataList, DecisionError>;
}
impl Backend for SystemOneBackend {
    /// JSON when request.image_parts is empty, else multipart/form-data (part "request" application/json
    /// first, then one part per image: name = ImagePart.name, image/png or image/jpeg; boundary "sc-agent-<ULID>").
    /// Headers: Content-Type; Bearer secret header when key is Some; TypeSafe-Danger-Zone-Images: true when
    /// needs_images_header(); TypeSafe-Danger-Zone-Long-Context: true when long_context.
    /// First line: `request.validate()?` (a-2; decision.image_parts). No part check of its own.
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError>;
    /// 2xx -> SystemOneResponse::from_json; 400 -> ApiBadRequest (ApiStatus if another shape);
    /// 422 -> ApiValidation (ApiStatus for Strands/Laya string details); other (incl. 402, 503) -> ApiStatus, body whole. `request` unused.
    fn decode(&self, response: HttpResponse, request: &DecisionRequest) -> Result<SystemOneResponse, DecisionError>;
}
/// Writes the parts in `request.image_parts` order; runs only after validate, so it checks no part names.
fn multipart_body(request: &DecisionRequest, boundary: &str) -> Result<Vec<u8>, DecisionError>;   // private
```

No error enum of its own: every failure is a `DecisionError` (the trait fixes it); an invalid base URL is the A-5 `HttpError::InvalidEndpoint` wrapped as `DecisionError::Http`.

### sc-decision-local

```rust
pub use backend::LocalBackend;
pub use daemon::{Availability, DaemonKind, LocalDaemon, ModelState};
pub use error::LocalError;
pub use generic::Generic;
pub use laya::Laya;
pub use mapika::Mapika;
pub use strands::Strands;

/// One daemon kind's discovery, sans-IO.
pub trait LocalDaemon: Send + Sync + fmt::Debug {
    fn name(&self) -> &'static str;                                  // "strands" | "laya" | "mapika" | "generic"
    fn probe(&self, base: &str, key: Option<&ApiKey>) -> Result<HttpRequest, LocalError>;
    fn availability(&self, response: &HttpResponse, model: &str) -> Result<Availability, LocalError>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DaemonKind { Strands, Laya, Mapika, Generic }
impl FromStr for DaemonKind { type Err = LocalError; }             // the config `kind` string; else UnknownDaemon
impl DaemonKind { pub fn daemon(self) -> Box<dyn LocalDaemon>; }

/// The daemon is up (the probe got an HTTP answer it understood).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Availability { pub daemon: &'static str, pub loaded: Vec<String>, pub model: ModelState }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelState { Loaded, LoadsOnDemand, NotReady, Unlisted }

#[derive(Debug, Clone, Copy, Default)] pub struct Strands;
#[derive(Debug, Clone, Copy, Default)] pub struct Laya;
#[derive(Debug, Clone, Copy, Default)] pub struct Mapika;
#[derive(Debug, Clone, Copy, Default)] pub struct Generic;
// private serde shapes (not Jev API types): StrandsHealth { model, checkpoint }, LayaHealth { loaded: Option<Vec<String>> }, MapikaHealth { ok: bool, model: Option<String> };
// Generic decodes sc_decision_schema::ModelMetadataList.

/// A System One backend at http://{host}:{port} with discovery; arguments come from a config
/// `local_runners` entry (A-4). This crate reads no config and hard-codes no host or port.
/// Debug is written by hand (GeneralLogger has no Debug): it prints daemon, base, model, `key: <set|unset>`, `log: <set|unset>`.
pub struct LocalBackend { daemon: Box<dyn LocalDaemon>, base: String, model: String, key: Option<ApiKey>, backend: SystemOneBackend, log: Option<GeneralLogger> }
impl fmt::Debug for LocalBackend { /* as above */ }
impl LocalBackend {
    /// base = http://{host}:{port}; the backend is SystemOneBackend::local(base, key). No logger until with_log.
    pub fn new(kind: DaemonKind, host: &str, port: u16, model: &str, key: Option<ApiKey>) -> Result<Self, LocalError>;
    /// The caller's GeneralLogger (default or injected, REQ-SAG-0008); discover writes to it.
    pub fn with_log(self, log: GeneralLogger) -> Self;
    /// Sends the probe once (no busy resend) and, when a logger is set, writes one `local.discover` record.
    pub fn discover(&self, transport: &dyn Transport) -> Result<Availability, LocalError>;
}
impl Backend for LocalBackend { /* encode/decode delegate to SystemOneBackend (encode validates first) */ }
```

## 5. Error enum (full, sc-decision-local; ErrorId, Display = detail(), no thiserror)

```rust
#[derive(Debug)]
#[non_exhaustive]
pub enum LocalError {
    /// Config `kind` is not strands, laya, mapika or generic.
    UnknownDaemon { kind: String },                                                     // "local.unknown_daemon"
    /// The probe could not be sent: daemon down (http.network) or bad endpoint; wrapped unchanged.
    Http(HttpError),                                                                    // "local.http"
    /// SystemOneBackend::new or decode_models failed; wrapped unchanged.
    Decision(DecisionError),                                                            // "local.decision"
    ProbeStatus { daemon: &'static str, path: &'static str, status: u16, body: Vec<u8> }, // "local.probe_status"
    ProbeParse { daemon: &'static str, path: &'static str, message: String },          // "local.probe_parse" (full serde message)
    ModelNotServed { daemon: &'static str, model: String, served: Vec<String> },       // "local.model_not_served"
}
impl ErrorId for LocalError {}
```

Log record (ANC-007): every `discover` call with a logger set writes exactly one record, `GeneralLogger::write(Level::Info, "sc_decision_local", "local.discover", fields)`; fields `daemon` (kind name), `model` (configured), and either `loaded` (the daemon's loaded-model list) and `state` (ModelState) on Ok, or `error` (the LocalError id, e.g. `local.http`) on Err. Never a key, header value or body. A log write error is ignored (counted by sc-agent-log) and never changes the discover result. Sending the probe is logged by sc-agent-http only when a caller sends through HttpClient (`http.send`, a-5); this crate is sans-IO and logs no send. sc-decision-systemone writes no records: it only builds and reads HTTP values, and its sends are logged by sc-agent-http.

`LocalBackend` as a `Backend` returns `DecisionError`; `LocalError` is only for construction and discovery, so `DecisionError` needs no local variant.

## 6. Dependencies

sc-decision-systemone: sc-decision-schema (A-2), sc-agent-http (A-5), sc-agent-secrets (A-4; ApiKey), sc-agent-types (ErrorId); serde_json = { version = "1", features = ["preserve_order", "float_roundtrip"] }; ulid = "1" (multipart boundary).
sc-decision-local: sc-decision-systemone, sc-decision-schema, sc-agent-http, sc-agent-secrets, sc-agent-types, sc-agent-log (GeneralLogger, Level) (all path); serde = { version = "1", features = ["derive"] }, serde_json (as above).
Dev-dependencies: sc-agent-http with feature `testing` (ScriptedTransport, a-5) in both crates; sc-agent-log with feature `testing` (MemoryLog, a-8) in sc-decision-local. No hand-rolled transport or logger fakes. Neither crate needs a fake Backend, so a-2's ScriptedBackend is not used.
Both: workspace package and lints; no tokio, no thiserror.

## 7. Required validation

- `cargo build -p sc-decision-systemone -p sc-decision-local`
- `cargo clippy -p sc-decision-systemone -p sc-decision-local --all-targets -- -D warnings` with the workspace denies (a-1)
- `cargo test -p sc-decision-systemone -p sc-decision-local`
- `cargo tree -p sc-decision-systemone -e normal --depth 1` and `cargo tree -p sc-decision-local -e normal --depth 1`: the direct sc-agent dependencies are exactly those in section 6; the full `cargo tree -p sc-decision-systemone -p sc-decision-local` has no sc-llm-provider, sc-decision-llm, sc-decision-runner, decide or tokio (AC 13, AC 16)
- `grep -rn "sleep" crates/sc-decision-systemone/src crates/sc-decision-local/src` finds nothing (AC 12)
- `grep -rnE "127\.0\.0\.1|localhost|:8000" crates/sc-decision-local/src crates/sc-decision-systemone/src` finds no runner host or port (AC 8)

## 8. Tests (no live calls; ScriptedTransport and canned HttpResponse values; no key printed). AC n = the nth acceptance line.

sc-decision-systemone (tests/encode.rs, tests/decode.rs):
- JSON encode, Bearer only with a key, request Debug hides the key -> AC 2, AC 10.
- Images header for assets and for a box question, unset otherwise; long-context header only when flagged -> AC 2.
- Multipart encode: part order, names, content types, boundary "sc-agent-" + 26-char ULID -> AC 2.
- Asset.part with no ImagePart, and an unreferenced ImagePart -> decision.image_parts, no HttpRequest -> AC 3.
- Decode: TypeSafe 200 example; Strands `latency_ms` and Laya `routing`/usage extras kept; 200 non-JSON; 400 string/object; 422 array/string; 401, 402 -> AC 2. 503 -> decision.api_status with the body -> AC 11.
- models_request / decode_models (openapi example) -> AC 4; base "not a url" -> decision.http (http.invalid_endpoint); `new("http://...")` refused, `local("http://127.0.0.1:8000")` accepted -> AC 8.
- The same SystemOneRequest encodes to the same body for a TypeSafe and a local base -> AC 5.

sc-decision-local (tests/discovery.rs, over ScriptedTransport):
- Strands, Mapika, Laya, Generic availability rows of section 3 (incl. Laya bearer only with a key, Generic 404 -> Unlisted, 500 -> local.probe_status, health not JSON -> local.probe_parse) -> AC 7.
- ScriptedTransport returns HttpError::Network -> local.http with cause http.network and the lower message; SystemOneBackend's DecisionError::Http chain likewise -> AC 7, AC 14.
- "ollama" -> local.unknown_daemon; each DaemonKind parses from its config name; 6 distinct ids -> AC 12.
- `LocalBackend::new(DaemonKind::Laya, "127.0.0.1", 8000, ..)` encodes POST http://127.0.0.1:8000/v1/systemone, decodes like SystemOneBackend, and a mismatched part -> decision.image_parts -> AC 3, AC 8, AC 9.
- Compile test: `Arc<dyn sc_decision_schema::Backend>` from SystemOneBackend and LocalBackend -> AC 9, AC 15.
- tests/log.rs: `let log = MemoryLog::new()?; let backend = LocalBackend::new(..)?.with_log(log.general());` runs one discover that succeeds (Laya loaded) and one that fails (Network); `log.records()?` (MemoryRecord, write order) holds exactly two records with `target == "sc_decision_local"` and `action == "local.discover"`, whose `fields` carry daemon, model and loaded/state or error, and no key text -> AC 17.

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-6.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] REQ-SAG-0201: sc-decision-systemone: SystemOneBackend::encode turns every DecisionRequest (JSON and multipart with image parts, images and long-context headers, Bearer when a key is set) into POST {base}/v1/systemone, and SystemOneBackend::decode turns 200/400/422/other final replies into SystemOneResponse or the matching DecisionError, covering 100% of the request and response.
- [ ] REQ-SAG-0201: sc-decision-systemone: SystemOneBackend::encode (and LocalBackend::encode through it) calls DecisionRequest::validate before encoding; an Asset.part with no matching ImagePart, or an unreferenced ImagePart, returns decision.image_parts and no HttpRequest; neither crate has a part check of its own.
- [ ] REQ-SAG-0201: sc-decision-systemone: SystemOneBackend::models_request encodes GET /v1/models and decode_models decodes the reply into ModelMetadataList.
- [ ] ADR-SAG-0001: sc-decision-systemone encodes the same SystemOneRequest unchanged for Jev, Strands Decider, Laya and Mapika/decider bases; no backend-specific request type exists.
- [ ] REQ-SAG-0201: sc-decision-systemone and sc-decision-local import SystemOneRequest, SystemOneResponse, ModelMetadataList and the Jev error types from sc-decision-schema and redefine none of them; daemon health shapes are private.
- [ ] REQ-SAG-0801: sc-decision-local: LocalBackend::discover reports Loaded / LoadsOnDemand / NotReady, Unlisted or local.model_not_served for Strands Decider (GET /health), Laya (GET /health, optional Bearer), Mapika/decider (GET /health) and a generic daemon (GET /v1/models), each selected by its config kind; a daemon that is not listening is reported down as local.http with cause http.network; a runner up without its model is reported up with the model not loaded.
- [ ] REQ-SAG-0801: sc-decision-local: LocalBackend::new takes a config local_runners entry's values (kind, host, port, model, key) from the caller, over http or https (SystemOneBackend::local); no host or port literal for a runner outside tests (grep).
- [ ] LocalBackend and SystemOneBackend implement the Backend trait imported from sc-decision-schema (no crate defines its own Backend trait) (encode(&DecisionRequest), decode(HttpResponse, &DecisionRequest)) and are usable as Arc<dyn Backend> (owner-local-decision-crate).
- [ ] REQ-SAG-0401: sc-decision-systemone and sc-decision-local take keys as sc_agent_secrets::ApiKey (credential variable names TYPESAFE_API_KEY, LAYA_API_KEY come from sc-agent-secrets); no test, Debug output or log record prints a key.
- [ ] REQ-SAG-0601: sc-decision-systemone: SystemOneBackend::decode returns a 503 from Laya or Mapika/decider as a final reply (decision.api_status with its body), not a busy signal.
- [ ] REQ-SAG-0002: sc-decision-local: LocalError implements ErrorId with 6 distinct local.* ids; wrapped HttpError/DecisionError are kept unchanged; clippy with the workspace denies passes; no sleep.
- [ ] ADR-SAG-0004: `cargo tree -p sc-decision-systemone -e normal --depth 1` lists exactly sc-decision-schema, sc-agent-http, sc-agent-secrets and sc-agent-types as sc-agent dependencies, and `cargo tree -p sc-decision-local -e normal --depth 1` lists exactly sc-decision-systemone, sc-decision-schema, sc-agent-http, sc-agent-secrets, sc-agent-types and sc-agent-log; the full `cargo tree -p sc-decision-systemone -p sc-decision-local` contains no sc-llm-provider, sc-decision-llm, sc-decision-runner or decide. No live API calls in tests. Neither crate's Cargo.toml depends on a crate in a higher layer: per ADR-SAG-0004 the layers from the bottom are types; config, secrets and log; http; decision schema; decision backends (sc-decision-llm over sc-llm-provider); runner, and decide sits above the runner.
- [ ] REQ-SAG-0002: a test makes the ScriptedTransport fail and asserts the returned LocalError and SystemOne DecisionError chains carry the lower HttpError's id and message.
- [ ] ADR-SAG-0001: every decision backend this sprint adds implements the Backend trait defined in the sc-decision-schema crate: SystemOneBackend (sc-decision-systemone) and LocalBackend (sc-decision-local) are each `impl sc_decision_schema::Backend`, and a compile-time test builds `Arc<dyn sc_decision_schema::Backend>` from both.
- [ ] REQ-SAG-0009: neither sc-decision-systemone nor sc-decision-local depends on tokio; `cargo tree -p sc-decision-systemone -p sc-decision-local` lists no tokio crate.
- [ ] ANC-007: the existing sc-agent library code in sc-decision-local is updated to write log records: LocalBackend::with_log takes the caller's GeneralLogger and each LocalBackend::discover call writes one `local.discover` record (daemon, model, loaded models and state, or the error id; never a key); a test builds `MemoryLog::new()?` (a-8's sc_agent_log::testing), passes `log.general()` to LocalBackend::with_log, runs one successful and one failed discover, and finds both records in `log.records()?` by `action == "local.discover"`.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7):
- Q1 a 503 is a final reply (REQ-SAG-0601 busy = 429/502/529; owner: only timeouts and busy are resent).
- Q2 Strands Decider with a configured model that is neither its served name nor its checkpoint -> local.model_not_served.
- Mapika/decider added as discovery kind `mapika` (GET /health); kind `decider` renamed `strands` to keep the two apart.
No open questions.
Plan hardening round 1 (2026-11-09): PLAN-CRIT-005 LocalBackend::with_log + local.discover record, sc-agent-log added to sc-decision-local; PLAN-CRIT-014 encode calls DecisionRequest::validate (a-2), no own part checks; PLAN-CRIT-015 discovery scope in does_not_close; PLAN-CRIT-010 tests use a-5 ScriptedTransport and a-8 MemoryLog (no fake Backend is needed, so a-2 ScriptedBackend is unused); front matter, Required validation, tests cite acceptance lines.
