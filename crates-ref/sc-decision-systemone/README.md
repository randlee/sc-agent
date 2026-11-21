# sc-decision-systemone

The System One decision backend of sc-agent. One sans-IO `Backend` serves every System One server: TypeSafe Jev (Bearer key) and the Jev-compatible local daemons (key optional). All of them take the same `SystemOneRequest` and answer the same `SystemOneResponse`. There is no backend-specific request type.

It sits in the decision-backend layer, above `sc-decision-schema`. It does no I/O and writes no log. The caller sends through `sc-agent-http`, which logs the send.

## What it provides

- `SystemOneBackend`, which implements `sc_decision_schema::Backend`:
  - `SystemOneBackend::new(base, key)` for an https server, `SystemOneBackend::local(base, key)` for a local http runner, and `SystemOneBackend::at(&BaseUrl, key)`.
  - `encode` builds `POST {base}/v1/systemone`: JSON, or multipart/form-data when the request carries image parts. The TypeSafe danger-zone headers and the Bearer key are attached through `HttpRequest::with_secret`.
  - `decode` turns the final reply into a response or the matching `DecisionError`.
  - `models_request` and `decode_models` for `GET {base}/v1/models`.
- `BaseUrl` (`BaseUrl::https`, `BaseUrl::local`, `join`): a base URL parsed and checked once.
- `TYPESAFE_BASE_URL`, `SYSTEMONE_PATH` and `MODELS_PATH`.

Busy replies (429, 502, 529) are handled by `sc-agent-http` and the runner before `decode`. A 503 is a final reply here. The crate has no error type of its own: every function returns the `DecisionError` that the `Backend` trait fixes (ADR-SAG-0004 exemption).

## Usage

```rust,ignore
use sc_agent_http::{HttpResponse, LogContext};
use sc_decision_schema::{Backend, DecisionRequest, SystemOneRequest};
use sc_decision_systemone::{SystemOneBackend, TYPESAFE_BASE_URL};

let backend = SystemOneBackend::new(TYPESAFE_BASE_URL, Some(key))?; // key: an sc_agent_secrets::ApiKey
let body = SystemOneRequest::from_json(
    r#"{"model":"jev-latest","state":"hi","questions":{"q":{"type":"noul"}}}"#,
)?;
let request = DecisionRequest::try_from(body)?;

let http_request = backend.encode(&request)?; // POST https://api.typesafe.ai/v1/systemone
// Send it through sc_agent_http::HttpClient, then:
let response = backend.decode(reply, &request, &LogContext::NONE)?; // reply: HttpResponse
```

## Depends on

- `sc-agent-http`, `sc-agent-secrets`, `sc-agent-types` and `sc-decision-schema`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). `sc-decision-local` builds on this crate and reuses its encode and decode.

## Requirements and decisions

- ADR-SAG-0001: decision backends implement one trait with one request form.
- ADR-SAG-0004: crate layering, and the exemption from a per-crate error union.
- REQ-SAG-0601: busy resend and holdoff.
- REQ-SAG-0007: return values are not rounded.
- REQ-SAG-0001: created ids are ULIDs (the multipart boundary is `sc-agent-` plus a fresh ULID).
- REQ-SAG-0013: config bounds (a key goes only over https or to a loopback host).
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top.
