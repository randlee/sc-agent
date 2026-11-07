# sc-jev-check — architecture

**Boundary:** `jev-check`  
**Depends on:** `sc-agent-types` (features `git`), `sc-agent-secrets`, `sc-jev`, `sc-jev-backends`

Library **engine** for the jev-check parity slice: `check.toml`, extraction,
canonical payload build, backend selection, sessions, equal-input, LLM
system-prompt loading, and the one run API (ADR-SAG-0024). Does **not** send
TypeSafe HTTP itself — delegates to `JudgeBackend` impls in `sc-jev` and
`sc-jev-backends`.

## Modules

| Module | Purpose |
| --- | --- |
| `config` | `check.toml` (`RepoRelativePath` paths, optional top-level `backends`), `[llm_judge]` path, packaged prompt |
| `extract` | `file`, `table-column`, `subjects`, `command` — matched directly by `extract_input` (no extractor trait); see [`extraction.md`](extraction.md); **no** tree-sitter deps |
| `payload` | `build_payload`: one canonical `PayloadBundle` per check; caller request documents. No byte cap (each backend owns its cap, REQ-SAG-0314) |
| `run` | `prepare` → `run`, `discover_repo_root`, `RunConfig`, `RunResponse` + `Envelope` (ADR-SAG-0024) |
| `session` | Stateless `FileSessionStore`: `save`, `load`, `ask(&registry, &log)`; no-lock concurrency model (REQ-SAG-0607) |
| `equal_input` | Equal-input record helpers |
| `registry` | `BackendRegistry::{from_env, from_resolver, register}`; private `FACTORIES` table (ADR-SAG-0009). Feature `test-util` adds `register_scripted_backends` and `RunResponse::test_builder` for tests only (ADR-SAG-0022). |

## Composition

```text
prepare(CheckSource, RunConfig) -> PreparedRun
    load check.toml (or a caller request) at rev → build PayloadBundle once
    select backends: RunConfig::backends | source `backends` | [jev]
    resolve the LLM system prompt when an LLM backend is selected
run(PreparedRun) -> RunResponse
    dry run: payload, payload_bytes, per-backend limits; nothing sent
    live:    registry (injected | from_env) → validate_for_send on every backend
             → judge each in order (later LLM backends get session context)
             → save the session once when an ask-capable backend answered
```

`sc-ai` and `sc-jev-py` depend on **`sc-jev-check`**, not on `sc-jev`
directly, and print `Envelope::run(&response)` unchanged. `prepare` is the
public payload assembly entry (ADR-SAG-0005); callers do not build payloads.
Without an explicit root both consumers use `discover_repo_root` (git
toplevel, ADR-SAG-0003).

## Test seams

Integration target `run_response` needs this crate's `test-util` feature and
declares `[[test]] required-features = ["test-util"]`; its validation lines
pass `--features test-util`. There is no self dev-dependency
(`boundaries/README.md`, D0 outcome 4).

## Non-responsibilities

- TypeSafe / Anthropic / OpenAI HTTP (backend crates).
- CLI argv, PyO3 glue (thin adapters only).
