# sc-agent-py

The `sc_agent` Python module: one function per `decidr` operation. It is a thin PyO3 layer over the `decidr` library (ADR-SAG-0004) and adds no wiring of its own. It is the top of the stack with `decidr`, and it has `publish = false`. Python users build it with maturin.

Each function returns the same JSON text that the CLI prints.

## What it provides

- `sc_agent.send(request, *, model, project, agent_role, ...)`: `decidr send`. Optional keywords are `provider`, `images` (a dict of part name to PNG or JPEG bytes), `long_context`, `cancel`, `general_log`, `session_log`, `config_dir`, `keyring_service`, `keyring_account` and `timeout_secs`.
- `sc_agent.send_batch(requests, *, model, project, agent_role, ...)`: `decidr send --batch`. Each item is a JSON string, or a tuple `(request, images, long_context)`. It returns result lines in input order and passes each to `on_result` as it finishes. Optional `workers` selects 1–64 workers for that batch.
- `sc_agent.models(runner=None, ...)`: `decidr models`.
- `sc_agent.doctor(...)`: `decidr doctor`.
- `sc_agent.CancelToken`: `cancel()` and `is_cancelled()`.
- `sc_agent.SagError`: every error raises it. `args` is `(id, message, detail)`, where `detail` is the JSON error chain.

A failed decision is data in the returned line, not an exception. `KeyboardInterrupt` cancels the run, drains it and is re-raised. `general_log` and `session_log` accept a caller-owned `sc_observability.Logger` directly, or the existing `Callable[[dict], None]`. Python and Rust records reach the same injected Logger through its public `log(LogEvent)` API; the caller retains flush and shutdown. Each omitted logger uses its default file. No consumer adapter or record conversion is required.

Keyring overrides apply only to the provider selected by the model. An ambiguous ranked model with an override raises `decidr.ambiguous_keyring_override`; no provider argument is required. Each supplied service/account must be 1–255 UTF-8 bytes. Omitted components independently use the provider credential-variable name and the current OS account. Lookup order is repository-root `.env`, process environment, then keyring; a present invalid higher-priority key never falls back. Invalid options raise `SagError("sc_agent_py.invalid_option", message, detail_json)` before network I/O.

On macOS, Keychain may ask for access for each new Python interpreter binary.
For a trusted interpreter, choose **Always Allow** to permit future reads by that
interpreter; a different interpreter may prompt again. If access is not resolved
within 15 seconds, the call raises `SagError` with id `decidr.secrets`, category
`auth`, and cause id `secrets.keyring_timeout`; no provider request was sent.
Keychain read errors likewise have cause id `secrets.keyring_read`. These are
credential lookup failures, not provider authentication responses. Consumers
should inspect the cause id before interpreting `auth` as a provider rejection:

```python
import json
import sc_agent

try:
    sc_agent.send(request_json, model="jev", project="my-project", agent_role="judge")
except sc_agent.SagError as error:
    if error.id == "decidr.secrets":
        cause = json.loads(error.detail).get("cause") or {}
        if cause.get("id") in {"secrets.keyring_timeout", "secrets.keyring_read"}:
            print("Could not read the credential from the keychain")
            raise
    raise
```

`send` and `send_batch` accept optional `timeout_secs`: finite numeric seconds in
10..=3600, including fractional seconds. It bounds each decision request’s connect,
headers and body read together, not the whole batch or retry budget. An omitted
value keeps hosted requests at 120 seconds and local requests at their configured
timeout (300 seconds by default). An explicit value overrides the request timeout;
local model-load and probe timeouts stay configured separately. Invalid values
raise `SagError("decidr.invalid_request_timeout", ...)` before config, loggers or
network I/O.

The direct Logger form uses the separately installed `sc-observability` package (tested with 1.5.0). The callable and default-file forms do not require it.

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

None. The native module is `sc_agent._native`; the Python package re-exports the public API and ships its stub and templates. The crate builds as a `cdylib` named `sc_agent` with PyO3's `abi3-py311` and `extension-module`.

## Depends on

- `decidr`, plus `sc-agent-types`, `sc-agent-config`, `sc-agent-log` and `sc-decision-schema`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). This crate calls the `decidr` library and never chooses a backend itself.

## Requirements and decisions

- REQ-SAG-0501: TypeSafe API as decidr subcommands.
- REQ-SAG-0008: optional logger injection with default loggers.
- REQ-SAG-0011: interrupt cancels cleanly.
- REQ-SAG-0104: cancel token.
- REQ-SAG-0007: return values are not rounded.
- REQ-SAG-0014: input size limits.
- REQ-SAG-0004: session log identity.
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top.
- ADR-SAG-0004: crate layering and per-crate error unions.

## Generated consumer

The wheel ships `sc_agent/templates/consumer.py.j2`, derived from the working
plan-anchor client at `fff69b2`. Render it with sc-compose >=1.8.0 and Python >=3.11:

```bash
VARS=$(mktemp)
trap 'rm -f "$VARS"' EXIT
python - "$VARS" <<'PY'
import json
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path
import sys

try:
    sitr_version = version("sitr")
except PackageNotFoundError:
    sitr_version = ""

Path(sys.argv[1]).write_text(json.dumps({
    "general_log": False,
    "models": ["jev"],
    "sitr": False,
    "sc_decidr_version": version("sc-decidr"),
    "sitr_version": sitr_version,
}, indent=2) + "\n")
PY
TEMPLATE=$(python -c 'import importlib.resources, sc_agent; print(importlib.resources.files(sc_agent) / "templates/consumer.py.j2")')
sc-compose render --root "$(dirname "$TEMPLATE")" --file "$TEMPLATE" \
  --var-file "$VARS" \
  --strict --output consumer.py
```

The JSON vars file preserves booleans and the model list as typed values;
`--var` values are strings and do not represent these options correctly.

`models` records the selectors the consumer offers (see the [short-name registry](../sc-agent-types/src/short_name.rs)); each call passes its chosen
`model` unchanged. `general_log` includes or omits forwarding the caller's general
logger. Session logging always uses the wheel default; the generated module does
not accept or forward `session_log` (REQ-SAG-0018). Keyring service and account are
runtime arguments, passed on every decision call without forcing a provider.

```python
import sc_agent
import consumer

cancel = sc_agent.CancelToken()
line, code = consumer.call(
    request_json, model="jev", project="my-project", agent_role="judge",
    abort_on=frozenset({"auth", "quota"}), category_codes={"auth": "AUTH", "quota": "QUOTA"},
    cancel=cancel, keyring_service=None, keyring_account=None, timeout_secs=120,
)
```

`abort_on` is required: empty, `{"auth"}`, or `{"auth", "quota"}`. Both single
and batch calls map stored categories through the caller's map, cancel on selected
errors and on `KeyboardInterrupt`, and preserve the wheel's result lines. Batch
callbacks run in finish order; returned tuples retain input order. Prompt authoring
and result interpretation remain in the consumer; there is no additional retry loop.

For owner-attended prerelease checks and per-platform reports, see
[the integration instructions](../../docs/integration/prerelease.md).

Set `sitr=true` to include `extract(language, query, source, captures=None)`.
It imports the separate, version-matched sitr wheel only when called and delegates
to `sc_treesitter.run_query`. Extraction stays outside the decision path: use its
captures when authoring a request, then call the same decision function. The
header records both wheel versions when sitr is enabled. Mixed versions are
unsupported and are not checked at import time.
