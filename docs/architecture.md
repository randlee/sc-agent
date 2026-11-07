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

**Jev API:** All TypeSafe Jev HTTP lives in **`sc-jev`** (REQ-SAG-0110).  
**Trait boundary:** `JudgeBackend` and shared domain types live in
`sc-agent-types`. LLM comparison backends implement the trait in
`sc-jev-backends`. Local judge (decider-4b / cyprid) adds a backend later
without changing callers.

## ADR index

ADR sections below keep their original order. This table lists them in id
order.

| ADR | Title | Status | Amends / amended by |
| --- | --- | --- | --- |
| ADR-SAG-0001 | Error enum is CLI authority | Active | — |
| ADR-SAG-0002 | Canonical judge payload bytes | Active | — |
| ADR-SAG-0003 | Secret resolution | Active | Amended by ADR-SAG-0020 |
| ADR-SAG-0004 | One check, one request | Active | — |
| ADR-SAG-0005 | Facade owns payload assembly | Active | — |
| ADR-SAG-0006 | Transport injection | Active | — |
| ADR-SAG-0007 | API-direct LLM backends | Active | — |
| ADR-SAG-0008 | Sessions carry prior answers to later backends | Active | — |
| ADR-SAG-0009 | Local backend registers without caller changes | Active | Amended by ADR-SAG-0022 |
| ADR-SAG-0010 | CLI thin adapter | Active | — |
| ADR-SAG-0011 | Python via sc-jev-py wheel | Active | — |
| ADR-SAG-0012 | sc-jev owns Jev API | Active | — |
| ADR-SAG-0013 | Session JSONL via sc-observability v2 | Active | Amended by ADR-SAG-0021 |
| ADR-SAG-0014 | CLI success body | Active | — |
| ADR-SAG-0015 | Subjects artifact outside the judge | Active | — |
| ADR-SAG-0016 | v2 shim in sc-agent-types (no extra crate) | Active | — |
| ADR-SAG-0017 | First release surfaces | Active | — |
| ADR-SAG-0018 | sc-publish kit owns release surface | Active | — |
| ADR-SAG-0019 | LLM wire envelope vs canonical payload | Active | — |
| ADR-SAG-0020 | Secret injection and redaction | Active | Amends ADR-SAG-0003; amended by ADR-SAG-0029 |
| ADR-SAG-0021 | Log and session path precedence | Active | Amends ADR-SAG-0013 |
| ADR-SAG-0022 | Scripted backends are a test-util feature | Active | Amends ADR-SAG-0009; amended by ADR-SAG-0029 |
| ADR-SAG-0029 | Phase A hardening amendments | Active | Amends ADR-SAG-0020 and ADR-SAG-0022 |

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

## ADR-SAG-0008 — Sessions carry prior answers to later backends

**Status:** Active

When multiple backends run in one invocation, **session state** is how a second
opinion receives prior answers. TypeSafe Jev has no rationale field on the wire;
Haiku/Luna session `ask` paths reuse stored context. Session files are UUID paths
with mode `0600` (sprint a-6).

## ADR-SAG-0009 — Local backend registers without caller changes

**Status:** Active

Future local judges (decider-4b / cyprid) implement `JudgeBackend` and register
in `sc-jev-check`’s backend registry. Callers keep `run_check(..., backends: &[BackendId])`;
no CLI or PyO3 API change when a local backend lands.

Amended by ADR-SAG-0022.

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

## ADR-SAG-0003 — Secret resolution

**Status:** Active

`sc-agent-secrets` resolves keys: **process env → repo-root `.env` → keyring**.
**Only `sc-jev-check`** calls `resolve_key`. `JevClient` and LLM backends receive
the resolved secret (and transport) at **construction**; they do **not** depend on
`sc-agent-secrets`. `JudgeRequest` carries neither keys nor `repo_root`.

Amended by ADR-SAG-0020.

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

## ADR-SAG-0004 — One check, one request

**Status:** Active

No splitting oversized inputs. Pre-send byte/token validation on every backend
via `validate_for_send` before `judge`.

## ADR-SAG-0006 — Transport injection

**Status:** Active

Jev HTTP client lives in `sc-jev`; LLM HTTP in `sc-jev-backends`. Transports are
**injected at backend construction** (not passed into `JudgeBackend::judge`).
Both use test doubles for unit tests.

## ADR-SAG-0007 — API-direct LLM backends

**Status:** Active

Haiku and Luna judges use **provider HTTP only** in Phase A: no vendor SDK,
agent harness, or tool-use loop. Wire format and token counting are implemented
in `sc-jev-backends` against documented REST endpoints. **HTTP timeouts:** connect
10s, request 120s (configurable on backend builder for tests); busy retry per
REQ-SAG-0505.

## ADR-SAG-0010 — CLI thin adapter

**Status:** Active

`sc-ai` parses argv and prints envelopes; no network code in the binary crate.
Depends on **`sc-jev-check`** + full `sc-observability`, not on `sc-jev` directly.

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
