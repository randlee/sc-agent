# Phase A — sc-jev and sc-ai CLI foundation

Status: **planned** — awaiting Rand approval before implementation.

Sprint doc shape follows
[`docs/development/sprint-planning-guidelines.md`](../../development/sprint-planning-guidelines.md).

Phase id: `a`. Plan branch: `plan/phase-a`. Phase integration branch:
`integrate/phase-a`. Beads epic: `agt-phase-a` (create at kickoff).

## Intent

See [`intent.md`](intent.md). Phase A delivers **`sc-jev`** (the sole TypeSafe
Jev library — full API over time; Phase A ships the **check/judge** slice with
Haiku/Luna comparison, sessions, equal-input record) and the **`sc-ai`** CLI.
Shared **`sc-agent-types`** and **`sc-agent-secrets`** serve this stack and
future agent launchers.

## Non-negotiables (Rand)

- **One client** for all Jev use — **`sc-jev`** only; Python `packages/jev-check`
  README is the Phase A behavioural contract for the check slice.
- **One check = one request**; over byte cap → `INPUT_TOO_LARGE`, never split;
  context-length check before every backend send.
- **Byte-identical payload** across backends (sorted compact JSON, sha256);
  cross-implementation fixture vs Python (tighten #1).
- **Haiku/Luna API-direct** (no SDK/harness/tools); equal-input record every run.
- **Sessions + `ask`** are required, not optional.
- **Typed errors**: one enum variant each with fix text; CLI stderr
  `{"error","message"}`, exit 2.
- **Crate split**: types/traits; Jev backend; LLM backend(s); library facade;
  CLI; PyO3 thin wrapper over the library.
- **Parallel questions** in the API design; Phase A may ship single-request paths
  first if batching lands in the same phase via library API without blocking
  parity.
- **sc-ai-cli** conventions from first commit: `just` targets, sc-lint boundaries.

## Crate layout

| Crate | Role | Public surface (summary) |
| --- | --- | --- |
| `sc-agent-types` | Shared types + **temporary** v2 log port | `JevError`, `JudgeBackend`, `observability::v2::SessionLog` (shim; remove after upstream v2) |
| `sc-agent-secrets` | API key resolution shared by all backends | `resolve_key(BackendKeyId) -> Result<String, JevError>` env → `.env` → keyring |
| `sc-jev` | **All TypeSafe Jev HTTP** + check orchestration | Jev API client (100% over time); `check.toml`, extractors, `run_check`, sessions; no CLI parsing |
| `sc-jev-backends` | LLM comparison judges only | `AnthropicHaikuBackend`, `OpenAiLunaBackend`; injectable HTTP/transports |
| `sc-ai` | Binary crate | `sc-ai jev …` subcommands (Phase A: check parity); JSON-first per sc-ai-cli |
| `sc-jev-py` | PyO3 + **PyPI wheel** | Published extension; Python apps call `sc-jev` in-process |

**Trait boundary** (in `sc-agent-types`):

```rust
pub trait JudgeBackend: Send + Sync {
    fn id(&self) -> BackendId;
    /// Validate size and shape; return canonical payload bytes + digest.
    fn validate_request(&self, questions: &QuestionMap, state: &JsonValue) -> Result<PayloadBundle, JevError>;
    /// One check = one HTTP exchange (LLM: count + message internally, still one logical check).
    fn judge(&self, bundle: &PayloadBundle, transport: &dyn BackendTransport) -> Result<JudgeOutcome, JevError>;
}
```

Local backend (`decider-4b` / cyprid) implements `JudgeBackend` in a **later**
crate; Phase A keeps the trait stable and registration point in `sc-jev`.

**Dependency direction:** `sc-ai` → (`sc-jev`, `sc-observability` 1.5.0 v2) →
`sc-jev-backends` → (`sc-agent-types`, `sc-agent-secrets`). Libraries never depend on CLI, PyO3, or full
`sc-observability`.

## Boundary map (Phase A)

| Boundary | Crates | Contract |
| --- | --- | --- |
| `agent-types` | `sc-agent-types` | Errors, domain types, `JudgeBackend`, temporary `observability::v2` |
| `agent-secrets` | `sc-agent-secrets` | Key resolution policy |
| `jev-backends` | `sc-jev-backends` | HTTP transports + backend impls |
| `jev` | `sc-jev`, `sc-jev-py` | Full Jev API home; Phase A check slice + orchestration |
| `sc-ai-cli` | `sc-ai` | argv → operations → JSON envelope |

Manifests under `boundaries/<id>/` land in sprint **a-1** (empty stubs allowed
only until a-1 commits real rules).

## Wave table

| Track | Wave | Sprint | `closure_type` | `target_boundary` | Notes |
| --- | --- | --- | --- | --- | --- |
| foundation | 1 | a-1 | contract | agent-types | workspace, sc-lint, errors, cross-impl fixture |
| secrets | 1 | a-2 | boundary | agent-secrets | parallel with a-3 after a-1 |
| build | 1 | a-3 | boundary | jev | config, extractors, payload; parallel with a-2 |
| jev | 2 | a-4 | boundary | jev | TypeSafe Jev HTTP (check endpoint); `must_follow` a-3 |
| llm | 2 | a-5 | boundary | jev-backends | Haiku + Luna; `must_follow` a-3 |
| runtime | 3 | a-6 | boundary | jev | sessions, run_request, multi-backend; `must_follow` a-4,a-5 |
| cli | 3 | a-7 | integration | sc-ai-cli | CLI features 1–3, parity CLI paths; `must_follow` a-6 |
| python | 3 | a-8 | integration | jev | PyO3 wrapper; `must_follow` a-6 |

**Critical path:** a-1 → a-3 → a-4 → a-6 → a-7 (5 sprints).  
**Width:** 2 (a-2 ∥ a-3; a-4 ∥ a-5). **Sprint count:** 8.

## Parity and features mapping

| Sprint | Parity rows closed (README checklist) | Requested features |
| --- | --- | --- |
| a-1 | (fixture only; enables payload tests) | — |
| a-3 | extractors (`file`, `table-column`, `delimited-regions`), config, payload | — (not python-branches AST) |
| a-4 | Jev transport rows | — |
| a-5 | Haiku/Luna rows | **8** (Luna live count) |
| a-6 | sessions, equal-input, keys via library | **3** (multi-backend) |
| a-7 | CLI session/ask/run | **1**, **2** |
| a-8 | library API parity for Python callers | PyO3 |

## Areas to tighten (Phase A vs defer)

| # | Item | Phase A |
| --- | --- | --- |
| 1 | Cross-impl byte fixture | **a-1** |
| 2 | Branch / region text | **a-3** — **delimited regions** in source (see resolved #3); no in-crate Python AST |
| 3 | Jev probabilities strict | **a-4** |
| 4 | LLM stop_reason / status | defer |
| 5 | LLM busy retry | defer |
| 6 | Session UUID + mode 0600 | **a-6** |
| 7 | Expiry uses `created_at` | **a-6** |
| 8 | Repo-root for git paths | **a-3** |
| 9 | Markdown escaped pipes | defer |
| 10 | Error enum + fix text | **a-1** |
| 11 | `consistent` flag on answers | defer |
| 12 | Vendor pins in config | **a-3** / **a-4** |

## Phase A CLI and session logging (Rand)

Phase A optimizes for a **blocking CLI** that prints **JSON on stdout** and exits
(non-zero → typed error JSON on stderr). Every Jev judge/check call also
**appends JSONL** session records (Claude/Codex-style retained logs): prompt
payload metadata, backend id, timestamps, equal-input digest — one line per
logical prompt/send. Use **`sc-observability` v2** (`LoggerBuilder`, not v1 `Logger::new`) via
**`sc-agent-types::observability::v2`** injection (ADR-SAG-0016). `sc-jev` logs through
`SharedSessionLog`; `sc-ai` owns the real logger and log root. Shim removed when
upstream v2 is released.

**Stdout success shape:** the **same JSON object TypeSafe Jev returns** for the
check/judge call, plus a required **`session_id`** (UUID for this run’s session
log correlation). Haiku/Luna paths still emit that **Jev-equivalent answer JSON**
on stdout (mocked/parsed as today in Python parity); `session_id` is always
present. sc-ai-cli version envelope wraps this if required by guidelines
(`data` holds Jev body + `session_id`).

## Resolved decisions (Rand)

1. **Key precedence:** process environment → **repo-root `.env`** → keyring
   (REQ-SAG-0202). Later waves may add a user-level `.env` path; Phase A is
   repo-root only.
2. **sc-runtime daemon / UDS** — out of Phase A.
3. **Branch / region extraction:** **Embedded delimiters** (`delimited-regions`;
   ADR-SAG-0015). **Many regions per file** (distinct ids); optional
   `regions = [...]` in check.toml **includes only listed ids** (others
   excluded). No in-crate Python AST — consumers add markers or pre-extract.
   README `test_python_branches_*` is **not** a Phase A gate.

## Open questions (for Rand)

None blocking plan approval. **Kickoff decisions** (can default in sprint if silent):

1. **Session JSONL root** — default directory and env var (e.g. `SC_AGENT_LOG_DIR` vs reuse an `sc-observability` convention)?
2. ~~**`sc-observability` pin**~~ — **`=1.5.0`**, v2 `LoggerBuilder` in `sc-ai` only (resolved).
3. **Cross-impl bytes test** — still gate Phase A on Python `packages/jev-check` fixture repo in-tree, or Rust-only fixture until Python wraps `sc-jev-py`?
4. **Luna live token-count (feature 8)** — manual PR evidence only (current sprint a-5), or required in CI with a secret?
5. **sc-ai-cli envelope** — confirm top-level `{ "version", "operation", "data": { …jev…, "session_id" } }` against synaptic-canvas `packages/sc-ai-cli` before a-7?

Follow-ups as beads: user-level `.env`, extra comment sigils in ADR-SAG-0015, parallel question fan-out.

## Roadmap after Phase A (product intent)

| Wave | In | Out |
| --- | --- | --- |
| Phase B (planned, not detailed here) | `sc-ai` subcommands + crates for **Anthropic** and **OpenAI/Codex API** agents (API-direct, no SDK harness); comparison workflows alongside Jev | Subprocess wrappers for **claude-code**, **codex-cli**, **cursor-cli** |
| Later | Cursor API agent if/when there is a stable API surface comparable to the above | Any “JSON launcher” that only forwards argv to a installed vendor CLI |

Phase A lays shared **`sc-agent-types`**, **`sc-agent-secrets`**, and
**`JudgeBackend`** so Phase B agents reuse the same boundaries.

## Out of scope (Phase A)

- Standalone Claude/Codex API agent crates and `sc-ai` subcommands beyond
  **`sc-ai jev`** check parity (Haiku/Luna **are** in scope via sprint a-5).
- **Non-check Jev API endpoints** beyond what Phase A needs for parity (tracked
  under REQ-SAG-0110 for later sprints).
- Wrapping **claude-code**, **codex-cli**, or **cursor-cli**.
- Replacing or wrapping plan-anchor / orchestration Python **call sites** in
  those repos (sc-agent ships the client; migration is a follow-on).
- `decider-4b` / cyprid local backend implementation.
- Requested features **4–7** (disagreement UI, `sessions list/show`, `ask --json`,
  local key implementation beyond resolver hook).
- **sc-runtime daemon / UDS** hosting for backends.
- Benchmark-run; full multi-channel publisher beyond **crates.io + PyPI + sc-ai** (stub manifest OK until a-8).
- Parallel fan-out of hundreds of questions in CLI (design in types; optional
  batch API in library if low cost, not required for phase close).

## Sprint index

| Sprint | Doc |
| --- | --- |
| a-1 | [`sprint-a-1-workspace-and-types.md`](sprint-a-1-workspace-and-types.md) |
| a-2 | [`sprint-a-2-agent-secrets.md`](sprint-a-2-agent-secrets.md) |
| a-3 | [`sprint-a-3-check-build-and-payload.md`](sprint-a-3-check-build-and-payload.md) |
| a-4 | [`sprint-a-4-backend-jev.md`](sprint-a-4-backend-jev.md) |
| a-5 | [`sprint-a-5-backend-llm.md`](sprint-a-5-backend-llm.md) |
| a-6 | [`sprint-a-6-sessions-and-orchestration.md`](sprint-a-6-sessions-and-orchestration.md) |
| a-7 | [`sprint-a-7-sc-ai-cli.md`](sprint-a-7-sc-ai-cli.md) |
| a-8 | [`sprint-a-8-pyo3-wrapper.md`](sprint-a-8-pyo3-wrapper.md) |

## First release artifacts (Phase A close)

| Channel | Packages |
| --- | --- |
| **crates.io** | `sc-jev`, `sc-agent-types`, `sc-agent-secrets`, `sc-jev-backends`, `sc-ai` (no separate observability crate) |
| **PyPI** | `sc-jev-py` (maturin-built wheel; manylinux + macOS arm64 minimum) |
| **CLI** | `sc-ai` — same release train as Rust crates |

Release manifest and CI publish steps land in sprint **a-8** (REQ-SAG-0802/0803).
Orchestration/plan-anchor **call-site migration** in other repos is post-release.

## Phase acceptance (integration)

Run on `integrate/phase-a` HEAD after all sprints merge:

```bash
just lint
just test
cargo test -p sc-jev -- --nocapture
cargo test -p sc-ai -- --nocapture
uv run --directory tests/fixtures/jev-python pytest -q tests/test_cross_impl_bytes.py
```

Expected: all exit 0; cross-impl test asserts Rust `prompt_sha256` matches
committed fixture from Python reference bytes.

Optional evidence (not CI-gated in Phase A): one live `sc-ai jev run`
against TypeSafe with Rand-provided key, artifact attached to phase PR.

## Approval gate

**Stop:** no implementation PRs until Rand approves this plan and sprint docs.
