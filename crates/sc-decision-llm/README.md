# sc-decision-llm

The Haiku and Luna decision backends of sc-agent. `LlmDecisionBackend` answers a Jev request with an LLM: it implements `sc_decision_schema::Backend` over `sc_llm_provider::LlmClient`. Haiku and Luna are this one type with a different (provider, model): Anthropic with the catalog's latest Haiku, and OpenAI with the latest Luna.

It sits in the decision-backend layer, above `sc-llm-provider`. The crate is sans-IO: the runner sends through `sc-agent-http`.

## What it provides

- `LlmDecisionBackend`:
  - `LlmDecisionBackend::new(llm, model)`, with `with_max_tokens` (default `LlmDecisionBackend::DEFAULT_MAX_TOKENS`, 16,000) and `with_log`.
  - `encode` turns the `SystemOneRequest` into one LLM call: a built-in system prompt, the state and questions as compact JSON, every asset as an image part, and a required per-question JSON schema.
  - `decode` turns the model's JSON into a `SystemOneResponse` with one Jev answer per question, in request order. Probabilities and confidence are the model's own, unrounded.
- `DecisionLlmError`. A reply that does not match the schema is `decision.schema_mismatch`, the signal the runner resends on, up to `max_sends`. Every other failure is `decision.backend`. Both carry the `DecisionLlmError` chain.

With a logger set, `decode` writes `llm.usage` (from `sc-llm-provider`) and one `llm.schema_mismatch` per mismatch.

## Usage

```rust,ignore
use std::sync::Arc;

use sc_decision_llm::LlmDecisionBackend;
use sc_decision_schema::Backend;
use sc_llm_provider::{LlmClient, LlmProvider};

// Haiku: Anthropic with a Haiku model id; key is an sc_agent_secrets::ApiKey.
let llm = LlmClient::new(LlmProvider::Anthropic, key)?;
let backend: Arc<dyn Backend> = Arc::new(LlmDecisionBackend::new(llm, haiku_model));
// Give `backend` to a sc_decision_runner::DecisionRunner.
```

For Luna, use `LlmProvider::OpenAi` and a Luna model id. `sc-agent-types` supplies the latest id of each line (`Catalog::resolve`). `decidr` makes this choice in one place.

## Depends on

- `sc-agent-http`, `sc-agent-log`, `sc-agent-types`, `sc-decision-schema` and `sc-llm-provider`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). An `LlmError` reaches a `DecisionError` as its id, message and cause chain.

## Requirements and decisions

- REQ-SAG-0901: LLM decision backends return the Jev schema.
- REQ-SAG-0007: return values are not rounded.
- REQ-SAG-0006: no consumer-application types in shared crates.
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top.
- ADR-SAG-0001: decision backends implement one trait with one request form.
- ADR-SAG-0004: crate layering and per-crate error unions.
