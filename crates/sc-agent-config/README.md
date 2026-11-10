# sc-agent-config

The global sc-agent configuration, loaded with sc-config from `~/.sc-agent/`
(`default.json`, optional `local.json`, then `SC_AGENT__*` environment
overrides such as `SC_AGENT__LOCAL_RUNNERS__LAYA__PORT=9000`).

```json
{ "workers": 8, "max_sends": 5,
  "models": { "claude-haiku-5-6": { "line": "haiku", "released": "2026-12-01" } },
  "local_runners": { "laya": { "kind": "laya", "host": "127.0.0.1", "port": 8000,
                               "model": "convaiinnovations/laya" } } }
```

A runner's optional `key_env` names the variable holding its key (such as
Laya's own key variable); sc-agent-secrets resolves it, and only on a loopback
host. Credential variable names live only in sc-agent-secrets (REQ-SAG-0401).

| Key | Default | Bound |
|---|---|---|
| `models` | none | ids must not sit on two lines |
| `log_root` | `~/.sc-agent` | |
| `workers` | 8 | 1..=64 |
| `max_sends` | 5 | 1..=10 |
| `local_runners.<name>` | none | name not a short name; `kind` strands/laya/mapika/generic; `host` hostname or IP; `port` 1..=65535; `key_env` only on a loopback host; `request_timeout_secs` 10..=3600 (300); `load_timeout_secs` 10..=7200 (600) |

A value that fails its bound is `config.invalid` with the key path. The crate
logs nothing and depends only on sc-agent-types, sc-config, serde, serde_json
and serde_path_to_error (no tokio).
