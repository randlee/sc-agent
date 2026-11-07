# Phase A.1 plan review round 2 — dispositions

Reviewed commit: `a1e3a52`. Reports:
`.cursor/phase-a-orchestration/refine-plan/plan-qa2/*.json` (seven reviewers).
This is the last plan-QA round: every finding and observation is applied or
declined with a concrete reason. Code claims were checked against the a-10
hardening branch (`sprint/a-10-phase-end-hardening` @ `1899319`). "Where"
names the file (`plan` = `plan.md`, `r-N` = that sprint doc, `arch` =
`docs/architecture.md`, `req` = `docs/requirements.md`, `types-req` =
`docs/sc-agent-types/requirements.md`, `qp` =
`.claude/project/quality-policy.md`).

Owner decisions (a) publish `sc-agent-compose` and (b) busy-retry numbers are
DECIDED; only (c) D9 stays pending.

## Summary

| Reviewer | Items | Applied | Declined (with reason) |
| --- | --- | --- | --- |
| critical-plan-reviewer | 18 (+6 round-1 carry-forward) | 18 | 0 |
| arch-qa | 6 | 6 | 0 |
| req-qa | 11 (+4 carry-forward) | 11 | 0 |
| rust-best-practices-agent | 2 + 5 observations | 7 | 0 (one sub-point declined, see obs-d) |
| rust-service-hardening-agent | 1 + 4 observations + 2 notes | 6 | 1 (advisory-scan note) |
| ruthless-boundary-qa | 4 residuals + 3 observations | 6 | 1 (compose re-export) |
| extensibility-qa | 3 | 3 | 0 (one sub-point declined, see EXT-021) |

## Code facts checked on a-10

- Three plain `#[allow]` remain: `sc-agent-types/src/ids.rs:124`,
  `sc-jev/src/client/https.rs:84`, `sc-jev-backends/src/transport.rs:81`.
  r-1 converts all three.
- `ScAgentErrorResponse::from_jev_error` is called in both `main.rs`; the
  name is kept.
- `sc-treesitter` constructs struct-like variants by literal in `main.rs`
  (`ExtractionFailed { detail }`), `git_io.rs` (24 sites), `grammar.rs`,
  `query.rs`; `sc-ai/src/main.rs` uses only constructors.
- The registry factory is a `match id` in `sc-jev-check/src/registry/mod.rs`;
  `sc-ai/src/command.rs:683` compares `outcome.backend == BackendId::Jev`.
- All eight existing crates already have `[lints] workspace = true`; the root
  has only `[workspace.lints.rust] unsafe_code = "forbid"` (RBQA's "none do
  today" read the pre-a-10 plan tree).
- `sc-observability-types` has one user (`sc-agent-types`), one root entry
  (`=1.5.0`), one `sc-lint.toml` record and one manifest entry.
- `sc-agent-types` and `sc-jev-check` already have a non-default
  `test-util` feature; `sc-ai` has none.

## critical-plan-reviewer

| Id | Disposition | Where |
| --- | --- | --- |
| CPR-101 | applied — r-1 owns `crates/sc-treesitter/src/**` (incl. `grammar.rs`, `query.rs`, `git_io.rs`, `main.rs`) and `crates/sc-ai/src/**` (incl. `main.rs`) in wave 1; every struct-like variant gets a public constructor; `from_jev_error` name kept; r-4 edits no `main.rs` (`[[bin]] required-features = ["cli"]` keeps the unconditional allocator valid); boundary map and r-3c/r-4 owned_paths re-derived | r-1 frontmatter, Outcome, D6.1, D6.7; r-4 frontmatter, D2; r-3c ownership note; plan boundary map, "Why r-4 runs in wave 1" |
| CPR-102 | applied — factory is `static FACTORIES: &[(BackendId, fn(ResolvedApiKey) -> Arc<dyn JudgeBackend>)]` checked by `test_every_backend_has_factory`; branching on descriptor facts only; `sc-ai` id equality converted; `wildcard_enum_match_arm` deny/expect dropped (and gated absent); test-and-review convention in ADR-0009/0027 and REQ-SAG-0105; "adding a backend" checklist | r-1 D1.6, D7.7; arch ADR-0009 r-1, ADR-0027; req REQ-SAG-0105 |
| CPR-103 | applied — r-1 converts all three a-10 `#[allow]` to `#[expect(.., reason)]`; grep gate `! rg -n '#!?\[allow\(' crates/` in every sprint's validation and the phase acceptance; `workspace-tests` `test_no_allow_attributes` | r-1 "Lint hygiene"; plan conventions "Lint hygiene", phase acceptance; r-3b D7.3 |
| CPR-104 | applied — r-1 deletes the root `sc-observability-types` line (it removes the only user); r-4 deletes the `sc-lint.toml` record; not on the ADR-0023 list; wave-1 merge order r-1 before r-4 so r-4's pin check runs on the rebased tree | r-1 D7.1, owned_paths; r-4 D1, "Why wave 1"; plan conventions "Wave-1 merge order", boundary map root row, ownership records; arch ADR-0023, ADR-0016 r-1 |
| CPR-105 | applied — per-field rule: `schema_version` always `1`, `retryable` only when true, `details` only for `ProducerError`; fixed key order; reader accepts absent or `1`; `test_error_response_stderr_golden` pins bytes; NFR-SAG-0001 amended | r-1 D6.7; arch ADR-0027; req NFR-SAG-0001 amendment; types-req |
| CPR-106 | applied — `RunResponse::warnings: Vec<String>` (last key, omitted when empty) and `with_warning`; fifth sample `session_save_failed.json`; r-2b emits the save warning there; r-3b routes the log-flush warning through compose in both consumers (`test_cli_log_flush_warning_in_data`) | r-2 D2.5, D2.6; r-2b D4; r-3b D2.2, D3.3; arch ADR-0014 r-2, ADR-0024, ADR-0025 |
| CPR-107 | applied — option (a): one `test-util` convention (self dev-dependency for a crate's own integration targets; gated off normal graphs by a `cargo tree` row and `workspace-tests --test test_util_off`); `test_release_build_refuses_http` kept as the integration proof; TLS loopback not needed (`Endpoint::loopback_http` + `UreqTransport::loopback_for_tests`) | plan conventions "Test seams", gate table; r-1 D10.11; r-2 D2.5–2.6; r-2b D6; r-3 D1; r-3b D7.3 |
| CPR-108 | applied — r-1 fixes the `MockJudgeBackend` contract (records requests; reports the received digest; `with_reported_digest`; `with_failure`); r-2b owns `registry/scripted.rs`; real-wire-builder tests move into `sc-jev`/`sc-jev-backends` tests (no `sc-jev-check` → `sc-agent-http` edge); r-2b owns both backend `Cargo.toml` and boundary manifests in wave 3 | r-1 D10.11; r-2b owned_paths, D5, D6 |
| CPR-109 | applied — `test_public_api_has_no_ureq` replaced by a stable check (private `ureq_transport` module + `cargo doc --no-deps` HTML grep for `ureq::`), in r-3 validation and r-3b CI; r-3c ADR-0015 row is an asserted `test -z` on `sc-agent-types` features and r-3b adds it to `cargo-tree-gates.sh` | r-3 D1, validation; r-3c acceptance, validation; r-3b D12.2; plan gate table |
| CPR-110 | applied — helper at `tests/support/bin_path.rs` (dev-only `serde_json`); `allowed_dependencies` = exact workspace dev set; which edges `sc-boundary` checks is settled by the r-3b spike (RBQA-F005a) | plan workspace-tests manifest; r-3b D7.0–7.2; arch ADR-0028 |
| CPR-111 | applied — this plan PR writes ADR index rows 0023–0028 as Proposed and the Amended-by markers in rows 0001, 0004, 0005, 0008, 0009, 0014, 0019, 0022; each landing sprint owns only its row's Status/Amends cells and its own markers, named per row in owned_paths | arch ADR index; plan conventions "ADR index table"; owned_paths of r-1, r-2, r-2b, r-3, r-3b, r-4 |
| CPR-M101 | applied — ADR-0022 r-2b digest clause moved to its own r-2b subsection (also ARCH-012) | arch ADR-0022; r-2b owned_paths, D7 |
| CPR-M102 | applied (verified) — every busy-retry reference is dated 2026-11-07 (plan (b), r-3 D2, arch ADR-0007, req REQ-SAG-0505, round-1 dispositions); the 2026-11-06 dates are D1–D13 acceptance and decision (a) | plan, r-3, arch, req |
| CPR-M103 | applied — ADR-0023 compose pin is its own r-3b subsection with its own Status | arch ADR-0023; r-3b owned_paths |
| CPR-M104 | applied — r-2b re-rated L; total ≈ 28–37 dev-days; critical path XL+L+L+XL ≈ 20–26 dev-days | r-2b frontmatter; plan sprints table, wave notes |
| CPR-M105 | applied — module and file renamed `ureq_transport` (`src/ureq_transport.rs`) | r-3 D1, D4; plan agent-http manifest; arch ADR-0020 r-3, ADR-0026 |
| CPR-M106 | applied — ADR-SAG-0025 added to r-1 `adrs` | r-1 frontmatter |
| CPR-M107 | applied — `ErrorCode::ALL` hand-written `const` table + `test_error_codes_match_errors_json`; no `build.rs` | r-1 D6.2, D6.6; arch ADR-0027 |
| CPR-001 (r1 open) | closed by CPR-101 | — |
| CPR-004 (r1 partial) | closed by CPR-102 | — |
| CPR-005 (r1 partial) | closed by CPR-107/108 (`test-util` `WireFault` hook on real backend types) | r-2b D6 |
| CPR-015 (r1 partial) | closed by CPR-111 | — |
| CPR-018 (r1 partial) | closed by CPR-107 (no `cfg(test)` from `tests/`) and CPR-109 (no vacuous rows) | — |
| CPR-023 (r1 partial) | closed by CPR-101 (r-1 owns the `sc-treesitter` files) | — |

## arch-qa

| Id | Disposition | Where |
| --- | --- | --- |
| ARCH-009 | applied — ADR-0014 "Amendment (Phase A.1, r-2)" subsection (data = `RunResponse` serialization, primary = first `Ok`, `warnings`); ADR-0024 Amends adds 0014; r-2 owns the subsection, the REQ-SAG-0710 amendment row and the index marker | arch ADR-0014, ADR-0024, index; r-2 owned_paths, ADR text; req REQ-SAG-0710; plan ADR ownership |
| ARCH-010 | applied — new ADR-SAG-0028 (purpose, dev-only edges, built-binary contract, forbidden edges) owned by r-3b; in r-3b `adrs`, plan union and ownership table | arch ADR-0028, index; r-3b; plan frontmatter, ADR ownership |
| ARCH-011 | applied — r-2 +0010/0011; r-2b +0005/0007 (and +0022 for its new subsection); r-4 +0010/0015; r-1 +0010/0011/0012 (+0025 per CPR-M106) | r-1, r-2, r-2b, r-4 frontmatter |
| ARCH-012 | applied — see CPR-M101 | arch ADR-0022 |
| ARCH-013 | applied — see CPR-111; r-4 no longer adds rows | r-4 D7; arch index |
| ARCH-014 | applied — `sc-observability-types` record deleted by r-4 (not assigned to compose); ADR-0016 retitled in its r-3b amendment and index title cell; ADR-0026 has an ADR-level Status line | plan ownership records; arch ADR-0016 r-3b, ADR-0026; r-3b owned_paths |

## req-qa

| Id | Disposition | Where |
| --- | --- | --- |
| REQ-QA-R2-001 | applied — REQ-SAG-0120/0121 added to r-1 with `(Proposed, r-1)` amendment rows in root and crate requirements; crate-level REQ-SAG-0711 (`docs/sc-ai/requirements.md`) edit is an r-3b deliverable | r-1 frontmatter, D7.1, acceptance; req; types-req; r-3b ADR text |
| REQ-QA-R2-002 | applied — ids added per sprint as listed (r-1, r-2, r-3, r-3b, r-3c, r-4, r-5), each with a named regression command in that sprint's `boundary:` row; none marked not-applicable (each sprint does touch the crate or subject) | every sprint frontmatter and acceptance table |
| REQ-QA-R2-003 | applied — see CPR-101 | r-1 owned_paths; plan boundary map |
| REQ-QA-R2-004 | applied — `test-util` convention (CPR-107); https-only stays (`Endpoint::parse` never accepts http; `loopback_http` builds only 127.0.0.1); deterministic public-API check (CPR-109); feature named in manifests and gated out of normal graphs | plan conventions, gate table; r-3 D1 |
| REQ-QA-R2-005 | applied — see ARCH-009; Haiku-primary sample in r-2 acceptance (`req:REQ-SAG-0710 (amended)` row) | r-2 D2.6, acceptance; req REQ-SAG-0710 |
| REQ-QA-R2-006 | applied — plan resolution 11 and the ARCHC-021 closure note now say "r-1 moves `InputSpec`; r-2 adds `Command.path`" | plan |
| REQ-QA-R2-007 | applied — `QA-003 | r-3` in the closure table and r-3 "Closes findings" | plan findings closure; r-3 |
| REQ-QA-R2-008 | applied — exception recorded now in `.claude/project/quality-policy.md` "Repository exceptions" (paths, branch, `r-N` ids, naming `req-qa` and `arch-qa`), as a deliverable of this plan PR | qp; plan header |
| REQ-QA-R2-009 | applied — `docs/sc-agent-http/{requirements,architecture}.md` (r-3 D6); `docs/sc-agent-compose/…` (r-3b D2.4); `docs/workspace-tests/…` (r-3b ADR text), with `docs/workspace-tests/**` in r-3b owned_paths | r-3, r-3b |
| REQ-QA-R2-010 | applied — see CPR-111 | arch index; plan conventions |
| REQ-QA-R2-011 | applied — (a) adjacent-row convention: the second sprint of a wave to merge resolves, keeping both rows verbatim; (b) one lockstep list in ADR-0023 between markers, which `check-exact-pins.sh` reads (no copy in the script), including `sc-agent-compose` (in force r-3b) | plan conventions; req Phase A.1 intro; arch ADR-0023; r-4 D1 |
| REQ-QA-003 (carry) | closed by R2-010 | — |
| REQ-QA-006 (carry) | closed by R2-006/R2-007 | — |
| REQ-QA-007 (carry) | closed by R2-004 | — |
| REQ-QA-012 (carry) | closed by R2-008 | — |

## rust-best-practices-agent

| Id | Disposition | Where |
| --- | --- | --- |
| RBP-F106 | applied — the `test-util` mechanism, stated once in plan conventions and per crate in r-1/r-2/r-2b/r-3; `BusyPolicy::with_test_hooks(clock, sleeper, jitter)` gives deterministic jitter and a monotonic clock; release-graph proof `test_util_off` | plan conventions; r-1 D10.11; r-2 D2.5; r-2b D6; r-3 D1–D2 |
| RBP-F109 | applied — `SubjectsDocument` is the one validated type (private fields, raw-form deserialize via `try_from`, built by `SubjectsBuilder::build`/`TryFrom`); `ValidatedSubjects` dropped; added to the triage table; `EqualInputRecord.equal` derived (serialized from the method; deserialize rejects a mismatch) | r-1 D7.3, D10.5, triage table; req REQ-SAG-0108 |
| obs-a | applied — `ChoiceId` newtype; finite-probability rule in `[0, 1]` through the constructor and `try_from`; `PartialEq` only; relation to `ChoiceAnswer` stated (output vs canonical input, one conversion); `JudgeOutcome::new` checks the answer key set against the request | r-1 D10.1–10.2, acceptance |
| obs-b | applied — type homes per feature table; `cancel` always-on core so `sc-agent-http` takes `features = ["secret"]`; `secret`/`judge` independent; `git` needs only core; `cargo hack check -p sc-agent-types --feature-powerset` in r-1 validation and r-4 CI | r-1 D7.2, D10.7; r-3 D1; r-4 D3; arch ADR-0027 |
| obs-c | applied — "1, 2, 4, 8" text says only three waits (1, 2, 4 s) occur by default; `HttpRequest`/`HttpResponse` private fields, constructors and accessors; `SecretHeader::new(HeaderName, ResolvedApiKey)` with a validated `HeaderName` newtype | plan (b); r-3 D1–D2; arch ADR-0007 r-3; req REQ-SAG-0505 |
| obs-d | applied — `BackendFailure` stores only `JevError`, response derived at serialize (`test_backend_failure_response_derived_from_error`); redundant `#[non_exhaustive]` dropped from `RunResponse` (private fields kept); `ErrorCode` `#[non_exhaustive]` enum with `as_str()`; newer session version → `SessionDocumentInvalid`; `BackendId` `Deserialize` via `FromStr` (descriptor aliases are the single source) | r-2 D2.5; r-1 D6.2, D7.6, D1.8; arch ADR-0024, ADR-0025, ADR-0027 |
| obs-e | applied — `read_at_rev` returns a `GitBlob { sha, bytes }` struct (private fields, accessors) | r-1 D7.5, triage table |

## rust-service-hardening-agent

| Id | Disposition | Where |
| --- | --- | --- |
| RSH-006 | applied — `test_ureq_agent_config_values` (`--lib` unit test on the private `AgentConfig`: connect 10 s, global 120 s, no redirects, https-only, User-Agent), named in r-3 validation | r-3 D1, validation |
| obs-1 | applied — loopback tests use the `test-util` constructor `Endpoint::loopback_http`, which can only build `http://127.0.0.1:<port>`; `Endpoint::parse` refuses http in every build; the feature-unification risk is closed by the loopback-only constructor plus the `test-util` normal-graph gate | r-3 D1; plan conventions, gate table |
| obs-2 | applied — cancel-mid-request latency ≤ 120 s stated in `send_with_busy_retry` rustdoc and the ADR-0007 r-3 amendment | r-3 D2; arch ADR-0007; plan (b) |
| obs-3 | applied — Retry-After grammar `^[0-9]+(\.[0-9]+)?$`, ≤ 32 bytes, parsed as digits (no `f64::from_str`, no unchecked `from_secs_f64`); everything else malformed; `test_retry_after_grammar` table | r-3 D2; plan (b); arch ADR-0007; req REQ-SAG-0505 |
| obs-4 | applied — redaction covers every `Err` `send_with_busy_retry` returns, including a `Transport` impl's own errors; `test_transport_error_with_secret_is_redacted`; direct `Transport::send` bypass documented | r-3 D1; arch ADR-0020 r-3 |
| carry RSH-004 | applied — ADR-0024 states the 1 h stale-tmp threshold | arch ADR-0024 |
| note (no `cargo audit`/`deny` step) | declined — advisory scanning is repository-wide CI policy, not one of the accepted decisions D1–D13 that bound A.1, and A.1 reduces the HTTP dependency surface (one `ureq` user instead of two) rather than adding to it; recorded as bead `agt-ci-advisory-scan` and in r-3 "does not close" | r-3 |

## ruthless-boundary-qa

| Id | Disposition | Where |
| --- | --- | --- |
| RBQA-F003 | applied — scan moved to `workspace-tests --test source_scans`: all targets (`crates/**/*.rs`, item- and crate-level), asserts `[lints] workspace = true` and no per-crate override in every crate; `[lints] workspace = true` stated for every existing crate (present since a-10) and the three new ones; phase acceptance updated | r-3 D4; r-3b D7.3; plan conventions, acceptance; arch ADR-0020 r-3 |
| RBQA-F005 | applied — (a) r-3b first step is a dev-edge spike recorded in `boundaries/README.md`; manifest and dependents written per the outcome, or the dev set policed by `cargo tree -e dev`; (b) unused `sc-agent-http` dev-dep and `workspace-tests` dependent permission dropped (and `sc-agent-http` added to its forbidden edges); (c) gate rows run with default and `--no-default-features` except *(lib)* rows; `sc-observability-types` record deleted (r-4) and root dep removed (r-1) | plan manifests, gate table; r-3b D7.0–7.1; r-4 D1 |
| RBQA-F006 | applied — `git` non-default (`default = [judge, secret, obs]`); `sc-jev-check` (r-2) and `sc-treesitter`'s `cli` (r-3c) are the only enablers; `test_git_spawn_only_in_types_git` source scan with the command-extractor allowlist; isolated per-library `cargo build -p <lib>` rows in CI (r-3b) and phase acceptance | r-1 D7.2; r-2 D2.6; r-3c D1; r-3b D7.3, D12.2; plan resolution 12, acceptance |
| RBQA-F009 | applied — `CancelToken` always-on core; `sc-agent-http` `features = ["secret"]` (asserted by a `cargo tree -e features` row); r-1 removes `sc-observability-types` from `agent-types.toml`; jitter from std and URL parsing via ureq's re-exported `http::Uri`, so `allowed_dependencies` stays `[sc-agent-types, ureq]` | r-1 D7.1, D10.7; r-3 D1, validation; plan agent-http manifest |
| obs (compose re-export) | declined — re-exporting `CheckSource`/`Envelope`/`RunResponse` from compose would give each item a second public path, against the r-1 rule "one path each"; the direct `sc-ai`/`sc-jev-py` → `sc-jev-check` edge is already in both manifests and costs nothing | r-3b D2.3 (stated) |
| obs (env-reading owner) | applied — ADR-0026: key/`.env` reading in `sc-jev-check` (`from_env`), path env vars in compose `PathPolicy`, nobody else | arch ADR-0026; r-3b D2.2 |
| obs (unanchored patterns) | applied — word-bounded `grep -E '(^|[^[:alnum:]_-])<name> v[0-9]'` in the gate table and every sprint validation line | plan gate table; r-1, r-2, r-2b, r-3, r-3c |

## extensibility-qa

| Id | Disposition | Where |
| --- | --- | --- |
| EXT-020 | applied — `schema_version` always `1` from `from_jev_error`, bumped only on a breaking error-shape change, independent of `Envelope.version`; pinned in the golden stderr test and the `jev_failed_haiku_ok.json` sample | r-1 D6.7; r-2 D2.6; arch ADR-0027 |
| EXT-021 | applied with one sub-point declined — `ComposeInputs` has private fields, `new(cwd, policy)` and one setter per optional field, so adding a field breaks no caller; ADR-0026 says external consumers call `prepare`/`run` directly. `#[non_exhaustive]` is not added: with private fields it is redundant (outside code already cannot build or exhaustively destructure the struct), and the plan drops the same redundancy from `RunResponse` (RBP obs-d) | r-3b D2.2; arch ADR-0026 |
| EXT-022 | applied — `register` is for doubles/scripted backends of existing ids; "adding a backend" checklist (types minor + factory row) in r-1 and ADR-0009/0027 | r-1 D1.6; r-2 D2.8; arch ADR-0009 r-1, ADR-0027 |

## Final wave table

| Wave | Sprints (size) | Note |
| --- | --- | --- |
| 1 | r-1 (XL), r-4 (M) | merge order r-1 → r-4 |
| 2 | r-2 (L), r-3 (L), r-3c (M) | disjoint paths |
| 3 | r-2b (L) | re-rated from M |
| 4 | r-3b (XL) | integration |
| 5 | r-5 (M) | blocked on D9 |

Critical path r-1 → r-2 → r-2b → r-3b = 4 sprints, ≈ 20–26 dev-days; total
≈ 28–37 dev-days; width 3.

## Unresolved

- D9 (owner decision (c)) — pending the TypeSafe vendor answer; r-5 stays
  blocked and the crates.io publish gate stays closed.
- The `sc-boundary` treatment of dev-only workspace edges is unknown until
  r-3b's spike; both outcomes are written out, so the plan does not depend
  on it.
