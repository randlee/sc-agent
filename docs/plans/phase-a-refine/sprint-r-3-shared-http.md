---
id: r-3
phase: r
status: planned
closure_type: boundary
target_boundary: agent-http
track: http
wave: 2
size: L
branch: sprint/r-3-shared-http
worktree: sprint/r-3-shared-http
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [r-1]
parallel_safe: [r-2, r-3c]
vertical_rationale: "sc-agent-http does not exist yet; r-3 creates it and moves its only two consumers (sc-jev, sc-jev-backends) onto it. r-3 touches no sc-jev-check file: the registry calls the infallible with_default_transport constructors r-1 fixed."
requirements:
  - REQ-SAG-0402
  - REQ-SAG-0400
  - REQ-SAG-0401
  - REQ-SAG-0505
  - REQ-SAG-0130
  - NFR-SAG-0002
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0003
  - ADR-SAG-0006
  - ADR-SAG-0007
  - ADR-SAG-0012
  - ADR-SAG-0017
  - ADR-SAG-0018
  - ADR-SAG-0020
  - ADR-SAG-0023
  - ADR-SAG-0026
  - ADR-SAG-0027
owned_paths:
  - "crates/sc-agent-http/**"
  - "boundaries/sc-agent-http/**"
  - "docs/sc-agent-http/**"
  - "crates/sc-jev/**"
  - "crates/sc-jev-backends/**"
  - "boundaries/sc-jev/**"
  - "boundaries/sc-jev-backends/**"
  - "boundaries/sc-agent-types/**"
  - "docs/sc-jev/**"
  - "docs/sc-jev-backends/**"
  - "Cargo.toml"
  - "clippy.toml"
  - "sc-lint.toml"
  - "release/**"
  - ".github/workflows/*-publish.yml"
  - ".github/workflows/release*.yml"
  - ".github/workflows/prerelease-archive.yml"
  - ".github/workflows/sc-publish-dry-run.yml"
  - "docs/architecture.md#ADR-SAG-0006 amendment (r-3)"
  - "docs/architecture.md#ADR-SAG-0007 amendment (r-3)"
  - "docs/architecture.md#ADR-SAG-0017 amendment (r-3)"
  - "docs/architecture.md#ADR-SAG-0020 amendment (r-3)"
  - "docs/architecture.md#ADR-SAG-0026 HTTP half"
  - "docs/requirements.md#REQ-SAG-0402 row; REQ-SAG-0505, REQ-SAG-0130 (r-3 bullet) amendments"
---

# Sprint r-3 — `sc-agent-http` shared HTTP crate (D5)

## Outcome

One HTTP layer. `sc-jev` and `sc-jev-backends` become wire-format crates on
`sc-agent-http`, which owns request/response types, the transport trait,
`UreqTransport`, busy retry with a cancellation hook, caps and redaction, and
the single `ResolvedApiKey::as_str` call site. ADR-SAG-0026's HTTP half goes
Active here. ADR-SAG-0012 still holds: `sc-jev` owns every Jev API operation;
`sc-agent-http` only carries bytes.

## Closes findings

EXT-005, RBQA-F003, RBP-F006, RBP-F007 (body), ARCHC-001, RSH-005; a-10
follow-up "hidden `ResolvedApiKey::new`" is closed by r-1.

## Deliverables

1. **Crate** `crates/sc-agent-http` (published; depends on `sc-agent-types`
   with `default-features = false, features = ["secret", "judge"]`, and
   `ureq`). Modules and public items:
   - `request`: `HttpRequest { method, endpoint: Endpoint, headers, secret: Option<SecretHeader>, body: Arc<[u8]> }`,
     `HttpResponse { status, headers, body: Vec<u8> }`, `Method`.
     `Endpoint` is an https-validated URL newtype (`Endpoint::parse`);
     `http://` is accepted only under `#[cfg(test)]` inside this crate
     (`test_release_build_refuses_http`, target `ureq`).
   - `transport`: `pub trait Transport: Send + Sync { fn send(&self, req: &HttpRequest) -> Result<HttpResponse, JevError>; }`.
     Open (test transports implement it). No `ureq` type appears in any public
     signature (`test_public_api_has_no_ureq`, target `api`, reads
     `cargo public-api` output if available, else rustdoc JSON).
   - `ureq`: `UreqTransport::new(Timeouts) -> Self` builds **one** `ureq::Agent`
     in `new` (connect 10 s, global request timeout 120 s covering body read,
     no redirects, https only, versioned User-Agent) and reuses it.
     Response body cap **8 MiB**, enforced while streaming, for error bodies
     too; `Retry-After` header longer than 32 bytes is malformed.
   - `retry`: `BusyPolicy` (private fields, constructor, `default()` per
     owner decision (b)), `send_with_busy_retry(&dyn Transport, &HttpRequest, &BusyPolicy, &CancelToken) -> Result<HttpResponse, JevError>`.
     Sleeping is part of `BusyPolicy` (a `#[cfg(test)]`-replaceable sleeper
     inside the crate; no public `RetrySleeper`).
   - `redact`: header and body redaction, applied **once**, inside
     `send_with_busy_retry` when it builds any `NetworkError` (not per
     transport impl).
2. **Retry rules** (owner decision (b), DECIDED 2026-11-07; REQ-SAG-0505
   amendment; ADR-SAG-0007 r-3 amendment). `BusyPolicy::default()`:
   statuses 429/502/529 only; at most 4 sends; a valid Retry-After (integer
   or decimal seconds) honoured, clamped 0–30 s; absent **or malformed**
   Retry-After (treated alike) → exponential backoff 1, 2, 4, 8 … s capped at
   30 s; up to 250 ms jitter on every wait; 90 s budget from the first send
   for starting attempts, every wait `min(wait + jitter, budget_remaining)`,
   no attempt starts after the budget; an in-flight attempt keeps its 120 s
   request timeout. The `CancelToken` is checked before each attempt and
   every wait is sliced (≤ 100 ms slices), so a cancel returns
   `NetworkError { kind: Cancelled }` promptly. Connect and DNS failures are
   not retried and there is no connectivity probing: they return
   `NetworkError { kind: Connect }` (`retryable() == true`) at once and the
   caller decides. A transport error or timeout after the body was sent is
   never retried. 502 retry can double-charge a provider that processed the
   request; documented in the amendment. Tests (target `retry`):
   `test_busy_policy_numbers`, `test_missing_and_malformed_retry_after_back_off_exponentially`,
   `test_retry_sleep_capped_by_budget`, `test_cancel_interrupts_sleep`,
   `test_no_retry_after_body_sent`, `test_connect_failure_returns_immediately_retryable`.
3. **Delete the parallel copies:** `sc-jev/src/client/{transport,retry,https}.rs`,
   `sc-jev-backends/src/{transport,retry}.rs`. Each crate keeps its wire
   builder and response parser. Backends are generic,
   `Backend<T: Transport = UreqTransport>`; `with_default_transport(key)`
   (r-1 signature) now builds `UreqTransport`. The `BackendTransport` /
   `JevTransport` traits disappear; tests use `sc-agent-http` test transports.
4. **Secret read site — honest mechanism** (ADR-SAG-0020 r-3 amendment). The
   secret travels only as `SecretHeader { name, value: ResolvedApiKey }` in
   `HttpRequest`. `ResolvedApiKey::as_str` stays `pub` and is listed in
   `clippy.toml` `disallowed-methods`. Its one call site,
   `crates/sc-agent-http/src/ureq.rs`, carries
   `#[expect(clippy::disallowed_methods, reason = "single secret read site (ADR-SAG-0020)")]`.
   Root `Cargo.toml` `[workspace.lints.clippy]` sets `allow_attributes = "deny"`
   and `allow_attributes_without_reason = "deny"`, so a second site needs a
   visible `#[expect]`. `crates/sc-agent-http/tests/secret_read_site.rs`
   (`test_disallowed_methods_expect_only_in_ureq_rs`) scans the workspace
   `crates/*/src/**` and fails if that `#[expect]` appears anywhere else.
   This is a lint-enforced workspace convention, not a type seal; crates
   outside the workspace are not constrained. The round-1 `Token` /
   `__transport` design is dropped. Test transports see
   `SecretHeader::redacted()` and `SecretHeader::matches(&str)`.
5. **Root and manifests.** Root `Cargo.toml`: add member + workspace dep,
   delete `sc-observability-types`, add the two workspace clippy lints.
   `sc-lint.toml` `ureq` record → `crates/sc-agent-http` only. Boundary
   manifests: `boundaries/sc-agent-http/agent-http.toml` (full text in
   plan.md); `sc-jev`/`sc-jev-backends` `allowed_dependencies -ureq +sc-agent-http`;
   `sc-agent-types` `allowed_dependents +sc-agent-http`. Consumer input: add
   `sc-agent-http` with `publish_order: 3`, shift later crates by one, re-run
   `install.py` (r-3 owns the rendered files this wave).
6. **ADR text.** ADR-SAG-0026 HTTP half → Active (states ADR-SAG-0012 still
   holds). ADR-SAG-0006 r-3 amendment → Active. ADR-SAG-0020 r-3 amendment →
   Active. ADR-SAG-0017 r-3 amendment (published library set gains
   `sc-agent-http`, publish order) → Active. ADR-SAG-0007 r-3 amendment →
   Active. REQ-SAG-0505 and REQ-SAG-0130 r-3 bullets → Active.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0402 | No transport/retry/redaction code in `sc-jev` or `sc-jev-backends` (the deleted files are gone; both compile only against `sc-agent-http`); `cargo tree --depth 1` of both has no `ureq`; loopback tests for `UreqTransport` (https-only, no redirect, 8 MiB streaming cap, agent reused) |
| adr:ADR-SAG-0026 | HTTP half Active |
| adr:ADR-SAG-0020 | r-3 subsection Active; `test_disallowed_methods_expect_only_in_ureq_rs`; existing leak tests moved into `sc-agent-http` |
| adr:ADR-SAG-0006, 0017 | r-3 subsections Active |
| boundary:agent-http | Manifest enforced by `just lint` (output attached); `test_public_api_has_no_ureq`; `test_release_build_refuses_http`; retry tests |
| boundary:jev | Regression: REQ-SAG-0400/0401 tests green |
| boundary:jev-backends | Regression: REQ-SAG-0505 tests green against the decided numbers |
| adr:ADR-SAG-0007 | r-3 subsection Active; text matches `BusyPolicy::default()` |

## Required validation

```bash
just lint
just test
cargo test -p sc-agent-http --test retry -- test_busy_policy_numbers --exact
cargo test -p sc-agent-http --test retry -- test_missing_and_malformed_retry_after_back_off_exponentially --exact
cargo test -p sc-agent-http --test retry -- test_retry_sleep_capped_by_budget --exact
cargo test -p sc-agent-http --test retry -- test_connect_failure_returns_immediately_retryable --exact
cargo test -p sc-agent-http --test retry -- test_cancel_interrupts_sleep --exact
cargo test -p sc-agent-http --test retry -- test_no_retry_after_body_sent --exact
cargo test -p sc-agent-http --test ureq
cargo test -p sc-agent-http --test api -- test_public_api_has_no_ureq --exact
cargo test -p sc-agent-http --test secret_read_site -- test_disallowed_methods_expect_only_in_ureq_rs --exact
cargo test -p sc-jev
cargo test -p sc-jev-backends
cargo clippy --workspace --all-targets -- -D warnings
test -z "$(cargo tree -p sc-jev -e normal --depth 1 | grep ureq)"
test -z "$(cargo tree -p sc-jev-backends -e normal --depth 1 | grep ureq)"
test -z "$(cargo tree -p sc-agent-http -e normal | grep -E 'sc-observability|pyo3|tree-sitter|sc-agent-secrets')"
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-agent-http -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## This sprint does not close

Backend-side `input_sha256` recovery (r-2b, which consumes the final
`HttpRequest.body` built here); async transport and provider-agent request
types (Phase B); D9 envelope (r-5); CI `cargo tree` script rows (r-3b).
