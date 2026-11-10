<!-- Generated from bead agt-4p2.2 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-2: sc-decision-schema
- id: `a-2` (bead `agt-4p2.2`)
- branch: `sprint/a-2-sc-decision-schema`
- depends on: agt-4p2.5
- copy script: `docs/plans/phase-a/copy/a-2.sh`

## Description

Build sc-decision-schema: Jev API types (the one request form), the sync Decision trait, the per-question sc-sha digest. Governing records: REQ-SAG-0201, REQ-SAG-0202, REQ-SAG-0006, ADR-SAG-0001; anchor ANC-001. Code to copy from sc-agent-ref: see design.

## Design

# Sprint A-2 design: sc-decision-schema (bead agt-4p2.2)

Governing: REQ-SAG-0201, REQ-SAG-0202, REQ-SAG-0006, ADR-SAG-0001, ADR-SAG-0004 (layer: above sc-agent-http, below every decision backend and the runner), ANC-001 (locked). Also binding: REQ-SAG-0001, REQ-SAG-0002, REQ-SAG-0007, REQ-SAG-0009, REQ-SAG-0901 (the schema-mismatch signal; resent up to max_sends), REQ-SAG-0504 (decide names the line, field and reason of a bad batch request), owner-architecture, owner-decision-trait, owner-error-contract, owner-no-sleep.
Wire shapes: https://api.typesafe.ai/openapi.json v0.2.0 (copy at scratchpad/research/openapi.json), verified 2026-11-09.

Scope: the Jev API types (POST /v1/systemone, GET /v1/models, the 400/422 error bodies), the sans-IO `Backend` trait, the sync `Decision` trait and `DecisionResult`, the per-question sc-sha digest, `DecisionError`. No queue, no workers, no I/O, no session rows (sc-decision-runner, A-9, implements `Decision`).

## 1. Copy script

`docs/plans/phase-a/copy/a-2.sh` (dry run 2026-11-09, reconciliation: 3 copy steps, 3 files, 168 lines).

| # | Source (sc-agent-ref@dce239a) | Target | Lines |
|---|---|---|---|
| C1 | crates/sc-decision-schema/LICENSE | crates/sc-decision-schema/LICENSE | 21 |
| C2 | crates/sc-decision-schema/src/jev/v1.rs:205-341 (OrderedMap, Pairs, PairsVisitor, duplicate_key, duplicate_message) | src/map.rs | 137 |
| C3 | crates/sc-agent-types/src/ids.rs:838-847 (Sha256Digest::of_text) | src/digest.rs | 10 |

Not copied: the old Jev question/answer/request/response types (wrong shapes against openapi.json); payload.rs/judge.rs/llm/variants/probability/ids/session_log/test_util (ADR-SAG-0001 canonical payload, REQ-SAG-0006, other crates); jev/v1.rs JevState (serde_json `preserve_order` plus an untagged derive does it); payload.rs decode_failed (the full serde message is kept); sc-jev-host pool.rs (no queue here).

## 2. Required edits after copying

- src/map.rs: module doc and `use` lines (BTreeSet, fmt, PhantomData, serde de/ser). `try_from_pairs` returns `Result<Self, DecisionError>` with `DuplicateKey { key }`. Remove `keys()` and `is_empty()`. Doc-test path `sc_decision_schema::OrderedMap`.
- src/digest.rs: the fragment becomes `QuestionDigest::of_text`; delete `unreachable!()`; `Err(error) => Err(DecisionError::Digest(error))`. Comment: compact serde_json escapes CR inside strings, so sc-sha's CRLF normalisation never alters the digest input.

## 3. Exact Rust surface (bodies elided)

```rust
// lib.rs
pub use api_error::{HttpBadRequestError, HttpValidationError, LocItem, ValidationError};
pub use backend::Backend;
pub use decision::{Decision, DecisionRequest, DecisionResult, ImagePart};
pub use digest::{question_digests, QuestionDigest};
pub use error::DecisionError;
pub use map::OrderedMap;
pub use models::{ModelMetadata, ModelMetadataList};
pub use request::{Asset, AssetKind, ImageFormat, NoulCriteria, Question, State, SystemOneRequest};
pub use response::{Answer, BoxCoordinates, SystemOneResponse, Usage};

// map.rs (copied, C2)
#[derive(Debug, Clone, PartialEq)]
pub struct OrderedMap<K, V>(Vec<(K, V)>);
impl<K: Ord + fmt::Display, V> OrderedMap<K, V> { pub fn try_from_pairs(pairs: Vec<(K, V)>) -> Result<Self, DecisionError>; }
impl<K, V> OrderedMap<K, V> { pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> + '_; pub fn len(&self) -> usize; }
impl<K: PartialEq, V> OrderedMap<K, V> { pub fn get(&self, key: &K) -> Option<&V>; }
// Serialize; Deserialize refuses a duplicate key: "duplicate key `k`; give every key once".

// request.rs
/// POST /v1/systemone body. Field order is the wire order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemOneRequest {
    pub model: String,
    pub state: State,
    pub questions: OrderedMap<String, Question>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<OrderedMap<String, Asset>>,
}
impl SystemOneRequest {
    /// Parses through serde_path_to_error so a failure names its field (REQ-SAG-0504).
    pub fn from_json(text: &str) -> Result<Self, DecisionError>;   // RequestParse { path, message }
    pub fn to_json(&self) -> Result<String, DecisionError>;         // Encode; compact
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum State { Text(String), Object(serde_json::Map<String, serde_json::Value>), Array(Vec<serde_json::Value>) }

/// Discriminated by `type`; instructions/criteria values are string | object | array | null.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Question {
    Noul {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")] instructions: Option<serde_json::Value>,
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")] criteria: Option<Option<NoulCriteria>>, // absent | null | {true,false}
    },
    Choice {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")] instructions: Option<serde_json::Value>,
        criteria: OrderedMap<String, serde_json::Value>,
    },
    Score {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")] instructions: Option<serde_json::Value>,
        criteria: Vec<serde_json::Value>,           // ordered; level = index from 0
    },
    Box {
        #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")] instructions: Option<serde_json::Value>,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoulCriteria {
    #[serde(rename = "true", default, deserialize_with = "present", skip_serializing_if = "Option::is_none")] pub yes: Option<serde_json::Value>,
    #[serde(rename = "false", default, deserialize_with = "present", skip_serializing_if = "Option::is_none")] pub no: Option<serde_json::Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    #[serde(rename = "type")] pub kind: AssetKind,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub format: Option<ImageFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub data_base64: Option<String>,  // opaque
    #[serde(default, skip_serializing_if = "Option::is_none")] pub part: Option<String>,         // multipart part name
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "lowercase")]
pub enum AssetKind { Image }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "lowercase")]
pub enum ImageFormat { Png, Jpg, Jpeg }
/// Keeps an explicit JSON null as Some(Null) so absent and null both round-trip.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error>;

// response.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemOneResponse {
    pub model: String,
    pub answers: OrderedMap<String, Answer>,
    pub usage: Usage,
    /// Fields outside the TypeSafe schema (Strands Decider `latency_ms`, Laya `routing`), kept verbatim.
    #[serde(flatten)] pub extra: serde_json::Map<String, serde_json::Value>,
}
impl SystemOneResponse { pub fn from_json(status: u16, body: &[u8]) -> Result<Self, DecisionError>; }  // ResponseParse
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage { pub input_tokens: u64, pub output_tokens: u64, #[serde(flatten)] pub extra: serde_json::Map<String, serde_json::Value> }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    Noul { noul: f64 },
    Choice { choice: String, confidence: f64, probabilities: OrderedMap<String, f64> },
    Score { score: f64, confidence: f64, legend: OrderedMap<String, serde_json::Value>, probabilities: OrderedMap<String, f64> },
    Box { #[serde(rename = "box")] coordinates: BoxCoordinates, confidence: Option<f64> },   // required, nullable
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxCoordinates { pub top: f64, pub bottom: f64, pub left: f64, pub right: f64 }

// models.rs  (GET /v1/models)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMetadataList { pub models: Vec<ModelMetadata> }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMetadata { pub name: String, pub description: String, pub release_date: String }
impl ModelMetadataList { pub fn from_json(status: u16, body: &[u8]) -> Result<Self, DecisionError>; }

// api_error.rs  (documented 400 and 422 bodies)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpBadRequestError { pub detail: serde_json::Value }     // string | object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpValidationError { #[serde(default)] pub detail: Vec<ValidationError> }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    pub loc: Vec<LocItem>, pub msg: String,
    #[serde(rename = "type")] pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub input: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub ctx: Option<serde_json::Map<String, serde_json::Value>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LocItem { Name(String), Index(u64) }

// decision.rs
/// Everything one send carries: JSON body, multipart image parts, long-context header.
/// TypeSafe-Danger-Zone-Images is derived (assets present or any box question).
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionRequest {
    pub body: SystemOneRequest,
    pub image_parts: Vec<ImagePart>,   // non-empty => multipart/form-data
    pub long_context: bool,            // TypeSafe-Danger-Zone-Long-Context: true
}
impl From<SystemOneRequest> for DecisionRequest {}
impl DecisionRequest { pub fn needs_images_header(&self) -> bool; }
#[derive(Clone, PartialEq, Eq)]
pub struct ImagePart { pub name: String, pub format: ImageFormat, pub bytes: Vec<u8> }   // Debug: name, format, byte length

/// One result of decide/decide_stream. The decided response is SystemOneResponse, the one
/// response form (ADR-SAG-0001); no wrapper type.
#[derive(Debug)]
pub struct DecisionResult {
    pub session_id: SessionId,                          // ULID (REQ-SAG-0001)
    pub position: u64,                                  // index of the request in the input; 0 for decide (REQ-SAG-0701)
    pub digests: OrderedMap<String, QuestionDigest>,    // one per question, request order; empty only when the digest failed
    pub outcome: Result<SystemOneResponse, DecisionError>,
}

/// Sync decision API, implemented by sc-decision-runner. Object-safe; no tokio; no associated error.
pub trait Decision: Send + Sync {
    fn decide(&self, request: DecisionRequest, cancel: &CancelToken) -> DecisionResult;
    /// Results arrive fastest first; a failed request is returned in its result and the stream
    /// continues; cancelling ends every unfinished request with DecisionError::Cancelled.
    fn decide_stream<'a>(
        &'a self,
        requests: Box<dyn Iterator<Item = DecisionRequest> + Send + 'a>,
        cancel: &CancelToken,
    ) -> Box<dyn Iterator<Item = DecisionResult> + Send + 'a>;
}

// backend.rs
/// Sans-IO translation between a DecisionRequest and one provider's HTTP exchange. Implemented by
/// sc-decision-systemone (Jev and any System One server), sc-decision-local (Strands Decider, Laya, Mapika/decider, generic) and sc-decision-llm. No I/O, no clock.
pub trait Backend: Send + Sync + fmt::Debug {
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError>;
    /// `response` is the final reply (any status; busy and quota never reach here, A-5).
    /// Return SchemaMismatch when the reply does not match the required schema and must be sent again (REQ-SAG-0901).
    fn decode(&self, response: HttpResponse, request: &DecisionRequest) -> Result<SystemOneResponse, DecisionError>;
}

// digest.rs
/// 64 lowercase hex chars; serialised as a JSON string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QuestionDigest(String);
impl QuestionDigest { pub fn of_text(text: &str) -> Result<Self, DecisionError>; pub fn as_str(&self) -> &str; }
pub fn question_digests(request: &DecisionRequest) -> Result<OrderedMap<String, QuestionDigest>, DecisionError>;
```

`HttpRequest`/`HttpResponse`/`HttpError` are sc-agent-http's (A-5); `SessionId`, `CancelToken`, `ErrorId`, `ErrorDetail` are sc-agent-types' (A-1).

## 4. Digest (REQ-SAG-0202)

For each question id `q`, in request order, the digest input is the compact JSON (serde_json, preserve_order) of

```json
{"body": {"model":…, "state":…, "questions": {"q": <question q>}, "assets":…},
 "long_context": <bool>,
 "parts": [{"name":…, "format":…, "sha256": <sc-sha hex of the lowercase hex of the bytes>}]}
```

hashed with `QuestionDigest::of_text`. The information is model, state, assets, every image part and the long-context header; the single question is `{q: question}`. Other questions are excluded. The images header is derived from the body, so it is covered. sc-sha hashes UTF-8 text only, so part bytes are hex-encoded and hashed once per part. Only sc-sha computes SHA-256.

## 5. Error enum (full; ErrorId from A-1, Display = detail(), source() = wrapped value)

```rust
#[derive(Debug)]
#[non_exhaustive]
pub enum DecisionError {
    /// Request text is not a SystemOneRequest. `path` = the serde_path_to_error field path (e.g.
    /// `questions.q1`, `.` at the root); `message` = the full serde_json message with line/column
    /// (e.g. "missing field `criteria`").
    RequestParse { path: String, message: String },                    // "decision.request_parse"
    /// A body of HTTP `status` is not the expected SystemOneResponse | ModelMetadataList.
    ResponseParse { status: u16, expected: &'static str, message: String }, // "decision.response_parse"
    /// OrderedMap::try_from_pairs got a key twice.
    DuplicateKey { key: String },                                      // "decision.duplicate_key"
    /// Serialising a body or building multipart failed.
    Encode { message: String },                                        // "decision.encode"
    /// sc-sha failed; cause = ErrorDetail::foreign(sha code, ..).
    Digest(sc_sha::ShaError),                                          // "decision.digest"
    /// HTTP 400 with the documented body.
    ApiBadRequest { body: HttpBadRequestError },                       // "decision.api_bad_request"
    /// HTTP 422 with the documented body.
    ApiValidation { body: HttpValidationError },                       // "decision.api_validation"
    /// Any other non-2xx, or a 400/422 body of another shape.
    ApiStatus { status: u16, body: Vec<u8> },                          // "decision.api_status"
    /// The send failed in sc-agent-http (provider_busy, quota_exhausted, network, ...), wrapped unchanged.
    Http(HttpError),                                                   // "decision.http"
    /// A backend crate above this one failed (e.g. sc-decision-llm's LlmError chain), carried whole.
    Backend { backend: &'static str, detail: ErrorDetail },            // "decision.backend"
    /// The reply does not match the required schema. The runner resends the request unchanged up to
    /// max_sends, then returns this, with what did not match in `detail` (REQ-SAG-0901).
    SchemaMismatch { backend: &'static str, detail: ErrorDetail },     // "decision.schema_mismatch"
    /// The request did not finish before its CancelToken was cancelled (queued, held off or refused).
    Cancelled { sends: u32 },                                          // "decision.cancelled"
    /// The runner stopped before the request finished (a worker thread could not start or stopped); reason kept.
    Shutdown { reason: String },                                       // "decision.shutdown"
}
impl ErrorId for DecisionError {}   // cause(): Http -> http detail; Backend/SchemaMismatch -> detail; Digest -> sha
```

13 variants. No unsupported-input variant: every input is passed to the provider and a rejection is the provider's error (REQ-SAG-0901). A session-log write failure is never a DecisionError: it never replaces a decision result (A-9). Rule: wrapping keeps the lower value unchanged, so every id and message reaches the top.

## 6. Dependencies

- serde = { version = "1", features = ["derive"] } (lock 1.0.229)
- serde_json = { version = "1", features = ["preserve_order", "float_roundtrip"] } (lock 1.0.154): nested key order round-trips, digests are stable, 0.123456789 prints exactly (REQ-SAG-0007).
- sc-sha = "1.7" (lock 1.7.0)
- serde_path_to_error = "0.1": the field path in `RequestParse` (REQ-SAG-0504).
- sc-agent-types (path; CancelToken, SessionId, ErrorId, ErrorDetail) — A-1
- sc-agent-http (path; HttpRequest, HttpResponse, HttpError) — A-5
- Not needed: thiserror, sc-llm-provider (an LlmError arrives as ErrorDetail), base64, ulid, sha2, ureq.

## 7. Tests

- tests/typesafe_examples.rs + tests/fixtures/typesafe/*.json (from openapi.json): every request example (noul with/without criteria and object instructions, choice, score, box, assets with data_base64 and part, null instructions) and every response example (four answers, box confidence null and number) round-trips byte-identical (compact). (REQ-SAG-0201 AC1)
- ModelMetadataList, HttpBadRequestError (string and object detail), HttpValidationError round-trip.
- 0.123456789 and 1.7 come back unchanged (REQ-SAG-0007).
- Strands Decider `latency_ms` and Laya `usage.truncated` kept in `extra`.
- Duplicate question id -> RequestParse naming the key; unknown request field -> RequestParse; a score question without `criteria` -> RequestParse whose path is `questions.<id>` and whose message names `criteria`; try_from_pairs duplicate -> DuplicateKey.
- Digest: same request twice -> equal; change model / state / assets / long_context / a part byte / question q -> q changes; change question r -> q unchanged; 3 questions -> 3 digests in order (REQ-SAG-0202 AC1-3).
- DecisionError: every id distinct and `^decision\.[a-z_]+$`; Display = detail(); `Http(Network)` detail has cause id `http.network` with its message; `Backend` with a two-level ErrorDetail keeps both levels.
- `Decision` and `Backend` are object-safe (`Box<dyn Decision>`, `Arc<dyn Backend>` compile).
- CI grep (workspace): no `sha2`/`Sha256::` outside sc-sha use; no `.round(`; no `sleep(`.

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-2.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] Every request and response example in the TypeSafe API docs/openapi.json, for noul, choice, score and box (plus assets, ModelMetadataList, 400 and 422 bodies), round-trips byte-identical through sc-decision-schema types (REQ-SAG-0201 AC1).
- [ ] REQ-SAG-0504 support: SystemOneRequest::from_json failures are decision.request_parse with the field path and the full serde message (test: score without criteria).
- [ ] No crate other than sc-decision-schema defines a type that maps to a Jev API field (REQ-SAG-0201 AC2).
- [ ] ANC-001: sc-decision-schema holds only the code needed to send and receive the decision API and nothing else (no HTTP client or transport, retries, cache, logging, report formatting or CLI code), with no exception; a test lists every public item of sc-decision-schema and fails on any item outside the TypeSafe request, response, answer and error types, the Backend and Decision traits (with DecisionRequest/DecisionResult), the question digest and DecisionError; every TypeSafe documentation example for POST /v1/systemone and GET /v1/models round-trips through these types.
- [ ] No type, field or constant named for or used only by a consumer application or a mock backend (REQ-SAG-0006 AC1).
- [ ] Two digests of the same information and question are equal; changing model, state, assets, an image part, long_context or the question changes it; changing another question does not; three questions give three digests (REQ-SAG-0202 AC1-3); SHA-256 only through sc-sha (REQ-SAG-0202 AC4).
- [ ] 0.123456789 is returned as 0.123456789; no rounding in the crate (REQ-SAG-0007).
- [ ] Backend is encode(&DecisionRequest) -> Result<HttpRequest, DecisionError> and decode(HttpResponse, &DecisionRequest) -> Result<SystemOneResponse, DecisionError> over sc-agent-http types; Decision has no associated error and returns DecisionResult {session_id, position, digests, outcome: Result<SystemOneResponse, DecisionError>}; both object-safe, sync, (ADR-SAG-0001, owner-decision-trait).
- [ ] REQ-SAG-0002: DecisionError implements ErrorId with 13 distinct decision.* ids (no unsupported-input variant); Http wraps HttpError unchanged; an LlmError chain arrives as ErrorDetail in Backend or SchemaMismatch with every id and message kept; clippy with the workspace denies passes; no sleep.
- [ ] ADR-SAG-0004: sc-decision-schema sits in the decision-schema layer and its only sc-agent crate dependencies are the lower-layer crates sc-agent-types and sc-agent-http (its other dependencies are the external crates serde, serde_json and sc-sha); it depends on no decision backend, sc-llm-provider or runner crate, checked by `cargo tree -p sc-decision-schema`.
- [ ] ADR-SAG-0001: the Backend trait is defined in sc-decision-schema and is the one trait every decision backend (Jev, local runners, Haiku, Luna) implements; a compile-time test builds `Arc<dyn Backend>` from a test backend (object safety).
- [ ] REQ-SAG-0010: no call to std::thread::sleep, std::thread::sleep_until, tokio::time::sleep or any other sleep function in sc-decision-schema source; enforced by the workspace clippy disallowed-methods rule.
- [ ] REQ-SAG-0009: sc-decision-schema's Cargo.toml has no tokio dependency; `cargo tree -p sc-decision-schema` lists no tokio crate (check).

## Notes

Verified 2026-11-09 against https://api.typesafe.ai/openapi.json (v0.2.0): ScoreQuestion.criteria is an array (min 1); ChoiceQuestion.criteria is an object (min 1 property); NoulQuestion.criteria is optional {true,false}; instructions accept string, object, array or null; NoulAnswer {noul}; ScoreAnswer {score, confidence, legend, probabilities}; ChoiceAnswer {choice, confidence, probabilities}; BoxAnswer {box{top,bottom,left,right}, confidence}; Asset {type, format, data_base64 | part}. The old sc-agent-ref Jev types do not match; build from the spec.
Closed 2026-11-09 by owner answers (search-for-truth step 7):
- Digest input: everything sent (long-context flag, every image part, model, state, assets) plus the single question, per the owner's '100% of information sent' rule (REQ-SAG-0202).
- decide_stream correlation: each DecisionResult carries session_id and the input position (REQ-SAG-0701).
No open questions.
