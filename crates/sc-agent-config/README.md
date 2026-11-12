# sc-agent-config

The global sc-agent configuration, loaded with sc-config from `~/.sc-agent/`. It sits in the config, secrets and logging layer, above `sc-agent-types`. It reads files and applies overrides. It logs nothing and resolves no keys.

`load` reads `<config_dir>/default.json`, merges an optional `local.json`, then applies `SC_AGENT__*` environment overrides such as `SC_AGENT__MAX_SENDS=2` or `SC_AGENT__LOCAL_RUNNERS__LAYA__PORT=9000`. Every value is checked at load.

## What it provides

- `load(config_dir, environment)` and `default_dir()`.
- `AgentConfig`: `models`, `log_root`, `workers`, `max_sends` and `local_runners`. Its methods are `log_root()` (default `~/.sc-agent`) and `catalog()` (the built-in catalog plus the configured models).
- `ExtraModel`, `LocalRunner`, `RunnerKind` (`strands`, `laya`, `mapika`, `generic`), `RunnerName` and `RunnerHost`.
- Constants for the defaults and bounds, such as `DEFAULT_WORKERS`, `DEFAULT_MAX_SENDS`, `MAX_WORKERS`, `MAX_MAX_SENDS`, `RUNNER_REQUEST_TIMEOUT_SECS` and `RUNNER_LOAD_TIMEOUT_SECS`.
- `AgentConfigError`. A bad value is `AgentConfigError::Invalid` with its key path, such as `local_runners.laya.host`.

## Usage

```json
{ "workers": 8, "max_sends": 5,
  "models": { "claude-haiku-5-6": { "line": "haiku", "released": "2026-12-01" } },
  "local_runners": { "laya": { "kind": "laya", "host": "127.0.0.1", "port": 8000,
                               "model": "convaiinnovations/laya" } } }
```

```rust,ignore
let dir = sc_agent_config::default_dir()?;
let environment: Vec<_> = std::env::vars_os().collect();
let config = sc_agent_config::load(dir, environment)?;

let workers = config.workers;
let log_root = config.log_root()?;
let catalog = config.catalog()?;
```

| Key | Default | Bound |
|---|---|---|
| `models` | none | at most `MAX_MODELS`; an id must not sit on two lines |
| `log_root` | `~/.sc-agent` | |
| `workers` | 8 | 1..=64 |
| `max_sends` | 5 | 1..=10 |
| `local_runners.<name>` | none | name is not a short name; `port` is not 0; `key_env` only on a loopback host; `request_timeout_secs` 10..=3600 (default 300); `load_timeout_secs` 10..=7200 (default 600) |

A runner's `key_env` names the variable that holds its key. `sc_agent_secrets::resolve_var` resolves it. Credential variable names live only in `sc-agent-secrets`.

## Depends on

- `sc-agent-types`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). The crate has no runtime.

## Requirements and decisions

- REQ-SAG-0013: config bounds.
- REQ-SAG-0801: local decision runners come from configuration.
- REQ-SAG-0015: local runner timeouts.
- REQ-SAG-0102: model catalog, built-in models plus global configuration.
- REQ-SAG-0003 and REQ-SAG-0005: the log root that sessions and logs are written under.
- ADR-SAG-0004: crate layering.
