# Phase A.1 plan review round 3 — dispositions

Reviewed commit: `c7c6f23`. Reports:
`.cursor/phase-a-orchestration/refine-plan/plan-qa3/*.json` (seven reviewers).
Round 3 was the final plan-QA round; this round-4 revision applies or declines
every id with a reason. Code claims were checked against
`sprint/a-10-phase-end-hardening` @ `3eab8db` (PR 20, ADR-SAG-0029 at
`190cb0b`). "Where" uses the round-2 abbreviations (`plan` = `plan.md`,
`r-N` = that sprint doc, `arch` = `docs/architecture.md`, `req` =
`docs/requirements.md`, `types-req` = `docs/sc-agent-types/requirements.md`).
D1–D13 and owner decisions (a), (b) are not reopened; (c) D9 stays pending.

## Summary

| Reviewer | Verdict | Items | Applied | Declined / partial |
| --- | --- | --- | --- | --- |
| critical-plan-reviewer | FAIL (0 B / 11 I / 12 m) | 23 (+ CPR-101, 108, 109, M102 partials) | 23 | 0 |
| arch-qa | FAIL (3 B / 2 I) | 5 | 5 | 0 |
| req-qa | FAIL (1 B / 2 I / 2 m) | 5 | 5 | 0 |
| ruthless-boundary-qa | 3 I / 4 m | 7 | 6 | F106 (c) declined (cosmetic) |
| extensibility-qa | PASS (4 m) | 4 | 3 | EXT-032 partial (see row) |
| rust-best-practices-agent | PASS (4 notes) | 4 | 4 | 0 |
| rust-service-hardening-agent | PASS (3 notes) | 3 | 3 | 0 |

## Code facts checked on a-10

- `crates/sc-jev/src/check.rs:126` requires `confidence` on every Jev answer;
  `:147` copies `answers` verbatim into `JudgeOutcome.answers` and the whole
  body into `provider_meta` (CPR-203).
- `crates/sc-jev-check/Cargo.toml:25` is `test-util = []`; its dev-dependency
  already enables `sc-agent-types/test-util` (CPR-204).
- `registry/mod.rs:27` re-exports `RequestLog`, `ScriptedJudge`,
  `register_scripted_backends` under `test-util`; `ScriptedJudge` has
  `with_provider_meta`, `with_request_log`, `validations`;
  `crates/sc-ai/tests/cli.rs` uses both types; `sessions_exact.rs` keeps a
  local copy (CPR-205). a-10 `MockJudgeBackend` has `new(id, outcome)` and
  `refusing`.
- `JevClient` struct, `new` and `impl JudgeBackend` are in
  `crates/sc-jev/src/client/mod.rs:79,98,180` (CPR-206).
- CI step: `cargo hack --each-feature test -p sc-treesitter`
  (`sc-agent-ci.yml:200-202`); `CARGO_BIN_EXE_*` in `sc-treesitter/tests/cli.rs:18`
  and `sc-ai/tests/cli.rs:120` (CPR-207).
- `sc_ai::execute_with_overrides` / `ExecuteOverrides` (registry, clock,
  session TTL, session log) at `sc-ai/src/command.rs:31,71` (CPR-208).
- `Sha256Digest::of` uses `sha2` in `ids.rs:293`, and the `sc-treesitter`
  library calls it (`extract.rs:275`) (CPR-M202).
- Root `Cargo.toml` has `resolver = "2"`; `sc-lint.toml:15` runs
  `cargo clippy --workspace --all-targets -- -D warnings` (what `just lint`
  delegates to); production `Command::new` sites: `sc-treesitter/src/git_io.rs`,
  `sc-jev-check/src/extract/{mod,read,command}.rs`, `sc-ai/src/command.rs`
  (all `git` ones move to `sc_agent_types::git` in r-2/r-3c); production
  environment reads: `sc-agent-secrets`, `sc-jev-check/src/extract/sanitize.rs`,
  `sc-ai/src/observe.rs`, `sc-jev-py/src/logging.rs` (the last two move to
  compose) (RBQA-F102, F103, F106).
- ADR-SAG-0029 (a): unchecked `ResolvedApiKey::new` "exists only under
  `cfg(test)` or the `sc-agent-types/test-util` feature"; the two sending
  transports "carry `#[allow(clippy::disallowed_methods, reason = ...)]`".
  (b): "`sc-jev-check` feature `test-util` makes `registry::ScriptedJudge`,
  `registry::RequestLog`, and `registry::register_scripted_backends`
  public"; `_testing.run_check_scripted`; "Everything else in ADR-SAG-0022
  stands".
- `PHASE-A-REVIEW.md:295`: "Owner confirmation (2026-11-06, retry policy)".

## critical-plan-reviewer

| Id | Disposition | Where |
| --- | --- | --- |
| CPR-201 (closes CPR-101) | Applied: triage row now `from_jev_error` (name kept) + `Deserialize`. | r-1 triage table |
| CPR-202 | Applied: `ProducerError { producer: String, code: ErrorCode, message: String }` in the code table, D6.6 and ADR-SAG-0027; constructor `producer_error(producer: impl Into<String>, code: ErrorCode, message: impl Into<String>)`; every `producer` field is `String`. | r-1 D6.1; arch ADR-0027 |
| CPR-203 | Applied, option (a) (recommended): `QuestionAnswer` gains `confidence: Option<f64>` under the finite-[0,1] rule; key order `choice`, `confidence`, `probabilities`; golden test round-trips a real a-10 Jev answer. Jev-primary serialization copies `provider_meta` fields except `answers`, then writes `answers` once from the typed answers (no duplicate key); `single_jev.json` pins it. Per-answer keys outside the three survive only in `provider_meta` (stated). | r-1 D10.1; r-2 D2.6 and acceptance; arch ADR-0014 r-2, ADR-0024 |
| CPR-204 | Applied: r-1 owns the one `[features]` line of `crates/sc-jev-check/Cargo.toml`, `test-util = ["sc-agent-types/test-util"]`; the factory test is a `--lib` test (`registry::tests::test_every_backend_has_factory`), `FACTORIES` stays private (no new public item, so r-2's facade needs none); r-1 validation, REQ-SAG-0105 criterion and must-follow row updated. | r-1 front matter, D1.6, D10.11, acceptance, validation; r-2 D2.6, D2.9; plan must-follow |
| CPR-205 (closes CPR-108 b) | Applied, decision **delete**: `ScriptedJudge` and `RequestLog` are deleted in r-1; callers (`sc-ai/tests/cli.rs`, the `sessions_exact.rs` copy) move to `MockJudgeBackend`; `register_scripted_backends` keeps signature and shorthand and builds mocks. Mock contract extended with `with_provider_meta`, `validations()`, `refusing(JevError)` (fails `validate_for_send`) beside `with_failure` (fails `judge`). Recorded as the ADR-SAG-0022 r-1 amendment, which names the superseded ADR-SAG-0029 (b) sentence; r-2 facade matches. | r-1 D10.11; arch ADR-0022 r-1; r-2 D2.9; plan ADR ownership |
| CPR-206 (closes CPR-108 a) | Applied: r-2b owns `crates/sc-jev/src/client/**`; r-3 leaves `JevClient` in `client/mod.rs`; boundary-map row and r-2b → r-3 must-follow updated. `sc-jev-backends` needs only `backend.rs`/`wire.rs` (owned). | r-2b owned_paths, D5; plan boundary map, must-follow |
| CPR-207 | Applied: `[[test]] name = "cli" required-features = ["cli"]` in both binary crates (r-4), `argv` the same in r-3c and r-3b; r-4, r-3c validation and phase acceptance run the real CI command `cargo hack --each-feature test -p sc-treesitter`. | r-4 D2, D3, validation; r-3c D2; r-3b D12.1, Manifests; plan |
| CPR-208 | Applied: r-2b D5 defines the scripted-answers schema (a-10 shared form unchanged; per-backend form `{"backends": {id: answers or {"failure": {error, kind, stage}}}}` → `with_failure` / `refusing`) with four `scripted` tests; r-3b D3.3 says the flag and `run_check_scripted` take that JSON. `execute_with_overrides` is **deleted** (one library seam, one binary seam); its clock/TTL case moves to `sc-jev-check` session tests. | r-2b D5, acceptance, validation; r-3b D3.3; arch ADR-0022 r-2b |
| CPR-209 (closes CPR-109) | Applied: `cargo doc -p sc-agent-http` **with** deps; grep fails on `ureq/<kind>.` hrefs or `title="<kind> ureq::` titles; r-3 records a `_canary() -> ureq::Agent` run that fails; r-3b CI and phase acceptance use the proven form. | r-3 D1, acceptance, validation; r-3b D12.2, validation; plan |
| CPR-210 | Applied: ADR-SAG-0029 index row (Active, a-10) and its markers on 0020/0022 added; a short 0029 section points at PR 20 (text not copied); r-1 ADR-0020 amendment names the superseded 0029 (a) sentences (test-util `new` removed, tests use `try_new`; `#[allow]` → `#[expect]` in r-1, files deleted in r-3); duplicated r-3 "Correction" deleted; r-2 ADR-0022 amendment names 0029 (b) where it supersedes it; plan.md records the PR route (plan PR → `integrate/phase-a`), merge order (PR 20 first, then rebase, then plan PR before r-1/r-4 branch) and the keep-both-markers rule. The actual rebase is a merge step, not doc text. | arch index, ADR-0020, ADR-0022, ADR-0029 stub; plan intro, ADR-0029 paragraph |
| CPR-211 | Applied with R3-001: REQ-SAG-0701 amendment row `(Proposed, r-1)`; r-1 lists 0700/0701 with `test_error_response_stderr_golden` and `cargo test -p sc-ai --test cli`; `docs/sc-ai/requirements.md` 0701 row updated by r-3b; "adding a key to `details` (or top level) is breaking under `deny_unknown_fields`" stated. | req; r-1 D6.7, acceptance; r-3b ADR text; arch ADR-0027 |
| CPR-M201 (closes CPR-M102) | Applied: retry decision dated 2026-11-06 everywhere (source `PHASE-A-REVIEW.md:295`). | plan (b); r-3 D2; arch ADR-0007 r-3; req REQ-SAG-0505 |
| CPR-M202 | Applied: `sha2` moves to core (`Sha256Digest::of`); gating `of` behind `judge` was rejected because the `sc-treesitter` library (`default-features = false`) calls it; pinned in `test_no_default_features_dep_set`. | r-1 D7.2 |
| CPR-M203 | Applied: `PayloadBundle::question_choices()` (set by `encode`; every bundle goes through `encode`) and `question_ids()`. | r-1 D10.2 |
| CPR-M204 | Applied: r-3b owns `ADR index row ADR-SAG-0023 (Status cell: compose pin)`; r-4 owns the r-4 Status cell. | r-3b, r-4 owned_paths |
| CPR-M205 | Applied with RBQA-F105: every Amends / amended-by cell filled in the plan PR by one rule; sprints flip only Status and marker text; plan-QA check added. | plan conventions; arch index |
| CPR-M206 | Applied: `RunResponse::{first_failure, is_partial, with_warning}` moved to a new r-3b → r-2 row; r-2b → r-3 row gains `test_util::{RecordingTransport, ScriptedTransport}`. | plan must-follow |
| CPR-M207 | Applied with RBQA-F107: needles built from fragments; no path excluded; rustdoc prose avoids the literal. | r-3b D7.3; plan lint hygiene |
| CPR-M208 | Applied: `-e normal,features` for the `sc-treesitter` and `sc-agent-http` feature rows. | r-3c; r-3; plan gate table |
| CPR-M209 | Applied: r-1 names the a-10 function `registry::registry_for_repo` (renamed `from_env` by r-2). | r-1 D1.6 |
| CPR-M210 | Applied: self dev-dependencies carry no `version` key (convention + each sprint that adds one). | plan Test seams; r-1, r-2, r-2b, r-3, r-3b; arch ADR-0027 |
| CPR-M211 | Applied: `Envelope::ask(outcome: &JudgeOutcome, session_id: &SessionId) -> Envelope`; `error::tests::test_error_code_all_is_complete` (exhaustive in-crate match). | r-2 D2.6; r-1 D6.2, validation |
| CPR-M212 | Applied: r-4 records the clap/mimalloc boundary-manifest deferral to r-3c/r-3b and states it in the PR. | r-4 D2, acceptance |

## arch-qa

| Id | Disposition | Where |
| --- | --- | --- |
| ARCH-015 | Applied: r-1 `adrs` += ADR-SAG-0004, ADR-SAG-0023 (and ADR-SAG-0029 for the superseded sentences). | r-1 front matter |
| ARCH-016 | Applied: r-5 `adrs` += ADR-SAG-0004, ADR-SAG-0007. | r-5 front matter |
| ARCH-017 | Applied: r-3b `adrs` += ADR-SAG-0002, ADR-SAG-0015, ADR-SAG-0020 (and ADR-SAG-0029 for the Python seam). | r-3b front matter |
| ARCH-018 | Applied: r-1 and r-3 ADR-0020 amendments name the superseded 0029 (a) sentences; r-3 "Closes findings" cites 0029 (a) and relies on its no-features sentence rather than restating it. | arch ADR-0020 r-1, r-3; r-1 D10.10, lint hygiene; r-3 Closes findings |
| ARCH-019 | Applied: r-3b D3.3 states Python injection is `_testing.run_check_scripted` per 0029 (b) (r-3b has no D6; the seam paragraph is D3.3); plan "Test seams" records the maturin-feature exception; r-2's sc-jev-py edit anchored by section "Contract"; merge note and keep-both-markers resolution recorded. | r-3b D3.3; plan Test seams, intro, ADR-0029 paragraph; r-2 owned_paths, ADR text |

## req-qa

| Id | Disposition | Where |
| --- | --- | --- |
| R3-001 | Applied: see CPR-211; plus r-1 owned_paths requirements row and plan.md amendment list. | req; r-1; plan REQ closure |
| R3-002 | Applied: `req:` rows with command and expected result: REQ-SAG-0505 and REQ-SAG-0130 (r-3 bullet) in r-3; REQ-SAG-0711 and REQ-SAG-0130 (r-3b bullet) in r-3b; REQ-SAG-0601 in r-2; REQ-SAG-0101 (and 0103, 0701) in r-1. | r-1, r-2, r-3, r-3b acceptance |
| R3-003 | Applied: (a) r-1 lists REQ-SAG-0600, 0610, 0700 with `sessions_exact`, `--test obs`, `--test cli`; (b) NFR-SAG-0001/0002/0003 mapped in r-2, r-2b, r-3b boundary rows and validation (r-3b maps NFR-0003 to compose `--test paths`, which tests the log/session roots, rather than `sessions_exact`); (c) REQ-SAG-0700/0701 dropped from r-2b, REQ-SAG-0710 mapped to `test_run_response_contract`; (d) REQ-SAG-0603 and 0611 added to r-3b with commands; r-2's unmapped 0305/0602/0700/0701 mapped too. | r-1, r-2, r-2b, r-3b |
| R3-004 | Applied: REQ-SAG-0101 and 0103 `(Proposed, r-1)` rows in the crate requirements; plan sentence names them. | types-req; plan REQ closure |
| R3-005 | Applied: r-4 rebases onto `origin/integrate/phase-a` and diffs `main.rs` against `git merge-base HEAD origin/integrate/phase-a`. | r-4 validation, acceptance |

## ruthless-boundary-qa

| Id | Disposition | Where |
| --- | --- | --- |
| RBQA-F101 | Applied: spike moved to wave 1 as **r-1 D0** (first step), not r-4, because r-1 writes the first self dev-dependency and merges before r-4, so it cannot wait on an r-4 result. Covers a dev-only cross-crate edge, a self edge, and `allowed_*` entries with no normal edge; result in `boundaries/README.md` before wave 2; three written outcomes say what each self edge does (ignored: nothing; counted + accepted: self in `allowed_*`; counted + rejected: no self dev-deps, `[[test]] required-features = ["test-util"]` and `--features test-util` lines). r-3b D7.0 applies the result. | r-1 D0, acceptance, validation; r-3b D7.0; plan Test seams, manifests; arch ADR-0028 |
| RBQA-F102 | Applied: `clippy.toml` gains `std::process::Command::new` (in r-3b, after r-2/r-3c moved every `git` caller); `test_disallowed_methods_expect_sites` limits production expect sites to `sc-agent-types/src/git.rs` and the command extractor (plus `ureq_transport.rs` for `as_str`, and test files that do not mention `ResolvedApiKey`). Test files are allowed because `--all-targets` lints test spawns of built binaries and fixture repos; listing only two sites would be false. `test_no_allow_attributes` reads attributes whole (whitespace, multi-line, `cfg_attr`) and bans an `expect(..)` of `warnings`, `clippy::all` or `clippy::restriction`. `test_lint_mechanism_present` asserts both clippy.toml entries, the two workspace lint keys, `resolver = "2"`, `members` under `crates/`, and the `sc-lint.toml` clippy command (`just lint` delegates there, so that is the real place to assert `--all-targets`). `tokio::process` is not a dependency. | r-3b D7.3, owned_paths, validation; plan resolution 12, boundary map |
| RBQA-F103 | Applied: `test_release_binary_rejects_scripted_answers` runs a release-built `sc-ai` with `--scripted-answers x`; `bin_path` always builds `--release` into an isolated `target/workspace-tests-bin`. Deviation: a fixed isolated directory instead of a per-test tempdir, so each test does not rebuild `sc-ai`/`sc-treesitter` from scratch; the isolation from `cargo test`'s uplift path is the same. `resolver = "2"` asserted (mechanism scan); release wheel imported and checked for no `_testing`. | r-3b D7.2, D7.3, D12.2, validation; plan Test seams, phase acceptance; arch ADR-0028 |
| RBQA-F104 | Applied: compose `allowed_dependents` marks `workspace-tests` conditional; workspace-tests TOML written in full (all blocks), compose blocks stated as copied; the D0 spike loads `facade = ""` (fallback facade named); dev-set gate is an exact set comparison with third-party names filtered. | plan manifests, gate table; r-1 D0 |
| RBQA-F105 | Applied with CPR-M205. ADR-SAG-0023 Amends cell lists 0016, 0017, not 0005: the 0005 r-4 subsection is the D13 kinds sentence, not a pin decision, so it is an "A.1 subsection" entry. | arch index; plan conventions |
| RBQA-F106 | (a) Applied: `sc-agent-http` `secret`-only feature row in the table and r-3b script. (b) Applied: `cargo tree -d` rows for `sc-ai` and `sc-jev-py`. (c) Declined: cosmetic, as the finding says; the closed `allowed_dependencies` lists already exclude `sc-agent-http`/`clap` as workspace edges and the `sc-lint.toml` records cover the third-party names. (d) Applied: `test_env_reads_only_in_owners` (text scan, limit stated). | plan gate table; r-3b D7.3, D12.2, validation |
| RBQA-F107 | Applied with CPR-M207; from r-3b on `source_scans` replaces the `rg` gate, so the phase run has no separate `rg` line. | r-3b D7.3; plan lint hygiene, phase acceptance; r-5 validation |

## extensibility-qa

| Id | Disposition | Where |
| --- | --- | --- |
| EXT-030 | Applied: consumer rule (a `schema_version` other than `1` = unknown shape; fall back to exit code and raw line; bumps announced in the changelog) in ADR-SAG-0027 and r-1 D6.7; r-3b adds it to `docs/sc-ai` and `docs/sc-jev-py`. | arch ADR-0027; r-1; r-3b D2.4 |
| EXT-031 | Applied: `DefaultRoot` / `SessionDefault` are data-carrying enums with `Fixed(PathBuf)`; `PathPolicy` has public fields; `test_custom_path_policy_resolves`. | r-3b D2.2, acceptance, validation |
| EXT-032 | Partial, as directed (consistent with RBQA-F102): kept the mechanism scans and the one robust allow check (`test_no_allow_attributes` now catches forms clippy and `rg` miss, and replaces the `rg` gate from r-3b on); dropped the text-match `test_git_spawn_only_in_types_git` (superseded by the type-aware clippy rule plus the expect-site scan); dropped `docs/workspace-tests/**` for one README paragraph per target. Declined: folding ADR-SAG-0028 away, because the ADR index, the r-3b `adrs` lists and arch-qa's ARCH-017 depend on it; compose's per-crate docs stay because compose is published. | r-3b D7.3, ADR text, owned_paths; plan boundary map; arch ADR-0028 |
| EXT-033 | Applied: type note in r-3b D2 (compose README/rustdoc) and ADR-SAG-0026 compose half. | r-3b D2.2, D2.4; arch ADR-0026 |

## rust-best-practices-agent (minor notes)

| Id | Disposition | Where |
| --- | --- | --- |
| RBP minor 1 | Applied: `JudgeOutcome::new` also checks each `choice` and `probabilities` key is one of the question's choices; `test_outcome_rejects_choice_not_offered`. | r-1 D10.2, acceptance, validation |
| RBP minor 2 | Applied as CPR-202. | r-1 |
| RBP minor 3 | Applied: gates run on plain `cargo build` / `cargo tree`, never a test build (stated in r-3b D12.2; `test_util_off` is a graph query plus a release build). | r-3b D7.3, D12.2 |
| RBP minor 4 | Applied: the factory test also fails on a duplicate `BackendId` row. | r-1 D1.6, acceptance |

## rust-service-hardening-agent (minor notes)

| Id | Disposition | Where |
| --- | --- | --- |
| RSH note a | Applied: `test_exhaustion_reason_distinguishable` (attempts vs budget, by detail text, both `Busy`, retryable). | r-3 D2, acceptance, validation |
| RSH note b | Applied: 32 nines in the `test_retry_after_grammar` table, valid and saturating to the 30 s clamp. | r-3 D2 |
| RSH note c | Applied: `"0"` in the grammar table (zero wait, bounded by the 4-send cap). | r-3 D2 |

## Decisions for the owner to check

1. `ScriptedJudge` / `RequestLog` deleted (not kept as wrappers) — r-1 D10.11.
2. `sc_ai::execute_with_overrides` deleted; `sc-ai` tests become binary-level — r-3b D3.3.
3. Jev `confidence` modelled in `QuestionAnswer` (option (a)) — r-1 D10.1.
4. Spike in r-1 (D0), not r-4 — r-1.
5. `std::process::Command::new` becomes a clippy disallowed method from r-3b, with test-file expect sites allowed — r-3b D7.3.
6. The plan PR targets `integrate/phase-a`, merging after PR 20 — plan intro.

## Round-4 verification additions (arch-qa)

| id | disposition | where |
| --- | --- | --- |
| ARCH-020 | applied — ADR-SAG-0029 added to r-2b `adrs` | r-2b frontmatter |
| ARCH-021 | applied — ADR-SAG-0029 added to r-3c `adrs`; `adr:ADR-SAG-0029` acceptance row keeps the 0029 (c) error-code dispositions | r-3c frontmatter, acceptance |
