---
id: a-7
phase: a
status: planned
closure_type: integration
target_boundary: sc-ai-cli
track: cli
wave: 4
branch: sprint/a-7-sc-ai-cli
worktree: sprint/a-7-sc-ai-cli
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [a-6, a-11]
parallel_safe: [a-8]
requirements:
  - REQ-SAG-0101
  - REQ-SAG-0503
  - REQ-SAG-0600
  - REQ-SAG-0601
  - REQ-SAG-0603
  - REQ-SAG-0610
  - REQ-SAG-0700
  - REQ-SAG-0701
  - REQ-SAG-0702
  - REQ-SAG-0703
  - REQ-SAG-0710
  - REQ-SAG-0711
  - NFR-SAG-0001
  - NFR-SAG-0002
  - NFR-SAG-0003
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0010
  - ADR-SAG-0013
  - ADR-SAG-0014
  - ADR-SAG-0015
  - ADR-SAG-0016
  - ADR-SAG-0019
owned_paths:
  - "crates/sc-ai/**"
  - "boundaries/sc-ai-cli/**"
  - "justfile"
  - "README.md"
---

# Sprint a-7 — sc-ai CLI (`jev`)

## Outcome

`sc-ai` binary with **`sc-ai jev`** subcommands (Phase A: check parity — `run`,
`dry-run`, `ask`): **blocking** process — JSON on stdout, typed errors on
stderr, exit 2 on failure. Success envelope
`{ "version", "operation", "data" }` where **`data`** = TypeSafe Jev response JSON +
**`session_id`** (REQ-SAG-0710 / ADR-SAG-0014). Feature **3** (multi-backend) proven
via integration test on this sprint (see acceptance).

## Deliverables

1. **REQ-SAG-0700** — Subcommands: `run`, `dry-run`, `ask` (session id + text); versioned response envelope.
2. **REQ-SAG-0701** — Errors: `{"error":"<JevError code>","message":"<fix hint>"}` on stderr only.
3. **REQ-SAG-0702** — Readback pairs for operations that mutate or consume paths (check.toml, repo root, optional `--llm-system-prompt-file` override per ADR-SAG-0019).
4. **REQ-SAG-0703** — `just install-sc-ai` or documented `cargo install --path crates/sc-ai` in README.
5. **NFR-SAG-0002** — No secrets in logs or JSON output.
6. **ADR-SAG-0010** — CLI is a thin adapter over **`sc-jev-check`**; no HTTP in `sc-ai`.
7. **REQ-SAG-0710** — Integration test asserts stdout is the versioned envelope and **`data`** includes Jev answer fields + `session_id` (ADR-SAG-0014).
8. **NFR-SAG-0003** — Document default log root; `LoggerBuilder` (v2) + adapter injects `SharedSessionLog`.
9. **REQ-SAG-0711** — Pin `sc-observability = "1.5.0"` in this crate; adapt v2 logger to `SharedSessionLog` for `sc-jev-check` (not inside `sc-jev`).
10. Integration — **`--llm-system-prompt-file`** override changes LLM wire, not `prompt_sha256` (ADR-SAG-0019).
11. Integration — one `sc-ai jev run` with two backend ids + fake transports: one payload, equal-input recorded (feature **3**).
12. **Cross-impl (consumer)** — `cargo test -p sc-jev-check cross_impl_branches` asserts payload digests match **`tests/fixtures/cross-impl/branches/`** producer fixture from **a-11** (README tighten **#1** / **#2** consumer side).

## Explicit code samples

```rust
// crates/sc-ai/src/error.rs — map JevError → stderr JSON + exit 2
fn emit_cli_error(err: &JevError) -> ! {
    let body = ScAgentErrorResponse {
        error: err.code().into(),
        message: err.fix_hint().into(),
    };
    eprintln!("{}", serde_json::to_string(&body).unwrap());
    std::process::exit(2);
}
```

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0101 | CLI stderr uses `JevError` codes from contract snapshot |
| req:REQ-SAG-0503 | CLI `--llm-system-prompt-file` override changes LLM wire only; `prompt_sha256` stable |
| req:REQ-SAG-0600 | Session create/load + TTL via CLI `ask` path |
| req:REQ-SAG-0601 | CLI `run`/`dry-run` resolve keys and inject LLM system prompt per ADR-SAG-0019 |
| req:REQ-SAG-0603 | Two backends in one CLI run share payload + equal-input |
| req:REQ-SAG-0700 | `run`, `dry-run`, `ask` emit versioned envelope |
| req:REQ-SAG-0701 | Failure exits 2; stderr JSON `{error, message}` only |
| req:REQ-SAG-0702 | Readback tests for path-consuming flags |
| req:REQ-SAG-0703 | README or `just` target documents install |
| req:REQ-SAG-0710 | `data` includes Jev body + `session_id` inside envelope |
| req:REQ-SAG-0711 | v2 logger adapter wired to `SharedSessionLog` |
| NFR-SAG-0001 | Typed errors only; no ad hoc exit messages |
| NFR-SAG-0002 | No secrets in stdout/stderr/logs |
| NFR-SAG-0003 | Default log root documented |
| boundary:sc-ai-cli | No HTTP crates in **direct** deps (`cargo tree -p sc-ai --depth 1`) |

## Required validation

```bash
just lint
cargo test -p sc-ai -- --nocapture
cargo test -p sc-ai -- test_multi_backend_cli_equal_input --exact --nocapture
cargo test -p sc-ai -- test_cli_prompt_override --exact --nocapture
cargo test -p sc-jev-check cross_impl_branches -- --nocapture
! cargo tree -p sc-ai --depth 1 | rg -q 'reqwest|hyper|ureq'
cargo build -p sc-ai --release
target/release/sc-ai --version
target/release/sc-ai jev dry-run --help
echo '{"invalid":true}' | target/release/sc-ai jev run --request-json - 2>/tmp/sc-ai-err.json; test $? -eq 2
python3 -c "import json; d=json.load(open('/tmp/sc-ai-err.json')); assert 'error' in d and 'message' in d"
```

Expected: exit 0 for build/test/help; run with bad JSON exits 2 with valid error object.

Integration tests (fake transport) must cover CLI parity rows tied to `run`, `dry-run`, `ask`.

## Parity checklist

See **plan.md** parity rows for sprint **a-7**.

## This sprint does not close

PyO3 (a-8), parallel question fan-out CLI, daemon/UDS transport.
