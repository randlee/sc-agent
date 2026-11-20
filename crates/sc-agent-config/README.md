# sc-agent-config

Shared configuration loading and doctor rows for `decidr` and `sitr`. The crate
reads JSON files, validates settings and records their sources. It writes no
files, logs nothing, resolves no credentials and needs no async runtime.

`load_layered(&LoadOptions)` resolves each key from these layers, highest first:

1. Explicit flags (dotted setting paths).
2. The selected CLI's environment prefix, plus `SC_PROJECT`.
3. `<worktree>/.sc/sc-agent/<cli>.json`.
4. `<worktree>/.sc/sc-agent/sc-agent.json`.
5. `<home>/.sc-agent/<cli>.json`.
6. `<home>/.sc-agent/sc-agent.json`.
7. Built-in values, or a typed missing-setting error when required by the caller.

`CliName` selects `decidr.json` / `DECIDR_` or `sitr.json` / `SITR_`.
With `worktree: None`, local files are neither inspected nor read. Objects merge
per key; scalar and array values replace the lower value. Supplied values are
validated even when a higher layer overrides them. Non-regular files and files
larger than `MAX_CONFIG_FILE_BYTES` are refused before reading.

Both scopes accept `project` and `log_root`. Only decidr's CLI file accepts
`models`, `workers`, `max_sends` and `local_runners`; shared files cannot contain
those settings. The loader ignores other CLIs' files and environment prefixes.

Nested environment names join upper-case key parts with `__`, for example
`DECIDR_LOCAL_RUNNERS__LAYA__PORT=9000`. Project/phase uses only `SC_PROJECT`.
Credential variable names are owned by `sc-agent-secrets`; this loader does
not read their values.

`LoadOptions` supplies `home`, optional `worktree`, `flags`, `environment`, and
`default_log_root`. The caller supplies the latter from the shared default-log-root
function; the loader does not choose a separate default path. No process
environment or home-directory discovery occurs in the layered loader.

The returned `Loaded` contains an `AgentConfig`, the winning `Source` of each
setting and the inspected `LayerFile` paths with their `exists` values.
`doctor_rows(&loaded)` builds deterministic, serializable `setting`, `value`,
`source` rows, with shared settings first. Missing optional values display as
`unset`. The two CLIs use the same routine for their shared rows.

`AgentConfig::project()` returns the resolved project/phase. A session caller
uses `require_project()` before sending or writing; absence yields
`config.project_required`, category `config`, naming every setting place.
Project names use the same `sc_agent_types::is_valid_slug` rule as the logger:
lowercase slugs using letters, digits, `.`, `_`, `-` that are safe directory
names on every supported platform, bounded by `MAX_SEGMENT_BYTES`.
Layer errors preserve the validation error and add its file, variable or flag.

Existing model, worker, send-count, runner, host, port and timeout checks are
shared with the legacy loader. Workers are bounded by `MAX_WORKERS`, sends by
`MAX_MAX_SENDS`, and model/runner maps by `MAX_MODELS` / `MAX_LOCAL_RUNNERS`.
Runner request and load timeouts use `RUNNER_REQUEST_TIMEOUT_SECS` and
`RUNNER_LOAD_TIMEOUT_SECS`; a runner key is accepted only on a loopback host.

The legacy `load`, `default_dir` and `APP_NAME` remain available for callers
that have not yet adopted the layered API. Their behavior is unchanged; the
integration sprint removes them when it migrates those callers.
