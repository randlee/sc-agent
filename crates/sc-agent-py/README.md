# sc-decidr Python wheel

`import sc_agent` exposes a thin PyO3 layer over decidr and the `sc_agent.sitr`
submodule. Python 3.11 or newer is required. Build with `maturin build --locked
--manifest-path crates/sc-agent-py/Cargo.toml` and install the resulting wheel.

Obtain one native Logger with `logger(name)`. It returns `(Logger, None)` or
`(None, error_json)`. The log root is `<NAME>_LOG_ROOT` when set, otherwise
`~/.sc-agent`; the general log is `logs/<name>.log.jsonl`. Inject the same Logger
into every decision and sitr query call.

```python
import json
import sc_agent

log, error = sc_agent.logger("my_client")
if error is None:
    token = sc_agent.CancelToken()
    line = sc_agent.send(request_json, "haiku", "my-project", "judge", log,
                         cancel=token, timeout_secs=120)
    result = json.loads(line)
    if not result["ok"]:
        kind = sc_agent.category(result["error"])
        can_retry_later = sc_agent.retryable(result["error"])
```

`send(request, model, project, agent_role, logger, provider=None, cancel=None, *,
timeout_secs=None, keyring_service=None, keyring_account=None)` returns JSON text
containing `{"ok": true, "value": response}` or
`{"ok": false, "error": {"id": ..., "message": ..., "category": ...}}`.
A decision failure is returned as a value; it is not raised.

`send_batch(requests, model, project, agent_role, logger, provider=None,
workers=None, cancel=None, *, timeout_secs=None, keyring_service=None,
keyring_account=None, on_result=None)` returns one JSON string per input, in input order.
An empty batch returns `[]`. `on_result(line)` receives each tagged JSON reply as it finishes; a callback exception becomes that request's non-retryable error value in the returned list. `workers` is a per-call integer override in 1..=64;
omitted, it uses the configured value (default 8). `send` does not accept `workers`.

`timeout_secs` is finite numeric seconds in 10..=3600, including fractions.
It overrides the HTTP send deadline for this call only. Omitted, it keeps the
configured value (default 120 seconds). Invalid limits return errors with ids
`decidr.invalid_request_timeout` or `decidr.invalid_workers`; no request is sent.

The optional keyring service and account override only this call. Missing service
uses the selected provider's credential-variable name; missing account uses the
OS user. Key lookup remains process environment, repository-root `.env`, then OS
keyring. Supplying a keyring override does not bypass either earlier source.

`category(error)` reads the stored category of an error dictionary or JSON string,
falling back to `"other"` for missing or invalid data. `retryable(error)` is true
only for `"busy"` and `"network"`: resubmission later may help after the wheel's
own resends. Neither helper raises on invalid input.

`CancelToken()` creates a token; `cancel()` cancels outstanding work and
`cancelled()` reads its state. Pass it with `cancel=` to either decision call.
`Logger` is the native handle returned by `logger`, with no Python constructor.
The exact decision API is declared in `python/sc_agent/__init__.pyi`.

## Generated consumer

Render the shipped template with typed variables. For example:

```bash
python - <<'PYCODE'
import importlib.resources, json, pathlib, sc_agent
root = importlib.resources.files(sc_agent) / "templates"
pathlib.Path("consumer-root.txt").write_text(str(root))
pathlib.Path("consumer-vars.json").write_text(json.dumps({
    "client_name": "my-client", "models": ["jev"], "sitr": False,
    "batch_records": False, "sc_decidr_version": "0.2.8", "sitr_version": "0.2.8",
    "settings_module": "my_settings", "settings_file": ".sc/my/settings.yaml",
    "settings_set_command": "my-settings set allow_remote_judge true|false",
}))
PYCODE
sc-compose render --root "$(cat consumer-root.txt)" --file consumer.py.j2 \
  --var-file consumer-vars.json --strict --output consumer.py
```

The generated client opens one native logger named `client_name` and passes it
to decision and sitr calls. It returns decoded tagged results and accepts the
optional `provider`, `timeout_secs`, keyring overrides and batch `on_result`.

The `sc_agent.sitr` submodule exposes `languages`, `queries`, `query_source`,
`run_query`, `Capture`, and `TreesitterError`. `run_query(language, query, source,
logger, captures=None)` uses the same Logger. The decision options above do not
apply to sitr.
