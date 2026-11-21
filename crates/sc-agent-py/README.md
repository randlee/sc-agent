# sc-decidr

`send` and `send_batch` return JSON replies in input order. Errors carry native
`id`, `message`, `category` and the available `status`, `attempts`, `retry_after_ms`.
`category(detail)` and `retryable(detail)` read those fields. `SagError` is the
command exception with read-only detail attributes; it supports pickling.

Pass a caller-owned `sc_observability.Logger` or record callable as `general_log`
and/or `session_log`. Each replaces only its own default. Caller loggers are never
flushed or shut down by the wheel. With nothing injected, the wheel uses decidr's
logs and session tree. Legacy `logger(name)` handles remain accepted positionally.
`CancelToken` cancels unfinished work. Batches accept `workers` and `abort_on`.
`keyring_service` and `keyring_account` are passed only when the consumer supplies
them; an override with multiple ranked providers returns a typed config error.
Tree-sitter functions are available through `sc_agent.sitr` in the same wheel.
The complete signatures are in `sc_agent/sc_agent.pyi`.

The wheel ships `sc_agent/templates/consumer.py.j2`. Render the integration module:

```sh
sc-compose render --root /path/to/site-packages/sc_agent/templates \
  --file consumer.py.j2 --var-file client-vars.json --strict --output client.py
```

Vars are `client_name` (the product's fixed agent role), `models` (selectors passed
unchanged), `general_log` and `sitr` (booleans). Every rendered client obtains one
Logger and injects it into its wheel calls; `sitr` selects the extraction methods.
Its app owns the Logger and logs its own records through that same Logger.
Prompt authoring and result processing remain in the consumer. The generated
header records the wheel version for troubleshooting, without a runtime check.
