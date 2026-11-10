<!-- Generated from bead agt-4p2.7 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-7: sc-llm-provider and sc-decision-llm
- id: `a-7` (bead `agt-4p2.7`)
- branch: `sprint/a-7-llm-provider`
- depends on: agt-4p2.2
- copy script: `docs/plans/phase-a/copy/a-7.sh`

## Description

sc-llm-provider and sc-decision-llm are sc-agent library crates. Anthropic Messages and OpenAI-compatible providers with JSON-schema output (LlmError); Haiku and Luna decision backends that translate the Jev request to a prompt and required schema, self-report probabilities and confidence, and signal a schema mismatch for resend. Records: REQ-SAG-0103, REQ-SAG-0502, REQ-SAG-0901, ADR-SAG-0001.

## Design

# Sprint A-7: sc-llm-provider and sc-decision-llm (bead agt-4p2.7)

Governing: REQ-SAG-0103, REQ-SAG-0502, REQ-SAG-0901 (docs/sc-decision-llm: in decider mode Haiku and Luna produce probabilities and confidence themselves and return the Jev answer schema, requested through the provider's structured-output feature; a reply that does not match is resent unchanged up to max_sends, then returned as decision.schema_mismatch with what did not match; every provider-supported input, images included, is passed through), ADR-SAG-0001, ADR-SAG-0004 (sc-llm-provider sits on sc-agent-http and needs no decision types; sc-decision-llm sits on sc-decision-schema and sc-llm-provider, below the runner; HttpError is wrapped by LlmError, and an LlmError reaches DecisionError as an ErrorDetail chain). Also binding: REQ-SAG-0002, REQ-SAG-0006, REQ-SAG-0007, REQ-SAG-0201, REQ-SAG-0401, REQ-SAG-0009, REQ-SAG-0010, owner-architecture, owner-decision-trait, owner-provider-and-model, owner-openai-naming, owner-lowlevel-no-tokio.

Copy script: `docs/plans/phase-a/copy/a-7.sh` (source sc-agent-ref@dce239a; dry run 2026-11-09, reconciliation: 14 copy steps, 8 files, 405 lines; no `sleep(`; the old `wire.rs:426` `expect` is not copied).

Both crates are sans-IO: they build an `HttpRequest` and read an `HttpResponse` (A-5). Sending, busy resend and holdoff are sc-agent-http plus the runner's queue; the schema-mismatch resend (up to max_sends) is the runner's too (A-9), signalled by `DecisionError::SchemaMismatch` from `decode`.

Decisions (assistant, 2026-11-09):
- Wire: Anthropic Messages for anthropic; Chat Completions for openai, openrouter, together and nous (one encoder). Token cap: `max_completion_tokens` for openai, `max_tokens` for the others.
- Nous: base `https://inference-api.nousresearch.com/v1`, Bearer, `response_format` json_schema; a model that rejects it returns the provider's error.
- Provider ids (REQ-SAG-0103): `provider_model_id` maps a catalog id to the provider's id: anthropic, openai, nous unchanged; openrouter `anthropic/<id>` for `claude-*` with the trailing `-<major>-<minor>` written `<major>.<minor>` (`claude-haiku-5-5` -> `anthropic/claude-haiku-5.5`) and `openai/<id>` for `gpt-*`; together `anthropic/<id>` / `openai/<id>` (Together's ids are unconfirmed; decide doctor reports any that do not map back). `catalog_model_id` is the inverse, used by decide doctor.
- Model lists for decide doctor (REQ-SAG-0503) live here: `LlmClient::models_request` / `decode_models`.
- Images: every asset is passed through (REQ-SAG-0901); a provider rejection is its own error (decision.backend, cause llm.api_status).
- Model name: the request sends `provider_model_id(provider, resolved ModelId)`; the response reports the model the provider names.
- System prompt: drafted below by the assistant (Mapika/decider uses none, plan-anchor has no decider mode); the owner reviews the draft.

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
- `answers.rs`: `LlmOutput::parse` keeps the not-JSON / no-`answers` split (`DecisionLlmError::AnswerInvalid` / `AnswerIds`). Replace the choice-only check with one check per Jev question type (section 3). Delete `input_bytes`, `parse_output`, `id_set`, `QuestionAnswer`.

## 3. New code (exact public surface; bodies elided)

### sc-llm-provider

```rust
#![warn(missing_docs)]
mod client; mod decode; mod encode; mod endpoints; mod error; mod ids; mod types;
pub use client::{LlmClient, Wire};
pub use error::LlmError;
pub use ids::{catalog_model_id, provider_model_id};
pub use types::{LlmRequest, LlmResponse, Message, Output, Part, Role, Usage};

// src/types.rs - any LLM call (REQ-SAG-0502), not only decisions
#[derive(Debug, Clone, PartialEq)]
pub struct LlmRequest { pub model: ModelId, pub system: Option<String>, pub messages: Vec<Message>, pub max_tokens: u32, pub output: Output }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message { pub role: Role, pub content: Vec<Part> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Text(String),
    /// Anthropic `{type:"image", source:{type:"base64", media_type, data}}`; Chat `{type:"image_url", image_url:{url:"data:<media_type>;base64,<data>"}}`.
    Image { media_type: &'static str, data_base64: String },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role { User, Assistant }
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
pub struct Usage { pub input_tokens: u64, pub output_tokens: u64 }

// src/client.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Wire { AnthropicMessages, ChatCompletions }
impl Wire { pub const fn of(provider: Provider) -> Option<Self>; }   // TypeSafe -> None

#[derive(Debug, Clone)]
pub struct LlmClient { provider: Provider, wire: Wire, endpoint: Endpoint, key: ApiKey }
impl LlmClient {
    pub fn new(provider: Provider, key: ApiKey) -> Result<Self, LlmError>;   // built-in endpoint; UnsupportedProvider
    pub fn with_endpoint(self, endpoint: Endpoint) -> Self;
    pub fn provider(&self) -> Provider;
    pub fn wire(&self) -> Wire;
    /// Sans-IO: the POST to send (JSON body, headers, secret header).
    pub fn encode(&self, request: &LlmRequest) -> Result<HttpRequest, LlmError>;
    /// Sans-IO: read a final reply. Non-2xx -> ApiStatus with ProviderReply::read(&response, Some(&key)).
    pub fn decode(&self, request: &LlmRequest, response: HttpResponse) -> Result<LlmResponse, LlmError>;
    /// Wrap a send failure with this client's provider, for callers that send.
    pub fn http_error(&self, error: HttpError) -> LlmError;
    /// GET {base}/v1/models with the credential header (decide doctor).
    pub fn models_request(&self) -> HttpRequest;
    /// The provider's model ids (`data[].id`, or a top-level array of `{id}`); non-2xx -> ApiStatus.
    pub fn decode_models(&self, response: HttpResponse) -> Result<Vec<String>, LlmError>;
}

// src/ids.rs (REQ-SAG-0103; rule in Decisions)
pub fn provider_model_id(provider: Provider, model: &ModelId) -> String;
pub fn catalog_model_id(provider: Provider, provider_id: &str) -> String;
```

Bodies: Anthropic `{"model","max_tokens","system"?,"messages":[{"role","content"}],"output_config"?}`, headers `anthropic-version: 2026-10-21`, `content-type: application/json`, secret `x-api-key`. Chat Completions `{"model","max_completion_tokens" (openai) | "max_tokens" (others),"messages":[{"role":"system"?},...],"response_format"?}`, `content-type`, `Authorization: Bearer`. `model` = `provider_model_id(provider, &request.model)` (REQ-SAG-0103).

### sc-decision-llm

```rust
#![warn(missing_docs)]
mod answers; mod backend; mod error; mod prompt; mod schema;
pub use backend::LlmDecisionBackend;
pub use error::DecisionLlmError;

// src/backend.rs - Haiku and Luna are this one type with a different (provider, model)
#[derive(Debug, Clone)]
pub struct LlmDecisionBackend { llm: LlmClient, model: ModelId, max_tokens: u32 }
impl LlmDecisionBackend {
    pub const DEFAULT_MAX_TOKENS: u32 = 16_000;
    /// Haiku: LlmClient::new(Provider::Anthropic, key) with catalog.resolve(Latest(Haiku));
    /// Luna: LlmClient::new(Provider::OpenAi, key) with catalog.resolve(Latest(Luna)).
    pub fn new(llm: LlmClient, model: ModelId) -> Self;
    pub fn with_max_tokens(self, max_tokens: u32) -> Self;
    pub fn model(&self) -> &ModelId;
}
impl Backend for LlmDecisionBackend {
    /// DecisionRequest -> LlmRequest (built-in prompt + per-question JSON schema) -> HttpRequest.
    /// Assets: after the JSON text part, one `Part::Text("asset <id>:")` and one `Part::Image` per asset in request order
    /// (`data_base64` as given; a multipart `part` from its ImagePart bytes, base64-encoded). Nothing is refused here;
    /// a provider that rejects an input returns its error (REQ-SAG-0901).
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError>;
    /// HttpResponse -> LlmResponse -> SystemOneResponse with one Answer per question, in request order.
    fn decode(&self, response: HttpResponse, request: &DecisionRequest) -> Result<SystemOneResponse, DecisionError>;
}

// src/prompt.rs
/// Built in (ADR-SAG-0001). Assistant draft for owner review:
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
- `DecisionError::Backend { backend: "llm", detail }`: every other LlmError (`ApiStatus`, `ReplyInvalid`, `Refused`, `UnsupportedProvider`, `Encode`) and `DecisionLlmError::Encode`. Not resent; a provider rejecting an image is `ApiStatus` with its own message.

## 4. Error lists - full enums (ErrorId from A-1, Display = detail(), no thiserror)

```rust
// sc-llm-provider/src/error.rs
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum LlmError {
    /// The send failed in sc-agent-http; the HttpError is kept whole.
    Http { provider: Provider, source: HttpError },                 // "llm.http"
    /// A non-2xx final reply, in the provider's own words (redacted).
    ApiStatus { provider: Provider, reply: ProviderReply },         // "llm.api_status"
    /// The provider has no LLM wire (TypeSafe).
    UnsupportedProvider { provider: Provider },                     // "llm.unsupported_provider"
    /// The request body could not be serialized.
    Encode { message: String },                                     // "llm.encode"
    /// The 2xx body is not JSON or not the provider's envelope (serde message, never body bytes).
    ReplyInvalid { provider: Provider, message: String },           // "llm.reply_invalid"
    /// The reply has no text content.
    NoText { provider: Provider },                                  // "llm.no_text"
    /// stop_reason max_tokens / finish_reason length.
    Truncated { provider: Provider, stop_reason: String },          // "llm.truncated"
    /// The model refused.
    Refused { provider: Provider, message: String },                // "llm.refused"
    /// Output::Json was asked for and the text does not parse.
    OutputNotJson { provider: Provider, message: String },          // "llm.output_not_json"
}
impl ErrorId for LlmError {}                                        // Http: cause = HttpError detail; source() = Some(source)

// sc-decision-llm/src/error.rs
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DecisionLlmError {
    /// Wraps LlmError (and through it HttpError), kept whole.
    Llm(LlmError),                                                   // "decision_llm.llm"
    /// State, questions or schema could not be serialized.
    Encode { message: String },                                      // "decision_llm.encode"
    /// The model's answer ids differ from the question ids.
    AnswerIds { missing: Vec<String>, extra: Vec<String> },          // "decision_llm.answer_ids"
    /// One answer lacks a required field, has a wrong type, or a choice outside the criteria.
    AnswerInvalid { question: String, message: String },             // "decision_llm.answer_invalid"
}
impl ErrorId for DecisionLlmError {}
impl From<LlmError> for DecisionLlmError {}
```

LlmError 9 variants, DecisionLlmError 4. sc-decision-schema does not depend on either: they reach DecisionError as ErrorDetail (ADR-SAG-0004).

## 5. Config keys

None. Model ids come from the A-1 catalog, keys from A-4 (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENROUTER_API_KEY`, `TOGETHER_API_KEY`, `NOUS_API_KEY`). `max_tokens` is a constructor argument, default 16 000.

## 6. Tests

- `sc-llm-provider/tests/encode.rs`: Anthropic body has `model`, `max_tokens`, `system`, `messages`, `output_config.format.type == "json_schema"`; headers `anthropic-version`, `content-type`; secret `x-api-key` (via `SecretHeader::redacted`); Chat body has `response_format.json_schema.strict == true` and Bearer; openai sends `max_completion_tokens`, openrouter `max_tokens`; `claude-haiku-5-5` through `Provider::Anthropic` and `Provider::OpenRouter` sends `claude-haiku-5-5` and `anthropic/claude-haiku-5.5` to different endpoints (REQ-SAG-0103 AC1); `catalog_model_id` inverts `provider_model_id` for every MODELS row; `Part::Image` encodes as an Anthropic image block and a Chat image_url data URL; `models_request` is GET /v1/models and `decode_models` reads both list shapes; `Output::Text` encodes with no format field (REQ-SAG-0502); `Provider::TypeSafe` -> llm.unsupported_provider.
- `sc-llm-provider/tests/decode.rs`: Anthropic and Chat replies decode; truncation -> llm.truncated; refusal -> llm.refused; non-JSON body -> llm.reply_invalid; 401 -> llm.api_status with the provider message and no key; JSON output parsed into `json`; usage as reported. A non-decision request for a model resolved by short name through the A-1 catalog is sent through `HttpClient::send_once` over a scripted transport and its response returned (REQ-SAG-0502).
- `sc-decision-llm/tests/translate.rs`: one request with noul, choice, score and box questions yields a schema requiring all four ids and every per-type field; a reply for all four decodes into a SystemOneResponse whose answers parse as the Jev answer types (REQ-SAG-0901 AC1); `legend` built from the score criteria; a choice reply missing `probabilities` -> decision.schema_mismatch with cause decision_llm.answer_invalid; missing/extra ids -> schema_mismatch (answer_ids); choice outside criteria -> schema_mismatch; refusal -> decision.backend; an asset (data_base64 and a multipart part) is sent as an image part after the JSON text; a provider 400 rejecting the image -> decision.backend with cause llm.api_status and the provider's whole message; the request asks for the schema through output_config / response_format (REQ-SAG-0901); `0.123456789` comes back unchanged (REQ-SAG-0007); an injected `HttpError::Network` wrapped as `DecisionLlmError::Llm(LlmError::Http{..})` shows `http.network` and its message in the detail chain (REQ-SAG-0002). The one-retry acceptance (REQ-SAG-0901 AC2) runs in A-9 over this backend's mismatch signal.

## 7. Dependencies

- sc-llm-provider: `sc-agent-types` (Provider, ModelId, ErrorId), `sc-agent-secrets` (ApiKey), `sc-agent-http` (HttpRequest, HttpResponse, Endpoint, SecretHeader, ProviderReply, HttpError), `serde_json = "1"`. No sc-decision-* crate.
- sc-decision-llm: `sc-decision-schema` (types, Backend, DecisionError), `sc-llm-provider`, `sc-agent-http`, `sc-agent-types`, `serde_json = { version = "1", features = ["preserve_order", "float_roundtrip"] }`, `base64 = "0.22"` (multipart image bytes).
- Neither depends on ureq, tokio, sc-observability or thiserror.

Estimated size: sc-llm-provider about 350 lines of code, sc-decision-llm about 320; tests about 420.

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-7.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] REQ-SAG-0103: a caller gives Provider and ModelId separately; one catalog id selects the same model through two providers, each sending its own id (OpenRouter anthropic/claude-haiku-5.5 for claude-haiku-5-5); catalog_model_id maps provider ids back; the OpenAI provider is named openai.
- [ ] REQ-SAG-0502: sc-llm-provider encodes/decodes a request to a non-decision LLM on any configured provider (Anthropic Messages; Chat Completions for openai, OpenRouter, Together, Nous); a test sends a non-decision request for a model selected by short name through HttpClient over a scripted transport and returns its response.
- [ ] JSON-schema output: Output::Json is sent as Anthropic output_config.format json_schema and as Chat Completions response_format json_schema (strict), and the reply text is returned parsed.
- [ ] REQ-SAG-0901 AC1: Haiku/Luna answers to choice, score, noul and box questions are self-reported values that parse as the Jev answer type for each question (probabilities and confidence where Jev returns them).
- [ ] REQ-SAG-0901: every request asks for the required schema through the provider's structured-output feature (Anthropic output_config json_schema; Chat Completions response_format json_schema strict).
- [ ] REQ-SAG-0901: a reply that does not match the required schema (not JSON, truncated, ids differ, a required field missing or mistyped) is returned from decode as decision.schema_mismatch carrying the decision_llm/llm chain (what did not match), so the runner resends it unchanged up to max_sends; other failures are decision.backend and are not resent.
- [ ] REQ-SAG-0901: images and every other provider-supported input are passed to the provider; a provider rejection returns the provider's error (decision.backend, cause llm.api_status); no unsupported-input error exists.
- [ ] REQ-SAG-0503 support: LlmClient::models_request/decode_models return a provider's model ids (tests over canned replies).
- [ ] ADR-SAG-0001: LlmDecisionBackend implements the Backend trait imported from sc-decision-schema (no crate defines its own Backend trait) (encode(&DecisionRequest), decode(HttpResponse, &DecisionRequest)), receives the SystemOneRequest unchanged and returns a SystemOneResponse; all Haiku/Luna adaptation lives only in sc-decision-llm.
- [ ] REQ-SAG-0007: a model probability of 0.123456789 is returned as 0.123456789; no rounding, normalizing or clamping.
- [ ] REQ-SAG-0002: LlmError (9 llm.* ids) wraps HttpError; DecisionLlmError (4 decision_llm.* ids) wraps LlmError; both implement ErrorId; an injected HttpError::Network reaches DecisionError's detail chain with http.network and its message; clippy with the workspace denies passes.
- [ ] ADR-SAG-0004: sc-llm-provider sits on the http layer and its only sc-agent crate dependencies are sc-agent-types, sc-agent-secrets, sc-agent-log and sc-agent-http; sc-decision-llm sits in the decision-backend layer and adds only sc-decision-schema and sc-llm-provider; neither depends on sc-decision-runner, checked by `cargo tree -p sc-llm-provider -p sc-decision-llm`.
- [ ] REQ-SAG-0006 / REQ-SAG-0401: no Haiku/Luna/mock type in sc-agent-types or sc-decision-schema; no consumer prompt; credential names only from sc-agent-secrets; keys only in SecretHeader, never in errors or Debug.
- [ ] REQ-SAG-0009 / REQ-SAG-0010: both crates are sans-IO; cargo tree lists no tokio or ureq; no sleep call.
- [ ] REQ-SAG-0201: sc-decision-llm imports every Jev-mapped type (SystemOneRequest, SystemOneResponse and the answer types) from sc-decision-schema and defines none of its own; sc-llm-provider uses no decision types.
- [ ] ANC-007: sc-llm-provider's LlmClient and sc-decision-llm's LlmDecisionBackend write log records for each request sent, its token usage and each schema mismatch (never a prompt, body or key) to an injected GeneralLogger; a test injects an in-memory GeneralLogger, runs one request and one schema mismatch, and finds those records.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); decisions are in the design:
- Q0 schema mismatch: resent unchanged up to max_sends, then decision.schema_mismatch with what did not match (REQ-SAG-0901).
- Q1 Chat Completions for every OpenAI-compatible provider (owner: IDK -> assistant); token field per provider.
- Q2 Nous shipped with the documented base and Bearer (assistant).
- Q3 images and every provider-supported input are passed through (REQ-SAG-0901).
- Q4 provider-mapped model id sent; response.model reported.
- Q6 model lists live in sc-llm-provider.
Owner review pending (not a question): the SYSTEM_PROMPT draft in the design.
