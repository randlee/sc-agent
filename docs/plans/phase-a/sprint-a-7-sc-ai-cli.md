---
id: a-7
phase: a
status: planned
closure_type: integration
target_boundary: sc-ai-cli
track: cli
wave: 3
branch: sprint/a-7-sc-ai-cli
worktree: sprint/a-7-sc-ai-cli
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [a-6]
parallel_safe: [a-8]
requirements:
  - REQ-SAG-0700
  - REQ-SAG-0701
  - REQ-SAG-0702
  - REQ-SAG-0703
  - REQ-SAG-0710
  - REQ-SAG-0711
  - NFR-SAG-0002
  - NFR-SAG-0003
adrs:
  - ADR-SAG-0010
  - ADR-SAG-0014
  - ADR-SAG-0016
  - ADR-SAG-0019
owned_paths:
  - "crates/sc-ai/**"
  - "boundaries/sc-ai-cli/**"
---

# Sprint a-7 — sc-ai CLI (`jev`)

## Outcome

`sc-ai` binary with **`sc-ai jev`** subcommands (Phase A: check parity — `run`,
`dry-run`, `ask`): **blocking** process — JSON on stdout, typed errors on
stderr, exit 2 on failure. Success **`data`** = TypeSafe Jev response JSON +
**`session_id`**. Features **1**–**3** per sc-ai-cli guidelines.

## Deliverables

1. **REQ-SAG-0700** — Subcommands: `run`, `dry-run`, `ask` (session id + text); versioned response envelope.
2. **REQ-SAG-0701** — Errors: `{"error":"<JevError code>","message":"<fix hint>"}` on stderr only.
3. **REQ-SAG-0702** — Readback pairs for operations that mutate or consume paths (check.toml, repo root, optional `--llm-system-prompt-file` override per ADR-SAG-0019).
4. **REQ-SAG-0703** — `just install-sc-ai` or documented `cargo install --path crates/sc-ai` in README.
5. **NFR-SAG-0002** — No secrets in logs or JSON output.
6. **ADR-SAG-0010** — CLI is a thin adapter over **`sc-jev-check`**; no HTTP in `sc-ai`.
7. **REQ-SAG-0710** — Integration test asserts stdout JSON includes Jev answer fields + `session_id`.
8. **NFR-SAG-0003** — Document default log root; `LoggerBuilder` (v2) + adapter injects `SharedSessionLog`.
9. **REQ-SAG-0711** — Pin `sc-observability = "1.5.0"` only in this crate; forward `try_log` / emit path.

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
| req:REQ-SAG-0700 | `run`, `dry-run`, `ask` emit versioned envelope |
| req:REQ-SAG-0710 | Success stdout includes Jev body + `session_id` |
| req:REQ-SAG-0711 | v2 logger adapter wired to `SharedSessionLog` |
| boundary:sc-ai-cli | No HTTP crates linked from `sc-ai` |

## Required validation

```bash
just lint
cargo test -p sc-ai -- --nocapture
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
