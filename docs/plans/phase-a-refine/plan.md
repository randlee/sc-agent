---
adrs:
  - ADR-SAG-0005
  - ADR-SAG-0006
  - ADR-SAG-0007
  - ADR-SAG-0008
  - ADR-SAG-0009
  - ADR-SAG-0010
  - ADR-SAG-0016
  - ADR-SAG-0017
  - ADR-SAG-0019
  - ADR-SAG-0020
  - ADR-SAG-0021
  - ADR-SAG-0022
  - ADR-SAG-0023
  - ADR-SAG-0024
  - ADR-SAG-0025
  - ADR-SAG-0026
  - ADR-SAG-0027
---

# Phase A.1 — architectural refinement

Status: **proposed** — plan review pending.

Phase id: `r`. Plan branch: `plan/phase-a-refine`. Integration branch:
`integrate/phase-a` (A.1 lands on the Phase A integration branch before
`integrate → develop`). Beads epic: `agt-phase-a-refine` (create at kickoff).
Sprint doc shape follows `docs/development/sprint-planning-guidelines.md` and
the Phase A sprint docs.

## Intent

Phase A delivered a working Jev check client, but its engine has four entry
points, its consumers duplicate orchestration, its backend set and error
taxonomy are closed, two HTTP stacks have drifted, and two guarantees
(equal-input, the `{path}` producer contract) are hollow. Phase A.1 makes
Phase A a foundation that Phase B (API-direct agents, MCP server) can extend
with **minor-version** changes only: one open types break before the first
crates.io publish, one engine API, one HTTP layer, one composition crate, and
honest multi-backend semantics. Decisions are D1–D13 in `PHASE-A-REVIEW.md`
§4, all accepted 2026-11-06 (§7); where reviewers disagreed, §4 wins and is
cited per sprint.

## Preconditions

- Both a-10 routine tracks (`sprint/a-10-phase-end-hardening`,
  `sprint/a-10-docs-ci-boundaries`) merged to `integrate/phase-a`. Their items
  (PHASE-A-REVIEW §3) are **not** re-planned here; A.1 builds on them
  (e.g. `impl FromStr for BackendId`, unified retry policy, `ResolvedApiKey::try_new`,
  consumer CI workflow, full boundary manifests).

## Cluster → sprint mapping

| Decision(s) | Sprint | Title |
| --- | --- | --- |
| D1, D6, D10-break, D7 (types part) | r-1 | sc-agent-types break |
| D2 (engine), D4, D11, D13 (code) | r-2 | engine shape |
| D3 | r-2b | multi-backend semantics + equal-input |
| D5 | r-3 | `sc-agent-http` shared HTTP crate |
| D2 (compose), D7 (workspace-tests), D12 | r-3b | composition crate, workspace tests, clap |
| D8, D13 (ADR-SAG-0005 wording) | r-4 | pin/allocator/binaries ADR + manifests |
| D9 | r-5 | Jev wire envelope (**blocked**) |

r-2 was split (D3 → r-2b) and r-3 was split (compose/tests/clap → r-3b) for
size and to keep owned paths disjoint within each wave.

## Wave table

| Track | Wave | Sprint | `closure_type` | `target_boundary` | `must_follow` | `parallel_safe` |
| --- | --- | --- | --- | --- | --- | --- |
| foundation | 1 | r-1 | contract | agent-types | a-10 | [r-4] |
| release | 1 | r-4 | contract | release | a-10 | [r-1] |
| runtime | 2 | r-2 | boundary | jev-check | r-1 | [r-3] |
| http | 2 | r-3 | boundary | agent-http | r-1, r-4 | [r-2] |
| runtime | 3 | r-2b | boundary | jev-check | r-2, r-3 | [r-3b] |
| integration | 3 | r-3b | integration | agent-compose | r-2, r-3, r-4 | [r-2b] |
| jev | — | r-5 | boundary | jev | r-2b, r-3, vendor answer | [] |

**Why r-4 runs in wave 1 alongside r-1:** r-4 is ADR text plus manifests
(root `[workspace.dependencies]`, the `cli` feature on the two binary crates,
consumer input channel coverage). Its owned paths are disjoint from r-1's
(`crates/sc-agent-types/**` plus `src/**` call sites, excluding both
`main.rs`). Landing it first gives r-3/r-3b a settled root `Cargo.toml` and
pin policy before they add workspace members and published crates.

**Critical path:** r-1 → r-2 → r-2b (3 sprints); r-1 → r-3 → r-3b ties.
**Width:** 2. **Sprint count:** 6 + 1 blocked.

### Must-follow artifacts

| Edge | Artifact consumed |
| --- | --- |
| r-2 → r-1 | `BackendCapabilities`, `BACKENDS`, typed `JevError`, `JevError::from_error_response`, `schema::check::InputSpec` |
| r-3 → r-1 | `JevError::Network { kind }`, `retryable()`, `ResolvedApiKey` |
| r-3 → r-4 | root `Cargo.toml` pin policy (adds `sc-agent-http` member) |
| r-2b → r-2 | `prepare`/`run`, `RunConfig`, `RunResponse`, `FileSessionStore` |
| r-2b → r-3 | backend crates' wire builders now sit on `sc-agent-http` (no transport churn under r-2b) |
| r-3b → r-2 | `RunResponse::to_envelope_data`, `discover_repo_root` |
| r-3b → r-3 | final crate graph for publish order |
| r-3b → r-4 | `cli` feature on `sc-ai` / `sc-treesitter` (clap rides it) |
| r-5 → r-2b, r-3 | `input_sha256` reporting; `sc-jev` as wire-format crate |

## REQ closure (one owning sprint per `req:` root)

| Sprint | `req:` roots |
| --- | --- |
| r-1 | REQ-SAG-0105, REQ-SAG-0106, REQ-SAG-0107, REQ-SAG-0108 |
| r-2 | REQ-SAG-0607, REQ-SAG-0620, REQ-SAG-0312, REQ-SAG-0313, REQ-SAG-0314 |
| r-2b | REQ-SAG-0604, REQ-SAG-0605, REQ-SAG-0606 |
| r-3 | REQ-SAG-0402 |
| r-3b | REQ-SAG-0109, REQ-SAG-0712 |
| r-4 | REQ-SAG-0133, REQ-SAG-0134, NFR-SAG-0004 |
| r-5 (blocked) | REQ-SAG-0403 |

Existing REQ rows are not re-closed; A.1 sprints cite them as regression
`boundary:` rows only.

## ADR ownership (each amendment bullet / new ADR has exactly one landing sprint)

| ADR | Change | Landing sprint |
| --- | --- | --- |
| ADR-SAG-0005 | narrowed to the four consumer extractor kinds; assembly entry named by ADR-SAG-0024 | r-4 |
| ADR-SAG-0005 | canonical-only schema + Jev JSON envelope (D9) | r-5 (blocked) |
| ADR-SAG-0006 | transports live in `sc-agent-http` | r-3 |
| ADR-SAG-0007 | stated retry numbers | r-4 |
| ADR-SAG-0008 | independent default; `chain_context` explicit | r-2b |
| ADR-SAG-0009 | registration = catalog row + capabilities | r-1 |
| ADR-SAG-0010 | `sc-ai` depends on `sc-agent-compose`; clap | r-3b |
| ADR-SAG-0016 | version text `=1.5.0` | r-4 |
| ADR-SAG-0016 | `sc-observability-types` dropped from types (`LogRejected`) | r-1 |
| ADR-SAG-0016 | "no extra crate" → `sc-agent-compose` hosts adapter | r-3b |
| ADR-SAG-0017 | two released binaries + per-channel coverage | r-4 |
| ADR-SAG-0019 | `RunOptions` text → `RunConfig` | r-2 |
| ADR-SAG-0019 | equal-input from backend-reported digest | r-2b |
| ADR-SAG-0020 | one sealing model; infallible constructors; secrets `test-util` gone | r-3 |
| ADR-SAG-0022 | scripted registry via `RunConfig::registry` | r-2 |
| Rust API patterns | Extractor sealing note removed (RBP-F004) | r-2 |
| ADR-SAG-0023 (new) | pins, allocator, released binaries | r-4 |
| ADR-SAG-0024 (new) | engine entry point + `RunResponse` | r-2 |
| ADR-SAG-0025 (new) | multi-backend + equal-input | r-2b |
| ADR-SAG-0026 (new) | `sc-agent-http` + `sc-agent-compose` | r-3b (r-3 proves the HTTP half via ADR-SAG-0006/0020) |
| ADR-SAG-0027 (new) | error taxonomy + extensibility | r-1 |

An amendment subsection flips from `Proposed` to `Active` when its last
bullet's landing sprint merges. ADR-SAG-0021 per-root path precedence is
unchanged and must remain unchanged (r-3b acceptance proves it).

## Phase acceptance

Run on `integrate/phase-a` HEAD after r-1…r-4 (and r-2b, r-3b) merge:

```bash
just lint
just test
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test -p workspace-tests -- --nocapture
cargo test -p sc-jev-check -- test_run_response_contract --exact
cargo test -p sc-jev-check -- test_partial_failure_keeps_answers --exact
cargo test -p sc-jev-check -- test_equal_false_when_backend_digest_differs --exact
cargo hack check -p sc-treesitter --feature-powerset --no-dev-deps
cargo build -p sc-treesitter --lib --no-default-features
cargo build -p sc-ai --lib --no-default-features
test -z "$(cargo tree -p sc-agent-types -e normal | grep sc-observability)"
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
uv run --directory tests/fixtures/jev-python pytest -q
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-agent-http -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-agent-compose -p sc-treesitter -p sc-ai
```

Expected: all exit 0, and the consumer CI workflow (a-10) is green on ubuntu,
macos and windows. Then re-run the phase-ending six-reviewer gate.

## This phase does not close

- **D9** (r-5): blocked until the TypeSafe Jev API's accepted question shape is
  confirmed. The `"model"` byte-splice remains a known limitation until then.
- Phase B features: API-direct agents (REQ-SAG-0900/0901), MCP server,
  async/prepare-send split of `JudgeBackend`, typed `JudgeMeta` (D10 deferral),
  a second `SessionStore` implementation, and a crate split of `sc-agent-types`.
- `agt-luna-live-count` (Luna live `equal`), still open from Phase A.
- New stable error codes in `errors.json` (ADR-SAG-0027 keeps the existing codes).

## Sprint index

| Sprint | Doc |
| --- | --- |
| r-1 | [`sprint-r-1-types-break.md`](sprint-r-1-types-break.md) |
| r-2 | [`sprint-r-2-engine-shape.md`](sprint-r-2-engine-shape.md) |
| r-2b | [`sprint-r-2b-multi-backend-semantics.md`](sprint-r-2b-multi-backend-semantics.md) |
| r-3 | [`sprint-r-3-shared-http.md`](sprint-r-3-shared-http.md) |
| r-3b | [`sprint-r-3b-compose-and-workspace-tests.md`](sprint-r-3b-compose-and-workspace-tests.md) |
| r-4 | [`sprint-r-4-adrs-and-manifests.md`](sprint-r-4-adrs-and-manifests.md) |
| r-5 | [`sprint-r-5-jev-wire-envelope.md`](sprint-r-5-jev-wire-envelope.md) (blocked) |

## Crate graph after A.1

```text
sc-ai (bin, feature cli: clap + mimalloc) ─┐
sc-jev-py (PyO3) ──────────────────────────┴── sc-agent-compose (= pin; obs adapter, path policy)
                                                 ├── sc-observability =1.5.0
                                                 └── sc-jev-check (prepare/run, RunResponse, discover_repo_root)
                                                       ├── sc-jev ──────────┐
                                                       ├── sc-jev-backends ─┴── sc-agent-http
                                                       ├── sc-agent-secrets
                                                       └── sc-agent-types (schema, catalog; no observability dep)
sc-treesitter (bin, feature cli) → sc-agent-types
crates/workspace-tests (publish = false) → all of the above (dev)
```
