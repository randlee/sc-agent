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
| `extract` | `file`, `table-column`, `subjects`, `command` — see [`extraction.md`](extraction.md); **no** tree-sitter deps |
| `payload` | One canonical `PayloadBundle` per check |
| `run` | `build_request`, `run_check`, `run_request`, dry-run |
| `session` | Store, TTL, `ask` |
| `equal_input` | README equal-input record |
| `registry` | `Vec<Arc<dyn JudgeBackend>>` — select backends by id (ADR-SAG-0009). Feature `test-util` adds `register_scripted_backends` for tests. Production builds leave the feature off. |

## Composition

```text
run_check → build PayloadBundle once
         → resolve llm system prompt file → LlmWireContext
         → for each backend: JudgeRequest { payload, llm? }
         → backend.judge(&req)
```

`sc-ai` and `sc-jev-py` depend on **`sc-jev-check`**, not on `sc-jev` directly.
`build_request` is the public payload assembly entry (ADR-SAG-0005). Callers
do not call `PayloadBuilder`.

## Non-responsibilities

- TypeSafe / Anthropic / OpenAI HTTP (backend crates).
- CLI argv, PyO3 glue (thin adapters only).
