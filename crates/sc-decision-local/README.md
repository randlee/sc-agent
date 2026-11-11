# sc-decision-local

The local System One decision backend of sc-agent. A configured local runner is a Jev-compatible daemon (Strands Decider, Laya, Mapika/decider, or a generic one) at `http://{host}:{port}`. This crate wraps `sc-decision-systemone`, so requests and replies are the same System One types as for TypeSafe Jev, and adds discovery of the daemon.

It sits in the decision-backend layer, above `sc-decision-systemone`. The crate reads no config, has no built-in host or port, starts no runtime, and waits only through `Clock::wait_until`.

## What it provides

- `LocalBackend`, which implements `sc_decision_schema::Backend`. `LocalBackend::new(kind, &host, port, model, key)` takes the fields of a config `local_runners` entry. `with_timeouts(LocalTimeouts)` sets the load and request timeouts. For Laya, the first requests carry the load plus request deadline. `probe_request` and `read_probe` are the sans-IO halves of discovery.
- `LocalProbe`, the I/O driver over the caller's `Transport` and `Clock`:
  - `discover` is one probe. It says whether the daemon is up and whether the model is loaded (`Availability`, `ModelState`).
  - `wait_until_loaded` polls until the model is loaded, so loading never counts toward a decision's request timeout.
- `DaemonKind` (from the config's `RunnerKind`), the `LocalDaemon` trait and one implementation per kind: `Strands`, `Laya`, `Mapika` and `Generic`.
- `LocalTimeouts`, `LOAD_POLL_INTERVAL` and `MAX_LOAD_WAIT`.
- `LocalError` and `ParseCause`. A daemon that is not listening is `LocalError::Http` with cause `http.network`.

## Usage

```rust,ignore
use std::num::NonZeroU16;

use sc_agent_http::{LogContext, Timeouts, UreqTransport};
use sc_agent_types::{CancelToken, Clock};
use sc_decision_local::{DaemonKind, LocalBackend, LocalProbe};

// `host` is a sc_agent_config::RunnerHost.
let backend = LocalBackend::new(
    DaemonKind::Mapika,
    &host,
    NonZeroU16::new(9000).expect("port"),
    "jev-latest".parse()?,
    None,
)?;

let transport = UreqTransport::new(Timeouts::probe());
let probe = LocalProbe::new(&backend, &transport, Clock::system());
let availability = probe.discover(LogContext::NONE)?;
probe.wait_until_loaded(&CancelToken::new(), load_timeout, LogContext::NONE)?; // load_timeout: Duration
```

## Depends on

- `sc-agent-config`, `sc-agent-http`, `sc-agent-log`, `sc-agent-secrets`, `sc-agent-types`, `sc-decision-schema` and `sc-decision-systemone`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004).

## Requirements and decisions

- REQ-SAG-0801: local decision runners come from configuration.
- REQ-SAG-0015: local runner timeouts.
- REQ-SAG-0013: config bounds.
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top.
- ADR-SAG-0001: decision backends implement one trait with one request form.
- ADR-SAG-0004: crate layering (`sc-decision-local` sits over `sc-decision-systemone`).
