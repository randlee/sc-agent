# sc-decision-llm

Haiku and Luna decision backends for sc-agent (REQ-SAG-0901, ADR-SAG-0001).

`LlmDecisionBackend` implements `sc_decision_schema::Backend` over `sc_llm_provider::LlmClient`:
it sends the Jev request (state, questions and every image) with a built-in system prompt and
a required per-question JSON schema through the provider's structured-output feature, and
returns the model's self-reported answers as a `SystemOneResponse`. A reply that does not match
the schema is `decision.schema_mismatch`, the signal the runner resends on; every other failure
is `decision.backend`.

Haiku is `LlmClient::new(LlmProvider::Anthropic, key)` with the catalog's latest Haiku; Luna is
`LlmClient::new(LlmProvider::OpenAi, key)` with the latest Luna. The crate is sans-IO.
