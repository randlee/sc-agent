# sc-agent-py

The `sc_agent` Python module: one function per `decide` operation. It is a thin PyO3 layer over the `decide` library (ADR-SAG-0004) and adds no wiring of its own. It is the top of the stack with `decide`, and it has `publish = false`. Python users build it with maturin.

Each function returns the same JSON text that the CLI prints.

## What it provides

- `sc_agent.send(request, *, model, project, agent_role, ...)`: `decide send`. Optional keywords are `provider`, `images` (a dict of part name to PNG or JPEG bytes), `long_context`, `cancel`, `general_log`, `session_log` and `config_dir`.
- `sc_agent.send_batch(requests, *, model, project, agent_role, ...)`: `decide send --batch`. Each item is a JSON string, or a tuple `(request, images, long_context)`. It returns the result lines in finish order and passes each to `on_result` as it arrives.
- `sc_agent.models(runner=None, ...)`: `decide models`.
- `sc_agent.doctor(...)`: `decide doctor`.
- `sc_agent.CancelToken`: `cancel()` and `is_cancelled()`.
- `sc_agent.SagError`: every error raises it. `args` is `(id, message, detail)`, where `detail` is the JSON error chain.

A failed decision is data in the returned line, not an exception. `KeyboardInterrupt` cancels the run, drains it and is re-raised. `general_log` and `session_log` take a `Callable[[dict], None]`, which injects the logger.

## Usage

```bash
cd crates/sc-agent-py && maturin develop --release
```

```python
import json
import sc_agent

line = sc_agent.send(request_json, model="jev", project="my-project", agent_role="judge")
print(json.loads(line))

token = sc_agent.CancelToken()
lines = sc_agent.send_batch(
    requests, model="haiku", project="my-project", agent_role="judge",
    cancel=token, on_result=print,
)

models = sc_agent.models()
report = sc_agent.doctor()

try:
    sc_agent.send(request_json, model="jev", project="Bad Slug", agent_role="judge")
except sc_agent.SagError as error:
    error_id, message, detail = error.args
```

## Features

None. The crate builds as a `cdylib` named `sc_agent` with PyO3's `abi3-py310` and `extension-module`.

## Depends on

- `decide`, plus `sc-agent-types`, `sc-agent-config`, `sc-agent-log` and `sc-decision-schema`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). This crate calls the `decide` library and never chooses a backend itself.

## Requirements and decisions

- REQ-SAG-0501: TypeSafe API as decide subcommands.
- REQ-SAG-0008: optional logger injection with default loggers.
- REQ-SAG-0011: interrupt cancels cleanly.
- REQ-SAG-0104: cancel token.
- REQ-SAG-0007: return values are not rounded.
- REQ-SAG-0014: input size limits.
- REQ-SAG-0004: session log identity.
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top.
- ADR-SAG-0004: crate layering and per-crate error unions.
