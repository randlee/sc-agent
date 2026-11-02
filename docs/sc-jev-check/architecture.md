# sc-jev-check — architecture

**Boundary:** `jev-check`  
**Depends on:** `sc-agent-types`, `sc-agent-secrets`, `sc-jev`, `sc-jev-backends`

Library **facade** for the jev-check parity slice: `check.toml`, extractors,
canonical payload build, sessions, equal-input, multi-backend `run_check`, and
LLM system-prompt loading. Does **not** send TypeSafe HTTP itself — delegates
to `JudgeBackend` impls in `sc-jev` and `sc-jev-backends`.

## Modules (Phase A)

| Module | Purpose |
| --- | --- |
| `config` | `check.toml`, packaged `jev.toml`, `[llm_judge]` paths |
| `extract` | `file`, `table-column`, `tree-sitter`, `command` — see [`extraction.md`](extraction.md) |
| `payload` | One canonical `PayloadBundle` per check |
| `run` | `run_check`, `run_request`, dry-run |
| `session` | Store, TTL, `ask` |
| `equal_input` | README equal-input record |
| `registry` | `Vec<Arc<dyn JudgeBackend>>` — select backends by id (ADR-SAG-0009) |

## Composition

```text
run_check → build PayloadBundle once
         → resolve llm system prompt file → LlmWireContext
         → for each backend: JudgeRequest { payload, llm? }
         → backend.judge(&req)
```

`sc-ai` and `sc-jev-py` depend on **`sc-jev-check`**, not on `sc-jev` directly.

## Non-responsibilities

- TypeSafe / Anthropic / OpenAI HTTP (backend crates).
- CLI argv, PyO3 glue (thin adapters only).
