# Phase j intent

Phase j fixes the gaps in sc-agent's minimal design so the first ~2,500 lines of code land correctly. Full product requirements come after.

The design supports two call patterns through one small interface each:

- **Decision pattern**: questions in, Jev-shaped answers out.
- **LLM pattern**: JSON in, JSON out.

Design shape (owner-approved 2026-11-26):

```rust
pub trait Decision {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error>;
}
pub trait Llm {
    // JSON in, JSON out
    fn call(&self, req: &LlmRequest) -> Result<serde_json::Value, Error>; // messages + optional JSON schema
}

pub struct Jev { … }             // TypeSafe SystemOne
pub struct OpenAiCompat { … }    // chat/completions; base url, key env, model
pub struct AnthropicCompat { … } // Messages; base url, key env, model

impl Decision for Jev { … }
impl Decision for OpenAiCompat { … }    // Jev answer schema + system prompt (ANC-014)
impl Llm      for OpenAiCompat { … }
impl Decision for AnthropicCompat { … } // Jev answer schema + system prompt (ANC-014)
impl Llm      for AnthropicCompat { … }
```

- The LLM-as-Jev system prompt has a built-in default; `decidr.luna-system-prompt` and `decidr.haiku-system-prompt` override it from config (ANC-014).
- Adding a compatible provider is a config or table row, never new code (ANC-009, ANC-012).
- Tools built on sc-agent supply their tool name and follow sc-agent's rules (ANC-015); logger injection is for tests (ANC-016).

Record rule: MUSTs in records cover only what is visible outside the code: wire formats, files another tool reads, config keys, CLI behaviour. Traits, structs, fields and methods live in the sprint plan's signature tables and the code.

Priorities: extensibility, modularity, and the absolute minimum code that supports this design. Phase non-test Rust ≤ 2,500 lines (ANC-007).
