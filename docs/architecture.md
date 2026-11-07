# sc-agent architecture

**Status:** Active (Phase A implemented on `integrate/phase-a`)  
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
                └── sc-observability-types (=1.5.0, v2 port types)

sc-jev-py (PyO3) ── sc-observability v2 Logger → SessionLog adapter
  └── sc-jev-check

sc-treesitter (library + binary) → sc-agent-types
```

Every workspace crate depends on `sc-agent-types`; the graph shows the edge
once. `sc-agent-types` is the only crate that depends on
`sc-observability-types` (ADR-SAG-0016). Only the composition roots `sc-ai`
and `sc-jev-py` depend on full `sc-observability`.

`sc-jev` and `sc-jev-backends` do not depend on each other.

**`sc-treesitter`** is the `sc/subjects/1` producer (ADR-SAG-0015). It depends
on `sc-agent-types` and links no judge, secrets or HTTP crate. No workspace
crate depends on it; `sc-jev-check` reaches it only by running its binary
through a `command` input. The crate ships a **second released binary**, `sc-treesitter`,
alongside `sc-ai`. Direct dependencies of every crate are listed in its
`boundaries/<crate>/*.toml` manifest ([`boundaries/README.md`](../boundaries/README.md)).

## Phase A.1 crate graph

**Status:** Proposed (Phase A.1) — lands in r-3b.

The target graph is in [`plans/phase-a-refine/plan.md`](plans/phase-a-refine/plan.md)
("Crate graph after A.1"). r-3b replaces this paragraph with that graph when
`sc-agent-compose` and `workspace-tests` land.

**Jev API:** All TypeSafe Jev HTTP lives in **`sc-jev`** (REQ-SAG-0110).  
**Trait boundary:** `JudgeBackend` and shared domain types live in
`sc-agent-types`. LLM comparison backends implement the trait in
`sc-jev-backends`. Local judge (decider-4b / cyprid) adds a backend later
without changing callers.

## ADR index

ADR sections below keep their original order. This table lists them in id
order. One rule fills the last column (plan.md conventions): a new ADR lists
every older ADR whose Phase A.1 amendment subsection it drives; an older ADR
lists every A.1 subsection it receives, as the driving ADR or as "A.1
subsection" when only a D decision drives it. Each A.1 entry carries a
`(Proposed, r-N)` marker. A landing sprint flips only its own row's Status
cell and its own marker text to `(Active, r-N)`; it adds or removes no entry.

| ADR | Title | Status | Amends / amended by |
| --- | --- | --- | --- |
| ADR-SAG-0001 | Error enum is CLI authority | Active | Amended by ADR-SAG-0027 (Proposed, r-1) |
| ADR-SAG-0002 | Canonical judge payload bytes | Active | — |
| ADR-SAG-0003 | Secret resolution | Active | Amended by ADR-SAG-0020; ADR-SAG-0024 (Proposed, r-2) |
| ADR-SAG-0004 | One check, one request | Active | Amended by ADR-SAG-0025 (Proposed, r-2b) |
| ADR-SAG-0005 | Facade owns payload assembly | Active | Amended by ADR-SAG-0024 (Proposed, r-2); A.1 subsections (Proposed, r-4), (Proposed, r-5) |
| ADR-SAG-0006 | Transport injection | Active | Amended by ADR-SAG-0026 (Proposed, r-3) |
| ADR-SAG-0007 | API-direct LLM backends | Active | Amended by ADR-SAG-0026 (Proposed, r-3) |
| ADR-SAG-0008 | Sessions carry prior answers to later backends | Active | Amended by ADR-SAG-0025 (Proposed, r-2b) |
| ADR-SAG-0009 | Local backend registers without caller changes | Active | Amended by ADR-SAG-0022; ADR-SAG-0027 (Proposed, r-1); ADR-SAG-0024 (Proposed, r-2) |
| ADR-SAG-0010 | CLI thin adapter | Active | Amended by ADR-SAG-0026 (Proposed, r-3b) |
| ADR-SAG-0011 | Python via sc-jev-py wheel | Active | Amended by ADR-SAG-0026 (Proposed, r-3b) |
| ADR-SAG-0012 | sc-jev owns Jev API | Active | — |
| ADR-SAG-0013 | Session JSONL via sc-observability v2 | Active | Amended by ADR-SAG-0021; ADR-SAG-0026 (Proposed, r-3b) |
| ADR-SAG-0014 | CLI success body | Active | Amended by ADR-SAG-0024 (Proposed, r-2) |
| ADR-SAG-0015 | Subjects artifact outside the judge | Active | — |
| ADR-SAG-0016 | v2 shim in sc-agent-types (no extra crate) | Active | Amended by ADR-SAG-0023 (Active, r-4); ADR-SAG-0027 (Proposed, r-1); ADR-SAG-0026 (Proposed, r-3b) |
| ADR-SAG-0017 | First release surfaces | Active | Amended by ADR-SAG-0023 (Proposed, r-4); ADR-SAG-0026 (Proposed, r-3), (Proposed, r-3b) |
| ADR-SAG-0018 | sc-publish kit owns release surface | Active | — |
| ADR-SAG-0019 | LLM wire envelope vs canonical payload | Active | Amended by ADR-SAG-0024 (Proposed, r-2); ADR-SAG-0025 (Proposed, r-2b) |
| ADR-SAG-0020 | Secret injection and redaction | Active | Amends ADR-SAG-0003; amended by ADR-SAG-0029; A.1 subsection (Proposed, r-1); ADR-SAG-0026 (Proposed, r-3) |
| ADR-SAG-0021 | Log and session path precedence | Active | Amends ADR-SAG-0013 |
| ADR-SAG-0022 | Scripted backends are a test-util feature | Active | Amends ADR-SAG-0009; amended by ADR-SAG-0029; A.1 subsection (Proposed, r-1); ADR-SAG-0024 (Proposed, r-2); ADR-SAG-0025 (Proposed, r-2b) |
| ADR-SAG-0023 | Dependency pins, allocator, released binaries | Proposed (r-4; compose pin r-3b) | Amends ADR-SAG-0016, ADR-SAG-0017 |
| ADR-SAG-0024 | Engine entry point and response contract | Proposed (r-2) | Amends ADR-SAG-0003, ADR-SAG-0005, ADR-SAG-0009, ADR-SAG-0014, ADR-SAG-0019, ADR-SAG-0022 |
| ADR-SAG-0025 | Multi-backend semantics and equal-input attestation | Proposed (r-2b) | Amends ADR-SAG-0004, ADR-SAG-0008, ADR-SAG-0019, ADR-SAG-0022 |
| ADR-SAG-0026 | Shared HTTP crate and composition crate | Proposed (HTTP half r-3; compose half r-3b) | Amends ADR-SAG-0006, ADR-SAG-0007, ADR-SAG-0020, ADR-SAG-0017 (HTTP half); ADR-SAG-0010, ADR-SAG-0011, ADR-SAG-0013, ADR-SAG-0016, ADR-SAG-0017 (compose half) |
| ADR-SAG-0027 | Error taxonomy and extensibility | Proposed (r-1) | Amends ADR-SAG-0001, ADR-SAG-0009, ADR-SAG-0016 |
| ADR-SAG-0028 | workspace-tests crate | Proposed (r-3b) | — |
| ADR-SAG-0029 | Phase A hardening amendments | Active (a-10) | Amends ADR-SAG-0020 and ADR-SAG-0022; A.1 supersedes named sentences of (a) (Proposed, r-1), (Proposed, r-3) and of (b) (Proposed, r-1), (Proposed, r-2) |

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

### Amendment (Phase A.1, r-4)

**Status:** Proposed (Phase A.1) — lands in r-4.

- (Proposed, r-4) `sc-jev-check` runs the four consumer extractor kinds
  (`file`, `table-column`, `subjects`, `command`); it is not the only
  producer of subjects (ADR-SAG-0015).

### Amendment (Phase A.1, r-2)

**Status:** Proposed (Phase A.1) — lands in r-2.

- (Proposed, r-2) The public assembly entry point is `prepare`
  (ADR-SAG-0024). The sentences "`run::build_request` is the public assembly
  entry" and "`run_check` uses that same assembly" above are superseded.

### Amendment (Phase A.1, r-5 — blocked on vendor answer)

**Status:** Proposed (Phase A.1) — lands in r-5; blocked (D9, owner decision (c)).

- (Proposed, r-5) `CanonicalJudgePayload` is the only payload schema. Both backend families
  validate by deserializing it. The Jev HTTP body is a JSON envelope built
  around the canonical object (no byte-splice); `prompt_sha256` stays on
  canonical bytes.

## ADR-SAG-0008 — Sessions carry prior answers to later backends

**Status:** Active

When multiple backends run in one invocation, **session state** is how a second
opinion receives prior answers. TypeSafe Jev has no rationale field on the wire;
Haiku/Luna session `ask` paths reuse stored context. Session files are UUID paths
with mode `0600` (sprint a-6).

### Amendment (Phase A.1, r-2b)

**Status:** Proposed (Phase A.1) — lands in r-2b.

- (Proposed, r-2b) Multi-backend runs are **independent by default**: no backend receives
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

### Amendment (Phase A.1, r-1)

**Status:** Proposed (Phase A.1) — lands in r-1.

- (Proposed, r-1) `BackendId` is `#[non_exhaustive]`. Static backend facts
  (`family`, `key_id`, `requires_llm_context`, `supports_session_ask`,
  `request_limit_bytes`) live on `BackendDescriptor` rows in `BACKENDS`;
  `JudgeBackend::capabilities()` is a provided method derived from the
  descriptor.
- (Proposed, r-1) Backend dispatch is a **test-and-review convention**, not a
  lint: the production factory is a table
  `&[(BackendId, fn(ResolvedApiKey) -> Arc<dyn JudgeBackend>)]` in
  `sc-jev-check::registry`, checked against `BACKENDS` by
  `test_every_backend_has_factory`. Everywhere else, code branches on
  descriptor facts (`family`, `requires_llm_context`,
  `supports_session_ask`, `request_limit_bytes`), never on `BackendId`
  equality or a `match`; review enforces it (no clippy lint expresses it).
- (Proposed, r-1) "Open" means additive: adding a built-in backend is a
  minor `sc-agent-types` change (variant + `BACKENDS` row) plus one factory
  table row, with no caller signature change (checklist in ADR-SAG-0027). It
  is not a runtime-open registry: `BackendRegistry::register` installs a
  double or scripted backend for an existing id and cannot add an id.
- (Proposed, r-1) `JudgeBackend` is intentionally open (not sealed); every
  method added after A.1 has a default body.

### Amendment (Phase A.1, r-2)

**Status:** Proposed (Phase A.1) — lands in r-2.

- (Proposed, r-2) The sentence "Callers keep `run_check(..., backends:
  &[BackendId])`" is superseded: callers use `prepare`/`run` (ADR-SAG-0024)
  with `RunConfig::backends(BackendSet)`. `run_check` is deleted. A new
  backend still needs no CLI or PyO3 signature change.

## ADR-SAG-0022 — Scripted backends are a test-util feature

**Status:** Active

Amends ADR-SAG-0009. Amended by ADR-SAG-0029.

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
  *Superseded by ADR-SAG-0029 (b):* `run_check` has one signature with no
  `scripted_answers` parameter in any build. Scripted injection is the
  separate function `sc_jev_py._testing.run_check_scripted`, registered only
  under feature `test-util`.
- Default features of both crates leave `test-util` off. The publish wheel
  does not pass `--features test-util`. Sprint a-8 pytest may enable it so
  session `ask` runs without a network send. Enabling it does not change
  production registration.
  *Superseded in part by ADR-SAG-0029 (b):* the a-8 pytest calls
  `sc_jev_py._testing.run_check_scripted`, not a `run_check` argument.

### Amendment (Phase A.1, r-1)

**Status:** Proposed (Phase A.1) — lands in r-1.

- (Proposed, r-1) `registry::ScriptedJudge` and `registry::RequestLog` are
  deleted; their callers use `sc_agent_types::test_util::MockJudgeBackend`
  (`requests()`, `validations()`, `with_provider_meta`, `with_failure`,
  `refusing`). `register_scripted_backends` keeps its signature and the
  string-shorthand normalisation, and builds `MockJudgeBackend`s; the
  `sc-jev-check` feature becomes `test-util = ["sc-agent-types/test-util"]`.
  Supersedes the ADR-SAG-0029 (b) sentence "`sc-jev-check` feature
  `test-util` makes `registry::ScriptedJudge`, `registry::RequestLog`, and
  `registry::register_scripted_backends` public": only
  `register_scripted_backends` stays.
- (Proposed, r-1) The sentence above "An LLM backend id still requires
  `JudgeRequest.llm` before it returns" still holds and is not superseded:
  `MockJudgeBackend` enforces it from the descriptor
  (`requires_llm_context()`), so `validate_for_send` (counted) and `judge`
  return `INPUT_INVALID` for an LLM id with no LLM context and record
  nothing.

### Amendment (Phase A.1, r-2)

**Status:** Proposed (Phase A.1) — lands in r-2.

- (Proposed, r-2) The scripted registry is passed via `RunConfig::registry`
  (ADR-SAG-0024), not `RunOptions::registry`; the sentences "not a second
  `run_check` policy" and "`run_check`'s parameters stay unchanged" now read
  against `prepare`/`run`. `register_scripted_backends` and its `test-util`
  gating are unchanged. Correction: an integration test target does not
  compile the library with `cfg(test)`, so "this crate's own `cargo test` …
  sees the function without enabling the feature" holds only for unit
  tests; `sc-jev-check`'s integration targets enable `test-util` through a
  self-referencing dev-dependency (plan.md "Test seams"). For that one
  sentence this supersedes ADR-SAG-0029 (b)'s "Everything else in
  ADR-SAG-0022 stands"; 0029 (b)'s `run_check` signature and
  `_testing.run_check_scripted` rules are unchanged.

### Amendment (Phase A.1, r-2b)

**Status:** Proposed (Phase A.1) — lands in r-2b.

- (Proposed, r-2b) The scripted backend reports, as
  `JudgeOutcome.input_sha256`, the digest of the bundle it received, so
  equal-input attestation (ADR-SAG-0025) treats it like a real backend;
  tests that need a mismatch use `MockJudgeBackend::with_reported_digest`.
- (Proposed, r-2b) `register_scripted_backends` accepts a second JSON form,
  `{"backends": {<backend id>: <answers> | {"failure": {"error", "kind"?, "stage"?}}}}`,
  so one backend can fail (`stage` `judge`: `with_failure`; `validate`:
  `refusing`) while others answer; the a-10 shared form (question id →
  answer, with the ADR-SAG-0029 (b) string shorthand) is unchanged. The
  `sc-ai` hidden `--scripted-answers` file and
  `sc_jev_py._testing.run_check_scripted(scripted_answers=...)` take the
  same JSON (r-3b). Schema in sprint r-2b D5.

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

### Amendment (Phase A.1, r-2)

**Status:** Proposed (Phase A.1) — lands in r-2.

- (Proposed, r-2) The `RunOptions` field list above is replaced by
  `RunConfig` (ADR-SAG-0024). The prompt precedence is unchanged.

### Amendment (Phase A.1, r-2b)

**Status:** Proposed (Phase A.1) — lands in r-2b.

- (Proposed, r-2b) Equal-input records, per backend, an `Attestation`
  computed from `JudgeOutcome.input_sha256`: the digest of the canonical blob
  that backend recovered from its final serialized wire body. `equal` is true
  only if every backend's attestation is `Matched`. See ADR-SAG-0025.

## ADR-SAG-0003 — Secret resolution

**Status:** Active

`sc-agent-secrets` resolves keys: **process env → repo-root `.env` → keyring**.
**Only `sc-jev-check`** calls `resolve_key`. `JevClient` and LLM backends receive
the resolved secret (and transport) at **construction**; they do **not** depend on
`sc-agent-secrets`. `JudgeRequest` carries neither keys nor `repo_root`.

Amended by ADR-SAG-0020.

### Amendment (Phase A.1, r-2)

**Status:** Proposed (Phase A.1) — lands in r-2.

- (Proposed, r-2) "Repo root" for `.env` lookup is
  `sc_jev_check::run::discover_repo_root`: the git toplevel of the start
  directory, or the start directory (absolutized) outside git. Both `sc-ai`
  and `sc-jev-py` use it. Behaviour change: `sc-jev-py` called from a
  subdirectory now reads the toplevel `.env` instead of the subdirectory's.

## ADR-SAG-0020 — Secret injection and redaction

**Status:** Active

Amends ADR-SAG-0003. Amended by ADR-SAG-0029.

Resolution order stays ADR-SAG-0003. This decision records the extension
points and the secret-bearing return type.

- `EnvSource` and `KeyringSource` are open injection traits. `resolve_key_with`
  takes them. Production `resolve_key` uses `ProcessEnv` and `PlatformKeyring`.
- Cargo feature `test-util` exposes the in-memory doubles `MapEnv` and
  `MapKeyring`. Production builds leave the feature off. The doubles are not
  a second resolution policy.
  *Superseded by ADR-SAG-0029 (a):* `sc-agent-secrets` has no `test-util`
  feature. `MapEnv` and `MapKeyring` are `#[cfg(test)]` crate-private code
  and are not exported.
- `resolve_key` and `resolve_key_with` return `ResolvedApiKey`. `Debug` and
  `Display` redact the secret. `as_str` is the only read, and only the
  transport that sends the key may call it.
  *Amended by ADR-SAG-0029 (a):* `ResolvedApiKey::try_new` is the one
  construction rule set, and `clippy.toml` `disallowed-methods` enforces the
  `as_str` rule.
- An unreadable repo-root `.env` or keyring is `CONFIG_INVALID`
  (`JevError::credential_store_invalid`). The cause is an I/O kind or a
  keyring variant label. It never includes secret bytes, passwords, or the
  raw platform error text. The fix hint names that cause and the recovery
  step. A miss stays `KEY_MISSING` (ADR-SAG-0001).
- This crate writes no log lines. Secrets in CLI output and logs remain
  NFR-SAG-0002 and ADR-SAG-0013.

### Amendment (Phase A.1, r-1)

**Status:** Proposed (Phase A.1) — lands in r-1.

- (Proposed, r-1) `ResolvedApiKey::try_new` is the only constructor in every
  build; the unchecked `new` is removed, and test keys use `try_new`; the
  secret is held as `Arc<str>`. Backend constructors (`JevClient::new`,
  Haiku, Luna, and their `with_default_transport`) are infallible.
- (Proposed, r-1) Supersedes these ADR-SAG-0029 (a) sentences: "The
  unchecked `ResolvedApiKey::new` exists only under `cfg(test)` or the
  `sc-agent-types/test-util` feature, for test doubles" (it no longer
  exists), and, for the two sending transports, "carry
  `#[allow(clippy::disallowed_methods, reason = ...)]`" (from r-1 they carry
  `#[expect(..)]`; r-3 deletes both files).

### Amendment (Phase A.1, r-3)

**Status:** Proposed (Phase A.1) — lands in r-3.

- (Proposed, r-3) A secret travels only as a `SecretHeader` holding
  `ResolvedApiKey` inside `sc_agent_http::HttpRequest`. The single read site
  is a **lint-enforced workspace convention, not a type seal**:
  `ResolvedApiKey::as_str` is listed in `clippy.toml` `disallowed-methods`;
  its one call site, `crates/sc-agent-http/src/ureq_transport.rs`, carries
  `#[expect(clippy::disallowed_methods, reason = ...)]`; workspace lints deny
  `allow_attributes` and `allow_attributes_without_reason`; every crate has
  `[lints] workspace = true` with no override; and
  `workspace-tests --test source_scans` (r-3b) fails if a
  `disallowed_methods` suppression appears in any other file of any target
  (`src`, `tests`, `benches`, `examples`, `build.rs`) or a crate stops
  inheriting the workspace lints. Crates outside the workspace are not
  constrained. Redaction is applied once, in `send_with_busy_retry`, to every
  error it returns, including errors a `Transport` impl returns; a caller of
  `Transport::send` directly bypasses it.
- (Proposed, r-3) The `Transport` trait is open; test transports see
  `SecretHeader::redacted()` / `matches()` only.
- (Proposed, r-3) Supersedes the ADR-SAG-0029 (a) sentence naming
  `crates/sc-jev/src/client/https.rs` and
  `crates/sc-jev-backends/src/transport.rs` as "the only production
  readers": r-3 deletes both, and the one reader is
  `crates/sc-agent-http/src/ureq_transport.rs`. That `sc-agent-secrets` has
  no Cargo features is ADR-SAG-0029 (a) and is not restated here.

## ADR-SAG-0004 — One check, one request

**Status:** Active

No splitting oversized inputs. Pre-send byte/token validation on every backend
via `validate_for_send` before `judge`.

### Amendment (Phase A.1, r-2b)

**Status:** Proposed (Phase A.1) — lands in r-2b.

- (Proposed, r-2b) "Before `judge`" applies to the request actually sent. In
  `MultiBackendMode::ChainContext`, each augmented request is re-validated
  immediately before its own send; a refusal there marks that backend
  `Failed` (`leak = true`) and sends nothing for it. The base request is still
  validated on every backend before the first send (ADR-SAG-0025).

## ADR-SAG-0006 — Transport injection

**Status:** Active

Jev HTTP client lives in `sc-jev`; LLM HTTP in `sc-jev-backends`. Transports are
**injected at backend construction** (not passed into `JudgeBackend::judge`).
Both use test doubles for unit tests.

### Amendment (Phase A.1, r-3)

**Status:** Proposed (Phase A.1) — lands in r-3.

- (Proposed, r-3) HTTP transport, retry, caps and redaction live in `sc-agent-http`
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

### Amendment (Phase A.1, r-3)

**Status:** Proposed (Phase A.1) — lands in r-3. Numbers decided by the
owner (2026-11-06).

- (Proposed, r-3) One busy-retry policy for every backend, in
  `sc_agent_http::BusyPolicy`: statuses 429/502/529 only; at most 4 sends;
  a valid Retry-After (integer or decimal seconds) is honoured, clamped to
  0–30 s. A valid Retry-After is 1–32 bytes of ASCII digits with an
  optional `.` and ASCII-digit fraction; anything else (HTTP-date, sign,
  exponent, `inf`, `NaN`, empty) is malformed. When Retry-After is absent
  **or malformed** (treated the same), the wait backs off exponentially from
  1 s (1, 2, 4, 8 … capped at 30 s); with the default 4 sends at most three
  waits occur (1, 2, 4 s), so 8 s and the cap apply only to a non-default
  policy. Every wait adds up to 250 ms jitter. A 90 s budget from the first send governs
  starting new attempts: no attempt starts after it, and every wait is capped
  to the remaining budget; an in-flight attempt keeps its 120 s request
  timeout (covering the body read). A `CancelToken` is checked before each
  attempt and during sliced waits (≤ 100 ms); a cancel during an in-flight
  request is observed when it returns, so worst-case cancel latency is the
  120 s request timeout. A transport error or timeout after the body was
  sent is never retried. A 502 retry can double-charge a provider
  that processed the request.
- (Proposed, r-3) No network-connectivity detection and no indefinite wait on
  network loss: connect and DNS failures return `NETWORK_ERROR`
  (`NetworkErrorKind::Connect`, `retryable() == true`) at once, and the caller
  (CLI, Python, future MCP) decides whether to retry.

## ADR-SAG-0010 — CLI thin adapter

**Status:** Active

`sc-ai` parses argv and prints envelopes; no network code in the binary crate.
Depends on **`sc-jev-check`** + full `sc-observability`, not on `sc-jev` directly.

### Amendment (Phase A.1, r-3b)

**Status:** Proposed (Phase A.1) — lands in r-3b.

- (Proposed, r-3b) `sc-ai` depends on `sc-agent-compose` (observability adapter, path policy)
  and `sc-jev-check`. Argv parsing uses clap derive behind the default `cli`
  feature. There is still no network code in the binary crate. It no longer
  depends on `sc-observability` directly (ADR-SAG-0013 amendment).

## ADR-SAG-0011 — Python via sc-jev-py wheel

**Status:** Active

Python apps use the **`sc-jev-py`** PyPI extension (PyO3 → `sc-jev-check`). Optional
thin wrappers (e.g. atm-bd `packages/jev-check`) must not reimplement Jev HTTP
or canonicalization.

### Amendment (Phase A.1, r-3b)

**Status:** Proposed (Phase A.1) — lands in r-3b.

- (Proposed, r-3b) The wheel's path is PyO3 → `sc-agent-compose` →
  `sc-jev-check`. `sc-jev-py` keeps only argument conversion, the GIL release
  and exception mapping; path policy, logging adapter and run wiring are
  `sc-agent-compose`'s (`PathPolicy::python()`).

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

### Amendment (Phase A.1, r-4)

**Status:** Proposed (Phase A.1) — lands in r-4.

- (Proposed, r-4) Two released binaries: `sc-ai` and `sc-treesitter`
  (ADR-SAG-0023). Per-channel coverage, matching
  `release/sc-publish-consumer-input.json` and the rendered
  `release/publish-artifacts.toml`:

  | Channel | sc-ai | sc-treesitter |
  | --- | --- | --- |
  | GitHub Release archive | yes | yes |
  | crates.io (`cargo install`) | yes | yes |
  | Homebrew | yes | yes (same formula) |
  | Scoop | shim | in archive, no shim |
  | WinGet | alias | in package, no alias |

  The Scoop/WinGet single-binary limit is an sc-publish kit limitation
  (`[channels.scoop].binary` is one string; WinGet has no per-binary alias
  field), so no consumer-input field can add the second shim. Bead
  `agt-scoop-multi-shim`; upstream sc-publish issue: not yet filed — its URL
  is recorded here when it is. The Windows invocation path for
  `sc-treesitter` is in `docs/development/sc-publish.md`. REQ-SAG-0133.

### Amendment (Phase A.1, r-3)

**Status:** Proposed (Phase A.1) — lands in r-3.

- (Proposed, r-3) Published library crates gain `sc-agent-http`. crates.io
  publish order: `sc-agent-types`, `sc-agent-secrets`, `sc-agent-http`,
  `sc-jev`, `sc-jev-backends`, `sc-jev-check`, `sc-treesitter`, `sc-ai`.

### Amendment (Phase A.1, r-3b)

**Status:** Proposed (Phase A.1) — lands in r-3b.

- (Proposed, r-3b) Published library crates gain `sc-agent-compose`
  (internal support, owner decision 2026-11-06), published directly after
  `sc-jev-check`. `workspace-tests` is `publish = false`. No crate is
  published to crates.io until D9 resolves (plan.md decision (c)).

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

### Amendment (Phase A.1, r-3b)

**Status:** Proposed (Phase A.1) — lands in r-3b.

- (Proposed, r-3b) The sentence "Composition roots `sc-ai` and `sc-jev-py`
  link full `sc-observability`" is superseded: `sc-agent-compose` is the only
  crate that links `sc-observability` (`sc-lint.toml` ownership record) and
  holds the one `SessionLog` adapter; both roots use it.

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

### Amendment (Phase A.1, r-4)

**Status:** Active (Phase A.1) — landed in r-4.

- (Active, r-4) The pin is exact: `sc-observability = "=1.5.0"`, a
  deliberate lockstep per ADR-SAG-0023. The version text `"1.5.0"` above is
  superseded by `=1.5.0` (exact).

### Amendment (Phase A.1, r-1)

**Status:** Proposed (Phase A.1) — lands in r-1.

- (Proposed, r-1) `sc-agent-types` no longer depends on
  `sc-observability-types`, and the root workspace entry is deleted (no
  user remains). `observability::v2::SessionLog::emit` returns the local
  `LogRejected`. The `observability` module is behind the default-on `obs`
  feature (ADR-SAG-0027). REQ-SAG-0120/0121 are amended to match.

### Amendment (Phase A.1, r-3b)

**Status:** Proposed (Phase A.1) — lands in r-3b.

- (Proposed, r-3b) "Do not publish a separate observability crate" and "no
  extra crate" are superseded for `sc-agent-compose`: the `sc-observability`
  adapter lives once in that crate, which is published to crates.io as
  internal support, `=`-pinned by `sc-ai` and `sc-jev-py`, with no semver
  promise of its own (owner decision 2026-11-06; ADR-SAG-0026). The port
  stays in `sc-agent-types`.
- (Proposed, r-3b) This ADR is retitled "v2 shim in sc-agent-types; adapter
  in sc-agent-compose" (heading and ADR index title cell), since "(no extra
  crate)" no longer holds.

## ADR-SAG-0014 — CLI success body

**Status:** Active

On success, stdout is the versioned sc-ai-cli envelope
`{ "version", "operation", "data" }` where **`data`** holds the **verbatim Jev
response JSON** for the operation plus **`session_id`** (REQ-SAG-0710). LLM
backends normalize to the same answer shape before printing.

Amended by ADR-SAG-0024 (Proposed, r-2).

### Amendment (Phase A.1, r-2)

**Status:** Proposed (Phase A.1) — lands in r-2.

- (Proposed, r-2) `data` is the single serialization of
  `sc_jev_check::run::RunResponse` (ADR-SAG-0024), printed unchanged by every
  consumer. The **primary backend** is the first backend in `BackendSet`
  order whose status is `Ok`; `primary_backend` names it. If it is Jev, its
  response fields other than `answers` are copied to the top level verbatim
  and `answers` is written once, from the typed answers (`choice`,
  `confidence`, `probabilities`, byte-identical to the Jev answer objects for
  those keys), so no key appears twice (the sentence above still holds for
  that case); if it is an LLM backend (Jev failed or
  was not selected), its normalized `answers`, `model` and `usage` are at the
  top level and no Jev answer is. Then `session_id` (null unless
  `session_saved`), `session_saved`, `prompt_sha256`, `payload_bytes`,
  `results` (per backend: `ok` + outcome, or `failed` + the
  `ScAgentErrorResponse` derived from its error), `equal_input`, dry-run
  `payload`/`limits`, and last `warnings` (array of strings, omitted when
  empty). `warnings` replaces the Phase A `data.warning` key; the
  composition crate adds the log-flush warning through
  `RunResponse::with_warning` (r-3b). The `{version, operation, data}`
  wrapper is `sc_jev_check::run::Envelope`; `readback` is the only key a
  consumer adds (`sc-ai`). REQ-SAG-0710 is amended to match.

## ADR-SAG-0012 — sc-jev owns Jev API

**Status:** Active

Every TypeSafe Jev HTTP operation is implemented in `sc-jev`. Other crates
must not send Jev API requests. Phase A ships the check/judge slice; remaining
endpoints extend `sc-jev` in place.

## ADR-SAG-0029 — Phase A hardening amendments

**Status:** Active

Amends ADR-SAG-0020 and ADR-SAG-0022. Recorded by the Phase A phase-end
hardening sprint (a-10, PR 20). The earlier ADR text stays in place; the
sentences this decision replaces carry a *Superseded by ADR-SAG-0029*
pointer.

**(a) Secrets (amends ADR-SAG-0020).**

- `sc-agent-secrets` has no Cargo features. The in-memory doubles `MapEnv`
  and `MapKeyring` are `#[cfg(test)]` code inside the crate
  (`crates/sc-agent-secrets/src/resolve.rs`) and are not exported. Other
  crates inject their own `EnvSource` / `KeyringSource` implementations
  through `resolve_key_with`.
- `ResolvedApiKey::try_new(BackendKeyId, String)` is the single construction
  rule set: it trims the value, maps an empty value to `KEY_MISSING`, and
  maps interior whitespace or control characters to
  `JevError::credential_store_invalid` (`CONFIG_INVALID`). Every resolution
  path in `sc-agent-secrets` goes through it. The unchecked
  `ResolvedApiKey::new` exists only under `cfg(test)` or the
  `sc-agent-types/test-util` feature, for test doubles.
- `clippy.toml` `disallowed-methods` lists `ResolvedApiKey::as_str`. The two
  sending transports (`crates/sc-jev/src/client/https.rs` and
  `crates/sc-jev-backends/src/transport.rs`) are the only production readers
  and carry `#[allow(clippy::disallowed_methods, reason = ...)]`.

**(b) Scripted backends in `sc-jev-py` (amends ADR-SAG-0022).**

- `sc_jev_py.run_check` has one signature in every build. It never takes
  `scripted_answers`.
- Scripted injection is `sc_jev_py._testing.run_check_scripted(check_path, *,
  scripted_answers, ...)`. The `_testing` submodule is registered only when
  the `sc-jev-py` feature `test-util` is on. The publish wheel leaves it off,
  so the wheel has no `_testing` submodule.
- `sc-jev-check` feature `test-util` makes `registry::ScriptedJudge`,
  `registry::RequestLog`, and `registry::register_scripted_backends` public.
  The string shorthand (`"yes"` for `{"choice": "yes"}`) is normalised inside
  `register_scripted_backends`, so every caller gets the same rule.
- Everything else in ADR-SAG-0022 stands: no production registration, no
  socket, no API key, default features off.

**(c) Error-code dispositions (upholds ADR-SAG-0001, REQ-SAG-0101,
REQ-SAG-0800).** Phase A hardening moved some failures to a different
existing id. Every id below is in
`tests/fixtures/jev/README-contract/errors.json` and is a `JevError`
variant; no id was added or removed, and `JevError::code()` still matches
the snapshot (test `codes_match_readme_contract_snapshot`).

| Surface | Failure | Before | After |
| --- | --- | --- | --- |
| `sc_jev_py.run_check` / `_testing.run_check_scripted` | unknown backend name | `CONFIG_INVALID` | `INPUT_INVALID` (`BackendId::from_str`, same as `sc-ai` and `check.toml`) |
| same | kebab-case alias (`anthropic-haiku`, `openai-luna`) | accepted | `INPUT_INVALID` (one documented spelling) |
| any crate | `serde_json` encode / decode failure (`JevError::encode_failed`) | mixed (`INPUT_INVALID`, `EXTRACTION_FAILED`) or raw text | `INPUT_INVALID`, detail keeps only category and line/column |
| any crate | file or process I/O failure (`JevError::io_failed`) | mixed (`INPUT_MISSING` for any read error, `EXTRACTION_FAILED` for git spawn) | `INPUT_INVALID` with the `io::ErrorKind`; a missing file stays `INPUT_MISSING` |
| `sc-treesitter` CLI | missing positional source path | `INPUT_MISSING` | `INPUT_INVALID` (an argument error, not a missing file) |
| `sc-treesitter` library | span or line number does not fit `u32` | `INPUT_TOO_LARGE` | `INPUT_INVALID` naming the field and value |
| `sc-treesitter` CLI | revision starting with `-`, path outside the repo or with a forbidden component, unresolvable revision, non-blob object | mixed | `INPUT_INVALID` (absent path at a revision stays `INPUT_MISSING`) |
| `sc-treesitter` CLI | blob or worktree file over 8 MiB | read unbounded | `INPUT_TOO_LARGE` |

Python exceptions carry the same `code` as the Rust `JevError`
(REQ-SAG-0800); the change is which existing id a failure maps to.

**Deferred.** ADR-SAG-0016 still names `"1.5.0"` while the workspace pins
`"=1.5.0"` (arch-qa ARCH-004). The version-form text is left to the D8
amendment (`PHASE-A-REVIEW.md` section 4).

Phase A.1 amendments that replace a 0029 sentence name it: ADR-SAG-0020 r-1 and
r-3, ADR-SAG-0022 r-1 and r-2 (plan.md "Plan PR route").

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
  `try_new` is the only production constructor; `clippy.toml` enforces the
  `as_str` rule (ADR-SAG-0029).
- **Extractor sealing:** `Extractor` is `pub trait Extractor: private::Sealed` —
  only the four Phase A kinds implement it; external crates cannot add kinds
  without a new ADR (RBP-F004).
- **Backend dispatch:** `sc-jev-check` holds `Vec<Arc<dyn JudgeBackend>>` (or
  equivalent registry) for multi-backend runs (RBP-F005).

### Amendment (Phase A.1, r-2)

**Status:** Proposed (Phase A.1) — lands in r-2.

- (Proposed, r-2) The "Extractor sealing" bullet (RBP-F004) is withdrawn: the
  `Extractor` trait is removed and `extract_input` matches the four
  `config::InputSpec` kinds directly. A new kind is an `InputSpec` variant
  plus an ADR.

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


## ADR-SAG-0023 — Dependency pins, allocator, released binaries

**Status:** Proposed (Phase A.1) — lands in r-4. Its r-3b subsection lands
in r-3b.

- (Proposed, r-4) Library crates declare caret ranges. Exact `=` pins are
  allowed only for deliberate lockstep. The list below, between the markers,
  is the **only** copy: `.github/scripts/check-exact-pins.sh` reads it and
  fails on any `=` requirement in any `Cargo.toml` whose crate is not on it.
  NFR-SAG-0004. Reasons: `sc-observability` — v2 shim alignment;
  `tree-sitter`, `tree-sitter-python`, `tree-sitter-rust` — output
  determinism; `sc-agent-compose` — lockstep internal support (owner
  decision 2026-11-06), in force from r-3b (subsection below; before the
  crate exists no `Cargo.toml` names it). `sc-observability-types` is not on
  the list: r-1 removes its only user and its root entry.

  <!-- lockstep-pins:start -->
  sc-observability
  tree-sitter
  tree-sitter-python
  tree-sitter-rust
  sc-agent-compose
  <!-- lockstep-pins:end -->

- (Proposed, r-4) Global allocators and argv parsers are linked only by
  binaries: `mimalloc` and `clap` are optional behind each binary crate's
  default `cli` feature, and the `[[bin]]` targets use
  `required-features = ["cli"]`. Libraries build with
  `--no-default-features` without mimalloc (REQ-SAG-0134) or clap
  (REQ-SAG-0135).
- (Proposed, r-4) Released binaries are `sc-ai` and `sc-treesitter`.
  Per-channel coverage is ADR-SAG-0017's r-4 amendment.

### Amendment (Phase A.1, r-3b)

**Status:** Proposed (Phase A.1) — lands in r-3b.

- (Proposed, r-3b) The intra-workspace `sc-agent-compose` dependency of
  `sc-ai` and `sc-jev-py` is `=`-pinned (it is already on the list above);
  `check-exact-pins.sh` re-runs green with the pin present.

## ADR-SAG-0024 — Engine entry point and response contract

**Status:** Proposed (Phase A.1) — lands in r-2. Amends ADR-SAG-0003,
ADR-SAG-0005, ADR-SAG-0009, ADR-SAG-0014, ADR-SAG-0019, ADR-SAG-0022.

- (Proposed, r-2) `sc-jev-check`'s only run API is
  `prepare(CheckSource, RunConfig) -> PreparedRun` then
  `run(PreparedRun) -> RunResponse`, plus `discover_repo_root(&Path)`.
  `PreparedRun` owns its config. `build_request`, `run_check`, `run_request`
  and `prepare_caller_request` are deleted (no shims; nothing published).
- (Proposed, r-2) Backend selection: `RunConfig::backends` if set, else the
  source document's `backends`, else `[jev]`. `prepare` and dry-run read only
  static `BackendDescriptor` facts and construct no backend.
- (Proposed, r-2) `RunResponse` (private fields) is the one result type
  every consumer (CLI, PyO3, future MCP) prints, and its single `Serialize`
  is the ADR-SAG-0014 `data` body (ADR-SAG-0014 r-2 amendment): the primary
  backend (first `Ok` in `BackendSet` order, possibly an LLM backend)
  contributes its answer fields at the top level, named by
  `primary_backend` (for Jev: its response fields except `answers`, then
  `answers` once from the typed answers), followed by `session_id` (null unless `session_saved`),
  `session_saved`, `prompt_sha256`, `payload_bytes`, `results` (per backend
  `ok` + outcome or `failed` + `ScAgentErrorResponse`), `equal_input` and
  `warnings` (omitted when empty; `RunResponse::with_warning` is how a
  composition root adds one). A `BackendFailure` stores only the `JevError`;
  its serialized error object is derived from it at serialize time. The
  `{version, operation, data}` wrapper is `sc_jev_check::run::Envelope`.
  `RunResponse::first_failure()` / `is_partial()` decide exit codes and
  exceptions for every consumer. REQ-SAG-0620, REQ-SAG-0710 (amended).
- (Proposed, r-2) Dry-run reports `payload_bytes` and, per selected backend,
  `request_limit_bytes` and `over_limit`; it does not fail on an over-limit
  payload. Each backend owns its byte cap and enforces it in
  `validate_for_send`; the engine does not pre-cap. REQ-SAG-0314.
- (Proposed, r-2) `FileSessionStore` is stateless (`dir`, `ttl`, `clock`
  fn pointer). `ask` takes the `&BackendRegistry` and does not rewrite the
  file. No lock: `save` writes a `create_new` tmp named with pid + random,
  fsyncs, renames, fsyncs the parent, removes the tmp on error; last writer
  wins; readers are protected by digest and id re-verification; tmp files
  older than 1 h are swept by `FileSessionStore::new`. The session is written once at the end of `run`, only
  when a backend with `supports_session_ask` returned `Ok`. REQ-SAG-0607.
- (Proposed, r-2) Repo-root discovery is the git toplevel for both consumers
  (ADR-SAG-0003 r-2 amendment).

## ADR-SAG-0025 — Multi-backend semantics and equal-input attestation

**Status:** Proposed (Phase A.1) — lands in r-2b. Amends ADR-SAG-0004,
ADR-SAG-0008, ADR-SAG-0019, ADR-SAG-0022.

- (Proposed, r-2b) The default mode is `Independent`; `ChainContext` is
  explicit and recorded on `EqualInputRecord.mode`. REQ-SAG-0605.
- (Proposed, r-2b) The base request is validated on every backend before any
  send; each `ChainContext`-augmented request is re-validated before its own
  send, and a refusal marks that backend `Failed`. Each backend yields
  `BackendStatus::Ok | Failed(BackendFailure)` (the `JevError`; its
  `ScAgentErrorResponse` is derived when serialized); a failure
  does not discard other answers; `leak = any Failed`; if all fail, `run`
  returns `Err`. `judge` is called once per backend; only `BusyPolicy`
  retries; a session-save failure keeps the answers with
  `session_saved = false` and one `warnings` entry. REQ-SAG-0604 (consumer output in r-3b: stdout
  envelope, stderr first failure, exit 2).
- (Proposed, r-2b) `equal` is derived from per-backend `Attestation`
  (`Matched`, `Mismatched`, `Unattested`, `Failed`, `NotSent`). Each backend
  reports `input_sha256`: the digest of the canonical blob recovered from its
  final serialized wire body. The engine trusts the reported digest. This
  re-encoding is attestation-only and never feeds the wire body, so it does
  not breach ADR-SAG-0005's "backends must not re-canonicalize". REQ-SAG-0606.

## ADR-SAG-0026 — Shared HTTP crate and composition crate

**Status:** Proposed (Phase A.1) — HTTP half lands in r-3, compose half in
r-3b. r-3 sets this line to "Active (HTTP half, r-3); compose half Proposed
(r-3b)"; r-3b sets it to "Active". HTTP half amends ADR-SAG-0006, 0007, 0017
and 0020; compose half amends ADR-SAG-0010, 0011, 0013, 0016 and 0017.

### HTTP half

**Status:** Proposed (Phase A.1) — lands in r-3.

- (Proposed, r-3) `sc-agent-http` (published) owns `HttpRequest`
  (`body: Arc<[u8]>`) / `HttpResponse`, `Endpoint`, `SecretHeader`,
  `Transport`, `UreqTransport` (one `Agent` built in `new`, 8 MiB streaming
  response cap; private module `ureq_transport`), `BusyPolicy` /
  `send_with_busy_retry` (with `CancelToken`) and redaction of every error it
  returns, and is the single secret-read site (ADR-SAG-0020 r-3 amendment).
  It depends only on `sc-agent-types` (`default-features = false`,
  `features = ["secret"]`) and `ureq`; no `ureq` type is public. Its
  non-default `test-util` feature (test hooks for clock, sleeper and jitter;
  loopback-only `http` endpoint; test transports) is enabled only by
  dev-dependencies. Phase B provider agents reuse it. REQ-SAG-0402.
- (Proposed, r-3) ADR-SAG-0012 still holds: `sc-jev` owns every TypeSafe
  Jev API operation (endpoints, wire body, response parsing);
  `sc-agent-http` only carries bytes.

### Compose half

**Status:** Proposed (Phase A.1) — lands in r-3b.

- (Proposed, r-3b) `sc-agent-compose` owns the `sc-observability` adapter,
  `PathPolicy` (one data type; `cli()` and `python()` presets encode
  ADR-SAG-0021 per root, unchanged), `ComposeInputs`, `run_from` and
  `ask_from`. **Decided (owner, 2026-11-06):** it is published to crates.io
  as an internal-support crate, `=`-pinned from `sc-ai` and `sc-jev-py`, with
  no semver promise of its own, because `sc-ai` is published and cannot
  depend on an unpublished crate. Publish order and the published-crate set
  are ADR-SAG-0017's r-3b amendment. `ComposeInputs` has private fields,
  `new(cwd, policy)` and one setter per optional field. `run_from` and
  `ask_from` signatures name types owned by `sc-jev-check` and
  `sc-agent-types` (`CheckSource`, `RunResponse`, `JudgeOutcome`,
  `JevError`); compose does not re-export them, so a consumer (a future MCP
  server included) depends on those crates directly at the versions the
  compose release pins, and the `cargo tree -d` gate keeps one version of
  each in the `sc-ai` and `sc-jev-py` graphs.
- (Proposed, r-3b) Environment ownership: API-key environment and `.env`
  reading belongs to `sc-jev-check` (`BackendRegistry::from_env` via
  `sc-agent-secrets`); log and session path environment variables belong to
  `PathPolicy` in `sc-agent-compose`. No other crate reads either set.

## ADR-SAG-0027 — Error taxonomy and extensibility

**Status:** Proposed (Phase A.1) — lands in r-1. Amends ADR-SAG-0001.

- (Proposed, r-1) `JevError`, `BackendId` and `BackendKeyId` are
  `#[non_exhaustive]`, and struct-like `JevError` variants are
  `#[non_exhaustive]` so downstream crates use constructors; every
  struct-like variant has a public constructor. Recovery
  categories are explicit variants (`CredentialStoreInvalid`,
  `RegistryInvalid`, `SessionStoreIo`, `Operational { area }`,
  `SessionDocumentInvalid`, `EncodeFailed`, `IoFailed`). Producer variants are
  neutral (`LanguageUnavailable { producer, language }`); producer stderr is
  re-raised as `ProducerError { producer: String, code: ErrorCode, message:
  String }`, whose `code()` is the carried id; every `producer` field is a
  `String`. No string-prefix parsing. REQ-SAG-0106.
- (Proposed, r-1) `code()` returns `ErrorCode`, a `#[non_exhaustive]` enum
  with `as_str()`; its ids are unchanged. `ErrorCode::ALL` is a hand-written
  `const` table (no `build.rs`) that a test compares with
  `errors.json`. Every variant maps to exactly one existing id; an id may
  have several variants (REQ-SAG-0101 amended). The variant → id table is in
  sprint r-1 and pinned by a golden test. A new stable code requires
  changing the README `errors.json` contract shared with Python. Revisit
  when Phase B needs an `INTERNAL` class.
- (Proposed, r-1) `retryable()` is true only for `NetworkError` kinds Busy,
  Timeout, Connect.
- (Proposed, r-1) `ScAgentErrorResponse::from_jev_error` (name unchanged)
  fills fields by one rule, in the fixed key order `error`, `message`,
  `schema_version`, `retryable`, `details`: `schema_version` is always `1`
  (versions this error shape only, independent of the stdout envelope
  `version`; bumped only by a breaking change of this shape); `retryable` is
  present, as `true`, only when `retryable()` is true; `details` is present
  only for `ProducerError`, as `{"producer": <name>}`, a closed object.
  Deserialization (for producer stderr) accepts `schema_version` absent or
  `1`, denies unknown fields at the top level and inside `details`, and
  treats anything else as not an error response; so adding any key, top
  level or in `details`, is a breaking change and bumps `schema_version`.
  Every stderr line gains `"schema_version":1`; NFR-SAG-0001 and
  REQ-SAG-0701 are amended and a golden stderr test pins the bytes.
- (Proposed, r-1) **Consumer rule.** Agents and CLI/MCP clients may read
  `schema_version` from every stderr line. A value other than `1` means
  "unknown error shape": fall back to the exit code and the raw line. A bump
  is a breaking change announced in the changelog.
- (Proposed, r-1) `Error::source()` is `None` for every redacted-cause
  variant (`CredentialStoreInvalid`, `NetworkError`, `EncodeFailed`,
  `IoFailed`, `SessionStoreIo`); no A.1 variant carries a non-redacted source.
- (Proposed, r-1) Serde spelling of backend ids matches `Display`
  (`openai_luna`, `openai`), with a one-release read alias for
  `open_ai_luna` / `open_ai` in config and caller-request input.
- (Proposed, r-1) Module seams are cargo features of `sc-agent-types`:
  `judge`, `secret`, `obs` (default-on), `git` and `test-util`
  (non-default); `error`, `envelope`, `ids`, `schema`, `cancel` are always
  on. `secret` and `judge` are independent; `git` needs only the core.
  `sc-jev-check` and `sc-treesitter`'s `cli` feature are the only enablers
  of `git`. `cargo hack --feature-powerset` checks every combination. `sc-treesitter` uses
  `default-features = false`. Module paths are the stable surface for a later
  re-export split.
- (Proposed, r-1) **Adding a built-in backend** (checklist): (1) a minor
  `sc-agent-types` release with the `BackendId` variant, its `BACKENDS` row
  (facts, aliases, limit) and a `BackendKeyId` variant if needed; (2) the
  backend type with `with_default_transport`; (3) one row in
  `sc-jev-check::registry`'s factory table; (4) the catalog and
  factory-coverage tests pass unchanged. Dispatch on ids is a
  test-and-review convention (ADR-SAG-0009 r-1 amendment);
  `BackendRegistry::register` is for doubles and scripted backends of
  existing ids only. `BackendId` deserializes through `FromStr`, so the
  descriptor's `aliases()` is the one alias source.
- (Proposed, r-1) **Test seams** are non-default, additive `test-util` cargo
  features of the crate that owns the hook, enabled only by
  dev-dependencies (a crate's own integration targets use a
  self-referencing dev-dependency with no `version` key), never by a normal
  dependency edge; `workspace-tests --test test_util_off` and a `cargo tree`
  gate prove it. The one exception is the `sc-jev-py` pytest wheel, built
  with `maturin --features extension-module,test-util` (ADR-SAG-0029 (b));
  the publish wheel has no `_testing` submodule. No acceptance test in
  `tests/` relies on `cfg(test)`.

## ADR-SAG-0028 — workspace-tests crate

**Status:** Proposed (Phase A.1) — lands in r-3b.

- (Proposed, r-3b) **Purpose.** `crates/workspace-tests` (`publish = false`)
  holds proofs that span crates and must not live inside a published crate:
  the Rust/Python cross-implementation digest (`jev_cross_impl`), the real
  `sc-treesitter` → `check.toml` producer run, CLI/Python output parity,
  git-read parity, source scans (allow attributes, `disallowed_methods`
  expect sites for the secret read and process spawns, lint-mechanism
  presence, lints inheritance, environment-read owners) and the
  `test-util`-off proof (graph query and a release-built `sc-ai`). Its
  `src/lib.rs` is empty; everything is under `tests/`; its documentation is
  one `README.md` paragraph per target (no `docs/workspace-tests/`).
  REQ-SAG-0109.
- (Proposed, r-3b) **Dev-only edges.** It has no normal dependencies. Its
  dev-dependencies are exactly `sc-agent-types` (`features = ["git"]`),
  `sc-jev-check`, `sc-agent-compose`, `serde_json`, `tempfile`. Whether
  `sc-boundary` checks dev edges is settled in wave 1 by r-1's spike and
  recorded in `boundaries/README.md`; if it does not, `cargo-tree-gates.sh`
  polices the exact dev set.
- (Proposed, r-3b) **Built-binary contract.** `sc-ai` and `sc-treesitter`
  are never linked: a test builds them with
  `cargo build --release -p <pkg> --features cli --target-dir <isolated> --message-format=json`
  (via `env!("CARGO")`) and runs the executable named in the
  `compiler-artifact` message (helper `tests/support/bin_path.rs`). The
  isolated target directory never holds the `cargo test` build of `sc-ai`,
  which has `test-util` on through its self dev-dependency. Stable Cargo only: no
  `CARGO_BIN_EXE_*` across packages, no artifact dependencies.
- (Proposed, r-3b) **Forbidden edges.** `pyo3`, `tree-sitter`,
  `sc-treesitter`, `sc-jev-py`, `sc-ai`, `sc-agent-http`, `sc-jev`,
  `sc-jev-backends` (manifest `forbidden_edges`; `cargo tree -p
  workspace-tests -e normal,dev` gate for `pyo3`/`tree-sitter`). A proof
  that needs one of them goes through a built binary or into that crate's own
  tests.
