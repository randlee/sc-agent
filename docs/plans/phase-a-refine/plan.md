---
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0003
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0006
  - ADR-SAG-0007
  - ADR-SAG-0008
  - ADR-SAG-0009
  - ADR-SAG-0010
  - ADR-SAG-0011
  - ADR-SAG-0012
  - ADR-SAG-0013
  - ADR-SAG-0014
  - ADR-SAG-0015
  - ADR-SAG-0016
  - ADR-SAG-0017
  - ADR-SAG-0018
  - ADR-SAG-0019
  - ADR-SAG-0020
  - ADR-SAG-0021
  - ADR-SAG-0022
  - ADR-SAG-0023
  - ADR-SAG-0024
  - ADR-SAG-0025
  - ADR-SAG-0026
  - ADR-SAG-0027
  - ADR-SAG-0028
---

# Phase A.1 — architectural refinement

Status: **proposed** — plan review round 3 (final) applied. Dispositions:
round 1 [`plan-review-1-dispositions.md`](plan-review-1-dispositions.md),
round 2 [`plan-review-2-dispositions.md`](plan-review-2-dispositions.md).

Phase id: `r`. Plan branch: `plan/phase-a-refine`. Integration branch:
`integrate/phase-a` (A.1 lands on the Phase A integration branch before
`integrate → develop`). Beads epic: `agt-phase-a-refine` (create at kickoff).
Sprint doc shape follows `.claude/skills/plan-hardening/sprint-planning-guidelines.md`.

**Repository exception (REQ-QA-012, REQ-QA-R2-008):** the phase id is `r`
while the docs directory and plan branch are named `phase-a-refine`. A.1
refines Phase A rather than opening a new phase, so its documents sit beside
`docs/plans/phase-a/` under a descriptive name; sprint ids, branches and beads
use `r-N`. The exception is recorded, naming `req-qa` and `arch-qa`, in
`.claude/project/quality-policy.md` "Repository exceptions" by this plan PR.

## Intent

Phase A delivered a working Jev check client, but its engine has four entry
points, its consumers duplicate orchestration, its backend set and error
taxonomy are closed, two HTTP stacks have drifted, and two guarantees
(equal-input, the `{path}` producer contract) are hollow. Phase A.1 makes
Phase A a foundation that Phase B (API-direct agents, MCP server) can extend
with **minor-version** changes only: one types break before the first
crates.io publish, one engine API, one HTTP layer, one composition crate, and
honest multi-backend semantics. Decisions are D1–D13 in `PHASE-A-REVIEW.md`
§4, all accepted 2026-11-06 (§7). Where plan reviewers disagreed, the
critical-plan-reviewer's structural points and arch-qa's ADR sequencing win;
other design points are applied unless they contradict an accepted D decision.

## Preconditions

- Both a-10 routine tracks (`sprint/a-10-phase-end-hardening`,
  `sprint/a-10-docs-ci-boundaries`) merged to `integrate/phase-a`. Their items
  (PHASE-A-REVIEW §3) are **not** re-planned here; A.1 builds on them:
  `impl FromStr for BackendId`, unified retry numbers, `ResolvedApiKey::try_new`,
  private `PayloadBundle` fields, `env_clear` + allowlist for command
  extractors, canonicalized `starts_with`-checked regular-file worktree reads,
  `.github/workflows/sc-agent-ci.yml`, the `sc-lint.toml`
  `global_dependency_ownership` records, and option-B boundary manifests.
  Every A.1 path below is read against that tree.

## Conventions used by every sprint doc

- **Section-scoped ownership of the two index docs.** `docs/architecture.md`
  and `docs/requirements.md` are edited by every sprint that lands ADR or REQ
  text. Each sprint's `owned_paths` names them as
  `docs/architecture.md#<section>` / `docs/requirements.md#<row>`. Within a
  wave no section or row is owned twice. Every amendment bullet carries a
  marker `(Proposed, r-N)` that the owning sprint flips to `(Active, r-N)`;
  each amendment subsection lands in exactly one sprint and has one
  **Status** line.
- **ADR index table.** This plan PR writes every ADR-SAG-0023..0028 row of
  the `docs/architecture.md` ADR index as `Proposed (r-N)` and every
  ADR-level "Amended by" cell of ADR-SAG-0001..0022 that A.1 changes, with a
  `(Proposed, r-N)` marker per entry. A landing sprint owns only its own
  row's **Status** and **Amends** cells and its own markers in other rows'
  "Amended by" cells; each sprint names them in `owned_paths` as
  `docs/architecture.md#ADR index row ADR-SAG-00NN` /
  `#ADR index Amended-by marker (r-N) in ADR-SAG-00NN`. No sprint adds or
  removes rows.
- **Adjacent rows.** Two sprints of one wave that own adjacent rows of
  `docs/requirements.md` or the ADR index can produce a textual merge
  conflict without a semantic overlap. The second sprint of the wave to merge
  rebases and resolves it, keeping both rows' text verbatim.
- **Wave-1 merge order.** r-1 merges before r-4. r-1 deletes the last user
  and the root entry of `sc-observability-types`, and r-4's
  `check-exact-pins.sh` fails on that `=` entry, which is not on the
  ADR-SAG-0023 list; r-4 rebases onto r-1 and runs its validation on the
  rebased tree. Development runs in parallel.
- **Test seams: one mechanism.** A library hook that an integration test
  (`crates/<crate>/tests/*.rs`) needs is behind a non-default, additive
  `test-util` cargo feature of the crate that owns the hook. The crate's own
  integration tests enable it through a self-referencing dev-dependency
  (`<crate> = { path = ".", features = ["test-util"] }`); other crates' tests
  enable it through their dev-dependencies. `test-util` never appears in a
  normal-dependency edge: the `cargo tree` gate row `test-util` (below) and
  `workspace-tests --test test_util_off` prove it. Every boundary manifest
  of a crate with the feature names it in `notes`. A test that cannot use a
  `test-util` hook is a `--lib` unit test named as
  `cargo test -p <crate> --lib -- <module>::tests::<name> --exact`. No
  acceptance test in `tests/` depends on `cfg(test)`. Hooks by crate:
  `sc-agent-types` `test_util::MockJudgeBackend` (r-1);
  `sc-agent-http` `BusyPolicy::with_test_hooks` (clock, sleeper, jitter) and
  `Endpoint::loopback_http` (r-3); `sc-jev-check` `RunResponse::test_builder`
  and `register_scripted_backends` (r-2); `sc-jev` / `sc-jev-backends`
  `WireFault` wire-builder hook (r-2b).
- **Lint hygiene.** Every workspace crate, existing and new, has
  `[lints] workspace = true` and no per-crate `[lints]` override (a-10 added
  it to the eight existing crates; r-3 and r-3b add it to the three new
  ones). No `#[allow(..)]` or `#![allow(..)]` exists under `crates/` from r-1
  on; suppressions are `#[expect(.., reason = "..")]`. Each sprint's
  validation runs `! rg -n '#!?\[allow\(' crates/`.
- **Generated files.** `Cargo.lock` is regenerated, never hand-edited; the
  second sprint of a wave to merge re-runs `cargo check --workspace` and
  commits the result. Files rendered by sc-publish `install.py` (`release/**`
  other than the consumer input, `.github/workflows/*-publish.yml`,
  `release*.yml`, `prerelease-archive.yml`, `sc-publish-dry-run.yml`) are
  owned by the sprint that edits `release/sc-publish-consumer-input.json` and
  re-runs `install.py`: r-4 (wave 1), r-3 (wave 2), r-3b (wave 4).
- **Validation honesty.** Every named test runs as
  `cargo test -p <crate> --test <target> -- <name> --exact` against a
  top-level `#[test] fn` in `crates/<crate>/tests/<target>.rs`, or as
  `cargo test -p <crate> --lib -- <module>::tests::<name> --exact`. Each such
  line must print `1 passed`; the scrum-master rejects `0 passed`.
  `cargo publish --dry-run` always uses the full ordered package list of the
  crates that exist at that point. The `just lint` boundary output
  (`sc-lint lint sc-boundary`) is attached to the QA input of r-3, r-3b, r-4.
- **Publish order** (crates.io): `sc-agent-types`, `sc-agent-secrets`,
  `sc-agent-http` (from r-3), `sc-jev`, `sc-jev-backends`, `sc-jev-check`,
  `sc-agent-compose` (from r-3b), `sc-treesitter`, `sc-ai`. `sc-jev-py` ships
  to PyPI only. `workspace-tests` is `publish = false`.

## Boundary map

Read from `boundaries/*/*.toml` and the crate manifests after a-10. Every
sprint's `owned_paths` is derived from this table; a sprint owns a row's
paths only in the wave named.

| Crate / file | Manifest | Contract delta in A.1 | Owning sprint (wave) |
| --- | --- | --- | --- |
| `sc-agent-types` | `boundaries/sc-agent-types/agent-types.toml` | The whole types break (D1, D6, D7, D10-break, D11 types part): `BackendDescriptor` static facts + `BACKENDS`; provided `capabilities()`; `BackendCapabilities` ctor + private fields; explicit `JevError` variants (constructor per struct-like variant) + `ProducerError`; `ErrorCode`; `ScAgentErrorResponse` `schema_version`/`retryable`/`details` with a per-field rule (`from_jev_error` name kept); `NetworkErrorKind`; `retryable()`; `RepoRelativePath`, `QuestionId`, `ChoiceId`, `BlobSha`, `Operation`, `BackendSet`, `CancelToken` (always-on core); typed `answers`; `JudgeOutcome.input_sha256: Option`; `Attestation` enum; `MultiBackendMode`; `PayloadBundle::encode`; private `CanonicalJudgePayload` fields; `schema::subjects` + `SubjectsBuilder`, `SubjectsDocument` as the one validated type; `LogRejected`; `git` module; features `judge`/`secret`/`obs` (default) and `git` (non-default), `test-util`; `ResolvedApiKey` on `Arc<str>`, hidden `new` deleted; the a-10 `#[allow]` in `ids.rs` → `#[expect]`. Manifest: facade, features, `-sc-observability-types` (r-1); `allowed_dependents` `+sc-agent-http` (r-3), `+sc-agent-compose` and (if the r-3b spike requires it) `+workspace-tests` (r-3b) | r-1 (1); manifest r-3 (2), r-3b (4); `judge.rs` r-5 (5, blocked) |
| `sc-agent-secrets` | `boundaries/sc-agent-secrets/agent-secrets.toml` | Call sites only (`ResolvedApiKey` shape). No manifest change | r-1 (1) |
| `sc-agent-http` (new, published) | `boundaries/sc-agent-http/agent-http.toml` (new) | Created: `HttpRequest` (`body: Arc<[u8]>`), `HttpResponse`, `Method`, `Endpoint`, `SecretHeader`, `Transport`, `UreqTransport`, `BusyPolicy`, `send_with_busy_retry`, `redact`. Manifest text below | r-3 (2) |
| `sc-jev` | `boundaries/sc-jev/jev.toml` | r-1: infallible `JevClient::new`, `with_default_transport`, descriptor-read byte cap, typed answers, `#[allow]` in `client/https.rs` → `#[expect]`. r-3: transport/retry/https deleted, built on `sc-agent-http`; dependencies `-ureq +sc-agent-http`. r-2b: `input_sha256` recovered from the final body; `test-util` feature with the `WireFault` hook; real-wire-builder digest test. r-5: envelope | r-1 (1), r-3 (2), r-2b (3), r-5 (5) |
| `sc-jev-backends` | `boundaries/sc-jev-backends/jev-backends.toml` | Same split as `sc-jev` (r-1 ctor/limit/answers and the `transport.rs` `#[allow]`; r-3 transport; r-2b digest, `test-util`/`WireFault`, real-wire-builder digest test; r-5 validate) | r-1 (1), r-3 (2), r-2b (3), r-5 (5) |
| `sc-jev-check` | `boundaries/sc-jev-check/jev-check.toml` | r-1: call sites, session document v2, `InputSpec` moved in unchanged from types, backend factory table. r-2: `Cargo.toml` (`sc-agent-types` `features = ["git"]`, `test-util` self dev-dependency), `prepare`/`run`/`discover_repo_root`, `RunConfig`, `PreparedRun`, `CheckSource`, `CallerRequest`, `RunResponse` (incl. `warnings`) + `Envelope`, `RunResponse::test_builder` (`test-util`), `FileSessionStore`, `BackendRegistry::{from_env, register}`, `InputSpec::Command.path`; facade rewritten. r-2b: run-loop semantics, `registry/scripted.rs` digest reporting. r-3b: manifest `allowed_dependents` `+sc-agent-compose` (and `+workspace-tests` if the spike requires it) | r-1 (1), r-2 (2), r-2b (3), r-3b manifest (4) |
| `sc-agent-compose` (new, published) | `boundaries/sc-agent-compose/agent-compose.toml` (new) | Created: `obs`, `paths::PathPolicy`, `compose::{run_from, ask_from}`, `ComposeInputs`. Manifest text below | r-3b (4) |
| `sc-ai` | `boundaries/sc-ai/sc-ai-cli.toml` | r-1: call sites incl. `main.rs` (error-variant migration; `command.rs` id equality → descriptor fact). r-4: `Cargo.toml` only — `cli` feature, optional mimalloc + clap, `[[bin]] required-features = ["cli"]` (the unconditional `#[global_allocator]` then compiles only with `cli`; `main.rs` unchanged). r-2: engine migration. r-3b: clap parser, compose, partial-failure stdout/stderr/exit 2, `--chain-context`; dependencies `-sc-observability +sc-agent-compose` | r-1 (1), r-4 (1, `Cargo.toml` only), r-2 (2), r-3b (4) |
| `sc-jev-py` | `boundaries/sc-jev-py/jev-py.toml` | r-1: call sites. r-2: engine migration. r-3b: compose, `chain_context` kwarg, `JevError.response`, `.pyi`; dependencies `-sc-observability +sc-agent-compose`; `jev_cross_impl` `[[test]]` deleted | r-1 (1), r-2 (2), r-3b (4) |
| `sc-treesitter` | `boundaries/sc-treesitter/treesitter.toml` | r-1: `SubjectsBuilder`, producer-neutral variants — owns `src/grammar.rs`, `src/query.rs`, `src/git_io.rs`, `src/main.rs`, `src/render.rs`, `src/extract.rs`, `src/lib.rs` for the error-variant migration. r-4: `Cargo.toml` only (`cli` feature, `[[bin]] required-features`). r-3c: clap parser (`main.rs`, `cli/`), `git` helper (`git_io.rs`), `sc-agent-types` `default-features = false`; dependencies `+clap` (optional) | r-1 (1), r-4 (1, `Cargo.toml` only), r-3c (2) |
| `workspace-tests` (new, `publish = false`) | `boundaries/workspace-tests/workspace-tests.toml` (new) | Created: targets `jev_cross_impl`, `e2e_treesitter_producer`, `consumer_parity`, `git_read_parity`, `source_scans` (secret read site, lints inheritance, git spawn sites, `#[allow]`), `test_util_off`; helper `tests/support/bin_path.rs`. Manifest text below | r-3b (4) |
| Root `Cargo.toml` | — | r-1: deletes the one `sc-observability-types` line in `[workspace.dependencies]` (section-scoped: that line only). r-4: every other `[workspace.dependencies]` change — caret pins, `clap` workspace dep, `mimalloc` caret, pin-policy comment. r-3: `sc-agent-http` member + dep, `[workspace.lints.clippy]` `allow_attributes = "deny"`, `allow_attributes_without_reason = "deny"`. r-3b: `sc-agent-compose`, `workspace-tests` members | r-1 (1, one line), r-4 (1), r-3 (2), r-3b (4) |
| `sc-lint.toml` | — | Ownership records, table below | r-4 (1), r-3 (2), r-3b (4) |
| `.github/workflows/sc-agent-ci.yml`, `.github/scripts/cargo-tree-gates.sh`, `.github/scripts/check-exact-pins.sh` | — | r-4: `--no-default-features` lib builds, mimalloc gates, pin check reading the ADR-SAG-0023 list, `cargo hack check -p sc-agent-types --feature-powerset`. r-3b: every remaining gate row (default and `--no-default-features`), the `test-util` row, the r-3c `sc-agent-types` feature row, the public-API (`cargo doc`) check, isolated per-library `cargo build -p <lib>` rows, `workspace-tests` job; pin check re-run | r-4 (1), r-3b (4) |
| `clippy.toml` | — | `disallowed-methods` for `ResolvedApiKey::as_str` (a-10) with the A.1 reason text | r-3 (2) |

**`vertical_rationale` for r-1:** a contract break of the shared types crate
cannot leave the workspace red, and nothing is published, so the sprint that
changes the types also updates every call site and every test it breaks
(`crates/*/src/**` call sites including both `main.rs` and every
`sc-treesitter` source file that constructs a `JevError` variant,
`crates/*/tests/**`). No behaviour above the contract changes in r-1 except
where a type change forces it (session document version, descriptor-read byte
cap, `ScAgentErrorResponse` fields).

**`vertical_rationale` for r-2:** the four deleted entry points have callers
in `sc-ai` and `sc-jev-py`; D2 forbids shims, so the consumer call sites move
in the same commit. r-2 changes no consumer behaviour beyond the response
shape pinned by `test_run_response_contract`.

**`vertical_rationale` for r-3:** `sc-agent-http` does not exist yet; r-3
creates it and moves its only two consumers (`sc-jev`, `sc-jev-backends`)
onto it.

**`vertical_rationale` for r-3b:** integration sprint; it creates the
composition crate and the cross-crate test crate and moves both composition
roots onto them.

### Ownership records in `sc-lint.toml`

| Dependency | Allowed manifests / source roots | Owning sprint |
| --- | --- | --- |
| `ureq` | `crates/sc-agent-http` only (replaces the a-10 `sc-jev` + `sc-jev-backends` record) | r-3 |
| `pyo3` | `crates/sc-jev-py` (a-10 record, unchanged) | a-10 |
| `tree-sitter`, `tree-sitter-python`, `tree-sitter-rust` | `crates/sc-treesitter` (a-10 records, unchanged) | a-10 |
| `sc-observability` | `crates/sc-agent-compose` only | r-3b |
| `sc-observability-types` | record deleted (no user after r-1) | r-4 |
| `mimalloc` | `crates/sc-ai/Cargo.toml`, `crates/sc-treesitter/Cargo.toml` (optional, `cli`) | r-4 |
| `clap` | `crates/sc-ai/Cargo.toml`, `crates/sc-treesitter/Cargo.toml` (optional, `cli`) | r-4 |

### `cargo tree` gates

`.github/scripts/cargo-tree-gates.sh` runs each row **twice**, with default
features and with `--no-default-features`, as
`cargo tree -p <crate> -e normal [--no-default-features]` (depth 1 where
noted), except rows marked *(lib)*, which run `--no-default-features` only
(the default `cli` feature of a binary crate legitimately links clap and
mimalloc). A row fails if a pattern matches. Patterns are word-bounded on the
package name and version so `sc-jev` does not match `sc-jev-check`:
`grep -E '(^|[^[:alnum:]_-])<name> v[0-9]'`.

| Crate | Must be absent | Added by |
| --- | --- | --- |
| `sc-agent-types` | `sc-observability`, `sc-observability-types`, `ureq`, `pyo3`, `tree-sitter` | r-1 (local check), r-3b (script) |
| `sc-agent-http` | `sc-observability`, `pyo3`, `tree-sitter`, `sc-agent-secrets`, `sc-jev`, `sc-jev-backends`, `sc-jev-check` | r-3 (local), r-3b (script) |
| `sc-jev`, `sc-jev-backends` | depth-1 `ureq`, `sc-observability`, `pyo3` | r-3 (local), r-3b (script) |
| `sc-jev-check` | depth-1 `ureq`, `sc-observability`, `pyo3`, `tree-sitter` | r-3 (local), r-3b (script) |
| `sc-agent-compose` | depth-1 `ureq`, `pyo3`, `tree-sitter`, `clap`, `mimalloc` | r-3b |
| `sc-ai` *(lib)* | `mimalloc`, `clap`; depth-1 `sc-observability`, `ureq` | r-4 (mimalloc), r-3b (rest) |
| `sc-treesitter` *(lib)* | `mimalloc`, `clap`, `sc-observability`; `-e features -i sc-agent-types`: features `judge`, `secret`, `obs`, `git` | r-4 (mimalloc), r-3c (local clap and features), r-3b (script) |
| `sc-jev-py` | depth-1 `sc-observability`, `ureq` | r-3b |
| `workspace-tests` | `pyo3`, `tree-sitter` (`-e normal,dev`) | r-3b |
| every workspace crate | `-e normal,features` (no dev edges): feature `test-util` of any workspace crate | r-3b |

### New boundary manifests

`boundaries/sc-agent-http/agent-http.toml` (r-3), full text:

```toml
boundary_id = "BOUNDARY-agent-http"
owner_package = "sc-agent-http"
owner_crate_path = "sc_agent_http"
name = "agent-http"

[public]
facade = "sc_agent_http::HttpRequest, sc_agent_http::HttpResponse, sc_agent_http::Method, sc_agent_http::Endpoint, sc_agent_http::SecretHeader, sc_agent_http::Transport, sc_agent_http::UreqTransport, sc_agent_http::BusyPolicy, sc_agent_http::send_with_busy_retry, sc_agent_http::redact"
notes = "Single HTTP layer and the single ResolvedApiKey::as_str call site (src/ureq_transport.rs, private module, #[expect(clippy::disallowed_methods, reason = ...)]). No ureq type in any pub signature (cargo doc check). Carries bytes only: Jev API semantics stay in sc-jev (ADR-SAG-0012). Depends on sc-agent-types with default-features = false, features = [secret]; jitter from std, URL parsing via ureq's re-exported http::Uri inside the private module (no rand, no url crate). Non-default feature test-util (BusyPolicy::with_test_hooks, Endpoint::loopback_http) is enabled only by dev-dependencies."

[implementation]
type = "marker"
module = "sc_agent_http"
visibility = "public"
constructor = "none"

[composition]
roots = ["request", "transport", "ureq_transport", "retry", "redact"]

[dependencies]
allowed_dependents = ["sc-jev", "sc-jev-backends"]
allowed_dependencies = ["sc-agent-types", "ureq"]
forbidden_edges = [
    { from = "sc-agent-http", to = "sc-agent-secrets" },
    { from = "sc-agent-http", to = "sc-jev" },
    { from = "sc-agent-http", to = "sc-jev-backends" },
    { from = "sc-agent-http", to = "sc-jev-check" },
    { from = "sc-agent-http", to = "sc-agent-compose" },
    { from = "sc-agent-http", to = "sc-observability" },
    { from = "sc-agent-http", to = "pyo3" },
    { from = "sc-agent-http", to = "tree-sitter" },
]

[references]
scope = "outside_owner_crate"
forbidden = []

[testing]
allowed_test_double_paths = ["crates/sc-agent-http/src/test_util.rs", "crates/sc-agent-http/tests/support/mod.rs"]
forbidden_test_bypasses = []

[enforcement]
lint_rules = ["SCB-DEPENDENCY-001", "SCB-DEPENDENCY-002", "SCB-DEPENDENCY-003", "SCB-DEPENDENCY-004", "SCB-MANIFEST-004"]
review_gates = []

[status]
state = "concrete_landed"
```

`boundaries/sc-agent-compose/agent-compose.toml` (r-3b). Same schema and the
same `[references]`, `[enforcement]`, `[status]` blocks; the fields that
differ:

```toml
boundary_id = "BOUNDARY-agent-compose"
owner_package = "sc-agent-compose"
owner_crate_path = "sc_agent_compose"
name = "agent-compose"

[public]
facade = "sc_agent_compose::obs::ObservabilityAdapter, sc_agent_compose::paths::PathPolicy, sc_agent_compose::compose::run_from, sc_agent_compose::compose::ask_from, sc_agent_compose::compose::ComposeInputs"
notes = "Composition-root support for sc-ai and sc-jev-py. Published as internal support, =-pinned by both, no semver promise of its own (owner decision 2026-11-06). The only crate that links sc-observability."

[composition]
roots = ["obs", "paths", "compose"]

[dependencies]
allowed_dependents = ["sc-ai", "sc-jev-py", "workspace-tests"]
allowed_dependencies = ["sc-agent-types", "sc-jev-check", "sc-observability", "serde_json"]
forbidden_edges = [
    { from = "sc-agent-compose", to = "sc-jev" },
    { from = "sc-agent-compose", to = "sc-jev-backends" },
    { from = "sc-agent-compose", to = "sc-agent-http" },
    { from = "sc-agent-compose", to = "sc-agent-secrets" },
    { from = "sc-agent-compose", to = "sc-treesitter" },
    { from = "sc-agent-compose", to = "ureq" },
    { from = "sc-agent-compose", to = "pyo3" },
    { from = "sc-agent-compose", to = "clap" },
    { from = "sc-agent-compose", to = "tree-sitter" },
]

[testing]
allowed_test_double_paths = []
forbidden_test_bypasses = []
```

`boundaries/workspace-tests/workspace-tests.toml` (r-3b). Same schema; the
fields that differ. `allowed_dependencies` is the exact workspace dev set.
How `sc-boundary` treats a dev-only workspace edge is not documented, so
r-3b's **first step** is a spike: a throwaway dev-only edge checked by
`sc-lint lint sc-boundary`, with the result recorded in
`boundaries/README.md`. If dev edges count, this text lands as written and
the three dependency manifests add `workspace-tests` to `allowed_dependents`.
If they are ignored, `allowed_dependencies` stays as written for review, no
dependent manifest gains `workspace-tests` (it would be a stale permission),
and the dev set is policed by the `cargo-tree-gates.sh` row
`cargo tree -p workspace-tests -e dev --depth 1`, which must list exactly
these three workspace crates. Third-party dev-deps (`serde_json`,
`tempfile`) are checked only by that row.

```toml
boundary_id = "BOUNDARY-workspace-tests"
owner_package = "workspace-tests"
owner_crate_path = "workspace_tests"
name = "workspace-tests"

[public]
facade = ""
notes = "publish = false (ADR-SAG-0028). Cross-crate proofs only: no normal dependencies and an empty src/lib.rs; dev-dependencies on sc-agent-types (features = [git]), sc-jev-check, sc-agent-compose, serde_json, tempfile. The bin-path helper is tests/support/bin_path.rs. sc-treesitter and sc-ai are driven as built binaries (cargo build --features cli --message-format=json), never linked, so pyo3 and tree-sitter stay out of this graph. Source scans read files and link nothing."

[dependencies]
allowed_dependents = []
allowed_dependencies = ["sc-agent-types", "sc-jev-check", "sc-agent-compose"]
forbidden_edges = [
    { from = "workspace-tests", to = "pyo3" },
    { from = "workspace-tests", to = "tree-sitter" },
    { from = "workspace-tests", to = "sc-treesitter" },
    { from = "workspace-tests", to = "sc-jev-py" },
    { from = "workspace-tests", to = "sc-ai" },
    { from = "workspace-tests", to = "sc-agent-http" },
    { from = "workspace-tests", to = "sc-jev" },
    { from = "workspace-tests", to = "sc-jev-backends" },
]
```

Edits to existing manifests: `sc-agent-types` facade (`schema::subjects`,
`git`, features, `test-util`), `-sc-observability-types` from its
dependency list (r-1), `allowed_dependents` `+sc-agent-http` (r-3),
`+sc-agent-compose`, `+workspace-tests` (r-3b); `sc-jev` / `sc-jev-backends`
`allowed_dependencies` `-ureq +sc-agent-http`, forbidden `ureq` direct is
enforced by the r-3 ownership record (r-3); `sc-jev-check` facade rewritten
(r-2), `allowed_dependents` `+sc-agent-compose`, `+workspace-tests` (r-3b);
`sc-ai` / `sc-jev-py` `allowed_dependencies` `-sc-observability
+sc-agent-compose`, forbidden `sc-observability`, `ureq` (r-3b), `sc-ai`
`+clap`, `+mimalloc` optional (r-3b; r-4 adds the dependency and the
`sc-lint.toml` record only); `sc-treesitter` `+clap` optional, `+mimalloc`
optional, facade and `git` note (r-3c). Every manifest of a crate with a
`test-util` feature names it in `notes` (the sprint that adds the feature).

## Sprints and waves

| Decision(s) | Sprint | Title | Size |
| --- | --- | --- | --- |
| D1, D6, D7 (types), D10-break, D11 (types), RBQA-F006 helper | r-1 | sc-agent-types break | XL |
| D8, D13 (ADR-SAG-0005 kinds sentence) | r-4 | pins, allocator, released binaries | M |
| D2 (engine), D4, D11 (engine), D13 (engine cap removal) | r-2 | engine shape | L |
| D5 | r-3 | `sc-agent-http` shared HTTP crate | L |
| D12 (sc-treesitter), RBQA-F006 (producer caller) | r-3c | sc-treesitter clap + git helper | M |
| D3 (engine + backend digest) | r-2b | multi-backend semantics | L |
| D2 (compose), D7 (workspace-tests), D12 (sc-ai), D3 (consumers) | r-3b | composition crate, workspace tests, consumers | XL |
| D9 | r-5 | Jev wire envelope (**blocked**) | M |

Sizes: M = 2–3 dev-days, L = 4–5, XL = 6–8. r-1 and r-3b are XL by honest
count (r-1 touches every crate's call sites and tests, both `main.rs` and the
`sc-treesitter` error sites; r-3b creates two crates, migrates two consumers,
ports a parser and adds the CI gates and source scans). Neither splits along
a boundary without re-creating the same-file conflicts this revision removes.
r-2b is L (re-rated in round 2, CPR-M104): beyond the run loop it adds the
`test-util` `WireFault` hook and real-wire-builder digest tests in both
backend crates and the scripted-backend digest.

### Wave table

| Track | Wave | Sprint | `closure_type` | `target_boundary` | `must_follow` | `parallel_safe` |
| --- | --- | --- | --- | --- | --- | --- |
| foundation | 1 | r-1 | contract | agent-types | a-10 | [r-4] |
| release | 1 | r-4 | contract | release | a-10 | [r-1] |
| runtime | 2 | r-2 | boundary | jev-check | r-1 | [r-3, r-3c] |
| http | 2 | r-3 | boundary | agent-http | r-1 | [r-2, r-3c] |
| producer | 2 | r-3c | boundary | treesitter | r-1, r-4 | [r-2, r-3] |
| runtime | 3 | r-2b | boundary | jev-check | r-2, r-3 | [] |
| integration | 4 | r-3b | integration | agent-compose | r-2b, r-3, r-3c | [] |
| jev | 5 | r-5 | boundary | jev | r-2b, r-3, r-3b, vendor answer | [] |

**Critical path:** r-1 → r-2 → r-2b → r-3b = **4 sprints** (5 including the
blocked r-5). **Width:** 3 (wave 2). **Sprint count:** 7 + 1 blocked.
**Effort:** r-1 XL + r-4 M + r-2 L + r-3 L + r-3c M + r-2b L + r-3b XL ≈
28–37 dev-days in total; the critical path (XL + L + L + XL) is ≈ 20–26
dev-days.

**Why the critical path is 4 (recorded reason).** r-3b is the integration
sprint of the runtime track: the CLI's partial-failure output (exit 2, stdout
envelope, stderr first failure), `--chain-context`, the Python `chain_context`
kwarg and `JevError.response` are feature-level behaviour that passes through
r-2b's run loop. r-2b cannot be hoisted into r-2 (it rewrites the loop r-2
creates) or run beside r-2 (it needs r-3's final-body send path to recover
`input_sha256`). Running r-3b beside r-2b was round-1 defect CPR-003. Wave 3
runs one sprint; r-3c moved to wave 2 to keep usable width at 3.

**Why r-4 runs in wave 1 beside r-1.** r-4 is ADR text plus manifests: root
`[workspace.dependencies]`, the `cli` feature with optional mimalloc and clap
on the two binary crates' `Cargo.toml`, and the CI feature matrix. It edits no
`main.rs`: `[[bin]] required-features = ["cli"]` means the binary target is
compiled only when `cli` (and so `dep:mimalloc`) is on, so the existing
unconditional `#[global_allocator]` line stays valid. Both `main.rs` files
belong to r-1 in wave 1 (error-variant migration). The one shared file is
root `Cargo.toml`, split by line (r-1 deletes the `sc-observability-types`
line; r-4 owns the rest) and ordered by the wave-1 merge rule (r-1 first).

### Must-follow artifacts

| Edge | Artifact consumed |
| --- | --- |
| r-2 → r-1 | `BackendDescriptor` + `BACKENDS`, `BackendSet`, backend factory table + `test_every_backend_has_factory`, `JevError::ProducerError` + `from_error_response`, `ErrorCode`, `RepoRelativePath`, `ScAgentErrorResponse` field rule, `git::{discover_toplevel, read_at_rev}`, `CancelToken`, `with_default_transport` constructors, `test_util::MockJudgeBackend` |
| r-3 → r-1 | `NetworkErrorKind`, `retryable()`, `CancelToken` (always-on core) + `JudgeRequest::cancel`, `ResolvedApiKey` (`Arc<str>`, no `new`), no `#[allow]` left in the files r-3 deletes or moves |
| r-3c → r-1 | `SubjectsBuilder`, producer-neutral variants, `git` module + `git` feature |
| r-3c → r-4 | `cli` feature and optional `clap` in `crates/sc-treesitter/Cargo.toml` |
| r-2b → r-2 | `run` loop, `RunConfig::mode`, `BackendStatus::Failed(BackendFailure)`, `RunResponse::warnings` + `test_builder`, `FileSessionStore::save` |
| r-2b → r-1 | `MockJudgeBackend::with_reported_digest` |
| r-2b → r-3 | final serialized `HttpRequest.body` in both backend crates (digest recovery point); `send_with_busy_retry` no-resend rule |
| r-3b → r-2b | `BackendStatus::Failed` produced at runtime, `RunResponse::{first_failure, is_partial, with_warning}`, `ChainContext` honoured |
| r-3b → r-3 | `sc-agent-http` in the consumer input publish order |
| r-3b → r-3c | `sc-treesitter` `cli` binary with clap and `--version` (driven by `e2e_treesitter_producer`) |
| r-5 → r-2b, r-3, r-3b | `input_sha256` recovery, `sc-jev` as wire-format crate, `workspace-tests --test jev_cross_impl` |

## REQ closure (one owning sprint per `req:` root)

| Sprint | `req:` roots closed |
| --- | --- |
| r-1 | REQ-SAG-0105, REQ-SAG-0106, REQ-SAG-0107, REQ-SAG-0108 |
| r-4 | REQ-SAG-0133, REQ-SAG-0134 (mimalloc), NFR-SAG-0004 |
| r-2 | REQ-SAG-0607, REQ-SAG-0620, REQ-SAG-0312, REQ-SAG-0313, REQ-SAG-0314 |
| r-3 | REQ-SAG-0402 |
| r-2b | REQ-SAG-0606 |
| r-3b | REQ-SAG-0109, REQ-SAG-0604, REQ-SAG-0605, REQ-SAG-0712, REQ-SAG-0135 (clap) |
| r-5 (blocked) | REQ-SAG-0403 |

Amendments to existing rows (each lands once): REQ-SAG-0101, REQ-SAG-0103,
REQ-SAG-0120, REQ-SAG-0121, NFR-SAG-0001 (r-1); REQ-SAG-0601, REQ-SAG-0710
(r-2); REQ-SAG-0505 (r-3); REQ-SAG-0130 (r-3 bullet for `sc-agent-http`, r-3b
bullet for `sc-agent-compose`); REQ-SAG-0711 (r-3b, root and
`docs/sc-ai/requirements.md`). The crate-level rows of REQ-SAG-0120/0121
(`docs/sc-agent-types/requirements.md`) carry the same `(Proposed, r-1)`
amendment. Every id a sprint's files touch is in its `requirements:` list; ids
that are not `req:` roots there are regressions, each with a named command in
that sprint's `boundary:` row.
r-3c closes no `req:` root (boundary-rooted; it contributes to REQ-SAG-0712
and REQ-SAG-0135, which r-3b closes). Existing rows not listed are regression
`boundary:` rows only.

## ADR ownership (each amendment subsection has exactly one landing sprint)

| ADR | Change | Landing sprint |
| --- | --- | --- |
| ADR-SAG-0003 | repo root = git toplevel for both consumers | r-2 |
| ADR-SAG-0004 | each `ChainContext`-augmented request re-validated before its own send | r-2b |
| ADR-SAG-0005 | runs the four consumer extractor kinds (no ADR-0024/`prepare` reference) | r-4 |
| ADR-SAG-0005 | public assembly entry is `prepare`; `run::build_request` sentence superseded | r-2 |
| ADR-SAG-0014 | `data` is the `RunResponse` serialization; primary backend = first `Ok`; `warnings` | r-2 |
| ADR-SAG-0005 | canonical-only schema + Jev JSON envelope (D9) | r-5 (blocked) |
| ADR-SAG-0006 | transports live in `sc-agent-http` | r-3 |
| ADR-SAG-0007 | busy-retry policy (owner decision (b), decided); Retry-After grammar; cancel latency ≤ 120 s | r-3 |
| ADR-SAG-0008 | independent default; `chain_context` explicit | r-2b |
| ADR-SAG-0009 | descriptor + catalog; open = additive; factory table + test-and-review dispatch convention; `register` for existing ids only; `JudgeBackend` intentionally open | r-1 |
| ADR-SAG-0009 | entry point is `prepare`/`run`; `run_check` deleted | r-2 |
| ADR-SAG-0010 | `sc-ai` on `sc-agent-compose`; clap | r-3b |
| ADR-SAG-0011 | `sc-jev-py` → `sc-agent-compose` → `sc-jev-check` | r-3b |
| ADR-SAG-0013 | roots no longer link `sc-observability`; adapter in `sc-agent-compose` | r-3b |
| ADR-SAG-0016 | version text `=1.5.0` | r-4 |
| ADR-SAG-0016 | `sc-observability-types` dropped from types (`LogRejected`); module features | r-1 |
| ADR-SAG-0016 | adapter in published `sc-agent-compose`; supersedes "do not publish a separate observability crate" for that crate; retitled "v2 shim in sc-agent-types; adapter in sc-agent-compose" (heading and index title cell) | r-3b |
| ADR-SAG-0017 | two released binaries + per-channel coverage | r-4 |
| ADR-SAG-0017 | published library set + publish order: `sc-agent-http` | r-3 |
| ADR-SAG-0017 | published library set + publish order: `sc-agent-compose` | r-3b |
| ADR-SAG-0019 | `RunOptions` text → `RunConfig` | r-2 |
| ADR-SAG-0019 | equal-input from backend-reported digest | r-2b |
| ADR-SAG-0020 | hidden `ResolvedApiKey::new` gone; `Arc<str>`; constructors infallible | r-1 |
| ADR-SAG-0020 | lint-enforced single read site (not a type seal); redaction once | r-3 |
| ADR-SAG-0022 | scripted registry via `RunConfig::registry`; `run_check` sentence superseded | r-2 |
| ADR-SAG-0022 | scripted backend reports the digest of the bundle it received | r-2b |
| Rust API patterns | Extractor sealing note withdrawn | r-2 |
| ADR-SAG-0023 (new) | pins (the one lockstep list, including `sc-agent-compose`), allocator, released binaries | r-4 |
| ADR-SAG-0023 (new) | compose pin in force (own subsection) | r-3b |
| ADR-SAG-0024 (new) | engine entry point + `RunResponse`; amends 0005, 0009, 0014, 0019, 0022 | r-2 |
| ADR-SAG-0025 (new) | multi-backend + equal-input; amends 0004, 0008, 0019 | r-2b |
| ADR-SAG-0026 (new) | HTTP half | r-3 |
| ADR-SAG-0026 (new) | compose half | r-3b |
| ADR-SAG-0027 (new) | error taxonomy, `ScAgentErrorResponse` field rule, extensibility, module features, test seams | r-1 |
| ADR-SAG-0028 (new) | `workspace-tests` crate: purpose, dev-only edges, built-binary contract, forbidden edges | r-3b |

ADR-SAG-0026 has an ADR-level **Status** line ("Proposed — HTTP half lands in
r-3, compose half in r-3b"); r-3 sets it to "Active (HTTP half, r-3); compose
half Proposed (r-3b)" and r-3b to "Active". The ADR index rows for
ADR-SAG-0023..0028 are written by this plan PR (conventions).

`docs/sc-agent-types/architecture.md` and REQ-SAG-0103: r-1 is the one
deliberate unfreeze of the a-1 frozen contract; the doc states "re-frozen at
r-1 merge". `docs/sc-jev-py/architecture.md` (the `build_request` sentence at
line 25) is owned by r-2; its compose and `chain_context` sentences by r-3b.
ADR-SAG-0012 is unchanged: `sc-jev` still owns every Jev API operation and
`sc-agent-http` only carries bytes (stated in ADR-SAG-0026). ADR-SAG-0021
per-root path precedence is unchanged (r-3b acceptance proves it).

## Underspecified decisions and owner confirmations

### Owner decisions

**(a) Publishing `sc-agent-compose` — DECIDED (owner, 2026-11-06).**
`sc-agent-compose` is published to crates.io as an internal-support crate,
`=`-pinned from `sc-ai` and `sc-jev-py`, with no semver promise of its own.
This amends D2's word "unpublished": `sc-ai` is published and cannot depend on
an unpublished crate. Consequences, all owned by r-3b: the ADR-SAG-0026
compose half, the ADR-SAG-0023 pin list (compose pin effective when the crate
lands), the ADR-SAG-0017 published-crate set, the ADR-SAG-0016 amendment
(supersedes "do not publish a separate observability crate" for this crate),
consumer input `publish_order` directly after `sc-jev-check`, and the
REQ-SAG-0130 amendment. (REQ-QA-002, CPR-019.)

**(b) Busy-retry numbers — DECIDED (owner, 2026-11-07).** (CPR-M7, OBS-F,
REQ-SAG-0505.) Encoded by r-3 in `BusyPolicy::default()`; ADR-SAG-0007 r-3
amendment and the REQ-SAG-0505 amendment record it.
- Statuses 429/502/529 only; at most 4 sends.
- A valid Retry-After is honoured, clamped 0–30 s. Valid means at most 32
  bytes of ASCII digits with an optional `.` and ASCII-digit fraction
  (`^[0-9]+(\.[0-9]+)?$`); anything else (HTTP-date, sign, exponent, `inf`,
  `NaN`, empty) is malformed. Absent **or malformed** Retry-After (malformed
  is treated like missing, not 30 s): exponential backoff from about 1 s —
  1, 2, 4, 8 … capped at 30 s. With at most 4 sends there are at most three
  waits, so the default sequence is 1, 2, 4 s; 8 s and the 30 s cap apply only
  to a non-default `BusyPolicy` with more sends. Every wait adds up to 250 ms
  jitter.
- 90 s budget from the first send for **starting** new attempts; every wait
  is capped to the remaining budget; an in-flight attempt keeps its 120 s
  request timeout. A cancel is observed before each attempt and within
  100 ms during a wait; a cancel during an in-flight request is observed when
  that request returns, so worst-case cancel latency is the 120 s request
  timeout (stated in rustdoc and ADR-SAG-0007).
- No network-connectivity detection and no indefinite wait on network loss:
  connect/DNS failures return `NETWORK_ERROR` with `retryable() == true`
  immediately, and the caller (CLI, Python, future MCP) decides whether to
  retry. A transport error or timeout after the body was sent is never
  retried.
- This removes round 1's malformed-header exhaustion (CPR-M7): 4 sends fit
  inside the budget unless the server's own Retry-After values exceed it.

**(c) D9 Jev payload shape — pending owner confirmation (blocked on the
TypeSafe vendor answer). This is the only item still pending.** r-5 holds both branches:
- *Branch A (array `{id, prompt, choices}` only):* delete the object-shape
  path in `sc-jev`; no types change.
- *Branch B (object `{type, instructions, criteria}`, or both):* additive.
  `CanonicalJudgePayload` (private fields since r-1) gains
  `criteria: Option<CriteriaSpec>` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`; exactly one of
  non-empty `questions` / `criteria` validates; array payloads stay
  byte-identical (cross-impl fixture).
- **Publish gate (CPR-014):** no crate is published to crates.io until r-5 is
  unblocked and either Branch A lands or Branch B is merged as the additive
  change above. `cargo publish --dry-run` runs throughout; a real publish does
  not.

### Planner resolutions (stand unless the owner objects)

1. **Byte cap owner (D13, REQ-QA-011):** each backend, through
   `BackendDescriptor::request_limit_bytes` read by its own
   `validate_for_send`. Alternative not taken: one engine cap. Dry-run reports
   `payload_bytes` and `over_limit` per selected backend (REQ-SAG-0314).
2. **Old entry points deleted with no shims** (nothing published).
3. **Python `run_check` keeps its Python name** and calls `prepare` + `run`.
4. **Serde spelling `openai_luna` / `openai`** with a one-release read alias
   (bead `agt-backend-serde-alias-removal`) for config and caller-request
   input. The alias is not a session-file compatibility path (see 7).
5. **No new stable error codes**; new variants map to existing ids (code
   table in r-1).
6. **`Error::source()` is `None`** for redacted-cause variants by design.
7. **Phase A session files are not readable after r-1:** session document
   version 2; a version-1 file is `SESSION_DOCUMENT_INVALID` with "re-run the
   check" recovery.
8. **Repo root is the git toplevel** for both consumers (ADR-SAG-0003
   amendment); outside git it is `start`, absolutized. `sc-jev-py` called from
   a subdirectory now reads the toplevel `.env`.
9. **Session written once**, at the end of `run`, only when a backend with
   `supports_session_ask` returned `Ok`.
10. **tree-sitter and grammar pins stay exact** (output determinism).
11. **`InputSpec` moves to `sc-jev-check::config`** in r-1, unchanged; r-2
    adds `Command.path`. No second consumer crate exists (RBQA Q4).
12. **Shared git read lives in `sc-agent-types::git`** behind the
    **non-default** `git` feature (std-only, no new dependency): both callers
    already depend on `sc-agent-types`, which owns `Rev`, `RepoRoot`,
    `RepoRelativePath` and `BlobSha`; `sc-treesitter` may not depend on
    `sc-jev-check`; a separate `sc-agent-git` crate would be a fourth new
    crate for about 150 lines. Only `sc-jev-check` (normal dependency) and
    `sc-treesitter`'s `cli` feature enable it, so `sc-jev`, `sc-jev-backends`,
    `sc-agent-secrets`, `sc-agent-http`, `sc-ai` and `sc-jev-py` do not link
    process-spawning code through their own feature requests (feature
    unification in a workspace build can still compile it; the isolated
    per-library `cargo build -p <lib>` CI rows prove the seam). The
    `sc-treesitter` library stays I/O-free. `workspace-tests --test
    source_scans` (`test_git_spawn_only_in_types_git`) fails on any
    `Command::new("git")` outside `sc-agent-types/src/git.rs` (allowlist: the
    `command` extractor's user-argv spawn in
    `sc-jev-check/src/extract/command.rs`).

Round-2 resolutions in the same standing: module seams are cargo features
(`judge`, `secret`, `obs` default-on; `git` and `test-util` non-default;
`cancel` is always-on core; `sc-treesitter` uses `default-features = false`);
partial failure exits 2; `JudgeBackend` stays open with future methods
defaulted; per-question answers use a new `QuestionAnswer` type rather than
`ChoiceAnswer`, which is part of the canonical `state` bytes.

## Phase acceptance

Run on `integrate/phase-a` HEAD after r-1, r-4, r-2, r-3, r-3c, r-2b, r-3b
merge:

```bash
just lint
just test
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test -p workspace-tests --test jev_cross_impl
cargo test -p workspace-tests --test e2e_treesitter_producer
cargo test -p workspace-tests --test consumer_parity -- test_cli_and_python_data_match_golden --exact
cargo test -p workspace-tests --test git_read_parity
cargo test -p workspace-tests --test source_scans
cargo test -p workspace-tests --test test_util_off
cargo test -p sc-jev-check --test run_response -- test_run_response_contract --exact
cargo test -p sc-jev-check --test multi_backend -- test_partial_failure_keeps_answers --exact
cargo test -p sc-jev-check --test multi_backend -- test_equal_false_when_backend_digest_differs --exact
cargo hack check -p sc-treesitter --feature-powerset --no-dev-deps
cargo hack check -p sc-agent-types --feature-powerset --no-dev-deps
cargo build -p sc-treesitter --lib --no-default-features
cargo build -p sc-ai --lib --no-default-features
for p in sc-agent-types sc-agent-secrets sc-agent-http sc-jev sc-jev-backends sc-jev-check sc-agent-compose; do cargo build -p "$p" --lib || exit 1; done
! rg -n '#!?\[allow\(' crates/
bash .github/scripts/cargo-tree-gates.sh
bash .github/scripts/check-exact-pins.sh
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
uv run --directory tests/fixtures/jev-python pytest -q
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-agent-http -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-agent-compose -p sc-treesitter -p sc-ai
```

Expected: all exit 0, and `.github/workflows/sc-agent-ci.yml` is green on
ubuntu, macos and windows. Then re-run the phase-ending six-reviewer gate. No
real crates.io publish (gate (c)).

## Findings closure (one owning sprint per id)

Every id the Phase A review classified architectural (the 19 phase-end
architectural ids plus every deep-review `architectural_refinement` or
owner-decision id), and the routine ids A.1 also touches. "Owner" closes the
id; other sprints may contribute but do not claim it. Ids collide across
reviews: `RBP-F001 (phase-end)` is the error-prefix finding; `RBP-F001 (a-9)`
is the a-9 pin finding, the same defect as `ARCH-001 (phase-end)`. Plan-review
round-1 ids are dispositioned in `plan-review-1-dispositions.md`.

| Id | Owner | Note |
| --- | --- | --- |
| ARCH-001 (phase-end), RBP-F001 (a-9) | r-4 | ADR-SAG-0023 |
| ARCH-002 (phase-end) | r-4 | ADR-SAG-0017 binaries amendment |
| ARCH-005 (phase-end) | r-3b | A.1 crate graph (a-10 fixed the Phase A graph) |
| ARCH-006 (phase-end) | r-4 | kinds sentence; r-2 contributes the `prepare` sentence |
| RBP-F001 (phase-end) | r-1 | explicit variants |
| RBP-F002 | r-1 | taxonomy part (routine part a-10) |
| RBP-F003 | r-1 | `RepoRelativePath`, `BlobSha` (Rev half a-10) |
| RBP-F004 | r-1 | catalog part; r-2 contributes the Extractor note |
| RBP-F005 | r-2 | `SessionStore` trait deleted |
| RBP-F006 | r-3 | |
| RBP-F007 | r-3 | `Arc<[u8]>` body; r-1 contributes `Arc<str>` key and `Arc<PayloadBundle>` request |
| RBP-F008 | r-2 | |
| RBP-F009 | r-2 | |
| RBP-F010 | r-1 | `Arc<str>`, hidden `new` deleted |
| RBP-F011 | r-1 | newtypes, `Operation`, struct for the bool pair |
| RBP-F012 | r-3b | sc-ai clap; r-3c contributes sc-treesitter |
| RBP-F013, QA-004 | r-3b | one adapter |
| RSH-005 | r-3 | budget-capped sleeps, `CancelToken` honoured |
| QA-003 | r-3 | D5: one HTTP layer (`sc-agent-http`) |
| EXT-001 | r-1 | |
| EXT-002 | r-3b | r-2 contributes the engine |
| EXT-004 | r-2 | Extractor trait deleted |
| EXT-005 | r-3 | |
| EXT-006 | r-2 | |
| EXT-007, EXT-011 | r-1 | |
| EXT-009 | r-2 | `BackendRegistry::from_env` + registry injection |
| EXT-010 | r-3b | r-3c contributes |
| ARCHC-001 | r-3 | |
| ARCHC-002, ARCHC-003 | r-5 (blocked) | |
| ARCHC-004, ARCHC-005 | r-1 | |
| ARCHC-008, ARCHC-011, ARCHC-021 | r-1 | ARCHC-021: r-1 moves `InputSpec` unchanged; r-2 adds `Command.path` |
| ARCHC-010 | r-1 | one `JudgeBackend` test double home: `sc_agent_types::test_util::MockJudgeBackend` (`test-util` feature), which records requests and reports `input_sha256`; the scripted registry stays a registry builder over it (ADR-SAG-0022) |
| ARCHE-001, ARCHE-013, CR-009 | r-2b | |
| ARCHE-002 | r-2b | r-3b contributes the consumer flag |
| ARCHE-003, ARCHE-004, ARCHE-011, ARCHE-012 | r-2 | |
| ARCHE-005, ARCHE-022 | r-1 | |
| ARCHE-006, CR-001, ARCHP-002 | r-3b | e2e proof; r-2 contributes the contract |
| ARCHE-007 | r-2 | |
| ARCHE-018 | r-3b | sc-ai parser |
| ARCHP-011 | r-3c | sc-treesitter parser |
| ARCHP-001, ARCHP-003 | r-1 | |
| ARCHP-020 | r-3b | test moved; r-1 contributes nothing |
| ARCHP-005, ARCHP-006, RBQA-F007 | r-4 | |
| ARCHP-014, RBQA-F005 | r-3b | |
| RBQA-F003 | r-3 | |
| RBQA-F004 | r-2 | engine cap removed; r-1 contributes the descriptor limit |
| RBQA-F006 | r-1 | helper + parity tests; r-2 and r-3c migrate callers; r-3b adds `git_read_parity` |
| RBQA-F009 | r-1 | features, `InputSpec` leaves the types crate |
| RBQA-F010 | r-1 | typed answers, `commands` removed, `PayloadBundle::encode` |

### This phase does not close

- **ARCHC-006** (blocking `JudgeBackend`, prepare/send split): D10 keeps the
  sync trait and defers the split to the first Phase B agent that needs
  async. A.1 adds the `# Blocking` rustdoc and `CancelToken`.
- **ARCHP-009** (one file per producer call, `Parser`/`Query` rebuilt per
  call, no match limit): producer throughput with no A.1 consumer; the Phase B
  multi-file producer owns it. Bead `agt-treesitter-multifile`.
- **D9** (r-5): blocked; the `"model"` byte-splice remains until then.
- Phase B features: API-direct agents (REQ-SAG-0900/0901), MCP server (a third
  thin adapter over `sc-agent-compose`), typed `JudgeMeta`, a second session
  store, a crate split of `sc-agent-types` (module paths are the stable surface
  for a later re-export split, ADR-SAG-0027).
- `agt-luna-live-count` (Luna live `equal`), still open from Phase A.
- New stable error codes in `errors.json`.
- `agt-obs-v2-shim-removal`: removal keeps `LogRejected`/`SessionLog` as the
  port; only the compose adapter changes.

## Sprint index

| Sprint | Doc |
| --- | --- |
| r-1 | [`sprint-r-1-types-break.md`](sprint-r-1-types-break.md) |
| r-4 | [`sprint-r-4-adrs-and-manifests.md`](sprint-r-4-adrs-and-manifests.md) |
| r-2 | [`sprint-r-2-engine-shape.md`](sprint-r-2-engine-shape.md) |
| r-3 | [`sprint-r-3-shared-http.md`](sprint-r-3-shared-http.md) |
| r-3c | [`sprint-r-3c-treesitter-cli.md`](sprint-r-3c-treesitter-cli.md) |
| r-2b | [`sprint-r-2b-multi-backend-semantics.md`](sprint-r-2b-multi-backend-semantics.md) |
| r-3b | [`sprint-r-3b-compose-and-workspace-tests.md`](sprint-r-3b-compose-and-workspace-tests.md) |
| r-5 | [`sprint-r-5-jev-wire-envelope.md`](sprint-r-5-jev-wire-envelope.md) (blocked) |

## Crate graph after A.1

```text
sc-ai (bin; feature cli: clap + mimalloc) ─┐
sc-jev-py (PyO3) ──────────────────────────┴── sc-agent-compose (published, = pin; obs, PathPolicy, run_from/ask_from)
                                                 ├── sc-observability =1.5.0
                                                 └── sc-jev-check (prepare/run, RunResponse + Envelope, discover_repo_root, InputSpec)
                                                       ├── sc-jev ──────────┐
                                                       ├── sc-jev-backends ─┴── sc-agent-http (ureq; sc-agent-types features = [secret])
                                                       ├── sc-agent-secrets
                                                       └── sc-agent-types (default judge/secret/obs; git enabled by sc-jev-check; no observability dep)
sc-treesitter (bin; feature cli: clap + mimalloc + sc-agent-types/git) → sc-agent-types (default-features = false)
crates/workspace-tests (publish = false; dev-deps only; drives built binaries; ADR-SAG-0028)
```
