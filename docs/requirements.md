# sc-agent requirements

**ID range:** REQ-SAG-0001+  
**Status:** Draft (Phase A planning)  
**Owner:** sc-agent maintainers

## Product purpose

Rust crates plus orchestration assets for launching Synaptic Canvas agents.
Phase A delivers **`sc-jev`** (full Jev API home; check/judge slice first +
Haiku/Luna comparison) and
the **`sc-ai`** CLI. Contract: atm-core `plan/phase-bo-replan` →
`docs/plans/phase-bo/design/packages/jev-check/README.md`.

## Bootstrap (done)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0001 | Agent assets under `.claude/`, `.cursor/`, skills as documented |
| REQ-SAG-0002 | `.claude/project/quality-policy.md` is the QA policy source |
| REQ-SAG-0003 | sc-ai team in `.atm.toml`; roster in `docs/development/developer-roster.md` |
| REQ-SAG-0004 | plan-anchor under `.sc/plan-anchor/` |
| REQ-SAG-0005 | Rust workspace follows `docs/architecture.md` and per-crate requirements |

## Phase A — sc-jev (north star)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0110 | **`sc-jev` implements 100% of TypeSafe Jev HTTP API** over product lifetime; Phase A closes check/judge parity only |

## Phase A — foundation (sprint a-1)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0100 | Root Cargo workspace, `justfile`, sc-lint, standard lint/test targets |
| REQ-SAG-0101 | `JevError`: one variant per README error id; each has `code()` and `fix_hint()` |
| REQ-SAG-0102 | Cross-implementation payload fixture: Rust digest matches committed Python reference bytes |
| NFR-SAG-0001 | CLI error JSON shape (`ScAgentErrorResponse`) defined in `sc-agent-types` |
| REQ-SAG-0120 | Temporary `sc-agent-types::observability::v2` SessionLog shim (not a separate crate) |
| REQ-SAG-0121 | Noop + test-util doubles; delete shim + trim deps after upstream v2 release |

## Phase A — secrets (sprint a-2)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0200 | Resolve API keys env → keyring for TypeSafe, Anthropic, OpenAI |
| REQ-SAG-0201 | Optional `ANTHROPIC_KEYCHAIN_SERVICE` override |
| REQ-SAG-0202 | Repo-root `.env` third; precedence **process env → `.env` → keyring** |

## Phase A — check build (sprint a-3)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0300 | Extractors: `file`, `table-column`, `delimited-regions` |
| REQ-SAG-0304 | `delimited-regions`: many ids per file; optional `regions = [...]` to include only listed ids |
| REQ-SAG-0301 | Refuse send on `CONFIG_INVALID`, `INPUT_*` errors |
| REQ-SAG-0302 | Git repo-root resolution for relative paths |
| REQ-SAG-0303 | Question-set schema `plan-anchoring/questions/1` |

## Phase A — Jev API in sc-jev (sprint a-4)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0400 | Injectable transport; byte-exact Jev check/judge body |
| REQ-SAG-0401 | Busy retry policy (429/502/529 + Retry-After) |

## Phase A — LLM backends (sprint a-5)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0500 | Haiku API-direct; system prompt allowed; canonical payload embedded; `prompt_sha256` on canonical bytes only |
| REQ-SAG-0501 | Luna API-direct + live input token count verification (feature 8) |
| REQ-SAG-0502 | LLM context-length refusal matches Jev semantics |

## Phase A — orchestration (sprint a-6)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0600 | Session store: UUID, mode 0600, expiry from `created_at` |
| REQ-SAG-0601 | `run_check` / dry-run library API |
| REQ-SAG-0602 | Equal-input record on every backend run |
| REQ-SAG-0603 | Multiple backends in one run (feature 3) |
| REQ-SAG-0610 | Every Jev/LLM prompt send emits `LogEvent` via `SharedSessionLog` (v2 path) |
| REQ-SAG-0611 | Log records include session correlation id, backend, payload digest, no secrets |

## Phase A — CLI (sprint a-7)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0700 | `sc-ai jev` subcommands with versioned JSON envelope |
| REQ-SAG-0701 | stderr `{"error","message"}`, exit 2 |
| REQ-SAG-0702 | Readback pairs per sc-ai-cli |
| REQ-SAG-0703 | Documented install path for `sc-ai` binary |
| REQ-SAG-0710 | Blocking CLI: stdout = Jev API JSON + `session_id`; stderr errors; exit 2 |
| NFR-SAG-0002 | Secrets never appear in CLI output or logs |
| NFR-SAG-0003 | Session JSONL retained on disk (path configurable; default documented) |
| REQ-SAG-0711 | `sc-ai` wires `v2::LoggerBuilder` + adapter into `sc-jev` |

## Phase A — sc-jev-py + first release (sprint a-8)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0800 | PyO3 exports `sc-jev`; Python errors match enum codes |
| REQ-SAG-0801 | Cross-impl pytest harness in repo |
| REQ-SAG-0802 | maturin wheel build; `pip install` from built artifact |

## Phase A — sc-publish kit (sprints a-9, a-10)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0130 | sc-publish consumer input enables **crates.io**, **pypi**, **homebrew**, **scoop**, **winget** + `sc-ai` release binaries |
| REQ-SAG-0131 | `docs/development/sc-publish.md`; do not hand-edit rendered publish manifests |
| REQ-SAG-0132 | Release preflight CI green; local preflight script matches kit |
| REQ-SAG-0803 | **First release:** all five channels via sc-publish (preflight + dry-run in CI) |

## Phase B (draft — not scheduled)

| ID | Requirement |
| --- | --- |
| REQ-SAG-0900 | `sc-ai` exposes API-direct Claude agent (Anthropic HTTP, no claude-code subprocess) |
| REQ-SAG-0901 | `sc-ai` exposes API-direct Codex/OpenAI agent (HTTP, no codex-cli subprocess) |
| REQ-SAG-0902 | Comparison flows may reuse `sc-jev` payload / equal-input rules where applicable |

## Traceability

Sprint docs under `docs/plans/phase-a/` list REQ ids in YAML frontmatter.
Phase plan: [`docs/plans/phase-a/plan.md`](plans/phase-a/plan.md).
