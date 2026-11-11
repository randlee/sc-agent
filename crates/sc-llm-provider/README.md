# sc-llm-provider

The sans-IO LLM provider wire for sc-agent. It encodes an LLM call into one HTTP request and decodes the provider's final reply. It never sends and never waits: callers send through `sc_agent_http::HttpClient`. It sits on `sc-agent-http`, uses no decision types, starts no runtime and never sleeps.

## What it provides

- `LlmClient`:
  - `LlmClient::new(provider, key)` builds a client at the provider's built-in endpoint. `with_endpoint` and `with_log` adjust it.
  - `encode` builds the `HttpRequest` for an `LlmRequest`. `decode` turns the final `HttpResponse` into an `LlmResponse`.
  - `models_request` and `decode_models` list a provider's models (`decide doctor` uses them).
- `LlmProvider`: `Anthropic`, `OpenAi`, `OpenRouter`, `Together` and `Nous`. `Wire::of(provider)` gives the wire. Anthropic speaks Anthropic Messages; the others speak Chat Completions.
- `LlmRequest`, `Message`, `Part` (text or an image), `Role`, `ImageMediaType`, `LlmResponse` and `Usage`.
- `Output::Json { name, schema }` asks for a JSON-schema reply through the provider's structured-output feature. `decode` returns the parsed value.
- `provider_model_id` and `catalog_model_id` map a provider-neutral catalog id to the id each provider uses, and back.
- `LlmError`, with stable `llm.*` ids. A send failure is kept whole as `LlmError::Http`. With a logger set, `decode` writes one `llm.usage` record per decoded reply.

## Usage

```rust,ignore
use sc_agent_http::LogContext;
use sc_llm_provider::{LlmClient, LlmProvider, LlmRequest, Message, Output, Part, Role};

let client = LlmClient::new(LlmProvider::Anthropic, key)?; // key: an sc_agent_secrets::ApiKey
let model = "claude-haiku-5-5".parse()?;
let request = LlmRequest {
    model,
    system: None,
    messages: vec![Message {
        role: Role::User,
        content: vec![Part::Text("Reply with the word ready.".to_owned())],
    }],
    max_tokens: 64,
    output: Output::Text,
};
let http_request = client.encode(&request)?;
// Send it through sc_agent_http::HttpClient, then:
let response = client.decode(&request, reply, LogContext::NONE)?; // reply: HttpResponse
```

For this model, `provider_model_id(LlmProvider::OpenRouter, &model)` is `anthropic/claude-haiku-5.5`.

## Depends on

- `sc-agent-http`, `sc-agent-log`, `sc-agent-secrets` and `sc-agent-types`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). `sc-decision-llm` builds on this crate.

## Requirements and decisions

- REQ-SAG-0502: sc-agent can launch any LLM.
- REQ-SAG-0103: provider and model are separate.
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top.
- REQ-SAG-0009 and REQ-SAG-0010: no tokio, no sleep.
- ADR-SAG-0004: crate layering and per-crate error unions.
