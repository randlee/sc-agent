<!-- Generated from bead agt-4p2.7 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-7: sc-llm-provider and sc-decision-llm
- id: `a-7` (bead `agt-4p2.7`)
- branch: `sprint/a-7-llm-provider`
- depends on: a-2, a-4, a-5, a-8
- copy script: `docs/plans/phase-a/copy/a-7.sh`

## Description

sc-llm-provider and sc-decision-llm are sc-agent library crates. Anthropic Messages and OpenAI-compatible providers with JSON-schema output (LlmError); Haiku and Luna decision backends that translate the Jev request to a prompt and required schema, self-report probabilities and confidence, and signal a schema mismatch for resend. Records: REQ-SAG-0103, REQ-SAG-0502, REQ-SAG-0901, ADR-SAG-0001.

## Design

```yaml
sprint:
  id: a-7
  closure_type: boundary
  target_boundary: sc-decision-llm
  vertical_rationale: sc-llm-provider's only decision consumer is sc-decision-llm, and both are built and tested against the same canned provider replies, so a separate sprint would add an edge and a hand-off without shortening the critical path.
  owned_paths: [crates/sc-llm-provider/**, crates/sc-decision-llm/**]
  must_follow:
    - sprint: a-2
      artifact: sc_decision_schema::{Backend, DecisionRequest (private fields, DecisionRequest::new and accessors), ImagePart, SystemOneRequest, SystemOneResponse, Answer, DecisionError} (crates/sc-decision-schema/src/)
      trigger: dev-push
    - sprint: a-5
      artifact: sc_agent_http::{HttpRequest, HttpResponse, HttpError} (src/request.rs, src/error.rs), plus Endpoint, SecretHeader, ProviderReply, Timeouts::probe, HttpClient::{may_start, send_once}, SendPermit and sc_agent_http::testing::ScriptedTransport (feature `testing`)
      trigger: dev-push
    - sprint: a-4
      artifact: sc_agent_secrets::ApiKey
      trigger: dev-push
    - sprint: a-8
      artifact: sc_agent_log::{GeneralLogger, Level, LogContext} and sc_agent_log::testing::MemoryLog (feature `testing`)
      trigger: dev-push
  requirements: [REQ-SAG-0103, REQ-SAG-0502, REQ-SAG-0901, REQ-SAG-0002, REQ-SAG-0006, REQ-SAG-0008, REQ-SAG-0007, REQ-SAG-0009, REQ-SAG-0010, REQ-SAG-0201, REQ-SAG-0401, REQ-SAG-0503]
  adrs: [ADR-SAG-0001, ADR-SAG-0004]
  does_not_close:
    - "REQ-SAG-0901 AC2 (a schema mismatch is resent unchanged up to max_sends, then returned): a-9 acceptance, over this backend's SchemaMismatch signal."
    - "REQ-SAG-0503 decide doctor reporting provider model lists: a-10 acceptance (this sprint ships models_request/decode_models only)."
    - "A Haiku/Luna decision sent end to end through decide (REQ-SAG-0901, REQ-SAG-0103 through the CLI): a-10 acceptance."
    - "Converting a config Provider to LlmProvider (LlmProvider::from_provider) once in decide's backend(), and sending models_request on a transport built with UreqTransport::new(Timeouts::probe()): a-10."
```

# Sprint A-7: sc-llm-provider and sc-decision-llm (bead agt-4p2.7)

Governing: REQ-SAG-0103, REQ-SAG-0502, REQ-SAG-0901 (docs/sc-decision-llm: in decider mode Haiku and Luna produce probabilities and confidence themselves and return the Jev answer schema, requested through the provider's structured-output feature; a reply that does not match is resent unchanged up to max_sends, then returned as decision.schema_mismatch with what did not match; every provider-supported input, images included, is passed through), ADR-SAG-0001, ADR-SAG-0004 (sc-llm-provider sits on sc-agent-http and needs no decision types; sc-decision-llm sits on sc-decision-schema and sc-llm-provider, below the runner; HttpError is wrapped by LlmError, and an LlmError reaches DecisionError as an ErrorDetail chain). Also binding: REQ-SAG-0002, REQ-SAG-0006, REQ-SAG-0007, REQ-SAG-0201, REQ-SAG-0401, REQ-SAG-0009, REQ-SAG-0010, owner-architecture, owner-decision-trait, owner-provider-and-model, owner-openai-naming, owner-lowlevel-no-tokio.

Copy script: `docs/plans/phase-a/copy/a-7.sh` (source sc-agent-ref@dce239a; dry run 2026-11-09, reconciliation: 14 copy steps, 8 files, 405 lines; no `sleep(`; the old `wire.rs:426` `expect` is not copied).

Both crates are sans-IO: they build an `HttpRequest` and read an `HttpResponse` (A-5). Sending, busy resend and holdoff are sc-agent-http plus the runner's queue; the schema-mismatch resend (up to max_sends) is the runner's too (A-9), signalled by `DecisionError::SchemaMismatch` from `decode`.

Decisions (assistant, 2026-11-09):
- Providers: `LlmProvider` (Anthropic, OpenAi, OpenRouter, Together, Nous) is the provider type in this crate; TypeSafe has no LLM wire and cannot be named here, so there is no unsupported-provider error (decide converts once).
- Wire: Anthropic Messages for anthropic; Chat Completions for openai, openrouter, together and nous (one encoder). Token cap: `max_completion_tokens` for openai, `max_tokens` for the others.
- Nous: base `https://inference-api.nousresearch.com/v1`, Bearer, `response_format` json_schema; a model that rejects it returns the provider's error.
- Provider ids (REQ-SAG-0103): `provider_model_id` maps a catalog id to the provider's id: anthropic, openai, nous unchanged; openrouter `anthropic/<id>` for `claude-*` with the trailing `-<major>-<minor>` written `<major>.<minor>` (`claude-haiku-5-5` -> `anthropic/claude-haiku-5.5`) and `openai/<id>` for `gpt-*`; together `anthropic/<id>` / `openai/<id>` (Together's ids are unconfirmed; decide doctor reports any that do not map back). `catalog_model_id` is the inverse, used by decide doctor.
- Model lists for decide doctor (REQ-SAG-0503) live here: `LlmClient::models_request` / `decode_models`.
- Images: every asset is passed through (REQ-SAG-0901); a provider rejection is its own error (decision.backend, cause llm.api_status).
- Model name: the request sends `provider_model_id(provider, resolved ModelId)`; the response reports the model the provider names.
- System prompt: drafted by the assistant (Mapika/decider uses none, plan-anchor has no decider mode); owner-approved as written 2026-11-10.

What the old code did, and why little is copied: old `sc-jev-backends` sent a consumer system prompt (judge-jev `check.toml`) plus the canonical payload to a token-count endpoint, then to Anthropic Messages or OpenAI Responses, with no JSON schema; old `sc-decision-schema/src/llm` checked only ids, choice membership and numeric types; `noul` was refused, `score` was a choice, `box` did not exist. Dropped: canonical payload and `payload_sha256` (ADR-SAG-0001), `WireFault`, token counting and byte limits (no record), the consumer prompt (REQ-SAG-0006), `ModelFamily`/`SchemaFamily`, the macro-built backends.

## 1. Copy list (source `path:lines` -> target)

| # | Source (sc-agent-ref@dce239a) | Target | What it becomes |
|---|---|---|---|
| C1 | `crates/sc-jev-backends/LICENSE` | both crates' `LICENSE` | unchanged |
| C2 | `crates/sc-jev-backends/src/wire.rs:13-30` | `sc-llm-provider/src/endpoints.rs` | hosts, paths, `ANTHROPIC_VERSION = "2026-10-21"`, `HAIKU_MAX_TOKENS = 16_000` (moves to sc-decision-llm as its default) |
| C3 | `crates/sc-jev-backends/src/wire.rs:200-223` | `sc-llm-provider/src/endpoints.rs` | non-secret headers and credential header per wire |
| C4 | `crates/sc-jev-backends/src/wire.rs:252-289, 302-339, 386-423` | `sc-llm-provider/src/decode.rs` | reply-envelope skeleton, Anthropic text gathering, truncation detection, usage counters |
| C5 | `crates/sc-jev-backends/src/backend.rs:325-336` | `sc-llm-provider/src/client.rs` | `HttpRequest` assembly (headers + secret) |
| C6 | `crates/sc-jev-backends/tests/llm.rs:51-68, 87-101, 498-533` | `sc-llm-provider/tests/decode.rs` | reply builders, truncation test |
| C7 | `crates/sc-decision-schema/src/llm/v1.rs:20-63` | `sc-decision-llm/src/answers.rs` | `LlmOutput` (model text -> `answers` object), ids-differ error |
| C8 | `crates/sc-decision-schema/src/variants.rs:51-120` | `sc-decision-llm/src/answers.rs` | old per-answer check (choice + confidence + probabilities keyed by criteria) |
| C9 | `crates/sc-jev-backends/tests/llm.rs:74-85` | `sc-decision-llm/tests/translate.rs` | old answer fixture |

## 2. Required edits after copying

- All: `JevError` -> `LlmError` / `DecisionLlmError`; `ResolvedApiKey` -> `sc_agent_secrets::ApiKey`; `HeaderName::new(..)` -> `&'static str`; `ResponseInvalidCause` goes.
- `endpoints.rs`: `ProviderKind::{Anthropic, OpenAi}` -> `Wire::{AnthropicMessages, ChatCompletions}`. Keep `ANTHROPIC_HOST`, `/v1/messages`, `ANTHROPIC_VERSION`. Chat Completions URL per provider: OpenAI `https://api.openai.com/v1/chat/completions`, OpenRouter `https://openrouter.ai/api/v1/chat/completions`, Together `https://api.together.ai/v1/chat/completions`, Nous `https://inference-api.nousresearch.com/v1/chat/completions`. Models list: `GET {base}/v1/models` per provider (Anthropic `https://api.anthropic.com/v1/models`). Delete the count paths. Anthropic `x-api-key`; Chat Completions `Authorization: Bearer`.
- `decode.rs`: `parse_vendor` -> `decode_anthropic` / `decode_chat` -> `LlmResponse`. Keep Anthropic text concatenation (drop `non_text_blocks`). Usage: provider's own counters (Chat `prompt_tokens`/`completion_tokens`); no cache sums. Truncation: `stop_reason == "max_tokens"`, Chat `finish_reason == "length"`. Refusal: Anthropic `stop_reason == "refusal"`, Chat `message.refusal` non-null. First choice via `.first()`. Negative counter -> `ReplyInvalid`.
- `client.rs`: the fold becomes `LlmClient::encode`'s tail.
- `tests/decode.rs`: port builders; add `chat_message(text, finish_reason)`; truncation asserts `LlmError::Truncated` on both wires.
- `answers.rs`: `LlmOutput::parse` is deleted (LlmClient::decode always returns parsed JSON for Output::Json; a missing reply.json is decision_llm.invariant). Replace the choice-only check with one check per Jev question type (section 3). Delete `input_bytes`, `parse_output`, `id_set`, `QuestionAnswer`.

## 3. New code (exact public surface; bodies elided)

### sc-llm-provider

```rust
#![warn(missing_docs)]
mod client; mod decode; mod encode; mod endpoints; mod error; mod ids; mod provider; mod types;
pub use client::{LlmClient, Wire};
pub use provider::LlmProvider;
pub use error::LlmError;
pub use ids::{catalog_model_id, provider_model_id};
pub use types::{ImageMediaType, LlmRequest, LlmResponse, Message, Output, Part, Role, Usage};

// src/types.rs - any LLM call (REQ-SAG-0502), not only decisions
#[derive(Debug, Clone, PartialEq)]
pub struct LlmRequest<'a> { pub model: ModelId, pub system: Option<String>, pub messages: Vec<Message<'a>>, pub max_tokens: u32, pub output: Output }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message<'a> { pub role: Role, pub content: Vec<Part<'a>> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part<'a> {
    Text(String),
    /// Anthropic `{type:"image", source:{type:"base64", media_type, data}}`; Chat `{type:"image_url", image_url:{url:"data:<media_type>;base64,<data>"}}`.
    Image { media_type: ImageMediaType, data_base64: Cow<'a, str> },   // ImageMediaType: #[non_exhaustive] { Png, Jpeg, OctetStream } with as_str()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role { User, Assistant }
impl Role { pub const fn as_str(self) -> &'static str; }
#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    Text,
    /// Anthropic `output_config.format = {type:"json_schema", schema}`;
    /// Chat Completions `response_format = {type:"json_schema", json_schema:{name, strict:true, schema}}`.
    Json { name: String, schema: serde_json::Value },
}
#[derive(Debug, Clone, PartialEq)]
pub struct LlmResponse {
    pub model: String,                       // as the provider reports it
    pub text: String,
    pub json: Option<serde_json::Value>,     // Some(parsed text) for Output::Json
    pub stop_reason: Option<String>,         // verbatim
    pub usage: Usage,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Maps the provider wire's usage fields (Anthropic `usage.input_tokens/output_tokens`, Chat `prompt_tokens/completion_tokens`).
/// Not a Jev-API-mapped type (the Jev usage is sc_decision_schema::Usage), so it is outside REQ-SAG-0201 AC2.
pub struct Usage { pub input_tokens: u64, pub output_tokens: u64 }

// src/provider.rs
/// The providers with an LLM wire (every sc_agent_types::Provider except TypeSafe).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LlmProvider { Anthropic, OpenAi, OpenRouter, Together, Nous }
impl LlmProvider { pub const fn from_provider(provider: Provider) -> Option<Self>; }   // TypeSafe -> None
impl From<LlmProvider> for Provider {}

// src/client.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Wire { AnthropicMessages, ChatCompletions }
impl Wire { pub const fn of(provider: LlmProvider) -> Self; }   // total

/// Debug is written by hand (GeneralLogger has no Debug): provider, wire, endpoint log form, `key: <redacted>`, `log: <set|unset>`.
#[derive(Clone)]
pub struct LlmClient { provider: LlmProvider, wire: Wire, endpoint: Endpoint, key: ApiKey, log: Option<GeneralLogger> }
impl fmt::Debug for LlmClient { /* as above */ }
impl LlmClient {
    /// Built-in endpoint; no logger. Err (LlmError::Http with http.invalid_endpoint) only if a built-in endpoint
    /// constant fails Endpoint::parse; a test builds a client for every LlmProvider.
    pub fn new(provider: LlmProvider, key: ApiKey) -> Result<Self, LlmError>;
    pub fn with_endpoint(self, endpoint: Endpoint) -> Self;
    /// The caller's GeneralLogger (default or injected, REQ-SAG-0008); decode writes `llm.usage` to it.
    pub fn with_log(self, log: GeneralLogger) -> Self;
    pub fn provider(&self) -> LlmProvider;
    pub fn wire(&self) -> Wire;
    /// Sans-IO: the POST to send (JSON body, headers, secret header attached with `HttpRequest::with_secret(..)?`, a-5;
    /// every built-in endpoint is https, so http.key_on_plain_http can only come from a with_endpoint override -> LlmError::Http).
    pub fn encode(&self, request: &LlmRequest) -> Result<HttpRequest, LlmError>;
    /// Sans-IO: read a final reply. Non-2xx -> ApiStatus with ProviderReply::read(&response, Some(&key)).
    /// With a logger set, every 2xx reply whose envelope decodes (usage read, including truncated, refused
    /// and not-JSON outputs) writes one `llm.usage` record carrying `ctx` (a-8 LogContext, re-exported by sc-agent-http: session_id, position).
    pub fn decode(&self, request: &LlmRequest, response: HttpResponse, ctx: LogContext) -> Result<LlmResponse, LlmError>;
    /// Wrap a send failure with this client's provider, for callers that send.
    pub fn http_error(&self, error: HttpError) -> LlmError;
    /// GET {base}/v1/models with the credential header (decide doctor). The caller sends it on a probe
    /// transport (`UreqTransport::new(Timeouts::probe())`, connect 5 s, request 10 s; a-5), not the 120 s decision one.
    pub fn models_request(&self) -> Result<HttpRequest, LlmError>;   // secret via with_secret(..)? -> LlmError::Http
    /// The provider's model ids (`data[].id`, or a top-level array of `{id}`); non-2xx -> ApiStatus.
    pub fn decode_models(&self, response: HttpResponse) -> Result<Vec<String>, LlmError>;
}

// src/ids.rs (REQ-SAG-0103; rule in Decisions)
pub fn provider_model_id(provider: LlmProvider, model: &ModelId) -> String;
pub fn catalog_model_id(provider: LlmProvider, provider_id: &str) -> String;
```

Bodies: Anthropic `{"model","max_tokens","system"?,"messages":[{"role","content"}],"output_config"?}`, headers `anthropic-version: 2026-10-21`, `content-type: application/json`, secret `x-api-key`. Chat Completions `{"model","max_completion_tokens" (openai) | "max_tokens" (others),"messages":[{"role":"system"?},...],"response_format"?}`, `content-type`, `Authorization: Bearer`. `model` = `provider_model_id(provider, &request.model)` (REQ-SAG-0103).

### sc-decision-llm

```rust
#![warn(missing_docs)]
mod answers; mod backend; mod error; mod prompt; mod schema;
pub use backend::LlmDecisionBackend;
pub use error::DecisionLlmError;

// src/backend.rs - Haiku and Luna are this one type with a different (provider, model)
/// Debug by hand like LlmClient (llm, model, max_tokens, `log: <set|unset>`).
#[derive(Clone)]
pub struct LlmDecisionBackend { llm: LlmClient, model: ModelId, max_tokens: u32, log: Option<GeneralLogger> }
impl fmt::Debug for LlmDecisionBackend { /* as above */ }
impl LlmDecisionBackend {
    pub const DEFAULT_MAX_TOKENS: u32 = 16_000;
    /// Haiku: LlmClient::new(LlmProvider::Anthropic, key) with catalog.resolve(Latest(Haiku));
    /// Luna: LlmClient::new(LlmProvider::OpenAi, key) with catalog.resolve(Latest(Luna)).
    /// (Catalog::resolve is infallible, a-1: it returns the ModelId, no error arm.)
    pub fn new(llm: LlmClient, model: ModelId) -> Self;
    pub fn with_max_tokens(self, max_tokens: u32) -> Result<Self, DecisionLlmError>;   // 0 -> decision_llm.invalid_max_tokens
    /// Sets the logger on itself and on its LlmClient (`self.llm = self.llm.with_log(log.clone())`), so one
    /// decode writes `llm.usage` (LlmClient) and, on a mismatch, `llm.schema_mismatch` (this backend).
    pub fn with_log(self, log: GeneralLogger) -> Self;
    pub fn model(&self) -> &ModelId;
}
impl Backend for LlmDecisionBackend {
    /// DecisionRequest -> LlmRequest (built-in prompt + per-question JSON schema) -> HttpRequest.
    /// Assets: after the JSON text part, one `Part::Text("asset <id>:")` and one `Part::Image` per asset in request order
    /// (`data_base64` as given; a multipart `part` from its ImagePart bytes, base64-encoded; DecisionRequest is valid by
    /// construction (a-2 DecisionRequest::new), so there is no validator call, part check or part error here). Nothing else is refused here;
    /// a provider that rejects an input returns its error (REQ-SAG-0901).
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError>;
    /// HttpResponse -> LlmResponse -> SystemOneResponse with one Answer per question, in request order.
    /// With a logger set, each DecisionError::SchemaMismatch it returns writes one `llm.schema_mismatch` record.
    /// It passes `ctx` (a-2 trait parameter) on to LlmClient::decode, so llm.usage and llm.schema_mismatch carry it.
    fn decode(&self, response: HttpResponse, request: &DecisionRequest, ctx: &LogContext) -> Result<SystemOneResponse, DecisionError>;
}

// src/prompt.rs
/// Built in (ADR-SAG-0001). Owner-approved 2026-11-10:
/// "You answer decision questions in place of a decision model. The user message is JSON with a state and
/// questions keyed by id; images, if any, follow it. Answer every question in the required JSON schema. For a
/// choice or score question give your own probability for every option and your confidence from 0 to 1; for a
/// noul question give the probability that the answer is true; for a box question give the box edges and your
/// confidence. Return only the JSON."
pub(crate) const SYSTEM_PROMPT: &str;
pub(crate) fn translate(request: &DecisionRequest, model: &ModelId, max_tokens: u32) -> Result<LlmRequest, DecisionLlmError>;
// user message = Part::Text(compact JSON {"state": <request.state>, "questions": <request.questions>} in document order), then the asset parts

// src/schema.rs - the required schema (REQ-SAG-0901)
pub(crate) fn answer_schema(request: &SystemOneRequest) -> serde_json::Value;
// {"type":"object","required":["answers"],"additionalProperties":false,
//  "properties":{"answers":{"type":"object","additionalProperties":false,
//    "required":[<every question id>],"properties":{<id>: <per-type schema>}}}}

// src/answers.rs - the schema check
pub(crate) fn answers(request: &SystemOneRequest, reply: &LlmResponse) -> Result<SystemOneResponse, DecisionLlmError>;
```

Per question type: the model self-reports every value (REQ-SAG-0901); the schema requires them; `answers` checks the reply against the same rules (a provider's strict mode is not trusted):

| Jev type | Required from the model (schema) | Filled locally |
|---|---|---|
| `noul` | `noul`: number | `type` |
| `choice` | `choice`: enum of the criteria keys; `confidence`: number; `probabilities`: object with every criteria key required, numbers, `additionalProperties:false` | `type` |
| `score` | `score`, `confidence`: number; `probabilities`: object with keys `"0".."n-1"` required | `type`; `legend` = `{"<index>": criteria[index]}` from the request |
| `box` | `box`: `{top,bottom,left,right}` numbers; `confidence`: `["number","null"]` | `type` |

Anthropic structured outputs accept no numeric min/max, so ranges are not in the schema; values pass through unrounded and unchecked for range or sum (REQ-SAG-0007). `usage` = `LlmResponse.usage`; `model` = `LlmResponse.model` (as the provider names it).

How decode maps failures (the runner resends on `SchemaMismatch` up to max_sends, then returns it, A-9; backend name `"llm"`):
- `DecisionError::SchemaMismatch { backend: "llm", detail }`: `LlmError::OutputNotJson`, `LlmError::Truncated`, `LlmError::NoText`, `DecisionLlmError::AnswerIds`, `DecisionLlmError::AnswerInvalid` (e.g. a choice answer missing `probabilities`). `detail` = `DecisionLlmError::detail()`, which carries the LlmError chain.
- `DecisionError::Backend { backend: "llm", detail }`: every other LlmError (`ApiStatus`, `ReplyInvalid`, `Refused`, `Http`) and `DecisionLlmError::Encode` (incl. an unsupported question type and an asset with no image), `InvalidMaxTokens` and `Invariant`. Not resent; a provider rejecting an image is `ApiStatus` with its own message.

Log records (ANC-007), via `GeneralLogger::write(Level::Info, target, action, ctx, fields)` (session_id/position travel in `ctx`, not in `fields`); never a prompt, body, header value or key; a log write error is ignored (counted by sc-agent-log) and never changes a decode result:
- `llm.usage`, target `"sc_llm_provider"`: `provider`, `model` (as the provider reports it), `input_tokens`, `output_tokens`; ctx = the caller's LogContext.
- `llm.schema_mismatch`, target `"sc_decision_llm"`: `provider`, `model` (the ModelId sent), `error` (the cause id, e.g. `decision_llm.answer_invalid`, `llm.truncated`), `question` (when the cause names one), ctx = the LogContext passed to Backend::decode. No model text.
- "Request sent" is logged by sc-agent-http (`http.send`, a-5) when a caller sends through HttpClient; these crates are sans-IO and log no send.

## 4. Error lists - full enums (ErrorId from A-1, Display prints `<id>: <message>` only (chain via source()/detail()), no thiserror)

```rust
// sc-llm-provider/src/error.rs
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum LlmError {
    /// The send failed in sc-agent-http; the HttpError is kept whole.
    Http { provider: LlmProvider, source: HttpError },                 // "llm.http"
    /// A non-2xx final reply, in the provider's own words (redacted).
    ApiStatus { provider: LlmProvider, reply: Box<ProviderReply> },         // "llm.api_status"
    /// The 2xx body is not JSON or not the provider's envelope (serde message, never body bytes).
    ReplyInvalid { provider: LlmProvider, message: String },           // "llm.reply_invalid"
    /// The reply has no text content.
    NoText { provider: LlmProvider },                                  // "llm.no_text"
    /// stop_reason max_tokens / finish_reason length.
    Truncated { provider: LlmProvider, stop_reason: String },          // "llm.truncated"
    /// The model refused.
    Refused { provider: LlmProvider, message: String },                // "llm.refused"
    /// Output::Json was asked for and the text does not parse.
    OutputNotJson { provider: LlmProvider, message: String },          // "llm.output_not_json"
}
impl ErrorId for LlmError {}                                        // Http: cause_detail() = HttpError detail; source() = Some(source)

// sc-decision-llm/src/error.rs
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DecisionLlmError {
    /// Wraps LlmError (and through it HttpError), kept whole.
    Llm(LlmError),                                                   // "decision_llm.llm"
    /// State, questions or schema could not be serialized.
    Encode { message: String },                                      // "decision_llm.encode" (also: unsupported question type, asset with no image)
    /// The model's answer ids differ from the question ids.
    /// `extra` lists at most MAX_LISTED_IDS (8); extra_count is the full count.
    AnswerIds { missing: Vec<String>, extra: Vec<String>, extra_count: usize },   // "decision_llm.answer_ids"
    /// One answer lacks a required field, has a wrong type, or a choice outside the criteria.
    AnswerInvalid { question: String, message: String },             // "decision_llm.answer_invalid"
    InvalidMaxTokens { max_tokens: u32 },                            // "decision_llm.invalid_max_tokens"
    Invariant { message: String },                                   // "decision_llm.invariant" (never a mismatch)
}
impl ErrorId for DecisionLlmError {}
impl From<LlmError> for DecisionLlmError {}
```

impl DecisionLlmError { pub const MAX_MODEL_TEXT_BYTES: usize = 64; pub const MAX_LISTED_IDS: usize = 8; }
LlmError 7 variants, DecisionLlmError 6 (DecisionLlmError::Llm: cause_detail() = the LlmError detail). sc-decision-schema does not depend on either: they reach DecisionError as ErrorDetail (ADR-SAG-0004).

## 5. Config keys

None. Model ids come from the A-1 catalog, keys from A-4 (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENROUTER_API_KEY`, `TOGETHER_API_KEY`, `NOUS_API_KEY`). `max_tokens` is a constructor argument, default 16 000.

## 6. Tests (AC n = the nth acceptance line; canned replies and ScriptedTransport, no live calls, no key printed)

- `sc-llm-provider/tests/encode.rs`: `claude-haiku-5-5` through Anthropic and OpenRouter sends `claude-haiku-5-5` and `anthropic/claude-haiku-5.5` to different endpoints; `catalog_model_id` inverts `provider_model_id` for every MODELS row -> AC 2. Anthropic body/headers (`anthropic-version`, `x-api-key` via `SecretHeader::redacted`), Chat body and Bearer, openai `max_completion_tokens` vs openrouter `max_tokens`, `Output::Text` with no format field, `LlmProvider::from_provider` is None only for TypeSafe and `LlmClient::new` succeeds for every LlmProvider -> AC 3. `output_config.format.type == "json_schema"` and `response_format.json_schema.strict == true` -> AC 4. `Part::Image` as an Anthropic image block and a Chat image_url data URL -> AC 8. `models_request` GET /v1/models, `decode_models` both list shapes -> AC 10.
- `sc-llm-provider/tests/decode.rs`: Anthropic and Chat replies, usage as reported, JSON output parsed into `json` -> AC 4; truncation -> llm.truncated, refusal -> llm.refused, non-JSON body -> llm.reply_invalid, 401 -> llm.api_status with the provider message and no key -> AC 13, AC 15. A non-decision request for a model resolved by short name through the A-1 catalog, sent with `let permit = client.may_start(..)?; client.send_once(&request, permit, LogContext::NONE, &now)` over ScriptedTransport, returns its response -> AC 3.
- `sc-llm-provider/tests/log.rs`: `let log = MemoryLog::new()?; let client = LlmClient::new(..)?.with_log(log.general());` decodes one Anthropic reply with `LogContext::request(session_id, Some(3))`; `log.records()?` holds one MemoryRecord with `target == "sc_llm_provider"`, `action == "llm.usage"` the reported tokens in `fields` and that session_id and position in its context, and no key or body text -> AC 18.
- `sc-decision-llm/tests/translate.rs`: noul/choice/score/box request -> schema requiring all four ids and every per-type field, asked through output_config / response_format -> AC 6; a reply for all four parses as the Jev answer types, `legend` from the score criteria -> AC 5; choice missing `probabilities`, missing/extra ids, choice outside criteria, truncated -> decision.schema_mismatch with the cause id -> AC 7; refusal -> decision.backend -> AC 7; an asset (data_base64 and a multipart part) sent as an image part after the JSON text, and a provider 400 rejecting it -> decision.backend with cause llm.api_status and the provider's whole message -> AC 8; a request from `DecisionRequest::new(body, image_parts, false)?` whose asset names a part sends that part's bytes as an image part, with no validator call in the crate (grep) -> AC 9; `0.123456789` unchanged -> AC 12; injected `HttpError::Network` shows `http.network` and its message in the detail chain -> AC 13; compile test `Arc<dyn sc_decision_schema::Backend>` from LlmDecisionBackend -> AC 11.
- `sc-decision-llm/tests/log.rs`: `let log = MemoryLog::new()?; let backend = LlmDecisionBackend::new(..).with_log(log.general());` decodes one matching reply and one choice reply missing `probabilities`; `log.records()?` (write order) holds exactly two `action == "llm.usage"` records and one `action == "llm.schema_mismatch"` (target "sc_decision_llm", `fields` error decision_llm.answer_invalid and its question id), none with prompt, body or key text -> AC 18.
- The one-resend acceptance (REQ-SAG-0901 AC2) runs in A-9 over this backend's mismatch signal (does_not_close).

## 6a. Required validation

- `cargo build -p sc-llm-provider -p sc-decision-llm`
- `cargo clippy -p sc-llm-provider -p sc-decision-llm --all-targets -- -D warnings` with the workspace denies (a-1) -> AC 13
- `cargo test -p sc-llm-provider -p sc-decision-llm`
- `cargo tree -p sc-llm-provider -e normal --depth 1` and `cargo tree -p sc-decision-llm -e normal --depth 1`: direct sc-agent dependencies exactly as in section 7; full `cargo tree -p sc-llm-provider -p sc-decision-llm` has no sc-decision-runner, decide or tokio, and ureq appears only under sc-agent-http (no direct dependency), and sc-llm-provider's has no sc-decision-* crate -> AC 14, AC 16, AC 17
- `grep -rn "sleep" crates/sc-llm-provider/src crates/sc-decision-llm/src` finds nothing -> AC 16

## 7. Dependencies

- sc-llm-provider: `sc-agent-types` (Provider (for LlmProvider conversion), ModelId, ErrorId), `sc-agent-secrets` (ApiKey), `sc-agent-log` (GeneralLogger, Level, LogContext), `sc-agent-http` (HttpRequest, HttpResponse, Endpoint, SecretHeader, ProviderReply, HttpError), `serde_json = "1"`. No sc-decision-* crate.
- sc-decision-llm: `sc-decision-schema` (types, Backend, DecisionError), `sc-llm-provider`, `sc-agent-http`, `sc-agent-types`, `sc-agent-log` (GeneralLogger, Level), `serde_json = { version = "1", features = ["preserve_order", "float_roundtrip"] }`, `base64 = "0.22"` (multipart image bytes).
- Dev-dependencies (both): `sc-agent-http` with feature `testing` (ScriptedTransport, a-5), `sc-agent-log` with feature `testing` (MemoryLog, a-8). No hand-rolled transport or logger fakes; no fake Backend is needed, so a-2's ScriptedBackend is not used.
- Neither depends on ureq, tokio or thiserror, nor directly on sc-observability (it arrives only through sc-agent-log).

Estimated size: sc-llm-provider about 350 lines of code, sc-decision-llm about 320; tests about 420.

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-7.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] REQ-SAG-0103: sc-llm-provider: a caller gives LlmProvider and ModelId separately; one catalog id selects the same model through two providers, each sending its own id (OpenRouter anthropic/claude-haiku-5.5 for claude-haiku-5-5); catalog_model_id maps provider ids back; the OpenAI provider is named openai.
- [ ] REQ-SAG-0502: sc-llm-provider: LlmClient encodes/decodes a request to a non-decision LLM on any configured provider (Anthropic Messages; Chat Completions for openai, OpenRouter, Together, Nous); a test sends a non-decision request for a model selected by short name through HttpClient::may_start and send_once (SendPermit, LogContext) over sc_agent_http::testing::ScriptedTransport and returns its response.
- [ ] JSON-schema output: sc-llm-provider sends Output::Json as Anthropic output_config.format json_schema and as Chat Completions response_format json_schema (strict), and returns the reply text parsed.
- [ ] REQ-SAG-0901 AC1: Haiku/Luna answers to choice, score, noul and box questions are self-reported values that parse as the Jev answer type for each question (probabilities and confidence where Jev returns them).
- [ ] REQ-SAG-0901: every request asks for the required schema through the provider's structured-output feature (Anthropic output_config json_schema; Chat Completions response_format json_schema strict).
- [ ] REQ-SAG-0901: sc-decision-llm: a reply that does not match the required schema (not JSON, truncated, ids differ, a required field missing or mistyped) is returned from LlmDecisionBackend::decode as decision.schema_mismatch carrying the decision_llm/llm chain (what did not match), the signal the runner resends on; other failures are decision.backend and carry no resend signal.
- [ ] REQ-SAG-0901: images and every other provider-supported input are passed to the provider; a provider rejection returns the provider's error (decision.backend, cause llm.api_status); no unsupported-input error exists.
- [ ] REQ-SAG-0901: sc-decision-llm: LlmDecisionBackend::encode takes a DecisionRequest that is valid by construction (a-2 DecisionRequest::new rejects a part mismatch) and sends each named part's bytes as an image part; sc-decision-llm calls no validator and has no part check of its own.
- [ ] REQ-SAG-0503 support: LlmClient::models_request/decode_models return a provider's model ids (tests over canned replies).
- [ ] ADR-SAG-0001: LlmDecisionBackend implements the Backend trait imported from sc-decision-schema (no crate defines its own Backend trait) (encode(&DecisionRequest), decode(HttpResponse, &DecisionRequest, &LogContext)), receives the SystemOneRequest unchanged and returns a SystemOneResponse; all Haiku/Luna adaptation lives only in sc-decision-llm.
- [ ] REQ-SAG-0007: a model probability of 0.123456789 is returned as 0.123456789; no rounding, normalizing or clamping.
- [ ] REQ-SAG-0002: LlmError (7 llm.* ids, no llm.encode; no unsupported-provider id, since LlmProvider cannot name TypeSafe) wraps HttpError; DecisionLlmError (6 decision_llm.* ids, incl. invalid_max_tokens and invariant) wraps LlmError; both implement ErrorId; an injected HttpError::Network reaches DecisionError's detail chain with http.network and its message; clippy with the workspace denies passes.
- [ ] ADR-SAG-0004: `cargo tree -p sc-llm-provider -e normal --depth 1` lists exactly sc-agent-types, sc-agent-secrets, sc-agent-log and sc-agent-http as sc-agent dependencies (the http layer, no decision types); `cargo tree -p sc-decision-llm -e normal --depth 1` lists exactly sc-decision-schema, sc-llm-provider, sc-agent-http, sc-agent-types and sc-agent-log (the decision-backend layer); the full `cargo tree -p sc-llm-provider -p sc-decision-llm` contains no sc-decision-runner or decide. Neither crate's Cargo.toml depends on a crate in a higher layer: per ADR-SAG-0004 the layers from the bottom are types; config, secrets and log; http; decision schema; decision backends (sc-decision-llm over sc-llm-provider); runner, and decide sits above the runner.
- [ ] REQ-SAG-0006 / REQ-SAG-0401: no Haiku/Luna/mock type in sc-agent-types or sc-decision-schema; no consumer prompt; credential names only from sc-agent-secrets; keys only in SecretHeader, never in errors or Debug.
- [ ] REQ-SAG-0009 / REQ-SAG-0010: both crates are sans-IO; cargo tree lists no tokio; neither crate depends directly on ureq (it arrives only through sc-agent-http, which owns the transport per ADR-SAG-0004); no sleep call.
- [ ] REQ-SAG-0201: sc-decision-llm imports every Jev-mapped type (SystemOneRequest, SystemOneResponse and the answer types) from sc-decision-schema and defines none of its own; sc-llm-provider uses no decision types.
- [ ] ANC-007: the existing sc-agent library code in sc-llm-provider and sc-decision-llm is updated to write log records: LlmClient::with_log and LlmDecisionBackend::with_log take the caller's GeneralLogger (REQ-SAG-0008: default or injected); LlmClient::decode(request, response, ctx) writes one `llm.usage` record (provider, model, input and output tokens, the LogContext session_id and position) per decoded reply and LlmDecisionBackend::decode writes one `llm.schema_mismatch` record (provider, model, cause id) per schema mismatch it returns, both carrying the LogContext passed to Backend::decode, never a prompt, body or key (sends are logged by sc-agent-http as http.send); a test in each crate builds `MemoryLog::new()?` (a-8's sc_agent_log::testing), passes `log.general()` to with_log, decodes one reply (and in sc-decision-llm one schema mismatch), and finds those records in `log.records()?` by `action`.
- [ ] REQ-SAG-0201: sc_llm_provider::Usage maps provider wire fields (Anthropic usage input/output_tokens, OpenAI-compatible prompt/completion_tokens); it is not a Jev-API-mapped type (the Jev Usage is sc_decision_schema::Usage) and is excluded from REQ-SAG-0201 AC2.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); decisions are in the design:
- Q0 schema mismatch: resent unchanged up to max_sends, then decision.schema_mismatch with what did not match (REQ-SAG-0901).
- Q1 Chat Completions for every OpenAI-compatible provider (owner: IDK -> assistant); token field per provider.
- Q2 Nous shipped with the documented base and Bearer (assistant).
- Q3 images and every provider-supported input are passed through (REQ-SAG-0901).
- Q4 provider-mapped model id sent; response.model reported.
- Q6 model lists live in sc-llm-provider.
Owner approved the SYSTEM_PROMPT as written (2026-11-10).
Plan hardening round 1 (2026-11-09): PLAN-CRIT-005 LlmClient::with_log (llm.usage) and LlmDecisionBackend::with_log (llm.schema_mismatch), sc-agent-log added to both crates and the ADR-SAG-0004 line; PLAN-CRIT-014 encode calls DecisionRequest::validate (superseded in round 2: validate removed) (a-2), no own part check; PLAN-CRIT-010 tests use a-5 ScriptedTransport and a-8 MemoryLog (no fake Backend is needed, so a-2 ScriptedBackend is unused); front matter, Required validation, tests cite acceptance lines.

Plan review round 2 (2026-11-09): REQ-QA-006 adds REQ-SAG-0008; RBP-F002 validate calls dropped; RBP-F007 LlmProvider enum (no TypeSafe) in LlmClient::new, Wire::of, provider_model_id/catalog_model_id and LlmError; UnsupportedProvider deleted (8 llm ids then; 7 after llm.encode was removed in the QA-1 fix round); RSH-009 LlmClient::decode takes &LogContext and llm.usage carries it; models_request is sent on a Timeouts::probe() transport (a-10); ErrorId::cause -> cause_detail. RBP-F018 (Cow<'static, str> for static strings) skipped: low priority, a small per-send allocation with no observed defect.
a-1 contract folded in: Catalog::resolve is infallible (no error arm); Display prints `<id>: <message>` only; cause_detail()/recovery() from ErrorId.
Resolved: a-2 Backend::decode takes ctx: &LogContext; LlmDecisionBackend passes it to LlmClient::decode, so llm.usage and llm.schema_mismatch carry it. LogContext/GeneralLogger::write(.., ctx, fields)/may_start+SendPermit+send_once match a-5/a-8.
Round 3 (REQ-QA-R2-007): sc_llm_provider::Usage is a provider-wire type, excluded from REQ-SAG-0201 AC2 (new acceptance line).
Round 4: HttpRequest::with_secret(..)? at the secret attach in encode and models_request (now Result<HttpRequest, LlmError>; a-10 callers add ?).
- Plan correction (2026-11-10, dev round 1): the 'no ureq in cargo tree' check contradicted the planned sc-agent-http dependency (sc-agent-http owns UreqTransport; ADR-SAG-0004). No REQ/ADR requires absence of ureq (REQ-SAG-0009 is tokio, REQ-SAG-0010 is sleep), so the check is now 'no direct ureq dependency'. Feature-gating ureq in sc-agent-http was considered and not taken in phase A.
- LlmError::ApiStatus.reply is Box<ProviderReply> (clippy result_large_err), matching sc-agent-http.
- LlmClient::decode takes ctx: LogContext by value (Copy), per the section 3 signature.
- QA-1 fix round (code at abd2b24): LlmRequest<'a>/Message<'a>/Part<'a>; Part::Image {ImageMediaType, Cow}; encode/decode take &LlmRequest<'_>; llm.encode removed (Value Display is infallible); LlmClient::new keeps Result (unreachable, documented); decode reads only request.output; models URL = call endpoint with /messages or /chat/completions replaced by /models; a reply without `model` is llm.reply_invalid; inline media type sniffed from the base64 prefix (iVBORw0KGgo png, /9j/ jpeg, else octet-stream); the AC 13 chain test is a unit test in src/backend.rs (the HttpError injection point is private); model text in our messages capped at 64 bytes, extra ids at 8; llm.usage model capped at 128 bytes with model_truncated; refusal and ApiStatus text kept whole with the key scrubbed (REQ-SAG-0002).
- Plan-hardening round-2 note 'LlmClient::decode takes &LogContext' is superseded: it takes LogContext by value (Copy), per section 3.
