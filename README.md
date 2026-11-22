# sc-agent

sc-agent is one launch interface for the AI backends an agent workflow calls:

- **AI SDK providers**: services in the [`Provider` registry](crates/sc-agent-types/src/provider.rs), called through their APIs.
- **System-one decision models**: TypeSafe's Jev model and local runners that serve the same Jev API.
- **Agents in a harness**: CLI agents such as Claude Code and Codex, and SDK agents (planned; see Status).

Every backend is launched the same way: one request form, one set of keys and configuration, one log layout, and one error shape. A caller can switch between a decision model, an LLM and, later, an agent without changing how it asks or what it gets back.

## Status

| Part | State |
|---|---|
| Decision models: Jev, local Jev-compatible runners, and supported LLMs answering in Jev's place | Done |
| `decidr` CLI and the `sc_agent` Python module | Done |
| `sitr` tree-sitter query tool and the `sc_treesitter` Python module | Done |
| CLI agents (Claude Code, Codex) and SDK agents | Planned: they get their own crates on the same lower layers |

## The `decidr` CLI

```bash
decidr send --model jev --project my-project --agent-role judge request.json
```

```bash
decidr send --batch --model haiku --project my-project --agent-role judge requests.jsonl
```

```bash
decidr models
```

```bash
decidr doctor
```

- `send`: one JSON request, or a JSONL batch with `--batch`.
  - `--model` takes a [short name](crates/sc-agent-types/src/short_name.rs), a model id, or the name of a configured local runner.
  - `--provider` overrides the provider. Without it, the provider is chosen by ranking the keys that are found.
  - `--image NAME=FILE` and `--long-context` apply to a single request.
  - One result line is printed per request.
- `models`: lists the models of TypeSafe or of a configured local runner.
- `doctor`: lists the keys found, in the order they are used, and reports catalog mismatches.
- `--config-dir PATH` (global): the configuration directory. The default is `~/.sc-agent`.

Ctrl-C or SIGTERM cancels the run. Every unfinished request ends as `decision.cancelled`, the session log is flushed, and `decidr` exits 130. A third interrupt exits at once.

## Python

```python
import sc_agent

line = sc_agent.send(request_json, model="jev", project="my-project", agent_role="judge")
lines = sc_agent.send_batch(requests, model="haiku", project="my-project", agent_role="judge")
sc_agent.sitr.languages()
```

The wheel exposes decision calls, digests, cancellation and sitr queries. Calls return tagged success or error values. `general_log=` accepts an opaque Logger handle obtained with `sc_agent.logger(name)`; the generated client creates one named Logger and injects it into its decision and sitr calls. `cancel=` accepts `sc_agent.Cancel()`. A keyboard interrupt cancels unfinished work and returns a cancellation error.

## Configuration, keys and logs

- **Configuration:** first value wins: CLI flag, environment variable, local `decidr.json`, local `sc-agent.json`, global `decidr.json`, global `sc-agent.json`, built-in default. Local files live in `<worktree>/.sc/sc-agent/`; global files live in `~/.sc-agent/`. `SC_PROJECT` sets project/phase; other variables use names such as `DECIDR_WORKERS` and `DECIDR_LAYA__PORT`. `decidr doctor` reports values, sources and layer-file paths/existence without sending a request.
- **Keys:** read from the environment, then the repository-root `.env`, then the OS keyring; the first source that has the key wins. An attached key is redacted from all error text.
- **Logs:** the fixed root is `~/.sc-agent/`, with a `<NAME>_LOG_ROOT` environment override for tests; it is not a configuration setting. General logs are `logs/<name>.log.jsonl`, rotated by sc-observability. Session rows append to `sessions/<repo>/<agent_role>/<project>/sessions.jsonl` and are never rotated or pruned.

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
| Top | `decidr` (library, composition root and CLI) and `sc-agent-py` (the PyO3 `sc_agent` module) |
| Stands alone | `sc-treesitter` (`sitr` and `sc_treesitter`) |
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
