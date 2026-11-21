//! Sans-IO LLM provider wire of sc-agent (REQ-SAG-0502, ADR-SAG-0004).
//!
//! - [`LlmClient`] encodes an [`LlmRequest`] into one `sc_agent_http::HttpRequest` and decodes
//!   the provider's final `HttpResponse` into an [`LlmResponse`]. Anthropic speaks Anthropic
//!   Messages; OpenAI, OpenRouter, Together and Nous speak Chat Completions ([`Wire`]).
//! - [`Output::Json`] asks for a JSON-schema reply through the provider's structured-output
//!   feature and returns the parsed value.
//! - [`provider_model_id`] / [`catalog_model_id`] map a provider-neutral catalog id to the id
//!   each provider uses and back (REQ-SAG-0103).
//! - [`LlmClient::models_request`] / [`LlmClient::decode_models`] list a provider's models.
//! - [`LlmError`] is the one error type, with stable `llm.*` ids; a send failure is kept whole
//!   as [`LlmError::Http`].
//!
//! The crate never sends and never waits: callers send through `sc_agent_http::HttpClient`
//! (REQ-SAG-0009, REQ-SAG-0010). It sits on sc-agent-http and uses no decision types; with a
//! logger set, [`LlmClient::decode`] writes one `llm.usage` record per decoded reply.
//!
//! # Examples
//!
//! ```
//! use sc_llm_provider::{LlmProvider, Wire, provider_model_id};
//!
//! let model = "claude-haiku-5-5".parse()?;
//! assert_eq!(provider_model_id(LlmProvider::Anthropic, &model), "claude-haiku-5-5");
//! assert_eq!(provider_model_id(LlmProvider::OpenRouter, &model), "anthropic/claude-haiku-5.5");
//! assert_eq!(Wire::of(LlmProvider::Nous), Wire::ChatCompletions);
//! # Ok::<(), sc_agent_types::TypesError>(())
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod client;
mod decode;
mod encode;
mod endpoints;
mod error;
mod ids;
mod provider;
mod types;

#[doc(inline)]
pub use client::{LlmClient, Wire};
#[doc(inline)]
pub use error::LlmError;
#[doc(inline)]
pub use ids::{catalog_model_id, provider_model_id};
#[doc(inline)]
pub use provider::LlmProvider;
#[doc(inline)]
pub use types::{ImageMediaType, LlmRequest, LlmResponse, Message, Output, Part, Role, Usage};
