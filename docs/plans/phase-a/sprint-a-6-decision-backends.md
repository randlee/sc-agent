<!-- Generated from bead agt-4p2.6 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-6: sc-decision-systemone and sc-decision-local
- id: `a-6` (bead `agt-4p2.6`)
- branch: `sprint/a-6-decision-backends`
- depends on: agt-4p2.2
- copy script: `docs/plans/phase-a/copy/a-6.sh`

## Description

sc-decision-systemone and sc-decision-local are sc-agent library crates; each provides a decision backend that implements the Backend trait from sc-decision-schema. Jev backend (encode/decode POST /v1/systemone, GET /v1/models). sc-decision-local: stub reusing the Jev encode/decode for any Jev-compatible local daemon from config by name (host and port in config): Strands Decider, Laya, Mapika/decider or generic; LocalDaemon trait whose main task is discovering whether a model is available/loaded. Records: REQ-SAG-0201, REQ-SAG-0801, ADR-SAG-0001.

## Design

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
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError>;
    /// 2xx -> SystemOneResponse::from_json; 400 -> ApiBadRequest (ApiStatus if another shape);
    /// 422 -> ApiValidation (ApiStatus for Strands/Laya string details); other (incl. 402, 503) -> ApiStatus, body whole. `request` unused.
    fn decode(&self, response: HttpResponse, request: &DecisionRequest) -> Result<SystemOneResponse, DecisionError>;
}
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
#[derive(Debug)]
pub struct LocalBackend { daemon: Box<dyn LocalDaemon>, base: String, model: String, key: Option<ApiKey>, backend: SystemOneBackend }
impl LocalBackend {
    /// base = http://{host}:{port}; the backend is SystemOneBackend::local(base, key).
    pub fn new(kind: DaemonKind, host: &str, port: u16, model: &str, key: Option<ApiKey>) -> Result<Self, LocalError>;
    /// Sends the probe once (no busy resend).
    pub fn discover(&self, transport: &dyn Transport) -> Result<Availability, LocalError>;
}
impl Backend for LocalBackend { /* encode/decode delegate to SystemOneBackend */ }
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

`LocalBackend` as a `Backend` returns `DecisionError`; `LocalError` is only for construction and discovery, so `DecisionError` needs no local variant.

## 6. Dependencies

sc-decision-systemone: sc-decision-schema (A-2), sc-agent-http (A-5), sc-agent-secrets (A-4; ApiKey), sc-agent-types (ErrorId); serde_json = { version = "1", features = ["preserve_order", "float_roundtrip"] }; ulid = "1" (multipart boundary).
sc-decision-local: sc-decision-systemone, sc-decision-schema, sc-agent-http, sc-agent-secrets, sc-agent-types (all path); serde = { version = "1", features = ["derive"] }, serde_json (as above).
Both: workspace package and lints; no tokio, no thiserror.

## 7. Tests (no live calls; fake transport and canned HttpResponse values; no key printed)

sc-decision-systemone (tests/encode.rs, tests/decode.rs):
- JSON encode: POST {base}/v1/systemone, Content-Type application/json, body equals `to_json`, Bearer secret present only with a key, request Debug hides the key.
- Images header set for assets and for a box question, unset otherwise; long-context header only when flagged.
- Multipart encode: first part `request` application/json; one part per image with its name and content type; boundary "sc-agent-" + 26-char ULID.
- Decode: TypeSafe 200 example -> equal response; Strands `latency_ms` and Laya `routing`/usage extras kept; 200 non-JSON -> decision.response_parse; 400 string and object detail -> decision.api_bad_request; 422 array -> decision.api_validation; 422 string -> decision.api_status; 401, 402 and 503 -> decision.api_status with the body whole.
- models_request is GET {base}/v1/models; decode_models parses the openapi example; base "not a url" -> decision.http with cause http.invalid_endpoint; `new("http://...")` refused, `local("http://127.0.0.1:8000")` accepted.

sc-decision-local (tests/discovery.rs):
- Strands matching model / checkpoint -> Loaded; other -> local.model_not_served listing both.
- Mapika `{"ok":true,"model":m}` -> Loaded; another model -> local.model_not_served; `{"ok":false}` -> NotReady.
- Laya loaded -> Loaded; not loaded -> LoadsOnDemand; `{"status":"ok"}` -> Unlisted; bearer only when a key is given.
- Generic listed -> Loaded; not listed -> local.model_not_served; 404 -> Unlisted; 500 -> local.probe_status.
- Health not JSON -> local.probe_parse; connect failure -> local.http with cause http.network.
- "ollama" -> local.unknown_daemon; each DaemonKind parses from its config name.
- `LocalBackend::new(DaemonKind::Laya, "127.0.0.1", 8000, ..)` encodes POST http://127.0.0.1:8000/v1/systemone and decodes like SystemOneBackend; it is usable as `Arc<dyn Backend>`.

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-6.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] SystemOneBackend encodes every DecisionRequest (JSON and multipart with image parts, images and long-context headers, Bearer when a key is set) to POST {base}/v1/systemone and decodes 200/400/422/other final replies into SystemOneResponse or the matching DecisionError, covering 100% of the request and response (REQ-SAG-0201).
- [ ] GET /v1/models is encoded and decoded into ModelMetadataList (REQ-SAG-0201).
- [ ] The same SystemOneRequest is sent unchanged to Jev, Strands Decider, Laya and Mapika/decider; no backend-specific request type exists (ADR-SAG-0001).
- [ ] Both crates import SystemOneRequest, SystemOneResponse, ModelMetadataList and the Jev error types from sc-decision-schema and redefine none of them; daemon health shapes are private (REQ-SAG-0201).
- [ ] REQ-SAG-0801: discovery reports Loaded / LoadsOnDemand / NotReady, Unlisted or local.model_not_served for Strands Decider (GET /health), Laya (GET /health, optional Bearer), Mapika/decider (GET /health) and a generic daemon (GET /v1/models), each selected by its config kind; a daemon that is not listening is reported down as local.http with cause http.network; a runner up without its model is reported up with the model not loaded.
- [ ] REQ-SAG-0801: LocalBackend is built from a config local_runners entry (kind, host, port, model, key_env) passed in by the caller, over http or https (SystemOneBackend::local); no host or port literal for a runner outside tests (grep).
- [ ] LocalBackend and SystemOneBackend implement the Backend trait imported from sc-decision-schema (no crate defines its own Backend trait) (encode(&DecisionRequest), decode(HttpResponse, &DecisionRequest)) and are usable as Arc<dyn Backend> by sc-decision-runner (owner-local-decision-crate).
- [ ] REQ-SAG-0401: credential variable names (TYPESAFE_API_KEY, LAYA_API_KEY) come from sc-agent-secrets; no test or log prints a key.
- [ ] REQ-SAG-0601: a 503 from Laya or Mapika/decider is a final reply (decision.api_status with its body), not resent.
- [ ] REQ-SAG-0002: LocalError implements ErrorId with 6 distinct local.* ids; wrapped HttpError/DecisionError are kept unchanged; clippy with the workspace denies passes; no sleep.
- [ ] ADR-SAG-0004: neither crate depends on sc-llm-provider, sc-decision-llm, sc-decision-runner or decide (cargo tree). No live API calls in tests.
- [ ] REQ-SAG-0002: a test makes the transport fail and asserts the returned LocalError and SystemOne DecisionError chains carry the lower HttpError's id and message.
- [ ] ADR-SAG-0001: every decision backend this sprint adds implements the Backend trait defined in the sc-decision-schema crate: SystemOneBackend (sc-decision-systemone) and LocalBackend (sc-decision-local) are each `impl sc_decision_schema::Backend`, and a compile-time test builds `Arc<dyn sc_decision_schema::Backend>` from both.
- [ ] REQ-SAG-0009: neither sc-decision-systemone nor sc-decision-local depends on tokio; `cargo tree -p sc-decision-systemone -p sc-decision-local` lists no tokio crate.
- [ ] ANC-007: the existing sc-agent library code in sc-decision-local is updated to write log records: each daemon availability check writes one (daemon, model, loaded models; never a key) to the GeneralLogger the caller injects; a test with an in-memory GeneralLogger finds it.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7):
- Q1 a 503 is a final reply (REQ-SAG-0601 busy = 429/502/529; owner: only timeouts and busy are resent).
- Q2 Strands Decider with a configured model that is neither its served name nor its checkpoint -> local.model_not_served.
- Mapika/decider added as discovery kind `mapika` (GET /health); kind `decider` renamed `strands` to keep the two apart.
No open questions.
