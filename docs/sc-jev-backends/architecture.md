# sc-jev-backends — architecture

**Boundary:** `jev-backends`  
**Depends on:** `sc-agent-types`, `sc-agent-http` (r-3), `serde_json` (API key resolved by **`sc-jev-check`** and passed at backend construction)

**HTTP policy:** `with_default_transport` builds `sc_agent_http::UreqTransport`
with connect 10 s and request 120 s (ADR-SAG-0007); busy retry is
`sc_agent_http::BusyPolicy` (REQ-SAG-0505 as amended in r-3). Other timeouts
are a transport choice at construction: `new(key, UreqTransport::new(Timeouts::new(..)))`.

Non-Jev **comparison** backends only. TypeSafe Jev HTTP lives in **`sc-jev`**.

## Modules

| Module | Role |
| --- | --- |
| `backend` | `AnthropicHaikuBackend` (Haiku) and `OpenAiLunaBackend` (Luna), API-direct; each implements `JudgeBackend` from `sc-agent-types` |
| `wire` | Provider request and reply shapes (Haiku `system`, Luna `instructions`; not the Jev wire document) |
| `validate` | Pre-send checks on the canonical payload and injected prompt: question types, choice lists, byte cap on the assembled body |
| `backend` (HTTP part) | Builds each `sc_agent_http::HttpRequest` (endpoint, headers, `SecretHeader`) and sends it with `send_with_busy_retry`; maps non-200 to `NETWORK_ERROR` kind `Status` with a redacted 300-byte snippet |

All modules are private; the crate root re-exports the public items.

## LLM wire envelope

Backends accept **caller-supplied** evaluator system prompt text plus canonical
payload bytes (ADR-SAG-0019). This crate does not read `check.toml` and does not
ship production prompt prose in source — only wiring, validation, and HTTP.

## Transport

From r-3 the transport is `sc_agent_http::Transport` (ADR-SAG-0026, HTTP
half). Both backends are generic, `AnthropicHaikuBackend<T: Transport =
UreqTransport>` and `OpenAiLunaBackend<T>`; `new(key, transport)`,
`with_default_transport(key)` (r-1 signature), `with_busy_policy(policy)` and
`with_cancel(token)`. The former `BackendTransport`, `RetrySleeper`,
`with_retry_sleeper`, `with_timeouts` and the `DEFAULT_*_TIMEOUT` constants
are gone; tests use `sc_agent_http::test_util` and
`BusyPolicy::with_test_hooks`.

No CLI, no PyO3, no `check.toml`, no Jev API endpoints.
