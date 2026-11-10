# sc-agent-http

Sync HTTP layer for sc-agent: one send per call, no queue, no sleep.

- `HttpRequest` / `HttpResponse`: byte bodies (`Arc<[u8]>`), so multipart works.
  `Endpoint::parse` accepts https only; `Endpoint::parse_local` also accepts http for a
  configured local runner. A key is attached only over https or to a loopback host
  (`HttpRequest::with_secret`, REQ-SAG-0013).
- `Transport` sends once; `UreqTransport` is the production transport, bounded by
  `Timeouts` (connect, plus one global request deadline; `Timeouts::probe()` for probes).
  `HttpRequest::with_deadline` replaces the request deadline for one request (REQ-SAG-0015).
- `HttpClient`: `may_start` / `may_resend` admit a send under `ResendPolicy` (`max_sends`,
  default 5), the 90 s start budget and the per-host `ProviderPause`; `send_once` sends and
  returns `Next::Done` (a final reply, decoded by the backend) or `Next::ResendAt` (busy or
  timed out: a not-before `Instant`). A busy quota reply is `http.quota_exhausted`
  (REQ-SAG-0601, REQ-SAG-0602, REQ-SAG-0012). The caller's queue waits and owns cancellation.
- Every `http.*` event goes to the injected `GeneralLogger` with the caller's `LogContext`,
  never with a body, header value or key (REQ-SAG-0008).
- Feature `testing`: `testing::{ScriptedTransport, RecordingTransport, BlockingTransport,
  ManualClock}` for consumers' tests.

Every error is an `HttpError` with a stable `http.*` id (`sc_agent_types::ErrorId`).
