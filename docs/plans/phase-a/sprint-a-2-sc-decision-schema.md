<!-- Generated from bead agt-4p2.2 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-2: sc-decision-schema
- id: `a-2` (bead `agt-4p2.2`)
- branch: `sprint/a-2-sc-decision-schema`
- depends on: a-5
- copy script: `docs/plans/phase-a/copy/a-2.sh`

## Description

Build sc-decision-schema: Jev API types (the one request form), the sync Decision trait, the per-question sc-sha digest. Governing records: REQ-SAG-0201, REQ-SAG-0202, REQ-SAG-0006, ADR-SAG-0001; anchor ANC-001. Code to copy from sc-agent-ref: see design.

## Design

```yaml
sprint:
  id: a-2
  closure_type: contract            # a-2 is the contract of the decision layer: Backend, Decision, DecisionRequest/DecisionResult, DecisionError
  target_boundary: sc-decision-schema
  owned_paths: [crates/sc-decision-schema/**]
  must_follow:                      # a-1 (sc-agent-types) precedes a-5 through a-4; no separate edge
    - sprint: a-5
      artifact: sc_agent_http::{HttpRequest, HttpResponse, HttpError, LogContext (re-export of sc-agent-log's)} (crates/sc-agent-http/src/request.rs + src/error.rs + lib.rs re-export)
      trigger: dev-push             # starts once a-5 pushed those files right after its copy commit; merges forward before each round; no wait for QA or PR merge
  requirements: [REQ-SAG-0201, REQ-SAG-0202, REQ-SAG-0006, REQ-SAG-0001, REQ-SAG-0002, REQ-SAG-0007, REQ-SAG-0009, REQ-SAG-0010, REQ-SAG-0504, REQ-SAG-0901, REQ-SAG-0701, REQ-SAG-0104, REQ-SAG-0014]
  adrs: [ADR-SAG-0001, ADR-SAG-0002, ADR-SAG-0004]
  does_not_close:
    - "REQ-SAG-0901 resend up to max_sends: a-9 (sc-decision-runner) acceptance REQ-SAG-0901 AC2/AC3; a-2 only defines decision.schema_mismatch"
    - "REQ-SAG-0202 digest recorded with the response in the session row: a-9 acceptance REQ-SAG-0202"
    - "REQ-SAG-0504 end to end (decide names the line, field and reason of a bad batch line): a-10; a-2 supplies only the field path and serde message"
    - "REQ-SAG-0701 / REQ-SAG-0104 behaviour (parallel batch, cancel stops work in another crate): a-9 (runner boundary, REQ-SAG-0104 AC1) and a-10 (end to end); a-2 closes only the trait shape and its doc contract"
    - "REQ-SAG-0014 request (8 MiB) and batch (10,000 lines, 256 MiB) limits: a-10; a-2 enforces only the image count/size limits in DecisionRequest::new"
```

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
- src/digest.rs: the fragment becomes `QuestionDigest::of_text`, infallible. sc-sha 1.8.0's `calculate_hash` has one error, `ShaError::InvalidUtf8`, which the bytes of a `&str` cannot produce; the copied `unreachable!()` arm stays with that comment (clippy `unreachable` is not in the workspace denies; no `#[allow]` needed). Comment: compact serde_json escapes CR inside strings, so sc-sha's CRLF normalisation never alters the digest input.

## 3. Exact Rust surface (bodies elided)

```rust
// lib.rs
pub use api_error::{HttpBadRequestError, HttpValidationError, LocItem, ValidationError};
pub use backend::Backend;
pub use decision::{Decision, DecisionRequest, DecisionResult, ImagePart, MAX_IMAGES, MAX_IMAGE_BYTES};
pub use digest::QuestionDigest;
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
/// Valid by construction: fields are private, `new` is the only way in, so no backend re-checks.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionRequest {
    body: SystemOneRequest,
    image_parts: Vec<ImagePart>,   // non-empty => multipart/form-data
    long_context: bool,            // TypeSafe-Danger-Zone-Long-Context: true
    digests: OrderedMap<String, QuestionDigest>,   // one per question, request order (section 4), computed here
}
/// REQ-SAG-0014 image limits.
pub const MAX_IMAGES: usize = 20;
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
impl DecisionRequest {
    /// Checks, in order: at most MAX_IMAGES parts and each at most MAX_IMAGE_BYTES
    /// (Err Limits { what: "images" | "image_bytes", max }); every `Asset.part` names exactly one
    /// `ImagePart` by `name`, no two parts share a name, none is unreferenced (Err ImageParts { detail }
    /// naming the asset id / part name); then computes the question digests (Err Encode only if
    /// serde_json refuses the digest input).
    pub fn new(body: SystemOneRequest, image_parts: Vec<ImagePart>, long_context: bool) -> Result<Self, DecisionError>;
    pub fn body(&self) -> &SystemOneRequest;
    pub fn image_parts(&self) -> &[ImagePart];
    pub fn long_context(&self) -> bool;
    pub fn digests(&self) -> &OrderedMap<String, QuestionDigest>;
    pub fn needs_images_header(&self) -> bool;
}
/// = DecisionRequest::new(body, Vec::new(), false): Ok when no asset names a part, else Err(ImageParts).
/// (TryFrom, not From: a body whose asset names a part has no valid request without its image.)
impl TryFrom<SystemOneRequest> for DecisionRequest { type Error = DecisionError; }
#[derive(Clone, PartialEq, Eq)]
pub struct ImagePart { pub name: String, pub format: ImageFormat, pub bytes: Vec<u8> }   // Debug: name, format, byte length

/// One result of decide/decide_stream. The decided response is SystemOneResponse, the one
/// response form (ADR-SAG-0001); no wrapper type.
#[derive(Debug)]
pub struct DecisionResult {
    pub session_id: SessionId,                          // ULID (REQ-SAG-0001)
    pub position: u64,                                  // index of the request in the input; 0 for decide (REQ-SAG-0701)
    pub digests: OrderedMap<String, QuestionDigest>,    // = request.digests().clone(); always one per question
    pub outcome: Result<SystemOneResponse, DecisionError>,
}

/// Sync decision API, implemented by sc-decision-runner. Object-safe; no tokio; no associated error.
/// Open trait: any crate may implement it (REQ-SAG-0701; cancel via A-1's CancelToken, REQ-SAG-0104).
///
/// Cancel contract (both methods): cancelling `cancel` ends every unfinished request at once with
/// `DecisionError::Cancelled { sends }` — queued, held off, and also a request whose send is in
/// flight (it does not wait for the transport to return; a reply arriving later is discarded and
/// produces no second result). Exactly one result per request, always.
pub trait Decision: Send + Sync {
    /// One request; position 0. The implementation must not run the send on the caller's thread,
    /// so cancel is immediate here too.
    fn decide(&self, request: DecisionRequest, cancel: &CancelToken) -> DecisionResult;
    /// Results arrive fastest first; a failed request is returned in its result and the stream
    /// continues. `requests` is owned and `'static` (callers pass e.g. `Box::new(vec.into_iter())`)
    /// because the implementation pulls it on its own feeder thread.
    fn decide_stream<'a>(
        &'a self,
        requests: Box<dyn Iterator<Item = DecisionRequest> + Send + 'static>,
        cancel: &CancelToken,
    ) -> Box<dyn Iterator<Item = DecisionResult> + Send + 'a>;
}

// backend.rs
/// Sans-IO translation between a DecisionRequest and one provider's HTTP exchange. Implemented by
/// sc-decision-systemone (Jev and any System One server), sc-decision-local (Strands Decider, Laya, Mapika/decider, generic) and sc-decision-llm. No I/O, no clock.
/// Open trait: any crate may implement it (ADR-SAG-0001). `request` is already valid (DecisionRequest::new).
pub trait Backend: Send + Sync + fmt::Debug {
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError>;
    /// `response` is the final reply (any status; busy and quota never reach here, A-5).
    /// Return SchemaMismatch when the reply does not match the required schema and must be sent again (REQ-SAG-0901).
    /// `ctx` (sc_agent_http::LogContext, re-exported from sc-agent-log) is only passed through so the
    /// backend's own records (a-6 local.*, a-7 llm.*) carry session_id/position; this crate writes no log.
    fn decode(&self, response: HttpResponse, request: &DecisionRequest, ctx: &LogContext) -> Result<SystemOneResponse, DecisionError>;
}

// digest.rs
/// 64 lowercase hex chars; serialised as a JSON string (the form ADR-SAG-0002 session rows carry).
/// Deserialize validates: `#[serde(try_from = "String", into = "String")]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct QuestionDigest(String);
impl QuestionDigest { pub fn of_text(text: &str) -> Self; pub fn as_str(&self) -> &str; }
/// Accepts exactly 64 lowercase hex chars (sc_sha::TemplateSha256::from_hex plus a lowercase check);
/// else sc_sha::ShaError::InvalidDigestHex.
impl TryFrom<String> for QuestionDigest { type Error = sc_sha::ShaError; }
impl From<QuestionDigest> for String {}
impl AsRef<str> for QuestionDigest {}
pub(crate) fn question_digests(body: &SystemOneRequest, image_parts: &[ImagePart], long_context: bool) -> Result<OrderedMap<String, QuestionDigest>, DecisionError>; // Err only Encode
```

`HttpRequest`/`HttpResponse`/`HttpError`/`LogContext` (re-export) are sc-agent-http's (A-5); `SessionId`, `CancelToken`, `ErrorId`, `ErrorDetail` are sc-agent-types' (A-1).

## 4. Digest (REQ-SAG-0202)

For each question id `q`, in request order, the digest input is the compact JSON (serde_json, preserve_order) of

```json
{"body": {"model":…, "state":…, "questions": {"q": <question q>}, "assets":…},
 "long_context": <bool>,
 "parts": [{"name":…, "format":…, "sha256": <sc-sha hex of the lowercase hex of the bytes>}]}
```

hashed with `QuestionDigest::of_text`. The information is model, state, assets, every image part and the long-context header; the single question is `{q: question}`. Other questions are excluded. The images header is derived from the body, so it is covered. sc-sha hashes UTF-8 text only, so part bytes are hex-encoded and hashed once per part. Only sc-sha computes SHA-256.

## 5. Error enum (full; ErrorId from A-1, Display = `id: message` only (the chain stays in source()/detail()), source() = wrapped value)

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
    /// DecisionRequest::new: an Asset.part with no (or more than one) matching ImagePart, or an
    /// ImagePart no asset references; `detail` names the asset id / part name.
    ImageParts { detail: String },                                     // "decision.image_parts"
    /// DecisionRequest::new: a REQ-SAG-0014 limit exceeded; `what` = "images" | "image_bytes", `max` = the limit.
    Limits { what: &'static str, max: u64 },                           // "decision.limits"
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
impl ErrorId for DecisionError {}   // cause_detail(): Http -> http detail; Backend/SchemaMismatch -> detail
// recovery(): RequestParse -> "fix the field named in `path` (see the TypeSafe API docs for its shape) and send again"; others None (Http forwards a-5's)
```

14 variants (decision.digest is gone: hashing a `&str` cannot fail in sc-sha 1.8.0; decision.limits is new). No unsupported-input variant: every input is passed to the provider and a rejection is the provider's error (REQ-SAG-0901). A session-log write failure is never a DecisionError: it never replaces a decision result (A-9). Rule: wrapping keeps the lower value unchanged, so every id and message reaches the top.

## 6. Dependencies

- serde = { version = "1", features = ["derive"] } (lock 1.0.229)
- serde_json = { version = "1", features = ["preserve_order", "float_roundtrip"] } (lock 1.0.154): nested key order round-trips, digests are stable, 0.123456789 prints exactly (REQ-SAG-0007).
- sc-sha = "1.8" (lock 1.8.0; `calculate_hash` fails only with InvalidUtf8, checked in the 1.8.0 source)
- serde_path_to_error = "0.1": the field path in `RequestParse` (REQ-SAG-0504).
- sc-agent-types (path; CancelToken, SessionId, ErrorId, ErrorDetail) — A-1
- sc-agent-http (path; HttpRequest, HttpResponse, HttpError, LogContext re-export as a parameter type only) — A-5
- Not needed: thiserror, sc-llm-provider (an LlmError arrives as ErrorDetail), base64, ulid, sha2, ureq.

## 7. Tests (each names the acceptance line it proves)

- `tests/typesafe_examples.rs` + `tests/fixtures/typesafe/*.json` (every request and response example in openapi.json, incl. assets with data_base64 and part, null instructions, box confidence null and number; ModelMetadataList; 400 string/object and 422 bodies) -> AC "REQ-SAG-0201 AC1" and the round-trip clause of AC "ANC-001".
- `extra_fields_kept` (Strands Decider `latency_ms`, Laya `usage.truncated` in `extra`) -> AC "REQ-SAG-0201 AC1" (non-TypeSafe servers round-trip).
- `exact_floats` (0.123456789, 1.7) -> AC "REQ-SAG-0007".
- `request_parse_errors` (duplicate question id, unknown field, score without `criteria` -> path `questions.<id>`; try_from_pairs duplicate -> DuplicateKey) -> AC "REQ-SAG-0504 support".
- `new_checks_image_parts` (asset part with no ImagePart; two ImageParts of one name; unreferenced ImagePart; a matching set is Ok; TryFrom of a body whose asset names a part is Err) -> AC "DecisionRequest::new".
- `new_checks_limits` (21 images; one image of MAX_IMAGE_BYTES + 1) -> AC "REQ-SAG-0014".
- `digest_deserialize_validates` (63 chars, uppercase, non-hex refused; of_text output round-trips) -> AC "ADR-SAG-0002".
- `digests` (same twice equal; model / state / assets / long_context / a part byte / question q each change q; question r leaves q; 3 questions -> 3 in order) -> AC "REQ-SAG-0202".
- `error_ids` (14 ids distinct, `^decision\.[a-z_]+$`; Display = `id: message`; `Http(Network)` cause_detail `http.network`; two-level `Backend` detail kept; request_parse recovery non-empty) -> AC "REQ-SAG-0002".
- `object_safety` (`Box<dyn Decision>`, `Arc<dyn Backend>` from an in-test backend) -> AC "Backend/Decision shape" and AC "ADR-SAG-0001".
- `public_items` (lists the crate's public items) -> AC "ANC-001" and AC "REQ-SAG-0006 AC1".

## 8. Required validation

- `cargo build -p sc-decision-schema`
- `cargo clippy -p sc-decision-schema --all-targets -- -D warnings` with the workspace lints (a-1's `[workspace.lints]` and clippy.toml disallowed-methods)
- `cargo test -p sc-decision-schema`
- `cargo tree -p sc-decision-schema -e normal`: sc-agent crates are only sc-agent-types and sc-agent-http (plus their own); no tokio, no sc-decision-* backend, no sc-llm-provider, no sc-decision-runner
- `rg -n 'sleep\(|\.round\(' crates/sc-decision-schema` finds nothing (AC REQ-SAG-0010, REQ-SAG-0007); `rg -n 'sha2|Sha256::' crates/` (workspace) finds no use outside sc-sha (AC REQ-SAG-0202 AC4)

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-2.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] REQ-SAG-0201 AC1: every request and response example in the TypeSafe API docs/openapi.json, for noul, choice, score and box (plus assets, ModelMetadataList, 400 and 422 bodies), round-trips byte-identical through sc-decision-schema types.
- [ ] REQ-SAG-0504 support: sc-decision-schema's SystemOneRequest::from_json failures are decision.request_parse with the field path and the full serde message (test: score without criteria).
- [ ] REQ-SAG-0201 AC2: no crate other than sc-decision-schema defines a type that maps to a Jev API field.
- [ ] ANC-001: sc-decision-schema holds only the code needed to send and receive the decision API and nothing else (no HTTP client or transport, retries, cache, logging, report formatting or CLI code), with no exception; a test lists every public item of sc-decision-schema and fails on any item outside the TypeSafe request, response, answer and error types, the Backend and Decision traits (with DecisionRequest and its constructor/accessors, MAX_IMAGES/MAX_IMAGE_BYTES, and DecisionResult), the question digest and DecisionError; every TypeSafe documentation example for POST /v1/systemone and GET /v1/models round-trips through these types.
- [ ] REQ-SAG-0006 AC1: sc-decision-schema has no type, field or constant named for or used only by a consumer application or a mock backend (Haiku, Luna); the public-item test checks it.
- [ ] REQ-SAG-0202 AC1-4: sc-decision-schema's DecisionRequest::digests gives equal digests for the same information and question; changing model, state, assets, an image part, long_context or the question changes it; changing another question does not; three questions give three digests; SHA-256 only through sc-sha (workspace search finds no sha2/Sha256:: use).
- [ ] REQ-SAG-0007: sc-decision-schema returns 0.123456789 as 0.123456789; no rounding in the crate.
- [ ] Backend/Decision shape: sc-decision-schema's Backend is encode(&DecisionRequest) -> Result<HttpRequest, DecisionError> and decode(HttpResponse, &DecisionRequest, &LogContext) -> Result<SystemOneResponse, DecisionError> over sc-agent-http types (LogContext is the sc-agent-http re-export, a parameter type only; no logging code); Decision has no associated error (REQ-SAG-0701: decide and decide_stream over one Decision trait; REQ-SAG-0104: cancel through A-1's CancelToken), decide_stream takes `Box<dyn Iterator<Item = DecisionRequest> + Send + 'static>`, and both methods return DecisionResult {session_id, position, digests, outcome: Result<SystemOneResponse, DecisionError>}; the Decision doc comment states the cancel contract (every unfinished request, in-flight sends included, ends at once with decision.cancelled; a late reply gives no second result); both traits object-safe and sync (ADR-SAG-0001, owner-decision-trait).
- [ ] DecisionRequest::new: sc-decision-schema's DecisionRequest has private fields and is built only by DecisionRequest::new (or TryFrom<SystemOneRequest> = new with no parts), which returns decision.image_parts naming the asset/part when an Asset.part has no or more than one matching ImagePart or an ImagePart is unreferenced, and Ok for a matching set; DecisionRequest has no public validate.
- [ ] REQ-SAG-0014: sc-decision-schema's DecisionRequest::new refuses more than 20 image parts or a part over 20 MiB with decision.limits naming the limit (what, max); the request and batch size limits are a-10's.
- [ ] ADR-SAG-0002: sc-decision-schema's QuestionDigest serialises as the 64-lowercase-hex JSON string the session rows carry and its Deserialize refuses any other string (63 chars, uppercase, non-hex).
- [ ] REQ-SAG-0002: sc-decision-schema's DecisionError implements ErrorId with 14 distinct decision.* ids (incl. decision.image_parts and decision.limits; no decision.digest, since QuestionDigest::of_text cannot fail; no unsupported-input variant), Display is `id: message`, cause_detail() carries the lower detail and decision.request_parse has a non-empty recovery(); Http wraps HttpError unchanged; an LlmError chain arrives as ErrorDetail in Backend or SchemaMismatch with every id and message kept; clippy with the workspace denies passes.
- [ ] ADR-SAG-0004: sc-decision-schema sits in the decision-schema layer and its only sc-agent crate dependencies are the lower-layer crates sc-agent-types and sc-agent-http (its other dependencies are the external crates serde, serde_json, serde_path_to_error and sc-sha 1.8); it depends on no decision backend, sc-llm-provider or runner crate, checked by `cargo tree -p sc-decision-schema -e normal`.
- [ ] ADR-SAG-0001: the Backend trait is defined in sc-decision-schema and is the one trait every decision backend (Jev, local runners, Haiku, Luna) implements; a compile-time test builds `Arc<dyn Backend>` from a test backend (object safety).
- [ ] REQ-SAG-0010: no call to std::thread::sleep, std::thread::sleep_until, tokio::time::sleep or any other sleep function in sc-decision-schema source; enforced by the workspace clippy disallowed-methods rule.
- [ ] REQ-SAG-0009: sc-decision-schema's Cargo.toml has no tokio dependency; `cargo tree -p sc-decision-schema` lists no tokio crate (check).

## Notes

Verified 2026-11-09 against https://api.typesafe.ai/openapi.json (v0.2.0): ScoreQuestion.criteria is an array (min 1); ChoiceQuestion.criteria is an object (min 1 property); NoulQuestion.criteria is optional {true,false}; instructions accept string, object, array or null; NoulAnswer {noul}; ScoreAnswer {score, confidence, legend, probabilities}; ChoiceAnswer {choice, confidence, probabilities}; BoxAnswer {box{top,bottom,left,right}, confidence}; Asset {type, format, data_base64 | part}. The old sc-agent-ref Jev types do not match; build from the spec.
Closed 2026-11-09 by owner answers (search-for-truth step 7):
- Digest input: everything sent (long-context flag, every image part, model, state, assets) plus the single question, per the owner's '100% of information sent' rule (REQ-SAG-0202).
- decide_stream correlation: each DecisionResult carries session_id and the input position (REQ-SAG-0701).
No open questions.
Hardening round 1 (2026-11-09, decisions.md): PLAN-CRIT-001 decide_stream takes a 'static boxed iterator; PLAN-CRIT-002 Decision doc states the in-flight cancel contract; PLAN-CRIT-014 DecisionRequest::validate + DecisionError::ImageParts (14 variants); PLAN-CRIT-010 follow-up: no test double in sc-decision-schema (ANC-001 keeps it to types and traits); the scripted test backend is private to a-9 (crates/sc-decision-runner/tests/support/).
Round 2 (2026-11-09, round2.md): RBP-F002 DecisionRequest private + new() (TryFrom<SystemOneRequest>, not From, since From cannot refuse a body whose asset names a part; validate removed); RBP-F008 sc-sha 1.8.0 calculate_hash fails only on InvalidUtf8, impossible for &str -> of_text infallible, decision.digest dropped, digests computed in new() so DecisionResult.digests is never empty; RBP-F012 QuestionDigest validating Deserialize; REQ-SAG-0014 image limits -> decision.limits; RBP-F001 cause_detail; RBP-F005 recovery for request_parse; RBP-F013 Display id: message; RBP-F014 open traits; RBP-F017 AsRef<str>; REQ-QA-006 REQ-SAG-0104/0701; ARCH-005 ADR-SAG-0002.
