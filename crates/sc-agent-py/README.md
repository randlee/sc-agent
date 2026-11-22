# sc-decidr Python wheel

Install `sc-decidr` and import `sc_agent`. The wheel contains the native decision API, `sc_agent.sitr` syntax queries and `sc_agent.logger(name)`. Pass that same Logger to the consumer and every decision/query call. Calls return `{ok: true, value: ...}` or `{ok: false, error: ...}`. Cancellation keeps finished results and returns cancelled values for unfinished requests.

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
