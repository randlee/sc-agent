//! Haiku and Luna decision backends of sc-agent (REQ-SAG-0901, ADR-SAG-0001).
//!
//! [`LlmDecisionBackend`] implements [`sc_decision_schema::Backend`] over
//! [`sc_llm_provider::LlmClient`]. Haiku and Luna are this one type with a different
//! (provider, model): Anthropic with the catalog's latest Haiku, OpenAI with the latest Luna.
//!
//! - `encode`: the `SystemOneRequest` becomes one LLM call: a built-in system prompt, the
//!   state and questions as compact JSON, then every asset as an image part, and a required
//!   per-question JSON schema asked for through the provider's structured-output feature.
//! - `decode`: the model's JSON becomes a `SystemOneResponse` with one Jev answer per question,
//!   in request order. Probabilities and confidence are the model's own, passed through
//!   unrounded (REQ-SAG-0007). A reply that does not match the schema is
//!   `DecisionError::SchemaMismatch`, which the runner resends up to `max_sends`; every other
//!   failure is `DecisionError::Backend`. Both carry the [`DecisionLlmError`] chain.
//!
//! Sans-IO (REQ-SAG-0009, REQ-SAG-0010): the runner sends through sc-agent-http. With a logger
//! set, decode writes `llm.usage` (sc-llm-provider) and one `llm.schema_mismatch` per mismatch.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod answers;
mod backend;
mod error;
mod prompt;
mod schema;

#[doc(inline)]
pub use backend::LlmDecisionBackend;
#[doc(inline)]
pub use error::DecisionLlmError;
