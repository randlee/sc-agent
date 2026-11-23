# sc-decidr Python wheel

Install `sc-decidr` and import `sc_agent`. The wheel contains the native decision API, `sc_agent.sitr` syntax queries and `sc_agent.logger(name)`. Pass that same Logger to the consumer and every decision/query call. Calls return `{ok: true, value: ...}` or `{ok: false, error: ...}`. Cancellation keeps finished results and returns cancelled values for unfinished requests.

## Public API

Every operation returns a result envelope and does not raise for an operation failure. A successful call has `{"ok": true, "value": ...}`. A failed call has `{"ok": false, "error": {"id": ..., "message": ..., "action": ..., "category": ...}}`; decision results may also include attempt, session, and question-digest metadata. Success values depend on the operation.

### Decision requests

- `send(request, *, model=None, provider=None, project=None, agent_role=None, config_dir=None, general_log=None, timeout_secs=None, cancel=None, base_url=None, sleep=None, keyring_lookup=None)` sends one JSON request. `request` may be a JSON string or object. `model` accepts a provider short name or model id. If `provider` is omitted, the first provider table containing the selector is used; reseller ids can pass through unchanged. `project` and `agent_role` tag the session record. `config_dir` selects configuration files. `timeout_secs` sets the per-call timeout; `cancel` is a `Cancel` shared with the caller. `base_url`, `sleep`, and `keyring_lookup` inject the endpoint and retry/keyring behavior when needed.
- `send_batch(requests, *, model=None, provider=None, project=None, agent_role=None, config_dir=None, general_log=None, timeout_secs=None, workers=None, on_result=None, cancel=None, base_url=None, sleep=None, keyring_lookup=None)` sends a sequence of requests and returns one indexed result for each input. `workers` sets the fan-out, `on_result(index, result)` receives each completed result, and the other options match `send`.
- `digest(request, question, *, general_log=None)` returns the digest for one question in a request.
- `Cancel()` creates a reusable cancellation flag. Call `cancel.cancel()` to stop new requests; completed results remain available and unfinished batch entries return a `decision.cancelled` error.

### Logging

- `logger(name)` creates a Logger handle for the named general log.
- `log(logger, event)` writes an event through that handle.
- `flush(logger)` flushes queued records before the caller exits.

Pass the returned handle as `general_log` to `send`, `send_batch`, `digest`, and sitr operations. Check each result envelope, including errors returned by `log` or `flush`.

### Syntax queries

`sc_agent.sitr` shares the wheel's native Logger and exposes:

- `languages(*, general_log=None)` for supported languages.
- `queries(language, *, general_log=None)` for named queries.
- `query(language, source, query, *, general_log=None)` for a caller-supplied tree-sitter query.
- `run(language, name, source, *, general_log=None)` for a named built-in query.

`__version__` is the installed wheel version. The public type stub is shipped at `sc_agent/__init__.pyi` and the native-extension stub at `sc_agent/_native.pyi`.

The shared client template is `sc_agent/templates/consumer.py.j2`. Render it from the installed wheel with sc-compose:

```bash
template_root="$(python3 -c 'import importlib.resources; print(importlib.resources.files("sc_agent") / "templates")')"
sc-compose render --root "$template_root" --file consumer.py.j2 --strict \
  --var client_name='"my-tool"' --var sitr=true \
  --var sc_decidr_version="\"$(python3 -c 'import importlib.metadata; print(importlib.metadata.version("sc-decidr"))')\"" \
  --output decidr_client.py
```

`client_name` sets the logger and session role and the default consent-settings module prefix. `sitr` includes the optional `extract` helper. The named `consent` block is the extension point for a consumer-specific remote-send policy; decision transport, returned errors and batch processing stay shared. Model selectors pass unchanged to the wheel. The generated module imports without loading the optional wheel until a call needs it.

The wheel ships the public package stub at `sc_agent/__init__.pyi`, the native-extension stub at `sc_agent/_native.pyi`, and `py.typed`. Because `sc_agent` is a package, its public package stub is `__init__.pyi`; a sibling `sc_agent.pyi` would describe a module instead of the package. CI renders both current consumer configurations and checks them with strict Pyright before their own suites verify adoption.
