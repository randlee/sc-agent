# sc-agent

sc-agent is one launch interface for the AI backends an agent workflow calls:

- **AI SDK providers**: Anthropic, OpenAI, OpenRouter, Nous and Together, called through their APIs.
- **System-one decision models**: TypeSafe's Jev model and local runners that serve the same Jev API.
- **Agents in a harness**: CLI agents such as Claude Code and Codex, and SDK agents (planned; see Status).

Every backend is launched the same way: one request form, one set of keys and configuration, one log layout, and one error shape. A caller can switch between a decision model, an LLM and, later, an agent without changing how it asks or what it gets back.

## Status

| Part | State |
|---|---|
| Decision models: Jev, local Jev-compatible runners, and Haiku and Luna answering in Jev's place | Done (phase A) |
| `decide` CLI and the `sc_agent` Python module | Done (phase A) |
| `tsq` tree-sitter query tool and the `sc_treesitter` Python module | Done (phase A) |
| CLI agents (Claude Code, Codex) and SDK agents | Planned: they get their own crates on the same lower layers |

## The `decide` CLI

```bash
decide send --model jev --project my-project --agent-role judge request.json
```

```bash
decide send --batch --model haiku --project my-project --agent-role judge requests.jsonl
```

```bash
decide models
```

```bash
decide doctor
```

- `send`: one JSON request, or a JSONL batch with `--batch`.
  - `--model` takes a short name (`jev`, `haiku`, `luna`, ...), a model id, or the name of a configured local runner.
  - `--provider` overrides the provider. Without it, the provider is chosen by ranking the keys that are found.
  - `--image NAME=FILE` and `--long-context` apply to a single request.
  - One result line is printed per request.
- `models`: lists the models of TypeSafe or of a configured local runner.
- `doctor`: lists the keys found, in the order they are used, and reports catalog mismatches.
- `--config-dir PATH` (global): the configuration directory. The default is `~/.sc-agent`.

Ctrl-C or SIGTERM cancels the run. Every unfinished request ends as `decision.cancelled`, the session log is flushed, and `decide` exits 130. A third interrupt exits at once.

## Python

```python
import sc_agent

line = sc_agent.send(request_json, model="jev", project="my-project", agent_role="judge")
lines = sc_agent.send_batch(requests, model="haiku", project="my-project", agent_role="judge")
sc_agent.models()
sc_agent.doctor()
```

Every `decide` operation has a matching Python function. They also accept `images=`, `long_context=`, `cancel=`, `config_dir=`, and the `general_log=` and `session_log=` callables for injecting loggers. `KeyboardInterrupt` cancels the run and is re-raised.

## Configuration, keys and logs

- **Configuration:** read from `~/.sc-agent/default.json`, then `local.json`, then `SC_AGENT__*` overrides. All values are checked when the configuration loads.
- **Keys:** read from the environment, from the repository-root `.env`, or from the OS keyring. An attached key is redacted from all error text.
- **Logs:** everything goes under `<log_root>/logs/` (default `~/.sc-agent/logs/`):
  - each CLI writes its own general log, rotated by sc-observability;
  - session logs go to `<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl`, with one row per request; they are never rotated or pruned.

## Workspace

The crates are layered: each one depends only on the layers below it (ADR-SAG-0004).

| Layer | Crates |
|---|---|
| Types | `sc-agent-types` |
| Config, secrets and logging | `sc-agent-config`, `sc-agent-secrets`, `sc-agent-log` |
| HTTP | `sc-agent-http` (sync ureq, busy resend, provider-wide pause) |
| Decision schema | `sc-decision-schema` (one sync `Decision` trait, one `DecisionRequest`) |
| Decision backends | `sc-decision-systemone`, `sc-decision-local`, `sc-llm-provider` and `sc-decision-llm` |
| Runner | `sc-decision-runner` (worker threads, holdoff, cancellation, session rows) |
| Top | `decide` (library, composition root and CLI) and `sc-agent-py` (the PyO3 `sc_agent` module) |
| Stands alone | `sc-treesitter` (`tsq` and `sc_treesitter`) |
| Test support only | `sc-agent-testkit` |

There is no async runtime: the workspace is synchronous and uses no tokio.

## Build and test

```bash
cargo test --workspace --all-features
```

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The Python modules build with maturin from `crates/sc-agent-py` and `crates/sc-treesitter-py`.

## Docs

- `docs/requirements/` and `docs/nfr/`: requirements (REQ-SAG-*, NFR-SAG-*).
- `docs/adr/`: architecture decisions (ADR-SAG-*).
- `docs/plans/`: phase plans and sprint docs.

## License

MIT. See [LICENSE](LICENSE).
