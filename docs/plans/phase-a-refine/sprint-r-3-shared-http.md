---
id: r-3
phase: r
status: planned
closure_type: boundary
target_boundary: agent-http
track: http
wave: 2
branch: sprint/r-3-shared-http
worktree: sprint/r-3-shared-http
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [r-1, r-4]
parallel_safe: [r-2]
requirements:
  - REQ-SAG-0402
  - REQ-SAG-0400
  - REQ-SAG-0401
  - REQ-SAG-0505
  - NFR-SAG-0002
adrs:
  - ADR-SAG-0006
  - ADR-SAG-0007
  - ADR-SAG-0020
  - ADR-SAG-0026
owned_paths:
  - "crates/sc-agent-http/**"
  - "boundaries/sc-agent-http/**"
  - "docs/sc-agent-http/**"
  - "crates/sc-jev/Cargo.toml"
  - "crates/sc-jev/src/client/**"
  - "crates/sc-jev/src/lib.rs"
  - "crates/sc-jev-backends/Cargo.toml"
  - "crates/sc-jev-backends/src/transport.rs"
  - "crates/sc-jev-backends/src/retry.rs"
  - "crates/sc-jev-backends/src/lib.rs"
  - "crates/sc-agent-types/src/secret.rs"
  - "boundaries/sc-jev/**"
  - "boundaries/sc-jev-backends/**"
  - "Cargo.toml"
  - "release/sc-publish-consumer-input.json"
  - "clippy.toml"
---

# Sprint r-3 — `sc-agent-http` shared HTTP crate (D5)

## Outcome

One HTTP layer. `sc-jev` and `sc-jev-backends` become wire-format crates on
top of `sc-agent-http`, which owns the request/response types, the transport
trait, `UreqTransport`, busy retry, caps and redaction, and the only
`ResolvedApiKey` read. There is one sealing model (ADR-SAG-0020 amendment), and
backend constructors are infallible.

## Closes findings

EXT-005, RBQA-F003, RBP-F006, ARCHC-001, QA-003 (structural part), a-10 wave-1
follow-up "constructors still return Result; hidden `ResolvedApiKey::new`".

## Deliverables

1. New published crate `crates/sc-agent-http` (depends on `sc-agent-types` and
   `ureq` only). Modules: `request` (`HttpRequest`, `HttpResponse`, `Method`,
   `SecretHeader`), `transport` (`pub trait Transport: Send + Sync { fn send(&self, req: &HttpRequest) -> Result<HttpResponse, JevError>; }`,
   which is open so tests can implement it), `ureq` (`UreqTransport`: https-only,
   no redirects, one `Agent` with connect timeout 10 s and request timeout 120 s
   configurable, response body cap, versioned User-Agent), `retry`
   (`pub trait RetrySleeper`, `BusyPolicy`, `send_with_busy_retry`), `redact`
   (header/body redaction used for every error and log string). Errors are
   `JevError::NetworkError { kind, .. }` (r-1), so there is one taxonomy.
2. `BusyPolicy::default()` encodes the a-10 numbers (ADR-SAG-0007 amendment):
   statuses 429/502/529; Retry-After integer or decimal seconds clamped 0–30;
   missing → 1 s; malformed → 30 s; jitter ≤ 250 ms; max 4 attempts; total
   budget 90 s from first send (no new attempt starts after the budget; an
   in-flight attempt is bounded by the request timeout).
3. Delete the parallel copies: `sc-jev/src/client/{transport,retry,https}.rs`
   and `sc-jev-backends/src/{transport,retry}.rs`. Each crate keeps only its
   wire builder and response parser, plus a re-export of
   `sc_agent_http::Transport` for construction.
4. **One sealing model (ADR-SAG-0020):** a secret travels only as
   `SecretHeader { name, value: ResolvedApiKey }` inside `HttpRequest`.
   `ResolvedApiKey::as_str` is `pub(crate)`-equivalent: it is reachable only
   through `sc_agent_types::secret::expose_for_transport(&ResolvedApiKey, Token)`,
   where `Token` has a private constructor exported only to `sc-agent-http` via
   `#[doc(hidden)] pub mod __transport` and gated by a clippy
   `disallowed-methods` rule allowlisting `crates/sc-agent-http/src/ureq.rs`.
   Test transports see `SecretHeader::redacted()` and
   `SecretHeader::matches(&str)` (feature `test-util`).
5. **Infallible constructors and a sealed `new`:** validation lives only in
   `ResolvedApiKey::try_new`. Delete the hidden `ResolvedApiKey::new`, or make
   it `pub(crate)` with no `#[doc(hidden)]` export. `JevClient::new`,
   `AnthropicHaikuBackend::new`, and `OpenAiLunaBackend::new` return `Self`, not
   `Result`. Call sites are updated (`sc-jev-check` registry construction,
   which r-2 owns; coordinate by landing r-3 after r-2 if both touch
   `registry/mod.rs`, otherwise r-3 edits only constructor signatures and r-2
   rebases).
6. Root `Cargo.toml`: add the `sc-agent-http` member and workspace dep; delete
   the `sc-observability-types` workspace entry (unused after r-1). Consumer
   input: add `sc-agent-http` with `publish_order: 3` and shift later crates
   by one. Add `boundaries/sc-agent-http/agent-http.toml` (deps: sc-agent-types,
   ureq; forbidden: sc-jev*, sc-agent-secrets, sc-observability, pyo3) and
   update the sc-jev / sc-jev-backends manifests.
7. Land the ADR-SAG-0006 and ADR-SAG-0020 amendments.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0402 | No `Transport`/retry/redaction code in `sc-jev` or `sc-jev-backends` (grep test); both depend on `sc-agent-http`; `test_busy_policy_numbers`; loopback tests for `UreqTransport` (https-only, no redirect, body cap) |
| adr:ADR-SAG-0020 | `as_str` unreachable outside `sc-agent-http` (clippy + `test_no_resolved_api_key_new`); constructors infallible; key never in `Debug`/errors (existing leak tests moved) |
| adr:ADR-SAG-0006 | Amendment Active |
| boundary:agent-http | Boundary manifest enforced by sc-lint |
| boundary:jev | Regression: REQ-SAG-0400/0401 tests green |
| boundary:jev-backends | Regression: REQ-SAG-0505 tests green |

## Required validation

```bash
just lint
just test
cargo test -p sc-agent-http -- --nocapture
cargo test -p sc-agent-http -- test_busy_policy_numbers --exact
cargo test -p sc-jev -- --nocapture
cargo test -p sc-jev-backends -- --nocapture
cargo clippy --workspace --all-targets -- -D warnings
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-http -p sc-jev -p sc-jev-backends
```

## This sprint does not close

Async transport (Phase B), provider-agent request types (Phase B), D9 envelope
(r-5).
