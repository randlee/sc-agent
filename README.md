# sc-agent

sc-agent provides one synchronous Rust core for decision requests, provider selection, credentials, cancellation and session logging, plus syntax queries through tree-sitter.

It ships the `decidr` and `sitr` CLIs and one Python wheel, `sc-decidr` (`import sc_agent`, including `sc_agent.sitr`).

## Install

For Python 3.11 or newer, install the wheel containing both the decision and syntax-query APIs:

```bash
pip install sc-decidr
```

Import them with `import sc_agent` and `import sc_agent.sitr`. Building from the source distribution requires Rust and Git access to the sc-config dependency; see the [source-build dependency note](docs/integration/prerelease.md#sc-config-dependency).

Install the `decidr` and `sitr` binaries from a matching platform archive on [GitHub Releases](https://github.com/randlee/sc-agent/releases), or use a package manager:

| Channel | Install command |
|---|---|
| Homebrew | `brew install randlee/tap/decidr` |
| Scoop | `scoop bucket add randlee https://github.com/randlee/scoop-bucket`, then `scoop install randlee/decidr` |
| WinGet | `winget install --id randlee.decidr --exact` |

Each binary package includes both CLIs. Channel names are declared in [the release manifest](release/publish-artifacts.toml).

### Migrating from the separate sitr wheel

PyPI `sitr` 0.2.9 was the last separate sitr wheel. Replace it with the combined wheel:

```bash
pip uninstall sitr
pip install sc-decidr
```

Use `import sc_agent.sitr` for Python queries. Install the `sitr` CLI from the binary release archives or package managers above.

## The CLIs

```bash
decidr send --model jev --project my-project --agent-role judge --input request.json
decidr send --batch --model haiku --project my-project --agent-role judge --input requests.jsonl
decidr models
decidr doctor
sitr languages
sitr queries --language rust
sitr query rust match-arms input.rs
```

- `decidr send` reads JSON from `--input` or stdin; `--batch` reads JSONL and returns one JSON array of indexed results. Single sends return one tagged JSON result.
- `--model` accepts a short name or model id. `--provider` overrides the provider; otherwise the first provider table supporting the selector is used. A local `laya` runner uses its configured host, port and model.
- `--workers` controls batch concurrency; `--timeout-secs` sets the request timeout. Images and long-context CLI flags are not exposed.
- `models` reports the built-in provider tables. `doctor` reports configuration values, sources, layer-file paths/existence and the fixed log root without sending or writing files.
- `--config-dir PATH` selects the global configuration directory; the default is `~/.sc-agent/`.

Ctrl-C or SIGTERM cancels unfinished work. Cancellation returns a typed error and exit status 1; success exits 0. There is no third-interrupt escape.

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

| Crate | Purpose |
|---|---|
| `sc-agent` | Shared decision, provider, configuration, credential and logging logic |
| `decidr` | Decision CLI |
| `sc-treesitter` | Grammar and query library |
| `sitr` | Syntax-query CLI |
| `sc-agent-py` | One wheel exposing `sc_agent` and `sc_agent.sitr` |

There is no async runtime: the workspace is synchronous and uses no tokio.

## Build and test

```bash
cargo test --workspace --all-features
```

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Build the Python wheel with maturin from `crates/sc-agent-py`.

## Docs

- `docs/requirements/` and `docs/nfr/`: requirements (REQ-SAG-*, NFR-SAG-*).
- `docs/adr/`: architecture decisions (ADR-SAG-*).
- `docs/plans/`: phase plans and sprint docs.

## License

MIT. See [LICENSE](LICENSE).
