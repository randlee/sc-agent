# sc-decidr Python wheel

Install `sc-decidr` and import `sc_agent`. The wheel contains the native decision API, `sc_agent.sitr` syntax queries and `sc_agent.logger(name)`. Pass that same Logger to the consumer and every decision/query call. Calls return `{ok: true, value: ...}` or `{ok: false, error: ...}`. Cancellation keeps finished results and returns cancelled values for unfinished requests.

## Public API

Every operation returns a result envelope and does not raise for an operation failure. A successful call has `{"ok": true, "value": ...}`. A failed call has `{"ok": false, "error": {"id": ..., "message": ..., "action": ..., "category": ...}}`; decision results may also include attempt, session, and question-digest metadata. Success values depend on the operation.

### Decision requests

Create one Run for a command and use it as a context manager:

```python
import sc_agent

with sc_agent.Run(tool="my-tool", model="haiku", project="phase", agent_role="my-tool",
                  timeout_secs=30, workers=2) as run:
    result = run.send({"questions": {"q": {"type": "noul"}}})
    batch = run.send_batch([{"questions": {"q": {"type": "noul"}}}],
                           on_result=lambda index, result: None)
```

`Run` requires `tool` and `agent_role`; `model` defaults to `jev`. Set `project`,
`config_dir`, `general_log`, `timeout_secs`, `workers`, `base_url`, `sleep`, and
`keyring_lookup` when creating the Run. Its `send` and `send_batch` methods take
no per-call overrides. Requests are JSON objects or strings. A creation error
is stored and returned by the Run's methods.

`run.cancel()` stops unfinished requests and keeps completed results.
`run.sends_to(provider)` returns a boolean envelope or the resolution error;
`sc_agent.TYPESAFE` identifies TypeSafe for a consent check. `provider_keys()`
returns an envelope whose `value` is the provider table's credential variable names,
for example `sc_agent.provider_keys()["value"]` on success. `models()` returns the
same table envelope as `decidr models`. Select a provider through
`models.<name>.provider` in configuration.

Context exit closes an owned logger and leaves an injected Logger usable.
`digest(request, question, *, general_log=None)` remains independent. The old
module-level `send`, `send_batch`, `Cancel`, and `provider=` argument are removed.
The consumer testing helpers `sc_agent.testing.FakeRun` and `fake_open_run` are
planned for the following template sprint; they are not exported by this wheel.

### Logging

- `logger(name)` creates a Logger handle for the named general log.
- `log(logger, event)` writes an event through that handle.
- `flush(logger)` flushes queued records before the caller exits.

Pass the returned handle as `general_log` to `Run`, `digest`, and sitr operations. Check each result envelope, including errors returned by `log` or `flush`.

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
