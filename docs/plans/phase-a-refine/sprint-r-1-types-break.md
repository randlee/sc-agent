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
vertical_rationale: "Contract break of the shared types crate; nothing is published, so the same sprint updates every call site (including both main.rs and every sc-treesitter error site) and every test it breaks, keeping the workspace green. See plan.md boundary map."
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
  - REQ-SAG-0120
  - REQ-SAG-0121
  - REQ-SAG-0200
  - REQ-SAG-0300
  - REQ-SAG-0310
  - REQ-SAG-0311
  - REQ-SAG-0400
  - REQ-SAG-0500
  - REQ-SAG-0501
  - REQ-SAG-0502
  - REQ-SAG-0800
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
  - ADR-SAG-0010
  - ADR-SAG-0011
  - ADR-SAG-0012
  - ADR-SAG-0013
  - ADR-SAG-0015
  - ADR-SAG-0016
  - ADR-SAG-0019
  - ADR-SAG-0020
  - ADR-SAG-0022
  - ADR-SAG-0025
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
  - "crates/sc-ai/src/**"
  - "crates/sc-ai/tests/**"
  - "crates/sc-treesitter/src/**"
  - "crates/sc-treesitter/tests/**"
  - "Cargo.toml#[workspace.dependencies] sc-observability-types line (delete only)"
  - "tests/jev_cross_impl/**"
  - "tests/fixtures/jev-python/**"
  - "boundaries/sc-agent-types/**"
  - "docs/sc-agent-types/**"
  - "docs/architecture.md#ADR-SAG-0009 amendment (r-1)"
  - "docs/architecture.md#ADR-SAG-0016 amendment (r-1)"
  - "docs/architecture.md#ADR-SAG-0020 amendment (r-1)"
  - "docs/architecture.md#ADR-SAG-0027"
  - "docs/architecture.md#ADR index row ADR-SAG-0027"
  - "docs/architecture.md#ADR index Amended-by marker (r-1) in ADR-SAG-0001"
  - "docs/requirements.md#REQ-SAG-0101, REQ-SAG-0103, REQ-SAG-0120, REQ-SAG-0121, NFR-SAG-0001 amendments; REQ-SAG-0105..0108 rows"
---

# Sprint r-1 — sc-agent-types break (D1, D6, D7 types, D10-break, D11 types)

## Outcome

One deliberate pre-1.0 break of `sc-agent-types`, carrying **every** types
change the phase needs, so no later sprint reopens the crate (r-5 Branch B is
additive by construction). After r-1 the backend set and error enum are open,
backend facts are static data on a descriptor, errors are explicit variants
with no string-prefix parsing, answers are typed, `sc-agent-types` links no
`sc-observability*`, and module seams are cargo features. All call sites and
tests in the workspace are updated in this sprint, including both `main.rs`
files and the `sc-treesitter` modules that construct `JevError` variants
(`grammar.rs`, `query.rs`, `git_io.rs`, `main.rs`): every struct-like
variant gets a constructor, so those files change from literals to
constructor calls and nothing else. `ScAgentErrorResponse::from_jev_error`
keeps its name. r-1 also removes every remaining `#[allow]` under `crates/`.

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
6. **Backend factory table (test-and-review convention).** The production
   factory in `sc-jev-check::registry` is data, not a `match`:
   `static FACTORIES: &[(BackendId, fn(ResolvedApiKey) -> Arc<dyn JudgeBackend>)]`,
   one row per built-in backend, each row calling that backend's
   `with_default_transport`. `BackendRegistry::from_env` looks the id up in
   the table; an id with no row is `RegistryInvalid` (`CONFIG_INVALID`).
   `test_every_backend_has_factory` (target `registry`) iterates `BACKENDS`
   and fails for a descriptor with no row or a row with no descriptor.
   Outside `ids.rs` (descriptor lookup) and this table, code branches on
   **descriptor facts** (`family()`, `requires_llm_context()`,
   `supports_session_ask()`, `request_limit_bytes()`), never on id equality
   or a `match` on `BackendId`; r-1 converts the one existing equality
   (`crates/sc-ai/src/command.rs` `outcome.backend == BackendId::Jev` →
   `family() == BackendFamily::TypeSafeJev`). No clippy lint enforces this
   (`clippy::wildcard_enum_match_arm` is silent when every variant is listed
   plus the required `_` arm, and a crate-wide deny would also hit
   `serde_json::Value` matches), so the rule is the test plus review, stated
   in ADR-SAG-0009/0027 and REQ-SAG-0105. Validation adds a cheap
   tripwire for the known pattern:
   `! rg -n 'BackendId::[A-Za-z]+ *==|== *BackendId::' crates/ --glob '!crates/sc-agent-types/src/ids.rs' --glob '!**/tests/**'`.
   `BackendRegistry::register(Arc<dyn JudgeBackend>)` (r-2) installs a double
   or scripted backend **for an existing id**; it cannot introduce a new id
   (the id set is the closed `#[non_exhaustive]` enum).

   **Adding a built-in backend** (checklist, ADR-SAG-0027): (1) minor
   `sc-agent-types` release: `BackendId` variant, `BACKENDS` row (facts,
   aliases, limit), `BackendKeyId` variant if it has a new key; (2) the
   backend type in its crate, with `with_default_transport`; (3) one
   `FACTORIES` row; (4) `test_every_backend_has_factory` and
   `test_backend_catalog_roundtrip` pass unchanged. No consumer signature
   changes.
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
   `agt-backend-serde-alias-removal`). One alias source: `BackendId`
   implements `Deserialize` through `FromStr`, which reads the descriptor's
   `name()` and `aliases()`; there is no `#[serde(alias)]`. `Serialize`
   writes `name()`. `test_backend_serde_documented_spelling` and
   `test_backend_serde_reads_old_alias` (target `ids`).

### D6 — error taxonomy (REQ-SAG-0106, REQ-SAG-0101 amendment, ADR-SAG-0027)

1. `#[non_exhaustive]` on `JevError`, and `#[non_exhaustive]` on each
   struct-like variant, so downstream crates construct only through
   constructors. **Every struct-like variant has a public constructor**
   (snake-case of the variant: `extraction_failed(detail)`,
   `input_invalid(..)`, `input_missing(path)`,
   `language_unavailable(producer, language)`,
   `query_not_found(producer, query)`, `query_invalid(producer, detail)`,
   `producer_error(producer, code, message)`, `network(kind, detail)`, and
   the existing `operational_io` / `operational_config` / `io_failed` thin
   constructors), so every cross-crate literal in `sc-treesitter`
   (`grammar.rs`, `query.rs`, `git_io.rs`, `main.rs`), `sc-ai/src/main.rs`
   and the other crates becomes a call. Unit variants stay unit variants.
   Code table (each new variant replaces the named Phase A
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
2. `ErrorCode`: `#[non_exhaustive]` enum with one variant per id in
   `errors.json`, `as_str() -> &'static str`, `Display`, `FromStr`, and
   `ErrorCode::ALL: &[ErrorCode]` (a hand-written `const` table; no
   `build.rs`). `JevError::code()` returns `ErrorCode`.
   `test_error_codes_match_errors_json` (target `errors`) asserts the set of
   `ErrorCode::ALL.as_str()` values equals the ids in
   `tests/fixtures/jev/README-contract/errors.json`.
   `test_every_variant_code_golden` (target `errors`) constructs each variant
   in the table **through its public constructor** (an integration target
   cannot use literals of a `#[non_exhaustive]` variant, so the test also
   proves every constructor exists) and asserts its `code()` and the first
   line of `fix_hint()` against
   `crates/sc-agent-types/tests/fixtures/error-codes.json`.
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
6. `ProducerError { producer, code: ErrorCode, message }` and
   `from_error_response(producer, &ScAgentErrorResponse) -> JevError`: a
   known code (`ErrorCode::from_str` over `ErrorCode::ALL`) becomes
   `ProducerError` whose `code()` returns the carried code and whose
   `fix_hint()` is `"{producer}: {message}"`; an unknown code becomes
   `ExtractionFailed` naming the code. `test_from_error_response_round_trip`
   (target `errors`) round-trips every code. r-2 consumes it for
   command-producer stderr only.
7. **`ScAgentErrorResponse` field rule** (NFR-SAG-0001 amendment,
   ADR-SAG-0027). The constructor keeps its name,
   `ScAgentErrorResponse::from_jev_error(&JevError)`; no rename, so neither
   `main.rs` call changes. Serialized key order is fixed:
   `error`, `message`, `schema_version`, `retryable`, `details`.
   - `schema_version`: always `1` from `from_jev_error`. It versions this
     error shape only, independent of the stdout `Envelope.version`, and is
     bumped only by a breaking change to this shape.
   - `retryable`: present (`true`) only when `JevError::retryable()` is true;
     absent otherwise (never `false`).
   - `details`: present only for `ProducerError`, as `{"producer": <name>}`;
     absent for every other variant.
   `Deserialize` (producer stderr): `deny_unknown_fields`; `schema_version`
   absent (a Phase A producer) or `1` is accepted; any other value, or an
   unknown key, fails to parse, so the `command` extractor falls back to
   `ExtractionFailed` (r-2 D11.3). This changes the stderr bytes of every
   error (one added `"schema_version":1` key): NFR-SAG-0001 is amended in
   this sprint and the bytes are pinned by
   `test_error_response_stderr_golden` (target `errors`), which serializes
   `INPUT_EMPTY`, `NETWORK_ERROR` (`Busy`) and a `ProducerError` and compares
   with `crates/sc-agent-types/tests/fixtures/error-stderr.jsonl`:
   `{"error":"INPUT_EMPTY","message":"…","schema_version":1}`,
   `{"error":"NETWORK_ERROR","message":"…","schema_version":1,"retryable":true}`,
   `{"error":"LANGUAGE_UNAVAILABLE","message":"…","schema_version":1,"details":{"producer":"sc-treesitter"}}`.

### D10-break — outcome and attestation shape

1. Remove `JudgeOutcome.probabilities`. `answers` becomes
   `BTreeMap<QuestionId, QuestionAnswer>`. `QuestionAnswer` has private
   fields `choice: ChoiceId` and optional
   `probabilities: BTreeMap<ChoiceId, f64>`, a constructor
   `QuestionAnswer::new(choice, probabilities) -> Result<Self, JevError>` and
   accessors. `ChoiceId` is a validated string newtype (non-empty, no
   control characters) in `ids`. **Finite-probability rule:** every
   probability is finite and in `[0, 1]` (NaN, infinity, negative or > 1 is
   `RESPONSE_INVALID`); `Deserialize` goes through the same constructor
   (`#[serde(try_from)]`), so an invalid backend response never becomes a
   `QuestionAnswer`. `QuestionAnswer` derives `PartialEq` only (not `Eq`,
   because of `f64`). Its serde form is byte-identical to today's `answers`
   JSON (golden test `test_answers_serde_golden`, target `judge`).
   **Relation to `ChoiceAnswer`:** `QuestionAnswer` is a backend's *output*
   for one question; `ChoiceAnswer { question_id, choice }` is an *input*
   (prior state inside the canonical payload, part of `prompt_sha256`
   bytes). They are deliberately separate types; `ChoiceAnswer` is untouched
   except `question_id: QuestionId` and `choice: ChoiceId` with byte-identical
   serde. `QuestionAnswer::to_choice_answer(&QuestionId)` is the one
   conversion (used when a session `ask` feeds prior answers back).
   `provider_meta` stays `Option<Value>` (D10 defers typed `JudgeMeta`).
2. `JudgeOutcome` gets private fields,
   `JudgeOutcome::new(backend, request: &JudgeRequest, answers) -> Result<Self, JevError>`,
   `with_provider_meta`, `with_input_sha256`, accessors. `new` checks that
   the answer key set equals the request's `QuestionId` set (a missing or
   extra key is `RESPONSE_INVALID`), so no outcome exists whose answers do
   not match the questions asked. It is not `#[non_exhaustive]`: backend
   crates construct it.
3. `JudgeOutcome.input_sha256: Option<Sha256Digest>` — `None` means "this
   backend does not attest". r-1 adds the field only; every backend returns
   `None` until r-2b, the single sprint that implements backend-side hashing.
4. `#[non_exhaustive] enum MultiBackendMode { #[default] Independent, ChainContext }`.
5. `#[non_exhaustive] enum Attestation { Matched, Mismatched { reported: Sha256Digest }, Unattested, Failed, NotSent }`
   and `BackendAttestation` (private `backend`, `attestation`; accessors).
   `EqualInputRecord` gets private fields, a constructor, and
   `{ prompt_sha256, mode, backends: Vec<BackendAttestation>, leak, producer }`.
   `equal` is **derived, not stored**: `EqualInputRecord::equal()` returns
   true iff every entry is `Matched`; the hand-written `Serialize` writes
   `equal` from that method (so the JSON key stays), and `Deserialize`
   rejects a document whose `equal` differs from the recomputed value
   (`SESSION_DOCUMENT_INVALID` when read from a session file). `commands:
   Vec<String>` is deleted (the typed `backends` list replaces it). Dry-run
   records `NotSent` per selected backend and `equal = false`. In r-1 the
   engine records `Unattested` for every backend (true today, not a claim).
6. `JudgeRequest` holds `Arc<PayloadBundle>` (no clone per backend) and
   `cancel: CancelToken`. Backends do not re-validate inside `judge`; the
   engine calls `validate_for_send` once per send.
7. `CancelToken` (`Arc<AtomicBool>`, `cancel()`, `is_cancelled()`,
   `Default` = never cancelled) in `sc-agent-types::cancel`, std-only and
   **always on** (not behind `judge`), so `sc-agent-http` needs only the
   `secret` feature.
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
11. One `JudgeBackend` test double home (ARCHC-010), behind the existing
    non-default `test-util` feature (plan "Test seams"):
    `test_util::MockJudgeBackend`. **Contract:** `new(id, answers)`;
    records every `JudgeRequest` it receives (`requests() -> Vec<JudgeRequest>`);
    returns a `JudgeOutcome` whose `input_sha256` is
    `Some(request.payload().sha256())`, the digest of the bundle it received;
    `with_reported_digest(Sha256Digest)` makes it report that digest instead
    (r-2b's mismatch tests); `with_failure(JevError)` makes `judge` return the
    error. The engine ignores `input_sha256` until r-2b (records
    `Unattested`). The second copy in `sc-jev-check` tests is deleted and
    `register_scripted_backends` builds over it. `sc-agent-types`' own
    integration tests enable the feature through a self dev-dependency
    (`sc-agent-types = { path = ".", features = ["test-util", "git"] }`);
    nothing in `tests/` relies on `cfg(test)`.

### `#[non_exhaustive]` triage (RBP-F104)

Rule: a type another crate constructs gets private fields and a constructor,
not `#[non_exhaustive]`; a type only this crate constructs and others match or
read may be `#[non_exhaustive]`.

| Type | Treatment |
| --- | --- |
| `BackendId`, `BackendKeyId`, `BackendFamily`, `JevError`, `NetworkErrorKind`, `OperationalArea`, `MultiBackendMode`, `Attestation`, `Operation` | `#[non_exhaustive]` enum |
| `BackendDescriptor`, `BackendCapabilities`, `BackendAttestation`, `EqualInputRecord`, `JudgeOutcome`, `QuestionAnswer`, `CanonicalJudgePayload`, `Question`, `PayloadBundle`, `Span`, `Subject`, `SubjectsDocument`, `GitBlob` | private fields + constructor/builder + accessors |
| `ChoiceId`, `QuestionId`, `BlobSha`, `RepoRelativePath` | validated newtype (private inner value, `TryFrom`/`FromStr`, `Deserialize` through the validator) |
| `ErrorCode` | `#[non_exhaustive]` enum with `as_str()` |
| `ScAgentErrorResponse` | private fields (a-10) + `from_error` + `Deserialize` |
| `LogEvent`, `JudgeRequest` | private fields + constructor |

### D7 — types scope and module seams (REQ-SAG-0107, REQ-SAG-0108, ADR-SAG-0016 r-1 amendment, ADR-SAG-0027)

1. `observability::v2::LogRejected { reason }` (`Display`, `Error`) replaces
   the re-exported `EventError`; `SessionLog::emit` returns
   `Result<(), LogRejected>`; drop `sc-observability-types` from
   `crates/sc-agent-types/Cargo.toml`, from
   `boundaries/sc-agent-types/agent-types.toml` `allowed_dependencies`, and
   the root `[workspace.dependencies]` line (r-1 removes the only user, so it
   removes the root entry in the same commit; r-4 deletes the `sc-lint.toml`
   record and keeps `sc-observability-types` off the ADR-SAG-0023 list).
   `SessionLog` is an open port (stated in rustdoc; RBP-F014).
   REQ-SAG-0120/0121 are amended (root and crate requirements): the port
   no longer depends on `sc-observability-types`; the noop and test-util
   doubles stay (their tests: target `obs`, incl. `test_log_rejected_display`
   moved there from `errors`).
2. Cargo features: `default = ["judge", "secret", "obs"]`; `git` and
   `test-util` are non-default. `judge` gates `judge` and `payload`;
   `secret` gates `secret`; `obs` gates `observability`; `git` gates `git`;
   `test-util` (requires `judge`) gates `test_util`. Always on: `error`,
   `envelope`, `ids`, `schema`, `cancel`. Dependencies used only by a gated
   module become optional deps of that feature. **Type homes:**

   | Module (feature) | Types | Extra deps |
   | --- | --- | --- |
   | `error` (core) | `JevError`, `ErrorCode`, `NetworkErrorKind`, `OperationalArea` | `thiserror` |
   | `envelope` (core) | `ScAgentErrorResponse` | `serde`, `serde_json` |
   | `ids` (core) | `BackendId`, `BackendKeyId`, `BackendFamily`, `BackendDescriptor`, `BACKENDS`, `BackendSet`, `SessionId`, `Rev`, `RepoRoot`, `RepoRelativePath`, `BlobSha`, `Sha256Digest`, `QuestionId`, `ChoiceId`, `Operation` | `uuid` (`SessionId`) |
   | `schema` (core) | `schema::subjects::*`, `SubjectsBuilder` | — |
   | `cancel` (core) | `CancelToken` | — (std) |
   | `judge` / `payload` (`judge`) | `JudgeBackend`, `JudgeRequest`, `JudgeOutcome`, `QuestionAnswer`, `BackendCapabilities`, `MultiBackendMode`, `Attestation`, `BackendAttestation`, `EqualInputRecord`; `PayloadBundle`, `CanonicalJudgePayload`, `Question`, `ChoiceAnswer` | `sha2` |
   | `secret` (`secret`) | `ResolvedApiKey` | — |
   | `observability::v2` (`obs`) | `SessionLog`, `SharedSessionLog`, `LogEvent`, `LogRejected` | — |
   | `git` (`git`) | `discover_toplevel`, `read_at_rev`, `GitBlob` | — (std) |
   | `test_util` (`test-util`) | `MockJudgeBackend` | — |

   `secret` and `judge` are independent (neither enables the other); `git`
   needs only core. `cargo hack check -p sc-agent-types --feature-powerset
   --no-dev-deps` proves every combination builds (r-1 validation; r-4 adds
   it to CI). `cargo tree -p sc-agent-types --no-default-features -e normal`
   is pinned by `test_no_default_features_dep_set` (target `features`, runs
   the command and compares to a committed list). `sc-treesitter` switches to
   `default-features = false` in r-3c; `sc-jev-check` enables `git` in r-2.
3. `schema::subjects` (moved from `subjects.rs`): `SubjectsDocument`,
   `Subject`, `Span`, `SubjectsProducer`, `SubjectsSource`, `SUBJECTS_SCHEMA_V1`.
   **`SubjectsDocument` is the one validated type** (there is no separate
   `ValidatedSubjects`): private fields, accessors, and construction only
   through `SubjectsBuilder::build` or `TryFrom<RawSubjectsDocument>`;
   `Deserialize` is `#[serde(try_from = "RawSubjectsDocument")]`, where the
   private `RawSubjectsDocument` mirrors the wire form and `try_from` runs
   `validate`. A `SubjectsDocument` value is therefore always valid.
   `SubjectsBuilder` (`new(source, producer)`,
   `push_subject(path: RepoRelativePath, span, text, kind)`, `build()`)
   assigns ids with the algorithm now in `sc-treesitter/src/render.rs`. The
   copies in `render.rs` and `sc-jev-check/src/extract/subjects.rs` are
   deleted. Crate-root re-exports of moved items are removed (one path each).
4. `InputSpec` leaves `sc-agent-types`: r-1 moves the type unchanged to
   `sc-jev-check::config::InputSpec` (its only consumer); r-2 adds
   `Command.path`.
5. `git` module (RBQA-F006 helper, non-default `git` feature):
   `discover_toplevel(start: &Path) -> Result<Option<RepoRoot>, JevError>` and
   `read_at_rev(root: &RepoRoot, rev: &Rev, path: &RepoRelativePath, max_bytes: u64) -> Result<GitBlob, JevError>`,
   where `GitBlob` has private `sha: BlobSha` and `bytes: Vec<u8>` with
   accessors and `into_bytes()`.
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
   files cannot be resumed"; a **newer** version (> 2) also loads as
   `SessionDocumentInvalid`, with recovery "written by a newer sc-agent;
   upgrade or re-run the check" (`test_newer_session_version_is_invalid`). `test_phase_a_session_is_invalid` (target
   `session` in `sc-jev-check`) loads
   `crates/sc-jev-check/tests/fixtures/session-v1.json`.
7. ADR-SAG-0027 states: adding a backend follows the D1.6 checklist (a
   minor `sc-agent-types` change — variant + `BACKENDS` row — plus one
   `FACTORIES` row in `sc-jev-check`); "open" means additive-closed, not
   runtime-open; `register` installs doubles of existing ids only; module
   paths are the stable surface for a later re-export split (EXT-019); test
   seams are `test-util` features (plan "Test seams").

### Lint hygiene (CPR-103, RBQA-F003)

1. Convert every remaining `#[allow(..)]` under `crates/` to
   `#[expect(.., reason = "..")]`. At a-10 there are three:
   `crates/sc-agent-types/src/ids.rs` (`clippy::new_without_default` on
   `SessionId::new`), `crates/sc-jev/src/client/https.rs` and
   `crates/sc-jev-backends/src/transport.rs` (`clippy::disallowed_methods`,
   the two secret reads r-3 replaces). Gate in validation:
   `! rg -n '#!?\[allow\(' crates/`. This makes r-3's workspace
   `allow_attributes = "deny"` a no-op on arrival.
2. Every existing crate keeps `[lints] workspace = true` (all eight have it
   at a-10) and has no per-crate `[lints]` table; r-1 adds none.
   `workspace-tests --test source_scans` (r-3b) asserts it for every crate.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0105 | `BACKENDS` is the only fact table; `capabilities()` provided from the descriptor; `test_backend_catalog_roundtrip` (serde, `Display`, `FromStr`, aliases agree), `test_every_backend_has_factory` over the `FACTORIES` table; no `== BackendId::` outside `ids.rs` (tripwire `rg`); no `wildcard_enum_match_arm` deny or expect anywhere |
| req:REQ-SAG-0106 | No `*_DETAIL_PREFIX`; `test_every_variant_code_golden` (constructors only); `test_error_codes_match_errors_json`; `test_retryable_only_network_transient`; `test_from_error_response_round_trip` |
| req:REQ-SAG-0107 | `cargo tree -p sc-agent-types -e normal` (default and `--no-default-features`) has no `sc-observability`, `sc-observability-types`, `ureq`, `pyo3`, `tree-sitter`; `test_log_rejected_display`; `test_no_default_features_dep_set`; `cargo hack check -p sc-agent-types --feature-powerset --no-dev-deps` |
| req:REQ-SAG-0108 | `test_subjects_builder_ids_match_golden` (golden unchanged); `test_unvalidated_subjects_rejected_on_deserialize`; `SubjectsDocument` has no public field and no `ValidatedSubjects` exists (compile); the two deleted copies are gone (compile) |
| req:NFR-SAG-0001 (amended) | `test_error_response_stderr_golden` pins the field rule bytes |
| req:REQ-SAG-0120, REQ-SAG-0121 (amended) | No `sc-observability-types` in any manifest; noop and test-util doubles still build (`cargo test -p sc-agent-types --test obs`) |
| adr:ADR-SAG-0027 | Status Active; text matches code (code table, field rule, source rule, features and type homes, factory checklist) |
| adr:ADR-SAG-0009 | r-1 amendment bullets `(Active, r-1)` |
| adr:ADR-SAG-0016 | r-1 amendment subsection Active |
| adr:ADR-SAG-0020 | r-1 amendment subsection Active; no `ResolvedApiKey::new` (compile) |
| boundary:agent-types | Cross-impl digests unchanged (`jev_cross_impl`, `cross_impl_branches`); `test_answers_serde_golden`; `test_question_answer_rejects_non_finite`; `test_outcome_rejects_mismatched_answer_keys`; `test_equal_input_equal_is_derived`; `test_backend_serde_documented_spelling`, `test_backend_serde_reads_old_alias`; git helper tests; `test_phase_a_session_is_invalid`, `test_newer_session_version_is_invalid`; no `#[allow(` under `crates/`; REQ-SAG-0103 amendment + `docs/sc-agent-types/architecture.md` "re-frozen at r-1 merge". Regression for ids this sprint touches only through call sites: REQ-SAG-0200 `cargo test -p sc-agent-secrets`; REQ-SAG-0300/0311 `cargo test -p sc-jev-check --lib -- extract::`; REQ-SAG-0310 `cargo test -p sc-treesitter --test cli`; REQ-SAG-0400 `cargo test -p sc-jev --test jev_api`; REQ-SAG-0500/0501/0502 `cargo test -p sc-jev-backends --test llm`; REQ-SAG-0800 pytest below; REQ-SAG-0711 `cargo test -p sc-ai --test cli` |

## Required validation

```bash
just lint
just test
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test -p sc-agent-types --test errors -- test_every_variant_code_golden --exact
cargo test -p sc-agent-types --test errors -- test_error_codes_match_errors_json --exact
cargo test -p sc-agent-types --test errors -- test_error_response_stderr_golden --exact
cargo test -p sc-agent-types --test errors -- test_retryable_only_network_transient --exact
cargo test -p sc-agent-types --test errors -- test_from_error_response_round_trip --exact
cargo test -p sc-agent-types --test obs -- test_log_rejected_display --exact
cargo test -p sc-agent-types --test ids -- test_backend_catalog_roundtrip --exact
cargo test -p sc-agent-types --test ids -- test_backend_serde_documented_spelling --exact
cargo test -p sc-agent-types --test ids -- test_backend_serde_reads_old_alias --exact
cargo test -p sc-agent-types --test judge -- test_answers_serde_golden --exact
cargo test -p sc-agent-types --test judge -- test_question_answer_rejects_non_finite --exact
cargo test -p sc-agent-types --test judge -- test_outcome_rejects_mismatched_answer_keys --exact
cargo test -p sc-agent-types --test judge -- test_equal_input_equal_is_derived --exact
cargo test -p sc-agent-types --test obs
cargo test -p sc-agent-types --test subjects -- test_subjects_builder_ids_match_golden --exact
cargo test -p sc-agent-types --test subjects -- test_unvalidated_subjects_rejected_on_deserialize --exact
cargo test -p sc-agent-types --test features -- test_no_default_features_dep_set --exact
cargo test -p sc-agent-types --test git
cargo test -p sc-jev-check --test registry -- test_every_backend_has_factory --exact
cargo test -p sc-jev-check --test session -- test_phase_a_session_is_invalid --exact
cargo test -p sc-jev-check --test session -- test_newer_session_version_is_invalid --exact
cargo test -p sc-jev-check --test cross_impl_branches
cargo test -p sc-jev-py --test jev_cross_impl
cargo test -p sc-agent-secrets
cargo test -p sc-jev-check --lib -- extract::
cargo test -p sc-treesitter --test cli
cargo test -p sc-jev --test jev_api
cargo test -p sc-jev-backends --test llm
cargo test -p sc-ai --test cli
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
uv run --directory tests/fixtures/jev-python pytest -q
cargo hack check -p sc-agent-types --feature-powerset --no-dev-deps
test -z "$(cargo tree -p sc-agent-types -e normal | grep -E '(^|[^[:alnum:]_-])(sc-observability|sc-observability-types|ureq|pyo3|tree-sitter) v[0-9]')"
test -z "$(cargo tree -p sc-agent-types -e normal --no-default-features | grep -E '(^|[^[:alnum:]_-])(sc-observability|sc-observability-types|ureq|pyo3|tree-sitter) v[0-9]')"
! rg -n '#!?\[allow\(' crates/
! rg -n 'BackendId::[A-Za-z]+ *==|== *BackendId::' crates/ --glob '!crates/sc-agent-types/src/ids.rs' --glob '!**/tests/**'
! rg -n 'wildcard_enum_match_arm' crates/
! rg -n 'sc-observability-types' Cargo.toml crates/*/Cargo.toml boundaries/*/*.toml
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## This sprint does not close

Engine API, `InputSpec::Command.path`, `RunResponse::warnings`, the `git`
feature enablement in `sc-jev-check`, session store (r-2); backend-side
`input_sha256` hashing and multi-backend semantics (r-2b); HTTP crate and the
lint-enforced secret read site (r-3); `sc-treesitter` git migration and clap
(r-3c); composition crate and CI gates (r-3b); pins and allocator (r-4); D9
(r-5); typed `JudgeMeta` and async trait (Phase B); new stable error codes.
