# Pre-release rename stack: `decidr` + `sitr`

Six append-only **gh stack** layers on trunk **`develop`**, each layer in its own **`/sc-git-worktree`**, with a **Composer QA** pass on that layer’s diff before the next layer is cut. Bead epic: **agt-edv**.

**Registry:** `decidr` and `sitr` are available on crates.io; `decidr`/`sitr` are available on PyPI (we do not publish those names as wheels—PyPI stays `sc-agent` / `sc-treesitter`).

**Do not rename (false-positive traps):**

- `Decision::decide`, `decide_stream`, and other **trait/method** names in `sc-decision-*`.
- plan-anchor **`decide()`** (judge API) and generic English “decide whether…”.
- Crate names `sc-decision-*`, `Mapika/decider`, etc.

---

## Roles

| Role | Responsibility |
|------|----------------|
| **Stack writer** (orchestrator) | Only actor that runs `gh stack link`, merge-forward, land. |
| **Dev (Grok)** | One worktree per wave; commit only on assigned branch; push before QA. |
| **Composer QA** | After each wave push: review **cumulative** `develop...HEAD` on the stack top (or layer-only diff + grep gates). Verdict **PASS** required before next `gh stack add` / cut. |

**Trunk:** `develop`. **Landing:** `gh stack merge <stack#> --yes --merge` when top is green + QA PASS (merge commits, no squash).

**Worktree base:** `../sc-agent-worktrees/` (per `sc-managing-worktrees`).

---

## Stack map (bottom → top)

| Wave | Bead (child of agt-edv) | Branch | Cuts from |
|------|-------------------------|--------|-----------|
| W1 | (first child) | `pre-release/rename-w1-decidr` | `develop` |
| W2 | | `pre-release/rename-w2-decidr-consumers` | pushed **W1** head |
| W3 | | `pre-release/rename-w3-sitr` | pushed **W2** head |
| W4 | | `pre-release/rename-w4-req-adr` | pushed **W3** head |
| W5 | | `pre-release/rename-w5-plans-anchors` | pushed **W4** head |
| W6 | | `pre-release/rename-w6-ci-release` | pushed **W5** head |

Open PR on first push of W1; link stack when W2 PR exists:

```bash
gh stack link --base develop <w1-pr#> <w2-pr#>   # first full link
gh stack link <stack#> <wN-pr#>                # append each upper layer
```

After each layer freezes, **no edits** on that branch—fixes go on a new top layer only.

---

## W1 — `decidr` crate (mechanical Rust)

**Scope:** `crates/decide/` → `crates/decidr/` (`git mv`).

- `Cargo.toml`: `[package] name = "decidr"`, `[lib] name`, `[[bin]] name = "decidr"`.
- Rename **`DecideError` → `DecidrError`** (crate root export).
- Error id prefix **`decide.` → `decidr.`**; log service **`decidr`**; general log **`decidr.log.jsonl`**; actions **`decidr.send`**, etc.
- clap `name` / `bin_name` = `decidr`.
- Update comment in `Cargo.toml` (crates.io name `decidr`).
- **Do not** edit `sc-agent-py` yet except if required to compile workspace (prefer W2).

**Gates (dev, before push):**

```bash
cargo test -p decidr --all-features
cargo clippy -p decidr --all-targets -- -D warnings
```

**Gates (QA):** see [Composer QA checklist](#composer-qa-checklist-per-wave); wave scope = W1 paths only acceptable if workspace still fails—document expected failures.

---

## W2 — `decidr` consumers

**Scope:**

- `crates/sc-agent-py/Cargo.toml` + `src/lib.rs`: path `../decidr`, `use decidr::`, binary name in tests/docs strings.
- `crates/decide/tests` → moved in W1 under `decidr`; fix any remaining `decide::` in workspace **outside** decision trait (grep).
- Comments/README in lower crates that say “decide CLI” → “decidr CLI” only when referring to the product binary.
- `.github/workflows/ci.yml` if it names the `decide` binary or `cargo build -p decide`.

**Gates:**

```bash
cargo test -p decidr -p sc-agent-py
# optional: maturin/pytest if env ready
rg -n 'path = \"\.\./decide\"|\"-p decide\"|CARGO_BIN_EXE_decide' crates .github || true
```

---

## W3 — `sitr` CLI

**Scope:** `sc-treesitter` only.

- `[[bin]] name = "sitr"`.
- clap / `SERVICE` / `sitr.log.jsonl` / `sitr.command`.
- `tests/cli.rs`: `CARGO_BIN_EXE_sitr`, hints `sitr languages`.
- CI assertion: bin `sitr` + `required-features = ["cli"]`.

**Gates:**

```bash
cargo test -p sc-treesitter --features cli,grammar-python,grammar-rust
cargo run -p sc-treesitter --features cli --bin sitr -- languages
```

---

## W4 — REQ / ADR / crate READMEs

**Scope:**

- `git mv docs/decide` → `docs/decidr` (if present).
- `docs/requirements/**`, `docs/adr/**`: product name `decide` → `decidr` where it means the CLI/library (not `Decision::decide`).
- `crates/*/README.md` install lines: `cargo install decidr`, `cargo install sc-treesitter --features cli` → binary `sitr`.

**Gates:** `rg -l '\bdecide\b' docs/requirements docs/adr docs/decidr | wc -l` triaged to zero **product** hits (manual allowlist file in QA report).

---

## W5 — Phase A plans + anchors (living docs only)

**Scope:**

- `docs/plans/phase-a/plan.md`, `sprint-a-*.md`, `anchors.json` → re-render `anchors.md`, `gate-items/`, `copy/a-3.sh`, `copy/a-10.sh`.
- **Owner (2026-11-11):** **Do not** rewrite historical plan artifacts: `anchor-runs/`, `reports/`, `anchor-tuning/`, phase-a review JSON/HTML. Living exports and anchors only.

## W4+ — REQ, ADR, architecture (mandatory)

**Owner (2026-11-11):** Every **REQ**, **ADR**, **NFR**, and crate-scoped requirement under `docs/` (including `docs/decide/` → `docs/decidr/`) must reflect **decidr** / **sitr**. Not optional.

**QA after docs wave:** scoped **req-qa** + **arch-qa** (or **cursor-quality-mgr** coordinating those two only) — **rename-impact only**; no new unrelated findings; ceremony screen applies.

**Gates:** ANC-006 / CLI parity language says **decidr**; ADR-SAG-0004 layer row says **decidr** (not `decide` crate).

---

## W6 — CI, release, publish stubs, memory

**Scope:**

- `.github/workflows/**` (bin names, cache keys).
- `release/publish-artifacts.toml`, future `sc-publish-consumer-input.json`: crate **`decidr`**, binaries **`decidr`** + **`sitr`**.
- `.cursor/` product references (not plan-anchor judge docs).
- `bd remember` updates: `owner-cli-requirement-1`, `owner-treesitter-cli-name` → `sitr`.

**Gates:**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
rg '\bcrates/decide\b|"name = \"decide\"|\[\[bin\]\]\nname = \"decide\"' . --glob '!docs/plans/phase-a/anchor-runs/**' --glob '!docs/plans/phase-a/reports/**'
rg '\btsq\b' crates docs/release .github --glob '!docs/plans/phase-a/anchor-runs/**'
```

Unblocks **agt-pb8** (a-11 sc-publish install) after land.

---

## Worktree recipe (each wave)

From repo root on `develop`:

```bash
git fetch origin --prune
# W1 only:
git worktree add -b pre-release/rename-w1-decidr \
  "../sc-agent-worktrees/pre-release-rename-w1-decidr" develop

# Wk (k>1): replace BASE with origin/pre-release/rename-w{k-1}-...
git worktree add -b pre-release/rename-w2-decidr-consumers \
  "../sc-agent-worktrees/pre-release-rename-w2-decidr-consumers" \
  origin/pre-release/rename-w1-decidr
```

Dev completes wave → push → PR → stack writer links → **Composer QA** → freeze → cut next worktree.

---

## Composer QA checklist (per wave)

Dispatch **one** Composer agent (`composer-2.5-fast` or equivalent) with:

**Inputs:**

- `Full Repository Path: /Volumes/Extreme Pro/github/sc-agent`
- `Diff: branch changes` (base = `develop`, head = current layer branch)
- `Change Description:` wave id (W1–W6) + scope from this doc
- `Custom Instructions:`

```text
Rename QA (wave Wn). Find:
1. OVER-RENAME: Decision::decide, decide_stream, plan-anchor decide(), sc-decision-* crate names changed.
2. UNDER-RENAME: crates/decide, package/bin "decide", decide.log.jsonl, decide.* error ids, CARGO_BIN_EXE_decide, tsq after W3.
3. INCONSISTENT: decidr binary but decide.* ids, or sitr binary but tsq.log.jsonl.
4. ALLOWLIST: English "decide" in plan-anchoring skills, judge API, generic prose — list but do not fail.

Run ripgrep from repo root; cite file:line for each FAIL.
Verdict: PASS only if zero FAIL items in wave scope (+ no new over-renames repo-wide).
Output: fenced JSON { "verdict": "PASS"|"FAIL", "over_rename": [], "under_rename": [], "inconsistent": [] }
```

Fixes for QA FAIL: **new stack layer on top** (or current top if not yet frozen), never edit frozen layer below.

---

## Grok handoff (one wave at a time)

Only dispatch Grok after QA PASS on the previous wave (W1 has no predecessor).

**Prompt skeleton:**

> Worktree: `../sc-agent-worktrees/pre-release-rename-wN-…`, branch `pre-release/rename-wN-…`, base = previous wave’s pushed head (W1: `develop`).  
> Implement **only** wave Wn from `docs/plans/pre-release/rename-decidr-sitr-stack.md`.  
> Run wave gates; push; stop. Do not cut the next branch.

Stack writer runs Composer QA, then cuts WN+1.

---

## Landing

1. Top PR (W6) CI green + Composer QA PASS.
2. `gh stack merge <stack#> --yes --merge` into `develop`.
3. Remove worktrees; close **agt-edv** children then epic.
4. Proceed with **agt-pb8** / a-11 publish consumer using **`decidr`** + **`sitr`**.
