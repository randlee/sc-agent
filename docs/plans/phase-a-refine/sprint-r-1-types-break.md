---
id: r-1
phase: r
status: planned
closure_type: contract
target_boundary: agent-types
track: foundation
wave: 1
size: XL
branch: sprint/r-1-types-break
worktree: sprint/r-1-types-break
recommended_agent: rust-architect
recommended_model: opus
must_follow: [a-10]
parallel_safe: [r-4]
vertical_rationale: "Contract break of the shared types crate; nothing is published, so the same sprint updates every call site (except both main.rs) and every test it breaks, keeping the workspace green. See plan.md boundary map."
requirements:
  - REQ-SAG-0105
  - REQ-SAG-0106
  - REQ-SAG-0107
  - REQ-SAG-0108
  - REQ-SAG-0101
  - REQ-SAG-0102
  - REQ-SAG-0103
  - REQ-SAG-0104
  - REQ-SAG-0305
  - REQ-SAG-0602
  - REQ-SAG-0711
  - NFR-SAG-0001
  - NFR-SAG-0002
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0003
  - ADR-SAG-0005
  - ADR-SAG-0006
  - ADR-SAG-0007
  - ADR-SAG-0008
  - ADR-SAG-0009
  - ADR-SAG-0013
  - ADR-SAG-0015
  - ADR-SAG-0016
  - ADR-SAG-0019
  - ADR-SAG-0020
  - ADR-SAG-0022
  - ADR-SAG-0027
owned_paths:
  - "crates/sc-agent-types/**"
  - "crates/sc-agent-secrets/src/**"
  - "crates/sc-agent-secrets/tests/**"
  - "crates/sc-jev/src/**"
  - "crates/sc-jev/tests/**"
  - "crates/sc-jev-backends/src/**"
  - "crates/sc-jev-backends/tests/**"
  - "crates/sc-jev-check/src/**"
  - "crates/sc-jev-check/tests/**"
  - "crates/sc-jev-py/src/**"
  - "crates/sc-ai/src/command.rs"
  - "crates/sc-ai/src/envelope.rs"
  - "crates/sc-ai/src/lib.rs"
  - "crates/sc-ai/src/observe.rs"
  - "crates/sc-ai/tests/**"
  - "crates/sc-treesitter/src/render.rs"
  - "crates/sc-treesitter/src/extract.rs"
  - "crates/sc-treesitter/src/lib.rs"
  - "crates/sc-treesitter/tests/**"
  - "tests/jev_cross_impl/**"
  - "tests/fixtures/jev-python/**"
  - "boundaries/sc-agent-types/**"
  - "docs/sc-agent-types/**"
  - "docs/architecture.md#ADR-SAG-0009 amendment (r-1)"
  - "docs/architecture.md#ADR-SAG-0016 amendment (r-1)"
  - "docs/architecture.md#ADR-SAG-0020 amendment (r-1)"
  - "docs/architecture.md#ADR-SAG-0027"
  - "docs/requirements.md#REQ-SAG-0101, REQ-SAG-0103 amendments; REQ-SAG-0105..0108 rows"
---

# Sprint r-1 — sc-agent-types break (D1, D6, D7 types, D10-break, D11 types)

## Outcome

One deliberate pre-1.0 break of `sc-agent-types`, carrying **every** types
change the phase needs, so no later sprint reopens the crate (r-5 Branch B is
additive by construction). After r-1 the backend set and error enum are open,
backend facts are static data on a descriptor, errors are explicit variants
with no string-prefix parsing, answers are typed, `sc-agent-types` links no
`sc-observability*`, and module seams are cargo features. All call sites and
tests in the workspace are updated in this sprint; neither `main.rs` changes,
because `JevError::operational_io` and `JevError::operational_config` stay as
thin constructors.

**Unfreezes the a-1 contract once** (sprint-a-1 deliverable 8, REQ-SAG-0103,
`docs/sc-agent-types/architecture.md`). The doc and REQ-SAG-0103 are amended
in this sprint to say the contract is re-frozen at r-1 merge.

## Closes findings

EXT-001, EXT-007, EXT-011, ARCHC-004, ARCHC-005, ARCHC-008, ARCHC-010,
ARCHC-011, ARCHC-021 (contract), ARCHE-005, ARCHE-022, ARCHP-001, ARCHP-003,
RBP-F001 (phase-end), RBP-F002 (taxonomy), RBP-F003 (`RepoRelativePath`),
RBP-F004 (catalog), RBP-F010, RBP-F011, RBQA-F006 (helper), RBQA-F009,
RBQA-F010; plan-review ids listed in `plan-review-1-dispositions.md`;
a-10 follow-ups "serde spelling of `openai_luna`" and "`encode_failed`/
`io_failed` code".

## Deliverables

### D1 — open backend set, static facts (REQ-SAG-0105, ADR-SAG-0009 r-1 amendment)

1. `ids.rs`: `BackendDescriptor` with **private** fields and `const fn`
   accessors: `id() -> BackendId`, `name() -> &'static str`,
   `aliases() -> &'static [&'static str]`, `key_id() -> Option<BackendKeyId>`,
   `family() -> BackendFamily`, `requires_llm_context() -> bool`,
   `supports_session_ask() -> bool`, `request_limit_bytes() -> u64`.
   `pub static BACKENDS: &[BackendDescriptor]` (rows `jev`, `haiku`,
   `openai_luna`; the two existing 96 000 limits move into the rows).
   `BackendId::descriptor() -> &'static BackendDescriptor`; `as_str`,
   `key_id`, `family`, `Display`, `FromStr` all read it. The one exhaustive
   `match` over `BackendId` is inside `ids.rs` (descriptor lookup).
2. `#[non_exhaustive]` on `BackendId`, `BackendKeyId`,
   `#[non_exhaustive] enum BackendFamily { TypeSafeJev, Llm, Local }`.
3. `judge.rs`: `BackendCapabilities` with private fields,
   `BackendCapabilities::from_descriptor(&BackendDescriptor)` and accessors.
   `JudgeBackend::capabilities(&self) -> BackendCapabilities` is a **provided**
   method returning `self.id().descriptor()`'s facts; implementors do not
   restate `family` or limits. Facts needed before any backend exists
   (`prepare`, dry-run, session-ask eligibility) are read from the descriptor.
4. `JudgeBackend` is **intentionally open** (not sealed): external crates may
   implement it; every method added after r-1 has a default body (rustdoc +
   ADR-SAG-0009/0027). `# Blocking` rustdoc section: calls block; async hosts
   use `spawn_blocking` (D10).
5. `BackendSet`: non-empty, duplicate-free `Vec<BackendId>` newtype with
   `TryFrom<Vec<BackendId>>` (`CONFIG_INVALID` on empty/duplicate) and
   `FromStr` for comma lists.
6. **Single allowlisted id dispatch.** The registry factory in
   `sc-jev-check::registry` (`fn build(descriptor: &BackendDescriptor, key: ResolvedApiKey) -> Arc<dyn JudgeBackend>`)
   is the only `match` on `BackendId` outside `ids.rs`; it carries
   `#[expect(clippy::wildcard_enum_match_arm, reason = "single allowlisted backend-id dispatch (ADR-SAG-0009)")]`.
   `sc-jev-check`, `sc-ai` and `sc-jev-py` set
   `#![deny(clippy::wildcard_enum_match_arm)]`, so any other match on the
   `#[non_exhaustive]` `BackendId` fails `just lint`. First task of the sprint:
   confirm the lint fires on a foreign `#[non_exhaustive]` enum and record the
   result in the PR; if it does not, raise it to the lead before building on it.
   `test_every_backend_has_factory` (iterates `BACKENDS`) replaces the
   round-1 grep test.
7. Backend constructors become infallible (validation is only
   `ResolvedApiKey::try_new`): `JevClient::new`, `AnthropicHaikuBackend::new`,
   `OpenAiLunaBackend::new` return `Self`. Each also gets
   `with_default_transport(key: ResolvedApiKey) -> Self`; the registry factory
   calls only these, so r-3 can change the transport type without touching
   `sc-jev-check`. Each backend's `validate_for_send` reads
   `self.id().descriptor().request_limit_bytes()` (one constant per backend).
8. **Serde spelling fix** (a-10 follow-up). `OpenAiLuna` → `openai_luna`,
   `BackendKeyId::OpenAi` → `openai`, read alias `open_ai_luna` / `open_ai`
   for config and caller-request input (bead
   `agt-backend-serde-alias-removal`). `test_backend_serde_documented_spelling`
   and `test_backend_serde_reads_old_alias` (target `ids`).

### D6 — error taxonomy (REQ-SAG-0106, REQ-SAG-0101 amendment, ADR-SAG-0027)

1. `#[non_exhaustive]` on `JevError`, and `#[non_exhaustive]` on each
   struct-like variant, so downstream crates construct only through
   constructors. Code table (each new variant replaces the named Phase A
   constructor; `code()` ids unchanged):

   | Variant | Replaces | `code()` |
   | --- | --- | --- |
   | `CredentialStoreInvalid { what, cause, recovery }` | `credential_store_invalid` | `CONFIG_INVALID` |
   | `RegistryInvalid { detail, recovery }` | `registry_invalid` | `CONFIG_INVALID` |
   | `SessionStoreIo { op, kind: io::ErrorKind, recovery }` | `session_store_io` | `CONFIG_INVALID` |
   | `Operational { area: OperationalArea::Config, recovery }` | `operational_config` (kept as thin ctor) | `CONFIG_INVALID` |
   | `Operational { area: OperationalArea::Io, recovery }` | `operational_io` (kept as thin ctor) | `INPUT_INVALID` |
   | `SessionDocumentInvalid { detail, recovery }` | `session_document_invalid` | `INPUT_INVALID` |
   | `EncodeFailed { what, cause }` | `encode_failed` | `INPUT_INVALID` |
   | `IoFailed { what, kind }` | `io_failed` | `INPUT_INVALID` |
   | `LanguageUnavailable { producer, language }` | sc-treesitter-named variant | `LANGUAGE_UNAVAILABLE` |
   | `QueryInvalid { producer, detail }`, `QueryNotFound { producer, query }` | sc-treesitter-named variants | `QUERY_INVALID`, `QUERY_NOT_FOUND` |
   | `ProducerError { producer, code: &'static str, message }` | (new; producer stderr) | the carried `code` |
   | `NetworkError { kind: NetworkErrorKind, detail }` | `NetworkError { detail }` | `NETWORK_ERROR` |

   `OperationalArea` is `#[non_exhaustive] { Config, Io }`; its `code()`
   depends on the area, as the table states. REQ-SAG-0101 is amended from
   "one variant per id" to "every variant maps to exactly one id; an id may
   have several variants".
2. `test_every_variant_code_golden` (target `errors`) constructs each variant
   in the table and asserts its `code()` and the first line of `fix_hint()`
   against `crates/sc-agent-types/tests/fixtures/error-codes.json`; every
   `code()` value is in `tests/fixtures/jev/README-contract/errors.json`.
3. `NetworkErrorKind` (`#[non_exhaustive]`: `Busy`, `Timeout`, `Connect`,
   `Tls`, `Status`, `BodyTooLarge`, `Cancelled`). `retryable()` is true only
   for `NetworkError { kind: Busy | Timeout | Connect }`.
4. `fix_hint()` is the one renderer of recovery text: an exhaustive `match`
   inside the crate; `recovery` fields are data it formats. Delete the four
   `*_DETAIL_PREFIX` constants and the `strip_prefix` chain.
5. `Error::source()`: `None` for every redacted-cause variant
   (`CredentialStoreInvalid`, `NetworkError`, `EncodeFailed`, `IoFailed`,
   `SessionStoreIo`); no variant carries a non-redacted source in A.1. Stated
   per variant class in ADR-SAG-0027.
6. `ProducerError` and `from_error_response(&ScAgentErrorResponse) -> JevError`:
   a known code (looked up in a static table generated from `errors.json`)
   becomes `ProducerError { producer, code, message }` whose `code()` returns
   the carried id and whose `fix_hint()` is `"{producer}: {message}"`; an
   unknown code becomes `ExtractionFailed` naming the code.
   `test_from_error_response_round_trip` (target `errors`) round-trips every
   code. r-2 consumes it for command-producer stderr only.
7. `ScAgentErrorResponse` gains optional `schema_version`, `retryable`,
   `details` (all `#[serde(skip_serializing_if = "Option::is_none")]`), set by
   `ScAgentErrorResponse::from_error(&JevError)`; existing `{error, message}`
   bytes are unchanged when the options are `None` (NFR-SAG-0001 regression).
   It implements `Deserialize` for producer stderr.

### D10-break — outcome and attestation shape

1. Remove `JudgeOutcome.probabilities`. `answers` becomes
   `BTreeMap<QuestionId, QuestionAnswer>`; `QuestionAnswer` has private fields
   (`choice`, optional `probabilities: BTreeMap<String, f64>`) and accessors;
   its serde form is byte-identical to today's `answers` JSON (golden test
   `test_answers_serde_golden`, target `judge`). `provider_meta` stays
   `Option<Value>` (D10 defers typed `JudgeMeta`). `ChoiceAnswer` (canonical
   `state` bytes) is untouched except `question_id: QuestionId`.
2. `JudgeOutcome` gets private fields, `JudgeOutcome::new(backend, answers)`,
   `with_provider_meta`, `with_input_sha256`, accessors. It is not
   `#[non_exhaustive]`: backend crates construct it.
3. `JudgeOutcome.input_sha256: Option<Sha256Digest>` — `None` means "this
   backend does not attest". r-1 adds the field only; every backend returns
   `None` until r-2b, the single sprint that implements backend-side hashing.
4. `#[non_exhaustive] enum MultiBackendMode { #[default] Independent, ChainContext }`.
5. `#[non_exhaustive] enum Attestation { Matched, Mismatched { reported: Sha256Digest }, Unattested, Failed, NotSent }`
   and `BackendAttestation` (private `backend`, `attestation`; accessors).
   `EqualInputRecord` gets private fields, a constructor, and
   `{ prompt_sha256, mode, backends: Vec<BackendAttestation>, leak, equal, producer }`;
   `equal` is derived (true iff every entry is `Matched`); `commands:
   Vec<String>` is deleted (the typed `backends` list replaces it). Dry-run
   records `NotSent` per selected backend and `equal = false`. In r-1 the
   engine records `Unattested` for every backend (true today, not a claim).
6. `JudgeRequest` holds `Arc<PayloadBundle>` (no clone per backend) and
   `cancel: CancelToken`. Backends do not re-validate inside `judge`; the
   engine calls `validate_for_send` once per send.
7. `CancelToken` (`Arc<AtomicBool>`, `cancel()`, `is_cancelled()`,
   `Default` = never cancelled) in `sc-agent-types::cancel`, std-only.
8. `PayloadBundle::encode(&CanonicalJudgePayload) -> Result<PayloadBundle, JevError>`
   replaces the public `payload_bundle_from_value` (made `pub(crate)` or
   deleted). `CanonicalJudgePayload` and `Question` get private fields with
   constructors and accessors so a D9 Branch B field is additive.
9. Newtypes: `QuestionId`, `BlobSha` (40/64-hex, reusing the a-10
   validator), `RepoRelativePath` (`TryFrom<&str>`, `FromStr`,
   `Deserialize`; the one validator for repo-relative paths: no `..`, no
   absolute, no `\`, NUL or `:`), `Operation` enum (`Judge`, `Ask`) for
   `LogEvent.operation`. `SubjectsBuilder::push_subject`, `Subject.path`
   and `Question.id` use them; the two-bool argument of
   `equal_input::record` becomes a struct.
10. `ResolvedApiKey` stores `Arc<str>`; the hidden `ResolvedApiKey::new` is
    deleted; `try_new` is the only constructor (ADR-SAG-0020 r-1 amendment).
11. One `JudgeBackend` test double home (ARCHC-010):
    `test_util::MockJudgeBackend` records every `JudgeRequest` it receives;
    the second copy in `sc-jev-check` tests is deleted and
    `register_scripted_backends` builds over it.

### `#[non_exhaustive]` triage (RBP-F104)

Rule: a type another crate constructs gets private fields and a constructor,
not `#[non_exhaustive]`; a type only this crate constructs and others match or
read may be `#[non_exhaustive]`.

| Type | Treatment |
| --- | --- |
| `BackendId`, `BackendKeyId`, `BackendFamily`, `JevError`, `NetworkErrorKind`, `OperationalArea`, `MultiBackendMode`, `Attestation`, `Operation` | `#[non_exhaustive]` enum |
| `BackendDescriptor`, `BackendCapabilities`, `BackendAttestation`, `EqualInputRecord`, `JudgeOutcome`, `QuestionAnswer`, `CanonicalJudgePayload`, `Question`, `PayloadBundle`, `Span`, `Subject` | private fields + constructor/builder + accessors |
| `ScAgentErrorResponse` | private fields (a-10) + `from_error` + `Deserialize` |
| `LogEvent`, `JudgeRequest` | private fields + constructor |

### D7 — types scope and module seams (REQ-SAG-0107, REQ-SAG-0108, ADR-SAG-0016 r-1 amendment, ADR-SAG-0027)

1. `observability::v2::LogRejected { reason }` (`Display`, `Error`) replaces
   the re-exported `EventError`; `SessionLog::emit` returns
   `Result<(), LogRejected>`; drop `sc-observability-types` from
   `crates/sc-agent-types/Cargo.toml` (the root workspace entry is removed by
   r-3). `SessionLog` is an open port (stated in rustdoc; RBP-F014).
2. Cargo features: `default = ["judge", "secret", "obs", "git"]`. `judge`
   gates `judge`, `payload`, `cancel`; `secret` gates `secret`; `obs` gates
   `observability`; `git` gates `git`. Always on: `error`, `envelope`, `ids`,
   `schema`. Dependencies used only by a gated module become optional deps of
   that feature. `cargo tree -p sc-agent-types --no-default-features -e normal`
   is pinned by `test_no_default_features_dep_set` (target `features`, runs
   the command and compares to a committed list). `sc-treesitter` switches to
   `default-features = false` in r-3c.
3. `schema::subjects` (moved from `subjects.rs`): `SubjectsDocument`,
   `Subject`, `Span`, `SubjectsProducer`, `SubjectsSource`, `SUBJECTS_SCHEMA_V1`.
   `ValidatedSubjects` is the only form the engine accepts:
   `SubjectsDocument` deserializes via `#[serde(try_from = "RawSubjectsDocument")]`
   and runs `validate`. `SubjectsBuilder` (`new(source, producer)`,
   `push_subject(path: RepoRelativePath, span, text, kind)`, `build()`)
   assigns ids with the algorithm now in `sc-treesitter/src/render.rs`. The
   copies in `render.rs` and `sc-jev-check/src/extract/subjects.rs` are
   deleted. Crate-root re-exports of moved items are removed (one path each).
4. `InputSpec` leaves `sc-agent-types`: r-1 moves the type unchanged to
   `sc-jev-check::config::InputSpec` (its only consumer); r-2 adds
   `Command.path`.
5. `git` module (RBQA-F006 helper):
   `discover_toplevel(start: &Path) -> Result<Option<RepoRoot>, JevError>` and
   `read_at_rev(root: &RepoRoot, rev: &Rev, path: &RepoRelativePath, max_bytes: u64) -> Result<(BlobSha, Vec<u8>), JevError>`.
   Rules: argv only; `--no-pager`, `--end-of-options`, `--no-textconv`;
   `GIT_TERMINAL_PROMPT=0`; `rev-parse --verify` to a blob sha, then
   `cat-file blob <sha>` (rejects trees); output read through a bounded reader
   (`max_bytes`, `INPUT_TOO_LARGE`); locale-independent classification
   (`LC_ALL=C`). Tests (target `git`, temp repo per test):
   `test_read_at_rev_matches_worktree_bytes`,
   `test_read_at_rev_rejects_option_injection` (`--output=FILE` as rev
   creates no file and returns `INPUT_INVALID`),
   `test_read_at_rev_rejects_tree`, `test_read_at_rev_bounded`,
   `test_discover_toplevel_from_subdirectory`. Callers migrate in r-2
   (`sc-jev-check`) and r-3c (`sc-treesitter`).
6. **Session document version 2.** `StoredSession` (in `sc-jev-check`) gets
   `version: 2`; a file with a missing or older version loads as
   `SessionDocumentInvalid` with recovery "re-run the check; Phase A session
   files cannot be resumed". `test_phase_a_session_is_invalid` (target
   `session` in `sc-jev-check`) loads
   `crates/sc-jev-check/tests/fixtures/session-v1.json`.
7. ADR-SAG-0027 states: adding a backend is a minor `sc-agent-types` change
   (variant + `BACKENDS` row) plus one registry factory arm in
   `sc-jev-check`; "open" means additive-closed, not runtime-open; module
   paths are the stable surface for a later re-export split (EXT-019).

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0105 | `BACKENDS` is the only fact table; `capabilities()` provided from the descriptor; `test_backend_catalog_roundtrip` (serde, `Display`, `FromStr`, aliases agree), `test_every_backend_has_factory`; `#![deny(clippy::wildcard_enum_match_arm)]` in the three consumer crates with the one `#[expect]` in the factory |
| req:REQ-SAG-0106 | No `*_DETAIL_PREFIX`; `test_every_variant_code_golden`; `test_retryable_only_network_transient`; `test_from_error_response_round_trip` |
| req:REQ-SAG-0107 | `cargo tree -p sc-agent-types -e normal` has no `sc-observability*`, `ureq`, `pyo3`, `tree-sitter`; `test_log_rejected_display`; `test_no_default_features_dep_set` |
| req:REQ-SAG-0108 | `test_subjects_builder_ids_match_golden` (golden unchanged); `test_unvalidated_subjects_rejected_on_deserialize`; the two deleted copies are gone (compile) |
| adr:ADR-SAG-0027 | Status Active; text matches code (code table, source rule, features) |
| adr:ADR-SAG-0009 | r-1 amendment bullets `(Active, r-1)` |
| adr:ADR-SAG-0016 | r-1 amendment subsection Active |
| adr:ADR-SAG-0020 | r-1 amendment subsection Active; no `ResolvedApiKey::new` (compile) |
| boundary:agent-types | Cross-impl digests unchanged (`jev_cross_impl`, `cross_impl_branches`); `test_answers_serde_golden`; `test_backend_serde_documented_spelling`, `test_backend_serde_reads_old_alias`; git helper tests; `test_phase_a_session_is_invalid`; REQ-SAG-0103 amendment + `docs/sc-agent-types/architecture.md` "re-frozen at r-1 merge" |

## Required validation

```bash
just lint
just test
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test -p sc-agent-types --test errors -- test_every_variant_code_golden --exact
cargo test -p sc-agent-types --test errors -- test_retryable_only_network_transient --exact
cargo test -p sc-agent-types --test errors -- test_from_error_response_round_trip --exact
cargo test -p sc-agent-types --test errors -- test_log_rejected_display --exact
cargo test -p sc-agent-types --test ids -- test_backend_catalog_roundtrip --exact
cargo test -p sc-agent-types --test ids -- test_backend_serde_documented_spelling --exact
cargo test -p sc-agent-types --test ids -- test_backend_serde_reads_old_alias --exact
cargo test -p sc-agent-types --test judge -- test_answers_serde_golden --exact
cargo test -p sc-agent-types --test subjects -- test_subjects_builder_ids_match_golden --exact
cargo test -p sc-agent-types --test subjects -- test_unvalidated_subjects_rejected_on_deserialize --exact
cargo test -p sc-agent-types --test features -- test_no_default_features_dep_set --exact
cargo test -p sc-agent-types --test git
cargo test -p sc-jev-check --test registry -- test_every_backend_has_factory --exact
cargo test -p sc-jev-check --test session -- test_phase_a_session_is_invalid --exact
cargo test -p sc-jev-check --test cross_impl_branches
cargo test -p sc-jev-py --test jev_cross_impl
test -z "$(cargo tree -p sc-agent-types -e normal | grep -E 'sc-observability|ureq|pyo3|tree-sitter')"
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## This sprint does not close

Engine API, `InputSpec::Command.path`, session store (r-2); backend-side
`input_sha256` hashing and multi-backend semantics (r-2b); HTTP crate and the
lint-enforced secret read site (r-3); `sc-treesitter` git migration and clap
(r-3c); composition crate and CI gates (r-3b); pins and allocator (r-4); D9
(r-5); typed `JudgeMeta` and async trait (Phase B); new stable error codes.
