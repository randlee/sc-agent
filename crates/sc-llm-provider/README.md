# sc-llm-provider

Sans-IO LLM provider wire for sc-agent (REQ-SAG-0502, ADR-SAG-0004).

- `LlmClient::encode` builds the `sc_agent_http::HttpRequest` for an `LlmRequest`:
  Anthropic Messages for `anthropic`, Chat Completions for `openai`, `openrouter`,
  `together` and `nous`.
- `Output::Json` asks for a JSON-schema reply (Anthropic `output_config.format`,
  Chat Completions `response_format` strict) and `LlmClient::decode` returns it parsed.
- `provider_model_id` / `catalog_model_id` map catalog model ids to provider ids (REQ-SAG-0103).
- `LlmClient::models_request` / `decode_models` list a provider's models (decidr doctor).

The crate never sends: callers send through `sc_agent_http::HttpClient`. It uses no
decision types, starts no runtime and never sleeps.
