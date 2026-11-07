---
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0003
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0006
  - ADR-SAG-0007
  - ADR-SAG-0012
  - ADR-SAG-0013
  - ADR-SAG-0015
  - ADR-SAG-0019
  - ADR-SAG-0020
---

# Phase A — sc-jev and sc-ai CLI foundation

Status: **approved** — implementation complete on `integrate/phase-a`
(`d4a0111`); phase-end review open — see the `PHASE-A-REVIEW.md` items and
[Phase close notes / open items](#phase-close-notes--open-items). Phase A is
not closed. (Plan PR #4 merged 2026-11-02.)

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
- **Canonical judge payload** — one sorted compact JSON blob (questions + state);
  **`prompt_sha256`** is computed on those bytes for cross-impl (tighten #1) and
  equal-input. **Not** byte-identical HTTP bodies across backends: TypeSafe Jev
  uses its evaluator wire shape; Haiku/Luna may add a **system prompt** and schema
  wrapper (and may tune that prompt over time) while still embedding the **same
  canonical payload** in what the model judges. That system prompt is **injected**
  from `check.toml` / asset files / CLI override — **not** hardcoded in Rust.
- **Haiku/Luna API-direct** (no SDK/harness/tools); equal-input record every run
  attests each backend saw the same canonical bytes (README `prompt_sha256`, `equal`).
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
| `sc-agent-types` | Shared types + **temporary** v2 log port | `JevError`, `JudgeBackend`, `JudgeRequest`, domain types, `observability::v2` (shim) |
| `sc-agent-secrets` | API key resolution | `resolve_key(id, repo_root)` env → repo `.env` → keyring |
| `sc-jev` | **TypeSafe Jev HTTP only** | Jev API client; `JevClient: JudgeBackend`; no `check.toml`, no extractors |
| `sc-jev-backends` | LLM comparison judges | Haiku/Luna `JudgeBackend` impls; injectable HTTP at construction |
| `sc-jev-check` | **Check facade** | `check.toml`, extractors, payload, sessions, `run_check`, backend registry |
| `sc-treesitter` | **Subjects producer** | Tree-sitter queries → `sc/subjects/1` JSON; no Jev deps |
| `sc-ai` | Binary crate | `sc-ai jev …`; JSON-first per sc-ai-cli |
| `sc-jev-py` | PyO3 + **PyPI wheel** | Thin bindings over `sc-jev-check` |

**Trait boundary** (frozen in sprint **a-1**, authoritative sample in
`sprint-a-1-workspace-and-types.md` and `docs/sc-agent-types/architecture.md`):

```rust
pub struct PayloadBundle { pub bytes: Vec<u8>, pub sha256: Sha256Digest }

pub struct LlmWireContext { pub system_prompt: String }

pub struct JudgeRequest {
    pub payload: PayloadBundle,
    pub llm: Option<LlmWireContext>,
}

pub trait JudgeBackend: Send + Sync {
    fn id(&self) -> BackendId;
    fn validate_for_send(&self, req: &JudgeRequest) -> Result<(), JevError>;
    fn judge(&self, req: &JudgeRequest) -> Result<JudgeOutcome, JevError>;
}
```

The facade builds **`PayloadBundle` once** and loads the injectable LLM system
prompt into `JudgeRequest.llm` (ADR-SAG-0019). Jev ignores `llm`; Haiku/Luna
require `Some`. Transport is **construction state** on each backend (`JevClient`,
`AnthropicHaikuBackend`, …), not a `judge` parameter.

Local backend (`decider-4b` / cyprid) implements `JudgeBackend` in a **later**
crate; Phase A registers backends in **`sc-jev-check`**.

**Dependency direction:**

```text
sc-ai ── sc-observability 1.5.0 (v2 logger)
  └── sc-jev-check ──┬── sc-jev (TypeSafe HTTP)
                     ├── sc-jev-backends (Haiku/Luna)
                     ├── sc-agent-secrets
                     └── sc-agent-types

sc-jev-py → sc-jev-check

sc-treesitter → sc-agent-types   (standalone; nothing in Phase A links it except CLI invocation via `command` / shell)
```

`sc-jev-check` does **not** depend on `sc-treesitter` or any grammar crate.
`sc-jev` and `sc-jev-backends` do **not** depend on each other. No library
depends on CLI, PyO3, or full `sc-observability`.

## Boundary map (Phase A)

| Boundary | Crates | Contract |
| --- | --- | --- |
| `agent-types` | `sc-agent-types` | Errors, `JudgeBackend`, `JudgeRequest`, `LogEvent`, v2 shim |
| `agent-secrets` | `sc-agent-secrets` | Key resolution policy |
| `jev` | `sc-jev` | TypeSafe Jev HTTP + `JudgeBackend` impl |
| `jev-backends` | `sc-jev-backends` | LLM `JudgeBackend` impls |
| `jev-check` | `sc-jev-check` | Facade: config, extractors, run, sessions |
| `treesitter` | `sc-treesitter` | Subjects producer (`sc/subjects/1`) |
| `jev-py` | `sc-jev-py` | PyO3 over facade |
| `sc-ai-cli` | `sc-ai` | argv → operations → JSON envelope |

Sprint **a-1** commits stub manifests for every Phase A boundary and lists
`boundaries/**` in its `owned_paths`, along with `.gitignore`,
`.sc-lint/bootstrap.py`, the `sc-jev-check` module files, and the `sc-ai`
library stub. Each later boundary sprint also lists `boundaries/<id>/**` for
the boundary it closes and may replace the a-1 stub for that id.

## Governing ADRs

The `adrs` list in this document's frontmatter governs wave 2 (sprints a-2,
a-3, a-11, a-4, and a-5). Each sprint doc repeats the subset that applies to
its owned paths. Those ids are ADR-SAG-0001, ADR-SAG-0002, ADR-SAG-0003,
ADR-SAG-0004, ADR-SAG-0005, ADR-SAG-0006, ADR-SAG-0007, ADR-SAG-0012,
ADR-SAG-0013, ADR-SAG-0015, ADR-SAG-0019, and ADR-SAG-0020.

## Wave table

| Track | Wave | Sprint | `closure_type` | `target_boundary` | Notes |
| --- | --- | --- | --- | --- | --- |
| foundation | 1 | a-1 | contract | agent-types | workspace, sc-lint, frozen trait + types, cross-impl fixture |
| secrets | 2 | a-2 | boundary | agent-secrets | `must_follow` a-1 (`JudgeBackend` + errors); ∥ a-3 |
| build | 2 | a-3 | boundary | jev-check | extractors (`subjects`, …) + payload; `must_follow` a-1 |
| build | 2 | a-11 | boundary | treesitter | `sc-treesitter` producer; `must_follow` a-1; **∥ a-3** |
| jev | 2 | a-4 | boundary | jev | TypeSafe HTTP; `must_follow` a-1; **∥ a-3, a-11, a-5** |
| llm | 2 | a-5 | boundary | jev-backends | Haiku + Luna; `must_follow` a-1; **∥ a-3, a-11, a-4** |
| runtime | 3 | a-6 | boundary | jev-check | run, sessions, prompt load; `must_follow` a-2,a-3,a-4,a-5 |
| cli | 4 | a-7 | integration | sc-ai-cli | `must_follow` a-6, **a-11** (cross-impl consumer) |
| python | 4 | a-8 | integration | jev-py | PyO3; `must_follow` a-6; **∥ a-7** |
| publish | 5 | a-9 | integration | release | sc-publish install + preflight + dry-run; `must_follow` **a-7 and a-8** |

**Critical path (longest `must_follow` chain):** a-1 → a-3 → a-6 → a-7 → a-9
(5 sprints). a-8 joins publish at a-9; a-7 and a-8 are not serial with each other.  
**Width:** after a-1, up to **5** parallel siblings (a-2 ∥ a-3 ∥ a-11 ∥ a-4 ∥ a-5).
**Sprint count:** 10.

Sprint frontmatter **`parallel_safe`** lists wave-2 siblings only; **`must_follow`**
and the wave table define the authoritative dependency graph.

**Why three `jev-check` / HTTP layer sprints:** a-3 (extractors + payload) and
a-4/a-5 (HTTP backends) are substantial and parallel; a-6 integrates secrets,
multi-backend, and logging without overloading a-3.

### Must-follow artifacts (not “same file”)

| Edge | Artifact consumed |
| --- | --- |
| a-2 → a-1 | `JevError`, `BackendKeyId` (types); `resolve_key` API in secrets |
| a-3 → a-1 | `PayloadBundle`, `RepoRoot`, `Rev`, `sc/subjects/1` types, `minimal.json` fixture |
| a-11 → a-1 | `sc/subjects/1` types + schema rules; owns branches producer fixture |
| a-7 → a-11 | Committed `tests/fixtures/cross-impl/branches/**` for consumer digest tests |
| a-4 → a-1 | `JudgeBackend`, `JudgeRequest` |
| a-5 → a-1 | `JudgeBackend`, `JudgeRequest`, `LlmWireContext` |
| a-6 → a-2 | `resolve_key(id, repo_root)` |
| a-6 → a-3 | `CheckSpec`, canonical payload builder |
| a-6 → a-4 / a-5 | `JevClient`, Haiku/Luna backends as `JudgeBackend` |
| a-7 / a-8 → a-6 | `run_check`, sessions, logging port |
| a-9 → a-7 / a-8 | `sc-ai` binary + `sc-jev-py` wheel artifacts |

## Parity and features mapping

| Sprint | Parity rows closed (README checklist) | Requested features |
| --- | --- | --- |
| a-1 | (fixture only; enables payload tests) | — |
| a-2 | key resolution order tests (library; full parity in a-6) | — |
| a-11 | — | `sc-treesitter` + committed branches producer fixture |
| a-3 | extractors (`file`, `table-column`, `subjects`, `command`), config, payload | `subjects` input (minimal fixture only) |
| a-7 | CLI session/ask/run; branches cross-impl (consumer digest) | **1**, **2** |
| a-4 | Jev transport rows | — |
| a-5 | Haiku/Luna rows (Luna `equal` unverified — decision **4**) | **8** open (`agt-luna-live-count`); a-5 fake-transport wire only |
| a-6 | sessions, equal-input, keys via library | **3** (multi-backend) |
| a-8 | library API parity for Python callers | PyO3 + wheel build |
| a-9 | — | sc-publish install + preflight + REQ-SAG-0803 dry-run |

## Areas to tighten (Phase A vs defer)

| # | Item | Phase A |
| --- | --- | --- |
| 1 | Cross-impl byte fixture | **a-1** |
| 2 | Branch / region text | **a-11** (`sc-treesitter`) + **a-3** (`subjects` input) + **a-7** (consumer); ADR-SAG-0015 |
| 3 | Jev probabilities strict | **a-4** |
| 4 | LLM stop_reason / status | **a-5** |
| 5 | LLM busy retry | **a-5** |
| 6 | Session UUID + mode 0600 | **a-6** |
| 7 | Expiry uses `created_at` | **a-6** |
| 8 | Repo-root for git paths | **a-3** |
| 9 | Markdown escaped pipes | **a-3** (`table-column`) |
| 10 | Error enum + fix text | **a-1** |
| 11 | `consistent` flag on answers | defer |
| 12 | Vendor pins in config | **a-3** (`assets/jev.toml`) + **a-4** (Jev client defaults) |

## Phase A CLI and session logging (Rand)

Phase A optimizes for a **blocking CLI** that prints **JSON on stdout** and exits
(non-zero → typed error JSON on stderr). Every Jev judge/check call also
**appends JSONL** session records (Claude/Codex-style retained logs): prompt
payload metadata, backend id, timestamps, equal-input digest — one line per
logical prompt/send. Use **`sc-observability` v2** (`LoggerBuilder`, not v1 `Logger::new`) via
**`sc-agent-types::observability::v2`** injection (ADR-SAG-0016). **`sc-jev-check`**
logs through `SharedSessionLog`; `sc-ai` / `sc-jev-py` own real loggers. Shim removed when
upstream v2 is released.

**Stdout success shape (ADR-SAG-0014):** blocking CLI prints the **versioned
sc-ai-cli envelope** on stdout — `{ "version", "operation", "data" }` where
**`data`** holds the **TypeSafe Jev-equivalent answer JSON** plus required
**`session_id`** (UUID for session log correlation). Haiku/Luna paths normalize
to the same Jev answer shape inside `data`; fake transports are test-only.

## Resolved decisions (Rand)

1. **Key precedence:** process environment → **repo-root `.env`** → keyring
   (REQ-SAG-0202). **Source:** Rand, 2026-11-01 — PR #4 review comment (amendment to §6).
2. **sc-runtime daemon / UDS** — out of Phase A. **Source:** Phase A planning thread, 2026-11-01.
3. **Branch extraction:** **`sc/subjects/1`** artifact; **`sc-treesitter`** producer
   (sprint **a-11**); **`subjects`** input in **sc-jev-check** (a-3). Extraction is
   **independent of the Jev path** — injected from outside judge code. Tool is
   tree-sitter-specific, named **`sc-treesitter`**; grammars **static/pinned**, no
   dynamic loading (Phase A). Python drops `python-branches` extractor; cross-impl
   reads committed **`subjects.json`**. **No** atm-core PR #2308 dependency.
   **Source:** Rand, 2026-11-02 — [PR #4 comment](https://github.com/randlee/sc-agent/pull/4#issuecomment-6588041822).
4. **Luna live token count:** no OpenAI key is available to the maintainer in
   Phase A. Luna's `equal` record is therefore **unverified**, and the plan does
   not claim Luna parity. Tracked as bead **`agt-luna-live-count`**; closes when a
   key exists and the a-5 probe is run once with its output attached. Requested
   feature **8** remains open until then. **Source:** fenix status, atm-core PR
   #2308, 2026-11-01; recorded here by review 2026-11-02.

## Open questions (for Rand)

None blocking plan approval. **Kickoff decisions** (can default in sprint if silent):

1. ~~**Session JSONL root**~~ — **Resolved:** ADR-SAG-0021. CLI (`sc-ai`): `--log-dir`, then `SC_LOG_ROOT`, then `<repo-root>/.sc-ai`; sessions `--sessions-dir`, then `<repo-root>/.sc-ai/sessions`. Python (`sc-jev-py`): `log_dir`, then `SC_AGENT_LOG_DIR`, then `SC_LOG_ROOT`, then `~/.sc-agent/log`; sessions `session_dir`, then `SC_AGENT_SESSION_DIR`, then `{log_root}/sessions`.
2. ~~**`sc-observability` pin**~~ — **`=1.5.0`**, v2 `LoggerBuilder` in `sc-ai` only (resolved).
3. ~~Cross-impl bytes~~ — **Resolved:** a-1 commits Python reference bytes;
   phase acceptance runs `tests/fixtures/jev-python` pytest (a-8); Rust-only
   fixtures are not a substitute.
4. ~~Luna live token-count~~ — **Resolved:** see resolved decision **4** above
   (`agt-luna-live-count`); a-5 still proves token-count **wire** with fake
   transport; feature **8** / live `equal` verification stays open.
5. ~~**sc-ai-cli envelope**~~ — **Resolved:** ADR-SAG-0014 + sprint **a-7** use
   `{ "version", "operation", "data" }` with Jev body + `session_id` in `data`.

Follow-ups as beads: user-level `.env`, parallel question fan-out.

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
- Benchmark-run; **npm** or other channels not listed above until added to consumer input.
- Parallel fan-out of hundreds of questions in CLI (design in types; optional
  batch API in library if low cost, not required for phase close).

## Sprint index

| Sprint | Doc |
| --- | --- |
| a-1 | [`sprint-a-1-workspace-and-types.md`](sprint-a-1-workspace-and-types.md) |
| a-2 | [`sprint-a-2-agent-secrets.md`](sprint-a-2-agent-secrets.md) |
| a-3 | [`sprint-a-3-check-build-and-payload.md`](sprint-a-3-check-build-and-payload.md) |
| a-11 | [`sprint-a-11-sc-treesitter.md`](sprint-a-11-sc-treesitter.md) |
| a-4 | [`sprint-a-4-backend-jev.md`](sprint-a-4-backend-jev.md) |
| a-5 | [`sprint-a-5-backend-llm.md`](sprint-a-5-backend-llm.md) |
| a-6 | [`sprint-a-6-sessions-and-orchestration.md`](sprint-a-6-sessions-and-orchestration.md) |
| a-7 | [`sprint-a-7-sc-ai-cli.md`](sprint-a-7-sc-ai-cli.md) |
| a-8 | [`sprint-a-8-pyo3-wrapper.md`](sprint-a-8-pyo3-wrapper.md) |
| a-9 | [`sprint-a-9-sc-publish-install.md`](sprint-a-9-sc-publish-install.md) |

## First release artifacts (Phase A close)

| Channel | Packages |
| --- | --- |
| **crates.io** | `sc-agent-types`, `sc-agent-secrets`, `sc-jev`, `sc-jev-backends`, `sc-jev-check`, `sc-treesitter`, `sc-ai` |
| **PyPI** | `sc-jev-py` (maturin wheel) |
| **Homebrew** | `sc-ai` formula (kit `homebrew` channel) |
| **Scoop** | `sc-ai` manifest (kit `scoop` channel) |
| **WinGet** | `sc-ai` manifest (kit `winget` channel) |

GitHub Release assets feed the package managers; one tag drives all channels via
sc-publish.

Release uses the **[sc-publish](https://github.com/randlee/sc-publish)** kit
(sprint **a-9**): consumer input, rendered manifests, preflight CI, publish dry-run.
Orchestration/plan-anchor **call-site migration** in other repos is post-release.

## Requirement closure (integration `req:` roots)

Authoritative **`req:<ID>` acceptance roots** live on integration sprints only.
Boundary sprints **a-1…a-6** prove `boundary:` / ADR rows; feature REQ closure
below is verified at phase integration:

Every `req:` root is owned by exactly one sprint. Rule applied when two
integration sprints prove the same id: the owner is the sprint whose surface
is the primary one for that requirement, and every other sprint records the
proof as an `evidence:<ID>` row, which is not an acceptance root. Applied at
phase end: **REQ-SAG-0610** → a-8 (in-process session JSONL; a-7 keeps an
`evidence:` row for the CLI adapter); **NFR-SAG-0002** and **NFR-SAG-0003** →
a-7 (CLI output is the primary surface; a-8 keeps `evidence:` rows for the
Python half). **REQ-SAG-0802** → a-8 (a-9 keeps an `evidence:` row for the CI
job).

| Sprint | REQ / NFR ids closed |
| --- | --- |
| a-1 (contract) | 0005 (workspace layout matches `docs/architecture.md` and the per-crate requirement tables) |
| a-7 | 0101, 0600, 0601, 0603, 0700–0703, 0710, 0711, 0503 (CLI override), NFR-0001–0003 |
| a-8 | 0102, 0610, 0611, 0800, 0801, 0802; evidence only: NFR-0002, NFR-0003 |
| a-9 | 0130, 0131, 0132, 0803; evidence only: 0802 |

Remaining feature REQs (0100, 0103, 0104, 0120, 0121, 0200–0202, 0300–0303,
0305, 0309, 0310, 0311, 0400–0401, 0500–0502, 0504–0505, 0602, 0110) close via
**boundary sprint deliverables** plus integration tests above (no duplicate `req:`
rows on a-1…a-6).

**REQ-SAG-0110** (lifetime Jev API coverage) — Phase A closes check/judge slice
in **a-4**; full API is a standing requirement with later sprints.

## Phase acceptance (integration)

Run on `integrate/phase-a` HEAD after all sprints merge:

```bash
just lint
just test
cargo test -p sc-jev -- --nocapture
cargo test -p sc-ai -- --nocapture
# Publish-surface wheel build: default features (`extension-module` only,
# ADR-SAG-0022). It is not the test build.
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
# Test build: a-8's `--features extension-module,test-util`. uv passes it
# through tests/fixtures/jev-python/pyproject.toml; `just test-python` passes
# it to `maturin develop` (the CI path).
uv run --directory tests/fixtures/jev-python pytest -q
just test-python
cargo test -p sc-jev-check cross_impl_branches -- --nocapture
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

Expected: all exit 0; cross-impl tests assert Rust digests match committed
fixtures; release preflight + publish dry-run satisfy REQ-SAG-0803 (a-9).

Optional evidence (not CI-gated in Phase A): one live `sc-ai jev run`
against TypeSafe with Rand-provided key, artifact attached to phase PR.

### Acceptance evidence

Recorded `sc-agent-ci` run: [42345904358](https://github.com/randlee/sc-agent/actions/runs/42345904358) on PR #18 at `5283307`,
all nine jobs green. It is the run on the commit **before** this
evidence-only docs commit; the docs commit changes no CI input.

| Command (above) | Where it runs | Recorded result |
| --- | --- | --- |
| `just lint` | `sc-agent-ci.yml` job `test`, ubuntu-24.04 only (the managed sc-lint bootstrap fails on Windows; [`ci.md`](../../development/ci.md#known-limits)) | Green: [test (ubuntu-24.04)](https://github.com/randlee/sc-agent/actions/runs/42345904358/job/128323467345). Re-run at a-10 QA-2 head 9482380 (after the boundary-manifest and clippy edits): green in [test (ubuntu-24.04)](https://github.com/randlee/sc-agent/actions/runs/42361431082/job/128370466596) (`compatibility.check: ok`, `lint.ci.consumer: ok`); local `lint.ci.consumer: pass (3 steps)`, sc-lint-boundary 0.8.0 pass with 0 findings over 9 crates. Later commits are docs-only |
| `cargo test -p …`, `cross_impl_branches`, and the `workspace` layer of `just test` (`cargo test --workspace`) | `sc-agent-ci.yml` job `test` (ubuntu-24.04, macos-15, windows-2022) | 194 passed, 0 failed on each OS: [ubuntu-24.04](https://github.com/randlee/sc-agent/actions/runs/42345904358/job/128323467345), [macos-15](https://github.com/randlee/sc-agent/actions/runs/42345904358/job/128323467493), [windows-2022](https://github.com/randlee/sc-agent/actions/runs/42345904358/job/128323467473) |
| `clippy --workspace --all-targets --all-features -- -D warnings` | `sc-agent-ci.yml` job `test` (three OSes) | Green in the same three jobs |
| `cargo doc` with `RUSTDOCFLAGS=-D warnings` | `sc-agent-ci.yml` job `doc` (a hard gate; no `continue-on-error` since a-10) | Historical: run [42345904358](https://github.com/randlee/sc-agent/actions/runs/42345904358/job/128323467366) failed underneath the former `continue-on-error` (could not document `sc-agent-types`, `sc-treesitter`). QA-005 (a-10) fixed the warnings and removed `continue-on-error`; the job now fails the run on any rustdoc warning |
| Test-util wheel + `tests/fixtures/jev-python` pytest (the `jev-python` layer of `just test`) | `sc-agent-ci.yml` job `python` (`just test-python`, three OSes) | 7 passed on each OS: [ubuntu-24.04](https://github.com/randlee/sc-agent/actions/runs/42345904358/job/128323467398), [macos-15](https://github.com/randlee/sc-agent/actions/runs/42345904358/job/128323467414), [windows-2022](https://github.com/randlee/sc-agent/actions/runs/42345904358/job/128323467289) |
| sc-treesitter feature matrix | `sc-agent-ci.yml` job `features` (`cargo hack --each-feature test -p sc-treesitter` since a-10) | Historical: [features](https://github.com/randlee/sc-agent/actions/runs/42345904358/job/128323467072) ran `check` only. a-10 feature-gated the per-language tests (QA-006) and the job now runs `test` for every feature combination |
| Coverage | `sc-agent-ci.yml` job `coverage` (cargo-llvm-cov) | 82.52% line (TOTAL row): [coverage job](https://github.com/randlee/sc-agent/actions/runs/42345904358/job/128323467256), artifact `coverage-summary` of run [42345904358](https://github.com/randlee/sc-agent/actions/runs/42345904358) |
| Preflight, `cargo publish --dry-run`, publish-surface `maturin build` | `.github/workflows/sc-publish-dry-run.yml` | Runs [42339767937](https://github.com/randlee/sc-agent/actions/runs/42339767937), [42341299125](https://github.com/randlee/sc-agent/actions/runs/42341299125), [42341873748](https://github.com/randlee/sc-agent/actions/runs/42341873748) on PR #17 (sprint a-9), all green |

Local equivalents of every CI job: [`docs/development/ci.md`](../../development/ci.md).

## Phase close notes / open items

Phase A is not closed. Open at phase end:

- Bead **`agt-luna-live-count`** — Luna live token count is unverified
  (resolved decision **4**); requested feature **8** stays open until a key
  exists and the a-5 probe runs once with its output attached.
- Phase-end review decisions D1–D13 (`PHASE-A-REVIEW.md` section 4) are
  awaiting Rand; nothing architectural has started.
- Closed in a-10: the `sc-agent-ci` `doc` job is a hard gate (no
  `continue-on-error`), and the sc-treesitter feature matrix runs
  `cargo hack --each-feature test`. The run ids in the table above are
  historical evidence from before those changes.
- Bead `agt-obs-v2-shim-removal` (sprint a-1) stays open until upstream v2 is
  released.

## Approval gate

Phase A plan and sprint docs were approved when PR #4 merged on 2026-11-02.
Implementation is complete on `integrate/phase-a` (`d4a0111`, all ten sprints
merged). The phase-end review is open; see
[Phase close notes / open items](#phase-close-notes--open-items). The phase
closes when those items are fixed or deferred and `integrate/phase-a` merges to
`develop`.
