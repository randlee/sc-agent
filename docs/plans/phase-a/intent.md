# Phase A intent — sc-agent

## Product

**sc-agent** is a Rust workspace plus agent orchestration assets (already in
`.claude/`, `.cursor/`, `.codex/`). The executable product is the **`sc-ai`**
CLI: JSON-first, versioned envelopes, typed errors — aligned with Synaptic
Canvas **sc-ai-cli** guidelines.

**In scope for the product (not Phase A):** API-direct **Claude** and **Codex**
agents launched via `sc-ai` — same JSON/sc-ai-cli posture, reusable secrets and
types, usable to **compare answers to Jev** (equal-input discipline where
applicable). Phase A compares via **`sc-jev`** check flow with **Haiku** and
**Luna** backends on the shared payload.

**Explicitly out of scope:** wrapping vendor harness CLIs (**claude-code**,
**codex-cli**, **cursor-cli**) as subprocess/json bridges. We call provider APIs
directly; we do not shell out to those tools.

## Phase A focus

Build **`sc-jev`**: the sole TypeSafe Jev library (**100% of Jev API** over
time). Phase A ports the **check/judge** slice from the Python reference
(atm-bd / `packages/jev-check`, atm-core `plan/phase-bo-replan` →
`docs/plans/phase-bo/design/packages/jev-check/README.md`). One Rust client
replaces every ad-hoc Jev caller (orchestration, plan-anchor, gates). Python
`packages/jev-check` is the **only** behavioural reference for Phase A; nothing
is copied from legacy `jev_client.py` / plan-anchor scripts.

## Success

- **Blocking `sc-ai jev` CLI** with JSON stdout (Jev response + `session_id`).
- **JSONL session logging** for every prompt via **`sc-observability` v2**
  (`LoggerBuilder`), injected via **`sc-agent-types::observability::v2`** (temporary
  module in the shared crate until upstream v2 release).
- Parity on every row of the README **Parity checklist**.
- Requested features **1** (caller-built requests), **2** (dry run), **3**
  (multi-backend one run), **8** (Luna token-count wire verified in tests; live
  OpenAI call optional — not required for phase close without a key).
- Selected **areas to tighten** closed where cheaper than copying Python
  weaknesses (see phase plan).

## Authority

- Contract: atm-core branch `plan/phase-bo-replan`, folder
  `docs/plans/phase-bo/design/packages/jev-check/` (README + tests mapping).
- Rationale: `docs/plans/phase-bo/design/jev-check-proposal.md`.
- CLI shape: synaptic-canvas `packages/sc-ai-cli` (JSON envelope, errors,
  readback).

## First release (Phase A close)

Three consumer surfaces, same `sc-jev` core:

| Surface | Artifact |
| --- | --- |
| Rust | **crates.io** — `sc-jev` + shared crates |
| Python | **PyPI** — **`sc-jev-py`** wheel |
| CLI | **`sc-ai`** — crates.io, **Homebrew**, **Scoop**, **WinGet** (sc-publish channels) |

Python apps import the extension directly; atm-bd `packages/jev-check` may thin-wrap
but is not required for PyPI consumers.

Publishing uses the **[sc-publish](https://github.com/randlee/sc-publish)** kit
(sprints a-9/a-10), not ad-hoc workflow edits.

Rand approves `docs/plans/phase-a/plan.md` and sprint docs before
implementation starts.
