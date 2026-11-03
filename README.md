# sc-agent

`sc-ai` is the JSON-first CLI for Jev check parity (`run`, `dry-run`, and `ask`).

## Install

```bash
just install-sc-ai
```

The same install without `just`:

```bash
cargo install --path crates/sc-ai --locked
```

## Session logs

The default log root is `<repo-root>/.sc-ai`. `--log-dir <path>` overrides it.
When that flag is absent, a non-empty `SC_LOG_ROOT` environment variable is
the log root.

The v2 file sink writes `<log-root>/logs/sc-ai.log.jsonl`. Session files
default to `<repo-root>/.sc-ai/sessions` (`--sessions-dir` overrides that
directory). Logs record the session id, backend, and payload digest. They do
not record API keys or prompt text.

## Commands

```bash
sc-ai jev run --repo-root . --check check.toml --backend jev
sc-ai jev dry-run --repo-root . --check check.toml
sc-ai jev ask --session-id <uuid> --text "why?"
```

Success writes one JSON object to stdout:

```json
{"version":"1","operation":"jev.run","data":{}}
```

`data` includes the Jev answer fields and `session_id`. `data.readback`
echoes the repo root, `check.toml`, and `--llm-system-prompt-file` path that
the command consumed. Failure exits 2 and writes `{"error","message"}` to
stderr. `error` is a `JevError` code.
