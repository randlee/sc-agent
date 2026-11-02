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
sc-ai (binary) ── builds sc_observability::v2::Logger, SessionLog adapter
  └── sc-jev (all Jev HTTP + check orchestration)
        ├── sc-jev-backends (Haiku, Luna only)
        ├── sc-agent-types (includes temporary `observability::v2` shim)
        └── sc-agent-secrets

sc-jev-py (PyO3) → sc-jev
```

**Jev API:** All TypeSafe Jev HTTP lives in **`sc-jev`** (REQ-SAG-0110).  
**Trait boundary:** `JudgeBackend` and shared domain types live in
`sc-agent-types`. LLM comparison backends implement the trait in
`sc-jev-backends`. Local judge (decider-4b / cyprid) adds a backend later
without changing callers.

## ADR-SAG-0001 — Error enum is CLI authority

All user-visible error ids come from `JevError` in `sc-agent-types`.
CLI and PyO3 map variants to JSON / exceptions; no ad-hoc strings.

## ADR-SAG-0002 — Canonical payload bytes

Judge requests use sorted-key compact JSON (UTF-8). SHA-256 of payload bytes
is recorded for equal-input proofs. Cross-language fixture in sprint a-1.

## ADR-SAG-0003 — Secret resolution

`sc-agent-secrets` resolves keys: **process env → repo-root `.env` → keyring**.

## ADR-SAG-0004 — One check, one request

No splitting oversized inputs. Pre-send byte/token validation on every backend.

## ADR-SAG-0006 — Transport injection

Jev HTTP client lives in `sc-jev`; LLM HTTP in `sc-jev-backends`. Both use
injectable transports for tests.

## ADR-SAG-0010 — CLI thin adapter

`sc-ai` parses argv and prints envelopes; no network code in the binary crate.

## ADR-SAG-0011 — Python via sc-jev-py wheel

Python apps use the **`sc-jev-py`** PyPI extension (PyO3 → `sc-jev`). Optional
thin wrappers (e.g. atm-bd `packages/jev-check`) must not reimplement Jev HTTP
or canonicalization.

## ADR-SAG-0018 — sc-publish kit owns release surface

Release manifests, channel contracts, and publish workflows are **installed**
from [sc-publish](https://github.com/randlee/sc-publish) via
`release/sc-publish-consumer-input.json`. Edit consumer input + re-run
`install.py`; do not hand-edit rendered TOML under `release/`.

## ADR-SAG-0017 — First release surfaces

Phase A first release ships **three** consumer paths: **crates.io** (Rust
libraries + `sc-ai`), **PyPI** (`sc-jev-py` wheel), and the **`sc-ai`** CLI
binary from the same version tag.

## ADR-SAG-0015 — Delimited regions (language-agnostic)

Branch-like inputs use **embedded delimiters** in source files, not AST parsers
inside `sc-jev`. Python/Rust-specific splitting belongs to **consumers**; they
either place markers or pre-extract via other extractors.

### Recommended marker tokens (defaults)

| Role | Substring on the line (after comment sigil) |
| --- | --- |
| Begin | `sc-jev:region:<id>` |
| End | `sc-jev:end:<id>` |

`<id>` matches `[A-Za-z0-9][A-Za-z0-9._-]{0,63}`. A file may contain **many
regions** with **distinct** ids. Duplicate begin/end for the same id, or
overlapping open regions with the **same** id → `CONFIG_INVALID`. Different ids
may appear in any order; **nesting** across ids is allowed only when each pair
is properly closed (inner `end` before outer `end`).

### How authors embed them (by language)

Use a **whole-line comment** whose body starts with the token (leading
whitespace allowed):

| Language | Example begin | Example end |
| --- | --- | --- |
| Python, shell, YAML | `# sc-jev:region:allow` | `# sc-jev:end:allow` |
| Rust, Go, JS, C++ | `// sc-jev:region:deny` | `// sc-jev:end:deny` |
| SQL | `-- sc-jev:region:path` | `-- sc-jev:end:path` |

The extractor does **not** parse ASTs; it scans text at the pinned git blob and
treats a line as a marker when, after trimming, it matches:

```text
^(#|//|--)\s*sc-jev:(region|end):<id>\s*$
```

(Extend the alternation when a consumer documents another comment sigil.)

### Region body

- Content is **lines strictly between** begin and end markers.
- Marker lines themselves are **excluded** from the body.
- Body is UTF-8, normalized to `\n`-terminated text for payload embedding.
- Any **included** id with unclosed region or `end` before `begin` → typed
  extraction error (no send).

### Including vs excluding regions

check.toml may list which ids to extract; **all other marked regions in the
file are ignored** (useful for leaving audit markers in source without feeding
Jev).

```toml
[[extractors]]
type = "delimited-regions"
path = "src/lib.rs"
rev = "HEAD"
regions = ["allow", "deny"]   # optional; omit to take every well-formed pair
```

- **`regions` omitted:** extract every complete `region`/`end` pair in the file.
  Payload map keys sorted lexicographically by `<id>` for stable bytes.
- **`regions` set:** extract only those ids; a listed id missing or incomplete →
  `INPUT_MISSING` / extraction error; unlisted ids in the file are skipped.

### Optional check.toml overrides

```toml
# Rare; defaults are sc-jev:region: and sc-jev:end:
# begin_token = "sc-jev:region:"
# end_token = "sc-jev:end:"
```

Override only the token prefix (`sc-jev:region:` / `sc-jev:end:`), not the
`<id>` grammar, unless a future ADR extends it.

### Why this pattern

- **Greppable** and unlikely to collide with normal prose (`sc-jev:` namespace).
- **Multiple ids per file** with optional **`regions` filter** excludes noise.
- **Comment-line rule** keeps markers out of string literals in most styles.
- Consumers can **generate** markers from AST tools without `sc-jev` knowing Python/Rust.

## ADR-SAG-0013 — Session JSONL via sc-observability v2

Jev session/prompt logging uses **`sc-observability` v2** (`LoggerBuilder`,
file/console sinks). `sc-jev` emits `LogEvent`s through **`SharedSessionLog`**
(see ADR-SAG-0016); only **`sc-ai`** links full `sc-observability`. No bespoke
JSONL writers. Secrets never appear in log lines.

## ADR-SAG-0016 — v2 shim in sc-agent-types (no extra crate)

Do **not** publish a separate observability crate. For the **first release or two**,
**`sc-agent-types::observability::v2`** holds the injectable `SessionLog` port
(aligned with registry `v2::Logger`). Temporary dependency:
`sc-observability-types = "1.5.0"` on **`sc-agent-types` only**; full
`sc-observability = "1.5.0"` only on **`sc-ai`**. When upstream v2 ships, **delete
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
| API-direct Anthropic / OpenAI (Haiku/Luna in `sc-jev` check flow; future `sc-ai` agents) | **Wanted** — Phase A and Phase B (standalone) |
| Subprocess JSON bridges to claude-code, codex-cli, cursor-cli | **Out of scope** |

Future agent crates sit beside `sc-jev` and depend on `sc-agent-types` +
`sc-agent-secrets`; they do not duplicate Jev HTTP or payload rules from `sc-jev` when running
comparison flows.

## Related documents

- [`requirements.md`](requirements.md) — REQ-SAG ids
- [`design.md`](design.md) — goals and non-goals
- [`framework-charter.md`](framework-charter.md) — multi-consumer Rust posture
- [`plans/phase-a/plan.md`](plans/phase-a/plan.md) — Phase A sprints and boundaries
- Per-crate: `docs/sc-agent-types/` (incl. temporary observability shim), `docs/sc-jev/`, etc.
