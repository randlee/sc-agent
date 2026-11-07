# sc-agent architecture

**Status:** Draft (Phase A planning)  
**Owner:** sc-agent maintainers

## System shape

sc-agent combines **agent orchestration assets** (prompts, skills, policy) with a
**Rust workspace** under `crates/` for agent launchers and shared libraries.

| Layer | Location | Role |
| --- | --- | --- |
| Agents | `.claude/agents/`, `.cursor/agents/`, `.codex/agents/` | Role prompts |
| Skills | `.claude/skills/`, `.cursor/skills/` | Orchestration, anchoring, publishing |
| Policy | `.claude/project/quality-policy.md` | Reviewers, validation, layout |
| Rust workspace | `crates/*`, root `Cargo.toml` | Libraries + `sc-ai` CLI |
| Boundaries | `boundaries/*` | sc-lint module separation (from sprint a-1) |
| Beads | `.beads/` | Issue tracking |
| Plan anchor | `.sc/plan-anchor/` | Plan tracker settings |

## Phase A crate graph

```
sc-ai (binary) ── sc-observability v2 Logger → SessionLog adapter
  └── sc-jev-check (facade: check.toml, extractors, run_check, sessions)
        ├── sc-jev (TypeSafe Jev HTTP + JudgeBackend impl)
        ├── sc-jev-backends (Haiku/Luna JudgeBackend impls)
        ├── sc-agent-secrets
        └── sc-agent-types (temporary observability::v2 shim)

sc-jev-py (PyO3) → sc-jev-check
```

`sc-jev` and `sc-jev-backends` do not depend on each other.

**Jev API:** All TypeSafe Jev HTTP lives in **`sc-jev`** (REQ-SAG-0110).  
**Trait boundary:** `JudgeBackend` and shared domain types live in
`sc-agent-types`. LLM comparison backends implement the trait in
`sc-jev-backends`. Local judge (decider-4b / cyprid) adds a backend later
without changing callers.

## ADR-SAG-0001 — Error enum is CLI authority

**Status:** Active

All user-visible error ids come from `JevError` in `sc-agent-types`.
CLI and PyO3 map variants to JSON / exceptions; no ad-hoc strings.

## ADR-SAG-0002 — Canonical judge payload bytes

**Status:** Active

The **canonical judge payload** is sorted-key compact JSON (UTF-8): the
questions + state the backends must judge. SHA-256 of **those** bytes is
`prompt_sha256` for equal-input and the cross-language fixture (sprint a-1).

## ADR-SAG-0005 — Facade owns payload assembly

**Status:** Active

`sc-jev-check` is the **only** crate that runs extractors and builds
`PayloadBundle`. `run::build_request` is the public assembly entry for a
check file. `run_check` uses that same assembly. Backend crates consume
`JudgeRequest` and must not re-canonicalize. Composition roots call the
facade and do not call `PayloadBuilder`. Pre-send refusal for config/input
errors happens before any backend call (ADR-SAG-0004).

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — lands in r-4.

- `sc-jev-check` runs the four consumer extractor kinds (`file`,
  `table-column`, `subjects`, `command`); it is not the only producer of
  subjects (ADR-SAG-0015). The public assembly entry point is `prepare`
  (ADR-SAG-0024); the `run::build_request` sentence above is superseded when
  r-2 lands.

### Amendment (Phase A.1, proposed — blocked on vendor answer)

**Status:** Proposed (Phase A.1) — lands in r-5; blocked (D9).

- `CanonicalJudgePayload` is the only payload schema. Both backend families
  validate by deserializing it. The Jev HTTP body is a JSON envelope built
  around the canonical object (no byte-splice); `prompt_sha256` stays on
  canonical bytes.

## ADR-SAG-0008 — Sessions carry prior answers to later backends

**Status:** Active

When multiple backends run in one invocation, **session state** is how a second
opinion receives prior answers. TypeSafe Jev has no rationale field on the wire;
Haiku/Luna session `ask` paths reuse stored context. Session files are UUID paths
with mode `0600` (sprint a-6).

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — lands in r-2b.

- Multi-backend runs are **independent by default**: no backend receives
  another's answers. `MultiBackendMode::ChainContext` passes prior successful
  LLM answers to later LLM backends through the system prompt, and the mode is
  recorded on `EqualInputRecord.mode`. Sessions continue to carry context for
  `ask`. See ADR-SAG-0025.

## ADR-SAG-0009 — Local backend registers without caller changes

**Status:** Active

Future local judges (decider-4b / cyprid) implement `JudgeBackend` and register
in `sc-jev-check`’s backend registry. Callers keep `run_check(..., backends: &[BackendId])`;
no CLI or PyO3 API change when a local backend lands.

Amended by ADR-SAG-0022.

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — lands in r-1.

- `BackendId` is `#[non_exhaustive]`. Adding a backend means a new variant, one
  `BACKENDS` catalog row, and a `JudgeBackend` implementation with
  `capabilities()`. That is a minor version bump with no caller signature
  change. The engine reads `capabilities()` and the catalog, never matches on
  ids.

## ADR-SAG-0022 — Scripted backends are a test-util feature

**Status:** Active

Amends ADR-SAG-0009.

Registration without a caller-signature change stays ADR-SAG-0009. This
decision records the test-only scripted judge. It is not a production backend
and not a second `run_check` policy.

- `sc-jev-check` Cargo feature `test-util` exposes
  `registry::register_scripted_backends`. It builds a `BackendRegistry` of
  in-process `JudgeBackend`s. Each judge returns the supplied answer object.
  It does not open a socket and does not store an API key. Callers pass that
  registry to `RunOptions::registry`. `run_check`'s parameters stay unchanged.
  A repeated backend id is `JevError::registry_invalid` (`CONFIG_INVALID`).
  An LLM backend id still requires `JudgeRequest.llm` before it returns
  (ADR-SAG-0019). A build that leaves the feature off does not export the
  function. This crate's own `cargo test` compiles the library with
  `cfg(test)`, so those tests see the function without enabling the feature.
  Every other crate, including `sc-jev-py`, sees it only by enabling
  `sc-jev-check/test-util`.
- `sc-jev-py` Cargo feature `test-util` enables `sc-jev-check/test-util` and
  adds keyword-only `scripted_answers` to `run_check`. That argument forwards
  to `register_scripted_backends`. This crate does not implement
  `JudgeBackend`. When the feature is off, production `run_check` has no
  `scripted_answers` parameter and does not register a judge. A value that is
  not a JSON object of question ids is `INPUT_INVALID` (ADR-SAG-0001). Omit
  the argument to use the HTTP backends inside `sc-jev-check`. `cargo test`
  of this crate enables `sc-jev-check/test-util` through its dev-dependency
  and compiles the Python signature with `cfg(test)`.
- Default features of both crates leave `test-util` off. The publish wheel
  does not pass `--features test-util`. Sprint a-8 pytest may enable it so
  session `ask` runs without a network send. Enabling it does not change
  production registration.

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — lands in r-2.

- The scripted registry is passed via `RunConfig::registry` (ADR-SAG-0024),
  not `RunOptions::registry`. `register_scripted_backends` and its `test-util`
  gating are unchanged. The scripted backend implements `capabilities()` and
  reports `input_sha256`.

## ADR-SAG-0019 — LLM wire envelope vs canonical payload

**Status:** Active

**Jev (TypeSafe)** sends the canonical payload in the API shape the evaluator
expects (including model selection on the wire).

**Haiku / Luna** send API-direct HTTP that is **not** byte-identical to Jev’s
body. They **must** embed the **same canonical payload bytes** in the user/judge
message (per README parity). A **system prompt** (evaluator instructions, JSON
schema hints) is allowed and **may change** over time without changing
`prompt_sha256`, as long as the embedded canonical blob is unchanged.

**System prompt injection (not hardcoded in Rust):** production crates must not
embed evaluator prose as string literals. Resolution **precedence** (first hit wins):

1. **`sc-ai jev --llm-system-prompt-file`** or `RunOptions.system_prompt_override`
   (working-tree read at invoke time).
2. Else **`check.toml`** `[llm_judge] system_prompt_file` at the same `rev` as
   other check inputs (`git show` when `rev` is not `WORKTREE`).
3. Else packaged default from **`crates/sc-jev-check/assets/jev.toml`** → asset path
   resolved via **`include_str!` at build time** into the wheel/binary (REQ-SAG-0503:
   prose lives in the asset file, not as Rust string literals).

Haiku and Luna require resolved text in `JudgeRequest.llm`. **Jev-only** runs set
`llm: None`; a missing prompt file is **not** an error when no LLM backend is selected.
When an LLM backend is selected and the resolved file is missing → `CONFIG_INVALID`
(or `PROMPT_FILE_MISSING` if added to the README contract in a-1).

`RunOptions` fields (facade): `repo_root`, `dry_run`, `system_prompt_override`,
`session_log: SharedSessionLog`.

Token/count checks apply to the full message the provider receives; equal-input
still records whether every backend saw the same canonical bytes.

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — bullets land in r-2, r-2b.

- (r-2) The `RunOptions` field list above is replaced by `RunConfig`
  (ADR-SAG-0024). The prompt precedence is unchanged.
- (r-2b) Equal-input records, per backend, the digest of the canonical bytes
  that backend embedded on the wire (`JudgeOutcome.input_sha256`). `equal` is
  true only if every successful backend's digest equals `prompt_sha256`.

## ADR-SAG-0003 — Secret resolution

**Status:** Active

`sc-agent-secrets` resolves keys: **process env → repo-root `.env` → keyring**.
**Only `sc-jev-check`** calls `resolve_key`. `JevClient` and LLM backends receive
the resolved secret (and transport) at **construction**; they do **not** depend on
`sc-agent-secrets`. `JudgeRequest` carries neither keys nor `repo_root`.

Amended by ADR-SAG-0020.

## ADR-SAG-0020 — Secret injection and redaction

**Status:** Active

Amends ADR-SAG-0003.

Resolution order stays ADR-SAG-0003. This decision records the extension
points and the secret-bearing return type.

- `EnvSource` and `KeyringSource` are open injection traits. `resolve_key_with`
  takes them. Production `resolve_key` uses `ProcessEnv` and `PlatformKeyring`.
- Cargo feature `test-util` exposes the in-memory doubles `MapEnv` and
  `MapKeyring`. Production builds leave the feature off. The doubles are not
  a second resolution policy.
- `resolve_key` and `resolve_key_with` return `ResolvedApiKey`. `Debug` and
  `Display` redact the secret. `as_str` is the only read, and only the
  transport that sends the key may call it.
- An unreadable repo-root `.env` or keyring is `CONFIG_INVALID`
  (`JevError::credential_store_invalid`). The cause is an I/O kind or a
  keyring variant label. It never includes secret bytes, passwords, or the
  raw platform error text. The fix hint names that cause and the recovery
  step. A miss stays `KEY_MISSING` (ADR-SAG-0001).
- This crate writes no log lines. Secrets in CLI output and logs remain
  NFR-SAG-0002 and ADR-SAG-0013.

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — lands in r-3.

- One sealing model: a secret travels only as a `SecretHeader` holding
  `ResolvedApiKey` inside `sc_agent_http::HttpRequest`. Only
  `sc-agent-http`'s `UreqTransport` reads it (clippy `disallowed-methods`
  allowlist). The `Transport` trait is open; test transports see redacted values.
- `ResolvedApiKey::try_new` is the only constructor. The hidden `new` is
  removed. Backend constructors (`JevClient::new`, Haiku, Luna) are infallible.
- Correction to the text above: `sc-agent-secrets` no longer has a
  `test-util` feature (removed in a-10). `MapEnv`/`MapKeyring` doubles live in
  its tests. Also as of a-10: `sc-jev`'s `api` module is renamed `config`,
  `client` is private, and `RecordingSessionLog`/`RejectingSessionLog` are
  deleted.

## ADR-SAG-0004 — One check, one request

**Status:** Active

No splitting oversized inputs. Pre-send byte/token validation on every backend
via `validate_for_send` before `judge`.

## ADR-SAG-0006 — Transport injection

**Status:** Active

Jev HTTP client lives in `sc-jev`; LLM HTTP in `sc-jev-backends`. Transports are
**injected at backend construction** (not passed into `JudgeBackend::judge`).
Both use test doubles for unit tests.

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — lands in r-3.

- HTTP transport, retry, caps and redaction live in `sc-agent-http`
  (ADR-SAG-0026). `sc-jev` and `sc-jev-backends` are wire-format crates that
  take an injected `sc_agent_http::Transport` at construction. Injection at
  construction (not per `judge`) is unchanged.

## ADR-SAG-0007 — API-direct LLM backends

**Status:** Active

Haiku and Luna judges use **provider HTTP only** in Phase A: no vendor SDK,
agent harness, or tool-use loop. Wire format and token counting are implemented
in `sc-jev-backends` against documented REST endpoints. **HTTP timeouts:** connect
10s, request 120s (configurable on backend builder for tests); busy retry per
REQ-SAG-0505.

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — lands in r-4 (numbers encoded in r-3).

- Busy retry policy for every backend: statuses 429/502/529; Retry-After as
  integer or decimal seconds clamped to 0–30; missing → 1 s; malformed → 30 s;
  jitter up to 250 ms; at most 4 attempts; 90 s total budget from the first
  send. No new attempt starts after the budget; an in-flight attempt is bounded
  by the request timeout (120 s). Three malformed-header sleeps alone use up
  the budget; accepted.

## ADR-SAG-0010 — CLI thin adapter

**Status:** Active

`sc-ai` parses argv and prints envelopes; no network code in the binary crate.
Depends on **`sc-jev-check`** + full `sc-observability`, not on `sc-jev` directly.

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — lands in r-3b.

- `sc-ai` depends on `sc-agent-compose` (observability adapter, path policy)
  and `sc-jev-check`. Argv parsing uses clap derive behind the default `cli`
  feature. There is still no network code in the binary crate.

## ADR-SAG-0011 — Python via sc-jev-py wheel

**Status:** Active

Python apps use the **`sc-jev-py`** PyPI extension (PyO3 → `sc-jev-check`). Optional
thin wrappers (e.g. atm-bd `packages/jev-check`) must not reimplement Jev HTTP
or canonicalization.

## ADR-SAG-0018 — sc-publish kit owns release surface

**Status:** Active

Release manifests, channel contracts, and publish workflows are **installed**
from [sc-publish](https://github.com/randlee/sc-publish) via
`release/sc-publish-consumer-input.json`. Edit consumer input + re-run
`install.py`; do not hand-edit rendered TOML under `release/`.

## ADR-SAG-0017 — First release surfaces

**Status:** Active

Phase A first release ships **five publish channels** from one tag via sc-publish:
**crates.io**, **PyPI** (`sc-jev-py`), **Homebrew**, **Scoop**, and **WinGet**
(`sc-ai` binary), plus GitHub Release assets. Sprint **a-9** closes
REQ-SAG-0130, REQ-SAG-0131, REQ-SAG-0132, and REQ-SAG-0803 (kit install,
preflight CI, and publish dry-run).

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — lands in r-4.

- Two released binaries: `sc-ai` and `sc-treesitter`. Coverage: GitHub Release,
  crates.io and Homebrew ship both. Scoop and WinGet expose only `sc-ai` as a
  shim/alias; `sc-treesitter` ships in the archive without a shim (sc-publish
  kit limitation; bead `agt-scoop-multi-shim`; upstream issue URL recorded by
  r-4). REQ-SAG-0133.

## ADR-SAG-0015 — Subjects artifact outside the judge

**Status:** Active

**Boundary (one line):** extraction produces the **subjects artifact outside the
judge**; the judge consumes it and **links no parser**.

Replaces delimited comment regions and in-crate tree-sitter (third review,
Rand 2026-11-02). **Normative artifact:** [`docs/sc-agent-types/subjects-schema.md`](sc-agent-types/subjects-schema.md) (`sc/subjects/1`). **Producer:** [`sc-treesitter`](sc-treesitter/architecture.md) (pinned static grammars, query assets, `matches` mode). **Consumer:** [`docs/sc-jev-check/extraction.md`](sc-jev-check/extraction.md) — kinds `file`, `table-column`, `subjects`, `command` only.

Delimited regions, in-facade tree-sitter, and Python `python-branches` extractor are **out of scope** for Phase A.

LLM system prompt loading: **ADR-SAG-0019** (`[llm_judge] system_prompt_file` in
check.toml; default asset under `sc-jev-check`).

## ADR-SAG-0013 — Session JSONL via sc-observability v2

**Status:** Active

Jev session/prompt logging uses **`sc-observability` v2** (`LoggerBuilder`,
file/console sinks). **`sc-jev-check`** emits `LogEvent`s through
**`SharedSessionLog`** (see ADR-SAG-0016). Composition roots **`sc-ai`** and
**`sc-jev-py`** link full `sc-observability` and adapt to `SharedSessionLog`.
**`sc-jev`** does not log and does not depend on observability crates. No bespoke
JSONL writers. Secrets never appear in log lines. Path precedence for the two
composition roots is ADR-SAG-0021.

## ADR-SAG-0021 — Log and session path precedence

**Status:** Active

Amends ADR-SAG-0013. The v2 file sink and the ban on bespoke JSONL writers
stay ADR-SAG-0013. This decision records where each composition root puts
that sink and its session files. First match wins. An empty flag, argument,
or environment value is absent.

**`sc-ai` (CLI):**

1. Log root: `--log-dir`, then `SC_LOG_ROOT`, then `<repo-root>/.sc-ai`.
2. Session directory: `--sessions-dir`, then `<repo-root>/.sc-ai/sessions`.
3. JSONL path: `<log-root>/logs/sc-ai.log.jsonl`.

**`sc-jev-py` (Python):**

1. Log root: `log_dir`, then `SC_AGENT_LOG_DIR`, then `SC_LOG_ROOT`, then
   `~/.sc-agent/log` (`%USERPROFILE%\.sc-agent\log` when `HOME` is unset; the
   process temp directory plus `sc-agent/log` when both are unset).
2. Session directory: `session_dir`, then `SC_AGENT_SESSION_DIR`, then
   `{log_root}/sessions`.
3. JSONL path: `{log_root}/logs/sc-jev-py.log.jsonl`.

`resolve_log_root` and `resolve_session_dir` are the Python contract for that
order. The two roots do not share a default directory. Secrets never appear
in either log (ADR-SAG-0013).

## ADR-SAG-0016 — v2 shim in sc-agent-types (no extra crate)

**Status:** Active

Do **not** publish a separate observability crate. For the **first release or two**,
**`sc-agent-types::observability::v2`** holds the injectable `SessionLog` port
(aligned with registry `v2::Logger`). Temporary dependency:
`sc-observability-types = "1.5.0"` on **`sc-agent-types` only**; full
`sc-observability = "1.5.0"` on composition roots **`sc-ai`** and **`sc-jev-py`**.
Libraries take only **`SharedSessionLog`**. When upstream v2 ships, **delete
the module** and drop the types dependency if possible (bead:
`agt-obs-v2-shim-removal`).

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — bullets land in r-4, r-1, r-3b.

- (r-4) The pin is exact: `sc-observability = "=1.5.0"`, a deliberate
  lockstep per ADR-SAG-0023.
- (r-1) `sc-agent-types` no longer depends on `sc-observability-types`.
  `observability::v2::SessionLog::emit` returns the local `LogRejected`.
- (r-3b) "No extra crate" is superseded for the adapter: the
  `sc-observability` adapter lives once in `sc-agent-compose` (ADR-SAG-0026).
  The port stays in `sc-agent-types`.

## ADR-SAG-0014 — CLI success body

**Status:** Active

On success, stdout is the versioned sc-ai-cli envelope
`{ "version", "operation", "data" }` where **`data`** holds the **verbatim Jev
response JSON** for the operation plus **`session_id`** (REQ-SAG-0710). LLM
backends normalize to the same answer shape before printing.

## ADR-SAG-0012 — sc-jev owns Jev API

**Status:** Active

Every TypeSafe Jev HTTP operation is implemented in `sc-jev`. Other crates
must not send Jev API requests. Phase A ships the check/judge slice; remaining
endpoints extend `sc-jev` in place.

## Rust API patterns (Phase A)

**Status:** Active

- **Digest newtypes:** `Sha256Digest` (lowercase hex, 64 chars) for
  `PayloadBundle.sha256`, `EqualInputRecord.prompt_sha256`, and log fields — not
  unconstrained `String` (RBP-F002).
- **Domain newtypes:** `RepoRoot`, `Rev`, `SessionId`, `BackendKeyId` in
  `sc-agent-types` (frozen in a-1); secrets APIs take `&RepoRoot` not raw paths
  where the contract requires repo context (RBP-F003).
- **Resolved secrets:** `ResolvedApiKey` in `sc-agent-types`, returned by
  `sc-agent-secrets::resolve_key` (ADR-SAG-0020). `Debug` and `Display` redact.
  `as_str` is the only read of the secret, and only the sending transport calls it.
- **Extractor sealing:** `Extractor` is `pub trait Extractor: private::Sealed` —
  only the four Phase A kinds implement it; external crates cannot add kinds
  without a new ADR (RBP-F004).
- **Backend dispatch:** `sc-jev-check` holds `Vec<Arc<dyn JudgeBackend>>` (or
  equivalent registry) for multi-backend runs (RBP-F005).

## Roadmap — API agents vs CLI wrappers

| Approach | Status |
| --- | --- |
| API-direct Anthropic / OpenAI (Haiku/Luna in `sc-jev-check` flow; future `sc-ai` agents) | **Wanted** — Phase A and Phase B (standalone) |
| Subprocess JSON bridges to claude-code, codex-cli, cursor-cli | **Out of scope** |

Future agent crates sit beside `sc-jev` and depend on `sc-agent-types` +
`sc-agent-secrets`; they do not duplicate Jev HTTP or payload rules from `sc-jev` when running
comparison flows.

## Related documents

- [`requirements.md`](requirements.md) — REQ-SAG ids
- [`design.md`](design.md) — goals and non-goals
- [`framework-charter.md`](framework-charter.md) — multi-consumer Rust posture
- [`plans/phase-a/plan.md`](plans/phase-a/plan.md) — Phase A sprints and boundaries
- Per-crate: `docs/sc-agent-types/`, `docs/sc-jev-check/` (+ [`extraction.md`](sc-jev-check/extraction.md)), `docs/sc-jev/`, etc.

### Amendment (Phase A.1, proposed)

**Status:** Proposed (Phase A.1) — lands in r-2.

- The "Extractor sealing" bullet (RBP-F004) is withdrawn: the `Extractor` trait
  is removed and `extract_input` matches the four `InputSpec` kinds directly.
  A new kind is an `InputSpec` variant plus an ADR.


## ADR-SAG-0023 — Dependency pins, allocator, released binaries

**Status:** Proposed (Phase A.1) — lands in r-4.

- Library crates declare caret ranges. Exact `=` pins are allowed only for
  deliberate lockstep, and every one is listed here: `sc-observability`
  (v2 shim alignment); `tree-sitter` and grammar crates (output determinism);
  the intra-workspace `sc-agent-compose`. NFR-SAG-0004.
- Global allocators are linked only by binaries: `mimalloc` is optional behind
  each binary crate's default `cli` feature, and the `[[bin]]` targets use
  `required-features = ["cli"]`. Libraries build with
  `--no-default-features` without mimalloc or clap (REQ-SAG-0134).
- Released binaries are `sc-ai` and `sc-treesitter`. Per-channel coverage is
  ADR-SAG-0017's amendment.

## ADR-SAG-0024 — Engine entry point and response contract

**Status:** Proposed (Phase A.1) — lands in r-2.

- `sc-jev-check`'s only run API is `prepare(CheckSource, &RunConfig) -> PreparedRun`
  then `run(PreparedRun, &RunConfig) -> RunResponse`, plus
  `discover_repo_root(&Path)`. `build_request`, `run_check`, `run_request` and
  `prepare_caller_request` are deleted (no shims; nothing published).
- `RunResponse` (`#[non_exhaustive]`) is the one result type every consumer
  (CLI, PyO3, future MCP) serializes. `RunResponse::to_envelope_data` is the
  ADR-SAG-0014 `data` body. REQ-SAG-0620.
- `FileSessionStore` is stateless (`dir`, `ttl`, `clock`). `ask` takes the
  `&BackendRegistry`. The session is written once at the end of `run`, only
  when a backend with `supports_session_ask` took part. REQ-SAG-0607.
- Each backend owns its request byte cap and enforces it in `validate_for_send`.
  The engine does not pre-cap. REQ-SAG-0314.

## ADR-SAG-0025 — Multi-backend semantics and equal-input attestation

**Status:** Proposed (Phase A.1) — lands in r-2b. Amends ADR-SAG-0008 and ADR-SAG-0019.

- The default mode is `Independent`; `ChainContext` is explicit and recorded.
  REQ-SAG-0605.
- All backends are validated before any send. Each backend then yields
  `BackendStatus::Ok | Failed`, and a failure does not discard other answers.
  `leak = any Failed`. If all fail, `run` returns `Err`. Consumers print the
  envelope on stdout and the first failure on stderr, and exit 2 on a partial
  failure. REQ-SAG-0604.
- `equal` is computed from each backend's reported `input_sha256`, not assumed.
  REQ-SAG-0606.

## ADR-SAG-0026 — Shared HTTP crate and composition crate

**Status:** Proposed (Phase A.1) — lands in r-3b (HTTP half proven in r-3 via ADR-SAG-0006/0020).

- `sc-agent-http` (published) owns `HttpRequest`/`HttpResponse`, `Transport`,
  `UreqTransport`, `RetrySleeper`/`BusyPolicy`, caps and redaction, and is the
  single secret-read site. It depends only on `sc-agent-types` and `ureq`.
  Phase B provider agents reuse it. REQ-SAG-0402.
- `sc-agent-compose` (published as internal support, `=`-pinned by `sc-ai` and
  `sc-jev-py`, no stability promise outside the workspace) owns the
  `sc-observability` adapter, the two ADR-SAG-0021 path policies (unchanged,
  per root), and `run_from`. It is published because `sc-ai` is on crates.io
  and cannot depend on an unpublished crate.

## ADR-SAG-0027 — Error taxonomy and extensibility

**Status:** Proposed (Phase A.1) — lands in r-1. Amends ADR-SAG-0001.

- `JevError`, `BackendId` and `BackendKeyId` are `#[non_exhaustive]`.
  Recovery categories are explicit variants (`CredentialStoreInvalid`,
  `RegistryInvalid`, `SessionStoreIo`, `Operational`, `SessionDocumentInvalid`,
  `EncodeFailed`, `IoFailed`). Producer variants are neutral
  (`LanguageUnavailable { producer, language }`). No string-prefix parsing.
  REQ-SAG-0106.
- `code()` ids are unchanged. New variants map to existing codes;
  `EncodeFailed`/`IoFailed` are `INPUT_INVALID`. A new stable code requires
  changing the README `errors.json` contract shared with Python. Revisit when
  Phase B needs an `INTERNAL` class.
- `retryable()` is true only for transient `NetworkError` kinds (Busy, Timeout, Connect).
- `Error::source()` is `None` by design: causes are redacted strings (ADR-SAG-0020).
- Serde spelling of backend ids matches `Display` (`openai_luna`, `openai`),
  with a one-release read alias for `open_ai_luna` / `open_ai`.
