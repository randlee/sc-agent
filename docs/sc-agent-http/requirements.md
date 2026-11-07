# sc-agent-http — requirements

**Sprint** = the sprint that implements the requirement. Root rows are in
[`../requirements.md`](../requirements.md); the A.1 closure table in
[`plans/phase-a-refine/plan.md`](../plans/phase-a-refine/plan.md) is
authoritative.

| ID | Requirement | Sprint | Proof |
| --- | --- | --- | --- |
| REQ-SAG-0402 | One shared HTTP layer for every backend family: `HttpRequest` / `HttpResponse`, `Transport`, `UreqTransport` (one agent, connect 10 s, request 120 s incl. body read, no redirects, https only, versioned `User-Agent`, 8 MiB streaming cap for every status), `Endpoint` (https only in every build), `SecretHeader`, `BusyPolicy` / `send_with_busy_retry`, `redact`. `sc-jev` and `sc-jev-backends` depend on it and not on ureq | r-3 | `--test ureq` (loopback), `--lib ureq_transport::tests::test_ureq_agent_config_values`, `--lib ureq_transport::tests::test_endpoint_parse_refuses_http`, `cargo tree --depth 1` of both backends |
| REQ-SAG-0505 (amended) | `BusyPolicy::default()`: 429/502/529 only; 4 sends; valid Retry-After (1–32 bytes, `^[0-9]+(\.[0-9]+)?$`, parsed without floating point) honoured, clamped 0–30 s; absent or malformed → 1, 2, 4, 8 … s capped at 30 s; ≤ 250 ms jitter; 90 s budget for starting attempts; waits sliced ≤ 100 ms with `CancelToken`; errors never retried; connect/DNS → `NETWORK_ERROR` kind `Connect`, `retryable() == true`; "N sends exhausted" vs "retry budget exhausted" | r-3 | `--features test-util --test retry` (nine named tests) |
| NFR-SAG-0002 | No secret in any error, log or `Debug` output: `send_with_busy_retry` redacts every `Err` it returns once, including errors from third-party transports; `HttpRequest` / `SecretHeader` `Debug` print `[redacted]` | r-3 | `--test retry -- test_transport_error_with_secret_is_redacted`, `--test ureq -- test_request_debug_and_secret_header_never_show_the_key`, `test_timeout_is_network_error_without_the_key` |
| ADR-SAG-0020 (r-3) | The single `ResolvedApiKey::as_str` call site is `src/ureq_transport.rs` (`#[expect(clippy::disallowed_methods, reason = ...)]`); lint-enforced convention, not a type seal | r-3 | `cargo clippy --workspace --all-targets -- -D warnings`; `rg` read-site check (sprint r-3 "Required validation"); r-3b `workspace-tests --test source_scans` |

`test-util` is non-default and enabled only by dev-dependencies; it never
appears in a normal-dependency graph (`cargo tree -p sc-agent-http -e
normal,features`).
