# Phase A.1 plan review round 1 — dispositions

Reviewed commit: `df935b5`. Reports:
`.cursor/phase-a-orchestration/refine-plan/plan-qa1/*.json` (seven reviewers).
Every finding is applied, declined with a reason, or settled by an owner
decision. Conflict rule: critical-plan-reviewer structure and arch-qa ADR
sequencing win; other design points are applied unless they contradict an
accepted D decision. "Where" names the file (`plan` = `plan.md`, `r-N` = that
sprint doc, `arch` = `docs/architecture.md`, `req` = `docs/requirements.md`).

## Summary

| Reviewer | Items | Applied | Declined | Owner decision |
| --- | --- | --- | --- | --- |
| critical-plan-reviewer | 30 | 28 | 0 | 2 |
| arch-qa | 15 | 15 | 0 | 0 |
| req-qa | 12 | 11 | 0 | 1 |
| ruthless-boundary-qa | 13 | 12 | 1 | 0 |
| rust-best-practices-agent | 17 | 16 | 1 | 0 |
| rust-service-hardening-agent | 11 | 10 | 0 | 1 |
| extensibility-qa | 8 | 7 | 1 | 0 |
| **Total** | **106** | **99** | **3** | **4** |

Owner decisions: (a) publish `sc-agent-compose` — decided 2026-11-06; (b)
busy-retry numbers — decided 2026-11-07; (c) D9 payload shape — still pending
(blocked on the vendor). Partially declined items (RBQA-F004, RBQA-F005, RBQA-F006, RSH-006) count as
applied because the finding's required outcome is met; the declined part and
its reason are named in the row.

## critical-plan-reviewer

| Id | Disposition | Where |
| --- | --- | --- |
| CPR-001 | applied — r-1 owns every `crates/*/tests/**` it breaks; thin `operational_io`/`operational_config` kept so neither `main.rs` changes | r-1 frontmatter, D6.1; plan "Why r-4 runs in wave 1" |
| CPR-002 | applied — infallible ctors + `with_default_transport` hoisted to r-1; r-3 touches no `sc-jev-check` file; r-3 owns all of `crates/sc-jev/**`, `crates/sc-jev-backends/**` in wave 2; r-2b (wave 3) owns `backend.rs` after r-3 | r-1 D1.7; r-3 frontmatter; r-2b |
| CPR-003 | applied — r-2b engine-only; flag, exit 2, kwarg, `.response`, `.pyi` moved to r-3b; r-3b must_follow r-2b | r-2b, r-3b D3 consumers; plan wave table |
| CPR-004 | applied — static facts on `BackendDescriptor`; provided `capabilities()`; registry factory is the single allowlisted dispatch; `prepare` reads descriptors | r-1 D1.1–1.6; r-2 D2.3 |
| CPR-005 | applied — digest of canonical blob recovered from the final serialized body; faulty-wire-builder test on real backend types; hashing only in r-2b (r-1 adds `Option` field, `None`) | r-1 D10.3; r-2b D5–D6 |
| CPR-006 | applied — each augmented request re-validated before its send; refusal → `Failed`, `leak = true`; ADR-SAG-0004 and 0025 amended | r-2b D2; arch ADR-0004 r-2b, ADR-0025 |
| CPR-007 | applied — lint-enforced convention stated honestly; `Token`/`__transport` dropped | r-3 D4; arch ADR-0020 r-3 |
| CPR-008 | applied — session document version 2; Phase A files → `SESSION_DOCUMENT_INVALID` + re-run hint; fixture test; alias rationale dropped (alias kept for config input) | r-1 D7.6, D1.8; plan resolution 4, 7 |
| CPR-009 | applied — variant → id table with replaced ctor; golden test constructing each variant; REQ-SAG-0101 amended | r-1 D6.1–6.2; req amendments |
| CPR-010 | applied — one `ProducerError { producer, code, message }`; round-trip test | r-1 D6.6 |
| CPR-011 | applied — JSON samples (single Jev, Jev+Haiku, Jev failed + Haiku ok, dry-run); primary-backend rule; backends precedence stated | r-2 D2.2, D2.6; arch ADR-0024 |
| CPR-012 | applied — dry-run reports `payload_bytes` + per-backend `over_limit`; ADR-0024 + REQ-SAG-0314; test | r-2 D13.2; arch ADR-0024; req REQ-SAG-0314 |
| CPR-013 | applied — `InputSpec` leaves the types crate (boundary reviewer Q4), so `Command.path` is not a types break; `RepoRelativePath` (the types part) is in r-1 | r-1 D7.4, D10.9; r-2 D11.1 |
| CPR-014 | applied — publish gate (no crates.io publish until D9 resolves); Branch B made additive by r-1's private fields; exact serde stated | plan decision (c); r-5 |
| CPR-015 | applied — section-scoped ownership with per-bullet `(Proposed/Active, r-N)` markers; ADR-0007 flips in r-3; ADR-0005 `prepare` clause in r-2 | plan conventions; every sprint frontmatter; arch |
| CPR-016 | applied — REQ-SAG-0134 (mimalloc, r-4) split from new REQ-SAG-0135 (clap, r-3b); `cargo tree` assertions in CI owned by r-3b | req; r-4 D3; r-3b D12.2 |
| CPR-017 | applied — CI file is `.github/workflows/sc-agent-ci.yml` | r-4, r-3b owned_paths |
| CPR-018 | applied — every named test has `--test <target>` + top-level fn, `1 passed` rule; `cross_impl_branches` run as a target; parity test named in workspace-tests | plan conventions; all sprint validation blocks; r-3b D7.3 |
| CPR-019 | owner decision (decided) — "Underspecified decisions and owner confirmations" section added with all twelve resolutions; compose publication confirmed 2026-11-06 | plan |
| CPR-020 | applied — r-1 amends REQ-SAG-0103 + types architecture doc (one unfreeze); r-2 amends REQ-SAG-0601 and supersedes `run_check` sentences in ADR-0009/0022 | r-1 outcome; r-2 ADR text; arch; req |
| CPR-021 | applied — ADR-SAG-0003 r-2 amendment + ADR-0024 bullet; Python subdirectory test | arch ADR-0003 r-2; r-2 D2.4 |
| CPR-022 | applied — e2e via `cargo build --message-format=json`; sc-treesitter clap split to r-3c (wave 2); r-3b rated XL, r-2 L | r-3b D7.2; r-3c; plan sizes |
| CPR-023 | applied — boundary-map table; `vertical_rationale` for r-1, r-2, r-3, r-3b; owned_paths re-derived | plan "Boundary map" |
| CPR-M1 | applied — r-5 wave 5 everywhere; must_follow includes r-3b | r-5; plan |
| CPR-M2 | applied — one owner per finding id; `RBP-F001 (phase-end)` vs `RBP-F001 (a-9)` disambiguated | plan "Findings closure" |
| CPR-M3 | applied — rendered `release/**` and `*-publish.yml` owned by the sprint that re-runs `install.py` (r-4, r-3, r-3b) | plan conventions; owned_paths |
| CPR-M4 | applied — pin check re-run in r-3b; mimalloc tree check on `sc-ai` too | r-4 D3; r-3b D12.2 |
| CPR-M5 | applied — r-3 → r-4 edge removed (same-file rationale was invalid) | plan wave table; r-3 |
| CPR-M6 | applied — `Failed` keeps the typed `JevError` in memory; `session_id` is `None` when `session_saved` is false | r-2 D2.5 |
| CPR-M7 | owner decision (decided 2026-11-07) — malformed Retry-After treated like missing with exponential backoff, so 4 sends fit the budget | plan (b); arch ADR-0007; req REQ-SAG-0505 |

## arch-qa

| Id | Disposition | Where |
| --- | --- | --- |
| ARCH-001 | applied — ADR-0026 split: HTTP half Active in r-3, compose half r-3b | arch ADR-0026; r-3, r-3b |
| ARCH-002 | applied — ADR-0005 narrowing split: kinds sentence r-4 (no ADR-0024 reference), `prepare` sentence r-2 | arch ADR-0005; r-4 D6; r-2 |
| ARCH-003 | applied — r-2 bullets supersede `run_check` in ADR-0009 and ADR-0022; ADR-0024 header "Amends 0005, 0009, 0019, 0022"; 0009 in r-2 adrs | arch; r-2 |
| ARCH-004 | applied — ADR-0011 and ADR-0013 r-3b amendments; r-3b adrs list completed | arch; r-3b |
| ARCH-005 | applied — r-2 adrs add 0001, 0005, 0008, 0009, 0013, 0014, 0015, 0027 | r-2 |
| ARCH-006 | applied — r-1 adrs add 0002, 0006, 0008, 0013, 0015, 0019 (plus 0003, 0005, 0007, 0020, 0022) | r-1 |
| ARCH-007 | applied — r-2b adrs add 0001, 0002, 0004, 0012, 0024 (0010/0011 moved with the consumer work to r-3b); r-3 adds 0001, 0003, 0012, 0017, 0018; r-5 adds 0006, 0012 | r-2b, r-3, r-5 |
| ARCH-008 | applied — ADR-0017 r-3 and r-3b amendments (published library set, publish order); ADR-0023 compose pin "effective when the crate lands (r-3b)"; ADR-0016 r-3b reconciles "do not publish a separate observability crate" | arch |
| ARCH-I1 | applied — ADR-0016 and ADR-0019 amendments split per landing sprint, one Status line each | arch |
| ARCH-I2 | applied — see ARCH-002 / ARCH-008 | arch |
| ARCH-I3 | applied — section-scoped ownership in every ADR/REQ-landing sprint | plan conventions; frontmatter |
| ARCH-I4 | applied — r-3 no longer edits `sc-jev-check`; r-3b follows r-2b | plan wave table |
| ARCH-I5 | applied (pass; no change) — a-1 freeze override kept and made explicit with re-freeze | r-1 outcome |
| ARCH-I6 | applied — ADR-0026 states ADR-0012 still holds; ADR-0025 carve-out: re-encoding is attestation-only | arch ADR-0026, ADR-0025 |
| ARCH-I7 | applied — plan frontmatter `adrs` is the union (0001–0027); `docs/sc-jev-py/architecture.md` owned by r-2 (line 25) and r-3b (compose) | plan; r-2; r-3b |

## req-qa

| Id | Disposition | Where |
| --- | --- | --- |
| REQ-QA-001 | applied — ARCHC-006, ARCHP-009 in "does not close" with Phase B rationale; ARCHC-010 owned by r-1; phase-level closure table | plan "Findings closure" |
| REQ-QA-002 | owner decision (decided 2026-11-06) — publish `sc-agent-compose` as internal support | plan (a); arch ADR-0026, 0016, 0017, 0023; r-3b |
| REQ-QA-003 | applied — section-scoped ownership in every sprint | plan conventions |
| REQ-QA-004 | applied — r-3 disjoint from r-2 (r-1 fixed the ctor contract) | r-1 D1.7; r-3 |
| REQ-QA-005 | applied — consumer behaviour moved to r-3b, which follows r-2b | r-2b, r-3b |
| REQ-QA-006 | applied — one owner per id (EXT-002, RBQA-F005, ARCHP-002, CR-001 → r-3b; others "contributes"); RBP-F001 disambiguated; RBP-F004 note part credited to r-2 | plan closure table |
| REQ-QA-007 | applied — `--test <target>` + top-level fns; `1 passed` rule; every acceptance-named test listed | all sprint validation blocks |
| REQ-QA-008 | applied — full ordered multi-package dry-run in every sprint (crates that exist at that point) | all sprints |
| REQ-QA-009 | applied — REQ-SAG-0135 clap gates in r-3b, which owns `sc-agent-ci.yml` in wave 4; NFR-SAG-0004 pin check re-run in r-3b | r-3b D12.2; req |
| REQ-QA-010 | applied — requirement lists completed as listed | r-1, r-2, r-2b, r-3b frontmatter |
| REQ-QA-011 | applied — per-backend cap recorded with the alternative as planner resolution 1; dry-run behaviour in REQ-SAG-0314; one constant per backend read via the descriptor | plan resolutions; req; r-1 D1.7 |
| REQ-QA-012 | applied — Repository Exception recorded (rename not taken: mid-review branch rename churns PR 19); r-5 wave consistent | plan header; r-5 |

## ruthless-boundary-qa

| Id | Disposition | Where |
| --- | --- | --- |
| RBQA-F003 | applied — lint-enforced wording + named gates; `with_default_transport` ctor; r-3 owns the `ureq` record; `cargo tree` ureq gates; full `sc-agent-http` manifest; no `ureq` in pub signatures | r-3 D1, D4, D5; plan manifests |
| RBQA-F004 | applied in part — `BackendCapabilities` ctor; one limit per backend read from the descriptor (enforced == reported by construction). Declined: `VendorPin` absence grep — a-10 deleted the type and the compiler rejects any reference | r-1 D1.3, D1.7 |
| RBQA-F005 | applied in part — `sc-observability*` ownership record → compose; depth-1 `cargo tree` gates; full manifest text; compose facade. Declined: sc-lint reference rule on `SC_LOG_ROOT`/`SC_AGENT_LOG_DIR` reads — sc-lint 0.8.0 accepts `references.forbidden` but does not enforce it (manifest comments); the deleted path-default code is the guard | r-3b; plan |
| RBQA-F006 | applied in part — `sc-agent-types::git` (`read_at_rev`, `discover_toplevel`) with `--end-of-options`, `--no-textconv`, `GIT_TERMINAL_PROMPT=0`, bounded read; parity + `--output=FILE` regression; owner choice justified. Declined: sc-lint rule forbidding `Command::new("git")` elsewhere — same sc-lint 0.8.0 limit | r-1 D7.5; r-2; r-3c; r-3b `git_read_parity`; plan resolution 12 |
| RBQA-F009 | applied — cargo features (`judge`/`secret`/`obs`/`git`, `sc-treesitter` `default-features = false`); `InputSpec` moved to `sc-jev-check`; `cargo tree` gate for ureq/pyo3/tree-sitter; shim-removal keeps `LogRejected`/`SessionLog` | r-1 D7.2, D7.4; r-3c; plan |
| RBQA-F010 | applied — typed answers (`BTreeMap<QuestionId, QuestionAnswer>`; `Value` only for `provider_meta`), `commands` replaced by typed attestations, `payload_bundle_from_value` replaced by `PayloadBundle::encode`; all in r-1's break | r-1 D10 |
| answer: new_crate_manifests | applied — full manifests for http, compose, workspace-tests; workspace-tests drives built binaries, no pyo3/tree-sitter | plan "New boundary manifests" |
| answer: leaks | applied — single ownership-record table; tree-sitter forbidden edges in compose/workspace-tests; ureq gates | plan |
| answer: InputSpec_placement | applied — moved to `sc-jev-check::config` | r-1 D7.4; r-2 D11 |
| answer: sealing_enforceable | applied — honest lint-enforced wording; `ResolvedApiKey` `Clone` is cheap and redacted (`Arc<str>`), stated | r-3 D4; r-1 D10.10 |
| obs: grep test for BackendId match | applied — replaced by `clippy::wildcard_enum_match_arm` deny + one `#[expect]` + `test_every_backend_has_factory` | r-1 D1.6 |
| obs: cross-sprint file overlap | applied — re-cut (see CPR-001..003) | plan wave table |
| obs: boundary validator report in QA input | declined as a separate artifact — `just lint` already runs `sc-lint lint sc-boundary`; its output is attached to the QA input, no new report file | plan conventions |

## rust-best-practices-agent

| Id | Disposition | Where |
| --- | --- | --- |
| RBP-F101 | applied — `prepare(source, RunConfig)` by value, `run(PreparedRun)` | r-2 D2.1 |
| RBP-F102 | applied — `Failed(BackendFailure)` = `ScAgentErrorResponse` + in-memory `JevError`; `from_error_response` limited to producer stderr | r-2 D2.5; r-1 D6.6 |
| RBP-F103 | applied — `Attestation` enum (`Matched`/`Mismatched`/`Unattested`/`Failed`/`NotSent`); private fields; dry-run value stated | r-1 D10.5 |
| RBP-F104 | applied — `BackendCapabilities` ctor; per-type `#[non_exhaustive]` triage table; `JudgeBackend` intentionally open, future methods defaulted | r-1 D1.3–1.4, triage table; arch ADR-0009 r-1 |
| RBP-F105 | applied — ADR-0020 says lint-enforced convention; acceptance row fixed. Alternative `SecretHeader::apply(&mut dyn HeaderSink)` not taken (honest wording chosen) | r-3 D4; arch |
| RBP-F003 | applied — `RepoRelativePath` in r-1 for `InputSpec::{File,Command}.path`, `Subject.path`, `push_subject` | r-1 D10.9 |
| RBP-F007 | applied — `HttpRequest.body: Arc<[u8]>`, `ResolvedApiKey` on `Arc<str>`, `JudgeRequest` holds `Arc<PayloadBundle>`, `judge` does not re-validate | r-3 D1; r-1 D10.6, D10.10 |
| RBP-F011 | applied — `Operation` enum, `QuestionId`, `BlobSha`, struct for the bool pair; plan reflects a-10 private `PayloadBundle` | r-1 D10.9; plan preconditions |
| RBP-F106 | applied — `Backend<T: Transport = UreqTransport>`; clock is a fn pointer; sleeper folded into `BusyPolicy` (test-only) | r-3 D1, D3; r-2 D4.1 |
| RBP-F107 | applied — variant-level `#[non_exhaustive]`; `fix_hint` sole renderer; `detail` added to `RegistryInvalid`/`SessionDocumentInvalid`; source rule per class | r-1 D6; arch ADR-0027 |
| RBP-F108 | applied — `Endpoint` https newtype; typed `CallerRequest`; `BackendSet` non-empty/unique | r-3 D1; r-2 D2.1; r-1 D1.5 |
| RBP-F109 | applied — `ValidatedSubjects` via `serde(try_from)`; one `RunResponse` serialization | r-1 D7.3; r-2 D2.6 |
| RBP-F110 | applied — session document version bump; Phase A fixture test | r-1 D7.6 |
| RBP-F111 | applied — `capabilities()` derived from the descriptor; roundtrip test over serde/`Display`/`FromStr`; "open = additive" in ADR-0009; one byte unit (`u64`) | r-1 D1; arch ADR-0009 r-1 |
| RBP-F014 (carry-forward) | applied — `SessionLog` stated as an open port | r-1 D7.1 |
| note: naming drift `JevError::Network` | applied — one name, `NetworkError { kind, detail }`, everywhere | r-1 D6.1; arch |
| note: dry-run `equal` unspecified | declined as separate work — covered by RBP-F103 (dry-run `NotSent`, `equal = false`) | r-1 D10.5 |

## rust-service-hardening-agent

| Id | Disposition | Where |
| --- | --- | --- |
| RSH-001 | applied — env allowlist contract stated in D11 | r-2 D11.2 |
| RSH-002 | applied — regular file, canonicalized, `starts_with` root (a-10 rule) for `Command.path`; a-10 is a precondition; symlink test | r-2 D11.1; plan preconditions |
| RSH-004 | applied — no-lock model (create_new tmp pid+random, fsync, rename, fsync parent, remove on error, last writer wins, `ask` does not write) + two-store and stale-tmp tests | r-2 D4.3; arch ADR-0024 |
| RSH-005 | applied — `CancelToken` before each attempt with sliced sleeps, via `RunConfig::cancel` for sc-jev-py; sleeps capped to budget; 120 s timeout covers body read | r-1 D10.7; r-3 D1–D2; r-2 D2.1 |
| RSH-006 | applied in part — `Agent` built once in `new()` stated and tested. Declined: loopback black-hole connect-timeout test — a non-routable address behaves differently per CI runner (flaky); connect timeout is a `ureq` config value covered by a unit assertion | r-3 D1 |
| OBS-A | applied — see CPR-006 | r-2b D2 |
| OBS-B | applied — judge once per backend; only BusyPolicy retries; no resend after body sent; save failure keeps answers with `session_saved = false`; 502 risk documented; tests | r-2b D4; r-3 D2 |
| OBS-C | applied — deny `allow_attributes` and `allow_attributes_without_reason`; `secret_read_site` test; guarantee stated as lint + test | r-3 D4 |
| OBS-D | applied — redaction once in `send_with_busy_retry` | r-3 D1 |
| OBS-E | applied — 8 MiB streaming cap incl. error bodies; Retry-After length bound; `http://` only under `cfg(test)`; release-build test | r-3 D1 |
| OBS-F | owner decision (decided 2026-11-07) — malformed treated like missing, exponential backoff from 1 s | plan (b); arch ADR-0007 |

## extensibility-qa

| Id | Disposition | Where |
| --- | --- | --- |
| EXT-012 | applied — one `PathPolicy` data type with `cli()`/`python()` presets; `ComposeInputs` fields defined | r-3b D2; arch ADR-0026 compose |
| EXT-013 | applied — one serialization; `Envelope` beside `RunResponse`; optional `schema_version`/`retryable`/`details` on `ScAgentErrorResponse` in r-1 | r-2 D2.6; r-1 D6.7 |
| EXT-014 | applied — `Failed` carries `ScAgentErrorResponse`; `first_failure()`/`is_partial()` | r-2 D2.5 |
| EXT-015 | applied — ADR-0009/0027 state "minor types change"; built-ins registered by `sc-jev-check`'s factory; compile-time lint instead of grep. `register(descriptor_id, factory)` taken as `register(Arc<dyn JudgeBackend>)` + the one factory | r-1 D1.6; r-2 D2.8; arch |
| EXT-016 | applied — public `BackendRegistry::from_env` / `from_resolver`; `compose::ask_from`; registry injection documented as the no-secrets path | r-2 D2.8; r-3b D2 |
| EXT-017 | applied — `PreparedRun` owns the config | r-2 D2.1 |
| EXT-018 | declined as proposed (`pub(crate)` accessor re-exported from `sc-agent-http`): `sc-agent-secrets` returns `ResolvedApiKey` and may not depend on `sc-agent-http`; the `Token` ceremony is dropped and the lint convention is stated honestly instead | r-3 D4 |
| EXT-019 | applied — ADR-0027 states module paths are the stable surface for a later split | arch ADR-0027 |
