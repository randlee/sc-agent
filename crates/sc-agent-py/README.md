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
keyring_account=None)` returns one JSON string per input, in input order.
An empty batch returns `[]`. `workers` is a per-call integer override in 1..=64;
omitted, it uses the current default of 8. `send` does not accept `workers`.

`timeout_secs` is finite numeric seconds in 10..=3600, including fractions.
It overrides the HTTP send deadline for this call only. Omitted, it keeps the
current default of 120 seconds. Invalid limits return errors with ids
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

The generated consumer module is owned by the companion consumer-template sprint.
Its wheel package resource is `sc_agent/templates/consumer.py.j2`; once shipped,
render it with a typed variables file:

```bash
sc-compose render --root <installed-sc_agent>/templates --file consumer.py.j2 \
  --var-file consumer-vars.json --strict --output consumer.py
```

The `sc_agent.sitr` submodule exposes `languages`, `queries`, `query_source`,
`run_query`, `Capture`, and `TreesitterError`. `run_query(language, query, source,
logger, captures=None)` uses the same Logger. The decision options above do not
apply to sitr.
