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

All user-visible error ids come from `JevError` in `sc-agent-types`.
CLI and PyO3 map variants to JSON / exceptions; no ad-hoc strings.

## ADR-SAG-0002 — Canonical judge payload bytes

The **canonical judge payload** is sorted-key compact JSON (UTF-8): the
questions + state the backends must judge. SHA-256 of **those** bytes is
`prompt_sha256` for equal-input and the cross-language fixture (sprint a-1).

## ADR-SAG-0019 — LLM wire envelope vs canonical payload

**Jev (TypeSafe)** sends the canonical payload in the API shape the evaluator
expects (including model selection on the wire).

**Haiku / Luna** send API-direct HTTP that is **not** byte-identical to Jev’s
body. They **must** embed the **same canonical payload bytes** in the user/judge
message (per README parity). A **system prompt** (evaluator instructions, JSON
schema hints) is allowed and **may change** over time without changing
`prompt_sha256`, as long as the embedded canonical blob is unchanged.

**System prompt injection (not hardcoded in Rust):** production crates must not
embed evaluator prose as string literals. Orchestration loads prompt text from
**`check.toml`** (`llm_judge.system_prompt_file`, path relative to the check
file) and/or packaged **`jev.toml`** defaults (path to a shipped **asset file**).
`sc-jev-backends` receives the resolved text from the caller when building the
wire envelope. **`sc-ai jev`** may override with `--llm-system-prompt-file` for
one run. Tests use fixture files or explicit test-only strings.

Token/count checks apply to the full message the provider receives; equal-input
still records whether every backend saw the same canonical bytes.

## ADR-SAG-0003 — Secret resolution

`sc-agent-secrets` resolves keys: **process env → repo-root `.env` → keyring**.

## ADR-SAG-0004 — One check, one request

No splitting oversized inputs. Pre-send byte/token validation on every backend.

## ADR-SAG-0006 — Transport injection

Jev HTTP client lives in `sc-jev`; LLM HTTP in `sc-jev-backends`. Transports are
**injected at backend construction** (not passed into `JudgeBackend::judge`).
Both use test doubles for unit tests.

## ADR-SAG-0007 — API-direct LLM backends

Haiku and Luna judges use **provider HTTP only** in Phase A: no vendor SDK,
agent harness, or tool-use loop. Wire format and token counting are implemented
in `sc-jev-backends` against documented REST endpoints.

## ADR-SAG-0010 — CLI thin adapter

`sc-ai` parses argv and prints envelopes; no network code in the binary crate.

## ADR-SAG-0011 — Python via sc-jev-py wheel

Python apps use the **`sc-jev-py`** PyPI extension (PyO3 → `sc-jev-check`). Optional
thin wrappers (e.g. atm-bd `packages/jev-check`) must not reimplement Jev HTTP
or canonicalization.

## ADR-SAG-0018 — sc-publish kit owns release surface

Release manifests, channel contracts, and publish workflows are **installed**
from [sc-publish](https://github.com/randlee/sc-publish) via
`release/sc-publish-consumer-input.json`. Edit consumer input + re-run
`install.py`; do not hand-edit rendered TOML under `release/`.

## ADR-SAG-0017 — First release surfaces

Phase A first release ships **five publish channels** from one tag via sc-publish:
**crates.io**, **PyPI** (`sc-jev-py`), **Homebrew**, **Scoop**, and **WinGet**
(`sc-ai` binary), plus GitHub Release assets.

## ADR-SAG-0015 — Language-agnostic extraction (`tree-sitter` + `command`)

Replaces delimited comment regions. **Normative spec:**
[`docs/sc-jev-check/extraction.md`](sc-jev-check/extraction.md) (PR review §9).

Summary:

- One **`Extractor`** trait; kinds **`file`**, **`table-column`**, **`tree-sitter`**, **`command`**.
- **`tree-sitter`**: grammar id + **`.scm` query file** (or inline query) — no
  per-language branch logic in Rust. Subject **text** = source byte slice; normative
  **id**, **order**, helper captures (`@_…`), parse errors, session metadata
  (`grammar_version`, `query_sha256`).
- **`command`**: argv-array subprocess → JSON `{id: text}` escape hatch.
- **Python reference** must switch `python-branches` from `ast.unparse` to the
  same slice (**atm-core PR #2308**); cross-impl fixture under
  `tests/fixtures/cross-impl/branches/`.
- Delimited regions and comment sigils are **out of scope**.

LLM system prompt loading: **ADR-SAG-0019** (`[llm_judge] system_prompt_file` in
check.toml; default asset under `sc-jev-check`).

## ADR-SAG-0013 — Session JSONL via sc-observability v2

Jev session/prompt logging uses **`sc-observability` v2** (`LoggerBuilder`,
file/console sinks). **`sc-jev-check`** emits `LogEvent`s through
**`SharedSessionLog`** (see ADR-SAG-0016); only **`sc-ai`** links full
`sc-observability`. No bespoke JSONL writers. Secrets never appear in log lines.

## ADR-SAG-0016 — v2 shim in sc-agent-types (no extra crate)

Do **not** publish a separate observability crate. For the **first release or two**,
**`sc-agent-types::observability::v2`** holds the injectable `SessionLog` port
(aligned with registry `v2::Logger`). Temporary dependency:
`sc-observability-types = "1.5.0"` on **`sc-agent-types` only**; full
`sc-observability = "1.5.0"` on composition roots **`sc-ai`** and **`sc-jev-py`**.
Libraries take only **`SharedSessionLog`**. When upstream v2 ships, **delete
the module** and drop the types dependency if possible (bead:
`agt-obs-v2-shim-removal`).

## ADR-SAG-0014 — CLI success body

On success, CLI `data` includes the **verbatim Jev response JSON** for the
operation plus **`session_id`**. LLM backends normalize to the same answer shape
before printing.

## ADR-SAG-0012 — sc-jev owns Jev API

Every TypeSafe Jev HTTP operation is implemented in `sc-jev`. Other crates
must not send Jev API requests. Phase A ships the check/judge slice; remaining
endpoints extend `sc-jev` in place.

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
