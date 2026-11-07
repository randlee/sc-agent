# sc-agent-types — architecture

**Boundary:** `agent-types`
**Depends on:** `serde`, `serde_json`, `sha2`, `thiserror`, `uuid`. No
`sc-observability*`, HTTP, Python, or tree-sitter crate (REQ-SAG-0107).

**Contract status:** the a-1 frozen contract was unfrozen once by sprint r-1
(one deliberate pre-1.0 break, ADR-SAG-0027) and is re-frozen at r-1 merge.
Later changes are additive: new enum variants (every public enum is
`#[non_exhaustive]`), new `BACKENDS` rows, new default-bodied trait methods.

## Responsibilities

- The one error enum for Jev and the CLIs (`JevError`) and the stderr error
  shape (`ScAgentErrorResponse`).
- Ids and the static backend catalog (`BackendId`, `BACKENDS`).
- The judge contract (`JudgeBackend`, `JudgeRequest`, `JudgeOutcome`) and the
  canonical payload (`PayloadBundle`).
- The `sc/subjects/1` schema (`schema::subjects`) and its id builder.
- The session log port (`observability::v2`) and argv-only git helpers.

## Cargo features (module seams)

| Feature | Default | Modules | Types |
| --- | --- | --- | --- |
| (core) | always | `error`, `envelope`, `ids`, `schema`, `cancel` | `JevError`, `ErrorCode`, `NetworkErrorKind`, `OperationalArea`; `ScAgentErrorResponse`; `BackendId`, `BackendKeyId`, `BackendFamily`, `BackendDescriptor`, `BACKENDS`, `BackendSet`, `SessionId`, `Rev`, `RepoRoot`, `RepoRelativePath`, `BlobSha`, `Sha256Digest`, `QuestionId`, `ChoiceId`, `Operation`; `schema::subjects::*`; `CancelToken` |
| `judge` | yes | `judge`, `payload` | `JudgeBackend`, `JudgeRequest`, `JudgeOutcome`, `QuestionAnswer`, `BackendCapabilities`, `MultiBackendMode`, `Attestation`, `BackendAttestation`, `EqualInputRecord`, `LlmWireContext`; `PayloadBundle`, `CanonicalJudgePayload`, `Question`, `ChoiceAnswer`, `canonical_json_bytes` |
| `secret` | yes | `secret` | `ResolvedApiKey` |
| `obs` | yes | `observability::v2` | `SessionLog`, `SharedSessionLog`, `LogEvent`, `LogRejected`, `NoopSessionLog` |
| `git` | no | `git` | `discover_toplevel`, `read_at_rev`, `GitBlob` |
| `test-util` | no (requires `judge`) | `test_util` | `MockJudgeBackend` |

`secret` and `judge` are independent; `git` needs only the core. Every
combination builds (`cargo hack check -p sc-agent-types --feature-powerset
--no-dev-deps`). Every type above is reached through a crate-root
re-export; the modules named in the table are private except `schema`,
`observability`, `git` and `test_util`. The catalog and the error-code enums
live in the private `catalog` and `code` modules because sc-boundary 0.8
reports a type cycle inside one module.

## Errors (ADR-SAG-0027)

Every variant maps to exactly one `errors.json` id; an id may have several
variants (REQ-SAG-0101 as amended). Recovery categories are explicit
variants (`CredentialStoreInvalid`, `RegistryInvalid`, `SessionStoreIo`,
`Operational { area }`, `SessionDocumentInvalid`, `EncodeFailed`, `IoFailed`);
producer variants carry a `producer: String`; `ProducerError` re-raises a
producer's stderr with its own `code`. Every struct-like variant has a
snake-case constructor. `fix_hint()` is the one renderer of recovery text;
`retryable()` is true only for `NetworkError` of kind `Busy`, `Timeout`,
`Connect`; `Error::source()` is `None` for every variant.

`ScAgentErrorResponse` keys, in order: `error`, `message`, `schema_version`
(always `1`), `retryable` (only when `true`), `details` (only for
`ProducerError`, `{"producer": ...}`). Readers deny unknown keys and accept
`schema_version` absent or `1`.

## Backends

`BACKENDS` holds one `BackendDescriptor` per built-in id: `name`, read
`aliases`, `key_id`, `family`, `requires_llm_context`,
`supports_session_ask`, `request_limit_bytes`. `BackendId::descriptor` is the
one exhaustive `match`; `Display`, serde and `FromStr` read the descriptor,
so `openai_luna` / `openai` are the only written spellings and `open_ai_luna`
/ `open_ai` are read aliases. `JudgeBackend::capabilities()` is provided from
the descriptor. Callers branch on descriptor facts, never on id equality.

## Judge contract

`PayloadBundle::encode(&CanonicalJudgePayload)` builds every bundle (bytes,
digest, and the question → choices map); `decode` re-encodes stored bytes and
refuses non-canonical ones. `JudgeRequest` shares the bundle as an `Arc` and
carries a `CancelToken`. `JudgeOutcome::new(backend, &request, answers)`
refuses answers whose ids differ from the questions or whose choice (or
probability key) was not offered. `QuestionAnswer` keeps `choice`, optional
`confidence` and `probabilities` in `[0, 1]`. `EqualInputRecord.equal()` is
derived from per-backend `Attestation`s.

`JudgeBackend` is intentionally open; calls block (async hosts use
`spawn_blocking`). `test_util::MockJudgeBackend` is the one double: it
answers through `JudgeOutcome::new`, records requests, counts validations,
reports the received digest, and enforces LLM context from the descriptor.

## Session log port (`observability::v2`)

`SessionLog::emit(LogEvent) -> Result<(), LogRejected>`; `LogEvent` carries
`session_id`, `backend`, `prompt_sha256`, `operation: Operation`, never a
secret. `sc-ai` and `sc-jev-py` adapt their `sc_observability::v2::Logger` to
it; `sc-jev-check` takes a `SharedSessionLog`. Removal of the module still
waits on upstream v2 (bead `agt-obs-v2-shim-removal`).

## Subjects (`schema::subjects`)

`SubjectsDocument` is the one validated type: private fields, built by
`SubjectsBuilder::build` or by parsing, both running the same validation.
`SubjectsBuilder` assigns ids (`{path}:{line}`, then `:1`, `:2`, ...). Rules:
[`subjects-schema.md`](subjects-schema.md). `InputSpec` moved to
`sc-jev-check::config` in r-1.

## Non-responsibilities

HTTP, keyring, `check.toml` parsing, CLI argv, building the real logger.
