---
id: r-3
phase: r
status: in-review # open row: boundary:release / req:REQ-SAG-0130 install.py re-render of release/publish-artifacts.toml (kit execution not permitted in the dev session; see Implementation notes)
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
  - REQ-SAG-0110
  - REQ-SAG-0400
  - REQ-SAG-0401
  - REQ-SAG-0500
  - REQ-SAG-0501
  - REQ-SAG-0502
  - REQ-SAG-0504
  - REQ-SAG-0505
  - REQ-SAG-0130
  - REQ-SAG-0131
  - REQ-SAG-0132
  - NFR-SAG-0001
  - NFR-SAG-0002
  - NFR-SAG-0004
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0003
  - ADR-SAG-0006
  - ADR-SAG-0007
  - ADR-SAG-0009
  - ADR-SAG-0010  # sc-lint ureq record: every crate but sc-agent-http stays HTTP-free
  - ADR-SAG-0012  # unchanged; stated in ADR-SAG-0026 (sc-jev owns every Jev API operation)
  - ADR-SAG-0016
  - ADR-SAG-0017
  - ADR-SAG-0018
  - ADR-SAG-0019
  - ADR-SAG-0020
  - ADR-SAG-0023  # binding once r-4 has made its r-4 part Active; r-3 does not cite its r-3b subsection
  - ADR-SAG-0026  # HTTP half only; the compose half stays Proposed (r-3b) and r-3 relies on none of it
  - ADR-SAG-0027
  - ADR-SAG-0029
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
  - "docs/architecture.md#ADR-SAG-0020 amendment (r-3b) — inserted as Proposed only (r-3b owns its text from then on and flips it)"
  - "docs/architecture.md#ADR-SAG-0026 Status line and HTTP half"
  - "docs/architecture.md#ADR index row ADR-SAG-0026 (Status cell: HTTP half)"
  - "docs/architecture.md#ADR index Amended-by marker (r-3) in ADR-SAG-0006, 0007, 0017, 0020, 0029"
  - "docs/architecture.md#ADR index row ADR-SAG-0020 (adds the marker 'A.1 subsection (Proposed, r-3b)')"
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

EXT-005, QA-003, RBQA-F003, RBP-F006, RBP-F007 (body), ARCHC-001, RSH-005;
a-10 follow-up "hidden `ResolvedApiKey::new`" is closed by r-1. Deleting
`sc-jev/src/client/https.rs` and `sc-jev-backends/src/transport.rs`
supersedes the ADR-SAG-0029 (a) sentence naming them as "the only production
readers" of `ResolvedApiKey::as_str`: from r-3 the one reader is
`crates/sc-agent-http/src/ureq_transport.rs` (ADR-SAG-0020 r-3 amendment).
ADR-SAG-0029 (a) already states that `sc-agent-secrets` has no Cargo
features; A.1 relies on it and does not restate it. (The
workspace-wide secret-read-site scan of RBQA-F003 lands in r-3b's
`workspace-tests --test source_scans`; r-3 lands the lint mechanism and a
local `rg` check.)

## Deliverables

1. **Crate** `crates/sc-agent-http` (published; `[lints] workspace = true`;
   depends on `sc-agent-types` with `default-features = false,
   features = ["secret"]` — `CancelToken` is in the always-on core since
   r-1 — and `ureq`; no `rand` or `url` crate: jitter comes from std
   (`std::hash::RandomState` hashed with the attempt time) and URL parsing
   uses ureq's re-exported `http::Uri` inside the private transport module).
   Modules and public items:
   - `request`: `HttpRequest` and `HttpResponse` with **private fields,
     constructors and accessors** (implementors of `Transport` build
     `HttpResponse`, so no public fields): `HttpRequest::new(method, endpoint, body: Arc<[u8]>)`,
     `with_header(name, value)`, `with_secret(SecretHeader)`, accessors
     `method()`, `endpoint()`, `headers()`, `secret()`, `body()`;
     `HttpResponse::new(status: u16, headers, body: Vec<u8>)`, accessors
     `status()`, `header(name)`, `body()`, `into_body()`. `Method`.
     `SecretHeader::new(name: HeaderName, value: ResolvedApiKey)` where
     `HeaderName` is a validated (RFC 9110 token) `&'static str` newtype.
     `Endpoint` is an https-validated URL newtype: `Endpoint::parse` accepts
     **only** `https://` in every build. The `test-util` feature adds
     `Endpoint::loopback_http(port: u16, path: &str)`, which can only build
     `http://127.0.0.1:<port>/…`; it cannot be pointed at a non-loopback host,
     so the feature cannot open plain HTTP to the network even if a build
     enables it by mistake (and the `test-util` `cargo tree` gate keeps it
     out of normal graphs). `test_release_build_refuses_http` (target
     `ureq`, needs `--features test-util`) and the `--lib` unit test
     `ureq_transport::tests::test_endpoint_parse_refuses_http` (runs without
     any feature) both assert `Endpoint::parse("http://…")` is refused.
   - `transport`: `pub trait Transport: Send + Sync { fn send(&self, req: &HttpRequest) -> Result<HttpResponse, JevError>; }`.
     Open (test transports implement it). Direct `Transport::send` callers
     bypass busy retry **and** redaction; rustdoc says so, and production
     code calls only `send_with_busy_retry`.
   - `ureq_transport` (**private module**, file `src/ureq_transport.rs`;
     named so it does not shadow the `ureq` crate): `UreqTransport` is
     re-exported at the crate root. `UreqTransport::new(Timeouts) -> Self`
     builds **one** `ureq::Agent` in `new` (connect 10 s, global request
     timeout 120 s covering body read, no redirects, https only, versioned
     User-Agent) and reuses it. The agent settings come from one private
     `AgentConfig` value; `test_ureq_agent_config_values` (unit test,
     `--lib`, no network) asserts connect 10 s, global 120 s, redirects 0,
     https-only and the User-Agent string. Under `test-util` only,
     `UreqTransport::loopback_for_tests()` builds the same `AgentConfig` with
     https-only off, for `Endpoint::loopback_http` targets; the production
     constructor has no such switch. Response body cap **8 MiB**,
     enforced while streaming, for error bodies too. No `ureq` type is in
     any public signature: the module is private and the public-API check
     greps the generated docs. The check runs `cargo doc -p sc-agent-http`
     **without** `--no-deps`, so ureq's own docs exist and a leaked type
     renders as a link `../ureq/<kind>.<Name>.html` with
     `title="<kind> ureq::<Name>"`; it then fails on either form:
     `! grep -rlE 'ureq/(struct|enum|trait|type|fn|union)\.|title="(struct|enum|trait|type|fn|union) ureq::' target/doc/sc_agent_http/`
     (with `--no-deps` rustdoc may print a bare `Agent` and the gate could
     never fail, CPR-209). **Canary:** before the final commit, r-3 adds a
     throwaway `pub fn _canary() -> ureq::Agent` to `lib.rs`, runs the check,
     records in the PR that it fails and on which pattern, then deletes the
     canary. Rustdoc prose names the crate as "ureq" without a path. r-3b
     puts the same proven command into CI.
   - `retry`: `BusyPolicy` (private fields, constructor, `default()` per
     owner decision (b)), `send_with_busy_retry(&dyn Transport, &HttpRequest, &BusyPolicy, &CancelToken) -> Result<HttpResponse, JevError>`.
     Sleeping, the monotonic clock and jitter are part of `BusyPolicy`. The
     `test-util` feature adds `BusyPolicy::with_test_hooks(clock, sleeper, jitter)`
     (deterministic jitter, a fake monotonic clock, a recording sleeper);
     there is no public `RetrySleeper` in a normal build. `test-util` also
     exposes `test_util::{RecordingTransport, ScriptedTransport}` for the
     backend crates' tests.
   - `redact`: header and body redaction, applied **once**, inside
     `send_with_busy_retry`, to **every** `Err` it returns — the ones it
     builds and every `Err` a `Transport` impl returns (third-party
     transports included), whatever the variant: the error's text fields
     are passed through `redact` before they leave the function.
     `test_transport_error_with_secret_is_redacted` (target `retry`): a
     scripted transport returns an error whose detail contains the request's
     secret value; the returned error does not contain it.
   - **Tests and the seam.** The crate's integration targets `retry` and
     `ureq` are declared `[[test]] required-features = ["test-util"]` and
     their validation lines pass `--features test-util`; there is no self
     dev-dependency (r-1's D0 spike result, `boundaries/README.md`
     "Dev-dependency and self edges"; plan "Test seams"); loopback tests in
     `--test ureq` use `Endpoint::loopback_http` and run against a std
     `TcpListener` on 127.0.0.1.
2. **Retry rules** (owner decision (b), DECIDED 2026-11-06; REQ-SAG-0505
   amendment; ADR-SAG-0007 r-3 amendment). `BusyPolicy::default()`:
   statuses 429/502/529 only; at most 4 sends; a valid Retry-After honoured,
   clamped 0–30 s. **Retry-After grammar:** valid is 1–32 bytes matching
   `^[0-9]+(\.[0-9]+)?$` (ASCII digits, optional ASCII-digit fraction),
   parsed without `f64::from_str` (integer and fraction parts are parsed as
   digits, so NaN, infinity, sign, exponent and overflow cannot occur and
   `Duration::from_secs_f64` is never called on an unchecked value);
   anything else — HTTP-date, `-1`, `1e9`, `inf`, `NaN`, empty, > 32 bytes —
   is malformed. Absent **or malformed** Retry-After (treated alike) →
   exponential backoff 1, 2, 4, 8 … s capped at 30 s; with the default 4
   sends at most three waits occur (1, 2, 4 s), so 8 s and the cap are
   reached only by a non-default policy; up to 250 ms jitter on every wait;
   90 s budget from the first send for starting attempts, every wait
   `min(wait + jitter, budget_remaining)`, no attempt starts after the
   budget; an in-flight attempt keeps its 120 s request timeout. The
   `CancelToken` is checked before each attempt and every wait is sliced
   (≤ 100 ms slices), so a cancel during a wait returns
   `NetworkError { kind: Cancelled }` within 100 ms. A cancel during an
   in-flight request is observed only when that request returns: the
   worst-case cancel latency is the 120 s request timeout (rustdoc of
   `send_with_busy_retry` and the ADR-SAG-0007 r-3 amendment say so). Connect and DNS failures are
   not retried and there is no connectivity probing: they return
   `NetworkError { kind: Connect }` (`retryable() == true`) at once and the
   caller decides. A transport error or timeout after the body was sent is
   never retried. 502 retry can double-charge a provider that processed the
   request; documented in the amendment. Tests (target `retry`):
   `test_busy_policy_numbers` (default: three waits 1, 2, 4 s + jitter),
   `test_missing_and_malformed_retry_after_back_off_exponentially`,
   `test_retry_after_grammar` (table: `"3"`, `"2.5"` valid; `"0"` valid with
   a zero wait, bounded by the 4-send cap; 32 nines valid and saturating to
   the 30 s clamp without overflow; `"-1"`, `"1e9"`, `"inf"`, `"NaN"`, `""`,
   an HTTP-date, 33 digits malformed),
   `test_exhaustion_reason_distinguishable` (the error after the fourth
   send's busy status says "4 sends exhausted"; the error when the 90 s
   budget forbids the next attempt says "retry budget exhausted"; both are
   `NetworkError { kind: Busy }` with `retryable() == true`, distinguished by
   detail text),
   `test_retry_sleep_capped_by_budget`, `test_cancel_interrupts_sleep`,
   `test_no_retry_after_body_sent`, `test_connect_failure_returns_immediately_retryable`,
   `test_transport_error_with_secret_is_redacted`; all drive
   `BusyPolicy::with_test_hooks` (no real sleeping).
3. **Delete the parallel copies:** `sc-jev/src/client/{transport,retry,https}.rs`,
   `sc-jev-backends/src/{transport,retry}.rs`. Each crate keeps its wire
   builder and response parser. Backends are generic,
   `Backend<T: Transport = UreqTransport>`; `with_default_transport(key)`
   (r-1 signature) now builds `UreqTransport`. The `BackendTransport` /
   `JevTransport` traits disappear; tests use `sc-agent-http` test transports.
4. **Secret read site — honest mechanism** (ADR-SAG-0020 r-3 amendment). The
   secret travels only as `SecretHeader` (holding `ResolvedApiKey`) in
   `HttpRequest`. `ResolvedApiKey::as_str` stays `pub` and is listed in
   `clippy.toml` `disallowed-methods`. Its one call site,
   `crates/sc-agent-http/src/ureq_transport.rs`, carries
   `#[expect(clippy::disallowed_methods, reason = "single secret read site (ADR-SAG-0020)")]`.
   Root `Cargo.toml` `[workspace.lints.clippy]` sets `allow_attributes = "deny"`
   and `allow_attributes_without_reason = "deny"`, so a second site needs a
   visible `#[expect]`; r-1 already removed every `#[allow]`, and every
   crate has `[lints] workspace = true` (a-10; `sc-agent-http` adds it here).
   r-3's validation runs a local check (`rg`, below). The cross-crate proof
   is `workspace-tests --test source_scans` in r-3b (moved out of this
   published crate, RBQA-F003), governed by an r-3b-owned ADR-SAG-0020
   bullet, not by r-3's (ADR text, item 7): it scans **all** targets
   (`crates/**/*.rs`, incl. `tests/`, `benches/`, `examples/`, `build.rs`)
   for `disallowed_methods` in any `allow`/`expect` (item- or crate-level)
   outside `crates/sc-agent-http/src/ureq_transport.rs` (the type-aware half
   — an actual `ResolvedApiKey::as_str` call — is clippy's
   `disallowed-methods` under `cargo clippy --workspace --all-targets`), and
   asserts every `crates/*/Cargo.toml` has `[lints] workspace = true` and no
   per-crate `[lints.*]` override. This is a lint-enforced workspace
   convention, not a type seal; crates outside the workspace are not
   constrained. The round-1 `Token` /
   `__transport` design is dropped. Test transports see
   `SecretHeader::redacted()` and `SecretHeader::matches(&str)`.
5. **Root and manifests.** Root `Cargo.toml`: add member + workspace dep, add
   the two workspace clippy lints (`sc-observability-types` was already
   deleted by r-1).
   `sc-lint.toml` `ureq` record → `crates/sc-agent-http` only. Boundary
   manifests: `boundaries/sc-agent-http/agent-http.toml` (full text in
   plan.md); `sc-jev`/`sc-jev-backends` `allowed_dependencies -ureq +sc-agent-http`;
   `sc-agent-types` `allowed_dependents +sc-agent-http`. The
   `agent-http` manifest names the `test-util` feature in `notes`. Consumer
   input: add `sc-agent-http` with `publish_order: 3`, shift later crates by
   one, re-run `install.py` (r-3 owns the rendered files this wave).
   `sc-agent-http` takes caret dependencies only (NFR-SAG-0004:
   `check-exact-pins.sh` stays green).
6. **Crate docs.** `docs/sc-agent-http/requirements.md` (rows mapping to
   REQ-SAG-0402, REQ-SAG-0505, NFR-SAG-0002 and the ADR-SAG-0020 read site)
   and `docs/sc-agent-http/architecture.md` (modules, `Transport` contract,
   redaction rule, `test-util` seam, cancel latency), per the per-crate docs
   policy.
7. **ADR text.** ADR-SAG-0026 HTTP half → Active (states ADR-SAG-0012 still
   holds); ADR-level Status → "Active (HTTP half, r-3); compose half
   Proposed (r-3b)"; index row ADR-SAG-0026 Status cell likewise. ADR-SAG-0006 r-3 amendment → Active. ADR-SAG-0020 r-3 amendment →
   Active, with its first bullet reworded at flip time so that it states
   only what r-3 enforces (below). ADR-SAG-0017 r-3 amendment (published library set gains
   `sc-agent-http`, publish order) → Active. ADR-SAG-0007 r-3 amendment →
   Active. REQ-SAG-0505 and REQ-SAG-0130 r-3 bullets → Active.

   **ADR-SAG-0020 r-3 bullet 1, replacement wording** (plan review 4,
   pre-screen r-3 fix 1). The Active bullet must not name a mechanism that
   does not exist until wave 4. Replace the whole first bullet of
   "Amendment (Phase A.1, r-3)" with:

   > - (Active, r-3) A secret travels only as a `SecretHeader` holding
   >   `ResolvedApiKey` inside `sc_agent_http::HttpRequest`. The single read
   >   site is a **lint-enforced workspace convention, not a type seal**:
   >   `ResolvedApiKey::as_str` is listed in `clippy.toml`
   >   `disallowed-methods`; its one call site,
   >   `crates/sc-agent-http/src/ureq_transport.rs`, carries
   >   `#[expect(clippy::disallowed_methods, reason = ...)]`; workspace lints
   >   deny `allow_attributes` and `allow_attributes_without_reason`; every
   >   crate has `[lints] workspace = true`, checked locally by r-3's
   >   validation; and an `rg` check finds `disallowed_methods` in no file
   >   under `crates/` other than that call site. Crates outside the
   >   workspace are not constrained. Redaction is applied once, in
   >   `send_with_busy_retry`, to every error it returns, including errors a
   >   `Transport` impl returns; a caller of `Transport::send` directly
   >   bypasses it.

   The removed `source_scans` clause is not dropped: in the same edit r-3
   inserts, after the r-3 subsection, a new subsection
   "### Amendment (Phase A.1, r-3b)" with
   "**Status:** Proposed (Phase A.1) — lands in r-3b." and the bullet whose
   exact text is in sprint r-3b ("ADR text", ADR-SAG-0020 r-3b bullet), and
   adds "A.1 subsection (Proposed, r-3b)" to the ADR-SAG-0020 index row.
   r-3b owns that subsection and marker from then on.

   **Markers r-3 flips and markers it must not touch.** r-3 flips the
   "(Proposed, r-3)" markers in index rows 0006, 0007, 0017, 0020 and 0029,
   the HTTP-half parts of the ADR-SAG-0026 Status and Amends cells, and the
   ADR-SAG-0026 Status line. It must **not** flip: the r-3b markers in rows
   0010, 0011, 0013, 0016 and 0017 ("ADR-SAG-0026 (Proposed, r-3b)"), the
   compose-half part of the ADR-SAG-0026 row, the new ADR-SAG-0020 r-3b
   marker, the r-4 marker in rows 0016 and 0017 ("ADR-SAG-0023 (Proposed,
   r-4)", which r-4 flips), the ADR-SAG-0023 row, or ADR-SAG-0028.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0402 | No transport/retry/redaction code in `sc-jev` or `sc-jev-backends` (the deleted files are gone; both compile only against `sc-agent-http`); `cargo tree --depth 1` of both has no `ureq`; loopback tests for `UreqTransport` (https-only, no redirect, 8 MiB streaming cap, agent reused) via `Endpoint::loopback_http`; `test_ureq_agent_config_values` |
| adr:ADR-SAG-0026 | HTTP half Active; ADR-level Status line and index row Status cell read exactly "Active (HTTP half, r-3); compose half Proposed (r-3b)"; r-3 asserts nothing about the compose half (no r-3 deliverable or test relies on `sc-agent-compose` or on any compose-half bullet) |
| adr:ADR-SAG-0020 | r-3 subsection Active with bullet 1 reading as the replacement wording above (no `source_scans` clause); the ADR-SAG-0020 r-3b subsection exists with Status Proposed and the index row carries "(Proposed, r-3b)"; local `rg` read-site check; existing leak tests moved into `sc-agent-http`; `test_transport_error_with_secret_is_redacted` |
| adr:ADR-SAG-0006, 0017 | r-3 subsections Active; the r-3b and r-4 markers in rows 0016 and 0017 unchanged |
| adr:ADR-SAG-0009, 0016, 0019 | Regression: `with_default_transport(key)` keeps the r-1 signature and the r-1 `FACTORIES` dispatch is unchanged (0009, `cargo test -p sc-jev-check --lib -- registry::tests::test_every_backend_has_factory --exact`); `sc-agent-http` reaches `sc-agent-types` with feature `secret` only and the `agent-types` manifest change is `allowed_dependents +sc-agent-http` only (0016); the wire builders and parsers stay in `sc-jev`/`sc-jev-backends` and `cargo test -p sc-jev-backends --test llm` is green (0019) |
| boundary:agent-http | Manifest enforced by `just lint` (output attached); public-API `cargo doc` (with deps) grep finds no `ureq/<kind>.` href or `ureq::` title, and the PR records the canary run in which it failed; `test_release_build_refuses_http`; retry tests; `test-util` absent from `cargo tree -p sc-agent-http -e normal,features`; `sc-agent-types` reached with feature `secret` only |
| req:REQ-SAG-0505 (amended) | `cargo test -p sc-agent-http --features test-util --test retry -- <name> --exact` prints `1 passed` for `test_busy_policy_numbers`, `test_missing_and_malformed_retry_after_back_off_exponentially`, `test_retry_after_grammar`, `test_retry_sleep_capped_by_budget`, `test_connect_failure_returns_immediately_retryable`, `test_exhaustion_reason_distinguishable` (the decided numbers) |
| req:REQ-SAG-0130 (amended, r-3 bullet) | `release/sc-publish-consumer-input.json` lists `sc-agent-http` with `publish_order: 3`; the `cargo publish --dry-run` below packages `sc-agent-http` third and exits 0 |
| boundary:jev | Regression: REQ-SAG-0110 / REQ-SAG-0400 / REQ-SAG-0401 `cargo test -p sc-jev --test jev_api` green (every Jev operation still in `sc-jev`, ADR-SAG-0012) |
| boundary:jev-backends | Regression: REQ-SAG-0500/0501/0502/0504/0505 `cargo test -p sc-jev-backends --test llm` green against the decided numbers |
| boundary:errors | Regression: NFR-SAG-0001 `cargo test -p sc-agent-types --test errors -- test_error_response_stderr_golden --exact` (network errors keep the r-1 field rule) |
| boundary:release | Regression: REQ-SAG-0130/0131/0132 `bash .github/scripts/run-consumer-preflight.sh` and the publish dry-run below with `sc-agent-http` in order; NFR-SAG-0004 `bash .github/scripts/check-exact-pins.sh` |
| adr:ADR-SAG-0007 | r-3 subsection Active; text matches `BusyPolicy::default()`, the Retry-After grammar and the cancel-latency statement |

## Required validation

```bash
just lint
just test
cargo test -p sc-agent-http --features test-util --test retry -- test_busy_policy_numbers --exact
cargo test -p sc-agent-http --features test-util --test retry -- test_missing_and_malformed_retry_after_back_off_exponentially --exact
cargo test -p sc-agent-http --features test-util --test retry -- test_retry_sleep_capped_by_budget --exact
cargo test -p sc-agent-http --features test-util --test retry -- test_connect_failure_returns_immediately_retryable --exact
cargo test -p sc-agent-http --features test-util --test retry -- test_cancel_interrupts_sleep --exact
cargo test -p sc-agent-http --features test-util --test retry -- test_no_retry_after_body_sent --exact
cargo test -p sc-agent-http --features test-util --test retry -- test_retry_after_grammar --exact
cargo test -p sc-agent-http --features test-util --test retry -- test_transport_error_with_secret_is_redacted --exact
cargo test -p sc-agent-http --features test-util --test retry -- test_exhaustion_reason_distinguishable --exact
cargo test -p sc-agent-http --features test-util --test ureq
cargo test -p sc-agent-http --features test-util --test ureq -- test_release_build_refuses_http --exact
# `Endpoint::parse` is feature-independent, so running this test with test-util on still proves the
# refusal; the `--lib` unit test `ureq_transport::tests::test_endpoint_parse_refuses_http` runs without it.
cargo test -p sc-agent-http --lib -- ureq_transport::tests::test_endpoint_parse_refuses_http --exact
cargo test -p sc-agent-http --lib -- ureq_transport::tests::test_ureq_agent_config_values --exact
cargo doc -p sc-agent-http && ! grep -rlE 'ureq/(struct|enum|trait|type|fn|union)\.|title="(struct|enum|trait|type|fn|union) ureq::' target/doc/sc_agent_http/
test -z "$(rg -l 'disallowed_methods' crates/ | grep -v '^crates/sc-agent-http/src/ureq_transport.rs$')"
! rg -n '#!?\[allow\(' crates/
grep -q '^\[lints\]' crates/sc-agent-http/Cargo.toml && grep -A1 '^\[lints\]' crates/sc-agent-http/Cargo.toml | grep -q 'workspace = true'
cargo test -p sc-jev
cargo test -p sc-jev --test jev_api
cargo test -p sc-jev-backends
cargo test -p sc-jev-backends --test llm
cargo test -p sc-agent-types --test errors -- test_error_response_stderr_golden --exact
cargo clippy --workspace --all-targets -- -D warnings
test -z "$(cargo tree -p sc-jev -e normal --depth 1 | grep -E '(^|[^[:alnum:]_-])ureq v[0-9]')"
test -z "$(cargo tree -p sc-jev-backends -e normal --depth 1 | grep -E '(^|[^[:alnum:]_-])ureq v[0-9]')"
test -z "$(cargo tree -p sc-agent-http -e normal | grep -E '(^|[^[:alnum:]_-])(sc-observability|pyo3|tree-sitter|sc-agent-secrets|sc-jev|sc-jev-backends|sc-jev-check) v[0-9]')"
test -z "$(cargo tree -p sc-agent-http -e normal,features | grep -F '"test-util"')"
test -z "$(cargo tree -p sc-agent-http -e normal,features -i sc-agent-types | grep -E 'sc-agent-types feature "(judge|obs|git|default)"')"
bash .github/scripts/check-exact-pins.sh
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-agent-http -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## This sprint does not close

Backend-side `input_sha256` recovery and the `WireFault` hook (r-2b, which
consumes the final `HttpRequest.body` built here); async transport and
provider-agent request types (Phase B); D9 envelope (r-5); CI `cargo tree`
script rows, the CI public-API check and the workspace-wide source scans
(r-3b); a dependency-advisory (`cargo deny`/`cargo audit`) CI step (bead
`agt-ci-advisory-scan`, not an A.1 decision).

## Implementation notes

Developer validation at the D7 head (every "Required validation" line plus
the workspace gates: `cargo clippy --workspace --all-targets --all-features`,
`cargo test --workspace`, `cargo test -p sc-agent-http --features test-util`,
`cargo test -p sc-agent-types --features git,test-util`,
`cargo test -p sc-jev-check --features test-util`,
`RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`,
`cargo hack --each-feature test -p sc-treesitter` and `-p sc-agent-http`,
`cargo-tree-gates.sh`, `just test-python`, after `cargo fetch --locked`):
all pass; each named test line prints `1 passed`. `just lint` boundary output:
`sc-lint-boundary: pass (0 findings)`.

**Open row (status in-review).** `install.py` was not re-run: the dev
session was not permitted to clone and execute the sc-publish kit. The
consumer input lists `sc-agent-http` with `publish_order: 3` (later crates
shifted), but the kit-rendered `release/publish-artifacts.toml` still has the
old order, so `scripts/sc-publish-dry-run.sh` (`kit` phase,
`install.py --dry-run`) will report drift until someone runs the documented
install (`docs/development/sc-publish.md` "Install") and commits the render.
No kit-written file was hand-edited. `run-consumer-preflight.sh`,
`check-exact-pins.sh` and the `cargo publish --dry-run --locked` line pass.
Cargo packages a multi-`-p` dry run in its own dependency order
(`sc-agent-types`, `sc-agent-http`, `sc-agent-secrets`, …): `sc-agent-http`
depends only on `sc-agent-types`, so cargo places it second; the
`publish_order` the kit uses is the consumer input's.

**Canaries.**
- Public-API check: a throwaway `pub fn _canary() -> ureq::Agent` in
  `lib.rs` made the check fail on the **title** pattern only
  (`target/doc/sc_agent_http/fn._canary.html`,
  `title="struct ureq::agent::Agent"`). The href is
  `../ureq/agent/struct.Agent.html`, so the href alternative
  `ureq/(struct|…)\.` does not match types in ureq submodules; r-3b may
  widen it to `ureq/([a-z0-9_]+/)*(struct|…)\.` when it copies the command
  into CI. Canary removed; the check passes.
- Lints: a temporary `#[allow(clippy::disallowed_methods)]` plus a second
  `ResolvedApiKey::as_str` call in `sc-jev` failed `cargo clippy -D warnings`
  with `allow_attributes`, `allow_attributes_without_reason` and
  `disallowed_methods`.
- `sc-lint.toml` ureq record: adding `ureq` to `crates/sc-jev/Cargo.toml`
  gives `SCB-MANIFEST-004` ("Only sc-agent-http may depend on ureq …").
- Agent reuse: with a second `UreqTransport` instead of a clone,
  `test_agent_is_reused_across_sends` fails (the one-connection server never
  answers a second connection).

**Deviations**, with the requirement or ADR each touches:

| Deviation | Touches | Why |
| --- | --- | --- |
| Public `Timeouts` (`new`, `connect`, `request`, `DEFAULT_*`, default 10 s / 120 s) and `HeaderName` added to the facade, with `test_util` | REQ-SAG-0402, boundary:agent-http, ADR-SAG-0026 | `UreqTransport::new(Timeouts)` and `HeaderName` are named by D1 but not by the plan facade text |
| `SecretHeader::bearer(key)` besides `new(name, key)` | ADR-SAG-0020 r-3 | `Authorization: Bearer <key>` needs a prefix; `new` sends the key verbatim (`x-api-key`) |
| `SecretHeader::matches` is implemented in `ureq_transport.rs` through the one `expose` fn | ADR-SAG-0020 r-3 | keeps a single `as_str` call site and a single `#[expect]` |
| `UreqTransport::loopback_for_tests(Timeouts)` takes timeouts | test-util seam | the loopback timeout test needs a short request timeout; production constructor unchanged |
| Exhaustion details: `HTTP <s> busy; N sends exhausted` / `HTTP <s> busy after N sends; retry budget exhausted`; backends prefix their name (`Jev …`, `luna …`) with `JevError::map_detail` | REQ-SAG-0505, ADR-SAG-0007 r-3 | RSH: distinguishable reasons; the old "retry attempts exhausted" wording in `jev_api`/`llm` tests updated |
| Malformed Retry-After now backs off (1, 2, 4 s) instead of waiting 30 s; `test_llm_busy_retry_same_body` expects 30, 1, 2 s | REQ-SAG-0505 (decided) | owner decision (b) |
| Removed `with_timeouts`, `with_retry_sleeper`, `JevClient::with_retry_delay`, `RetrySleeper`, `RetryDelay`, `DEFAULT_CONNECT_TIMEOUT` / `DEFAULT_REQUEST_TIMEOUT`; added `with_busy_policy`, `with_cancel` on every backend | ADR-SAG-0006, ADR-SAG-0007 ("configurable … for tests") | timeouts are transport construction state (`new(key, UreqTransport::new(Timeouts::new(..)))`); supersedes a-10 ARCHC-018's `with_timeouts`; `test_with_timeouts_reaches_every_request` replaced by `test_timeouts_are_transport_construction_state` |
| `JevClient<T: Transport = UreqTransport>` is generic too; `with_config` keeps `jev.toml` `attempts` (packaged 4 = `BusyPolicy::default()`, test `packaged_attempts_match_the_shared_busy_policy`); `JevClientConfig::endpoint()` returns `&Endpoint`, validated in `try_new` | REQ-SAG-0400/0401, ADR-SAG-0009 (registry unchanged) | one transport model for every backend |
| `User-Agent` is `sc-agent-http/<ver>` for every backend (was `sc-jev/…`, `sc-jev-backends/…`) | REQ-SAG-0402 | one agent configuration |
| Also deleted: both `retry_policy.rs` and `test_server.rs` copies and the `shared_copies` test; loopback tests moved to `sc-agent-http --test ureq` (`tests/support/mod.rs`); the sc-jev client-over-loopback test is covered by `jev_api` (scripted) plus `--test ureq` | REQ-SAG-0402 | the copies existed only until D5 |
| Backends redact non-2xx snippets and reply-parse errors with `sc_agent_http::redact` (no redaction code of their own) | NFR-SAG-0002, ADR-SAG-0020 r-3 | a response body may echo the key; `test_llm_non_2xx_detail_is_redacted` |
| `agent-http.toml`: no `sc-agent-compose` forbidden edge; header comment added | boundary:agent-http | sc-lint 0.8.0 rejects an edge to a non-member (`CLI.CONFIG_ERROR`); r-3b adds it with the crate |
| `jev.toml` manifest lists `serde` (a direct dependency missing before) and both backend manifests add a `ureq` forbidden edge | boundary:jev, jev-backends | manifest completeness; the ureq record enforces it |
| `Endpoint::parse` failures are `CONFIG_INVALID`; the production transport refuses a non-https endpoint as `NETWORK_ERROR` kind `Tls` | REQ-SAG-0402 | no new error ids (planner resolution 5) |
| Frontmatter `adrs` adds ADR-SAG-0010 | ADR-SAG-0010 | the ureq ownership record's "stays HTTP-free" rule |

**Files outside `owned_paths`:** `Cargo.lock` (regenerated by cargo) and this
sprint doc (status, `adrs`, these notes).
