# sc-agent-http

The sync HTTP layer of sc-agent: one send per call, no queue, no sleep. It sits above `sc-agent-types`, `sc-agent-secrets` and `sc-agent-log`, and below `sc-decision-schema` and `sc-llm-provider`. It starts no runtime and never waits. The caller's queue holds the holdoff and owns cancellation.

## What it provides

- `HttpRequest` and `HttpResponse`: byte bodies (`Arc<[u8]>`), so multipart works. `Method`, and `Endpoint` (`Endpoint::parse` accepts https only; `Endpoint::parse_local` also accepts http for a configured local runner).
- `SecretHeader` and `HttpRequest::with_secret`: a key is attached only over https or to a loopback host. `with_header` refuses credential header names. `HttpRequest::with_deadline` replaces the request deadline for one request.
- `Transport` (sends once), `UreqTransport` (the production transport) and `Timeouts` (connect plus one request deadline; `Timeouts::probe()` for probes).
- `HttpClient`, with `ResendPolicy`, `Attempts`, `Start`, `SendPermit`, `Pending`, `Next` and `Jitter`. `may_start` and `may_resend` admit a send under `max_sends` (default 5), the 90 s start budget and the per-host `ProviderPause`. `send_once` sends and returns `Next::Done` (a final reply, decoded by the backend) or `Next::ResendAt` (busy or timed out: a not-before instant).
- `ProviderReply`, `BusyReason` and `NetworkKind`.
- `HttpError`, with stable `http.*` ids. A busy quota reply is `HttpError::QuotaExhausted`, not a resend. No error or log event contains a key, a body or a header value.

## Usage

```rust,ignore
use std::sync::Arc;

use sc_agent_http::{
    Attempts, Endpoint, HttpClient, HttpRequest, LogContext, Method, Next, ResendPolicy, Start,
    Timeouts, UreqTransport,
};
use sc_agent_types::Clock;

// `general` is a sc_agent_log::GeneralLogger.
let client = HttpClient::new(
    Arc::new(UreqTransport::new(Timeouts::default())),
    ResendPolicy::default(),
    general,
);
let request = HttpRequest::new(
    Method::Get,
    Endpoint::parse("https://api.example.com/v1/models")?,
    Vec::new(),
);

let clock = Clock::system();
if let Ok(Start::Go(permit)) = client.may_start(&request, Attempts::new(), clock.now()) {
    match client.send_once(&request, permit, LogContext::NONE, &clock)? {
        Next::Done { response, .. } => println!("status {}", response.status()),
        Next::ResendAt { at, pending } => {
            // Hold `pending`, wait until `at`, then call `client.may_resend(&request, pending, now)`.
        }
    }
}
```

## Features

- `testing` (not default): `testing::{ScriptedTransport, RecordingTransport, BlockingTransport, RefusingTransport, ManualClock}`, transport test doubles for consumers' tests. It adds no dependency. Use it as a dev-dependency only.

## Depends on

- `sc-agent-types`, `sc-agent-secrets` and `sc-agent-log`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). `HttpError` is wrapped by the errors of the layers above.

## Requirements and decisions

- REQ-SAG-0601: busy resend and holdoff.
- REQ-SAG-0602: quota exhaustion is not busy.
- REQ-SAG-0012: provider-wide busy pause.
- REQ-SAG-0013: config bounds (a key is attached only over https or to a loopback host).
- REQ-SAG-0015: local runner timeouts (the per-request deadline).
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top.
- REQ-SAG-0008: optional logger injection.
- REQ-SAG-0009 and REQ-SAG-0010: no tokio, no sleep.
- ADR-SAG-0004: crate layering and per-crate error unions.
