# Decision: Configuration has a shared scope and one scope per CLI, each global and per worktree

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-18  

---

## ADR-SAG-0020: Configuration has a shared scope and one scope per CLI, each global and per worktree

**Status:** Approved  

### Related Documents

**Requirements:**

- REQ-SAG-0013
- REQ-SAG-0019
- REQ-SAG-0020
- REQ-SAG-0021

**Architecture Decisions:**

- ADR-SAG-0021

### Context

sc-agent ships two CLIs today, `decidr` and `sitr`. CLI launchers for other agent harnesses, such as Claude Code, Codex CLI, Hermes and pi-agent, will be added as their peers. Apps such as plan-anchor and judge-jev wrap a CLI the way a named agent (`.claude/agents/<agent>.md`) wraps a harness. Agent-level settings, for agents such as `lai`, `sai`, `sanity`, `plan-anchor` and `judge-jev`, will later be provided by the CLI launchers.

Before this decision, every CLI read one shared file pair, `~/.sc-agent/default.json` and `~/.sc-agent/local.json`, with `SC_AGENT__<KEY>` environment overrides. That layout has no per-CLI settings and no per-repository layer: two worktrees of one repository cannot hold different settings, and a setting meant for one CLI applies to all of them.

**Problem Statement:**

Each CLI needs its own settings, some settings must apply to every CLI, and a repository or worktree must be able to override both.

### Decision

Configuration has two kinds of scope and two locations.

- The shared scope is the file `sc-agent.json`. It holds settings that apply to every CLI, such as the project/phase (ADR-SAG-0021) and the log root.
- Each CLI that reads configuration has its own scope, the file `<cli>.json` named after the CLI: `decidr.json`, and the same pattern for every CLI added later. `sitr` reads no configuration.
- The global location is `~/.sc-agent/`, the user's home directory joined with `.sc-agent` on every platform.
- The local location is `<worktree>/.sc/sc-agent/`, where `<worktree>` is the root of the git work tree the CLI runs in. It holds the same file names with the same keys.

The CLIs and the files each one reads are these, and no others:

| CLI | Global files | Local files | Environment prefix |
|---|---|---|---|
| `decidr` | `~/.sc-agent/decidr.json`, `~/.sc-agent/sc-agent.json` | `<worktree>/.sc/sc-agent/decidr.json`, `<worktree>/.sc/sc-agent/sc-agent.json` | `DECIDR_`, plus `SC_PROJECT` |
| `sitr` | none | none | none |

Every CLI has a row, and every cell is filled; a CLI that reads no file of a kind says `none`. The table is updated when a CLI is added.

A setting is looked up in seven layers, and the first layer that sets it wins:

1. the CLI flag;
2. the environment variable;
3. the local `<cli>.json`;
4. the local `sc-agent.json`;
5. the global `<cli>.json`;
6. the global `sc-agent.json`;
7. the built-in default, or a typed error when the setting is required and has no default.

Environment variables are named after the CLI: `DECIDR_<SETTING>`, with the parts of a nested key joined by a double underscore (`DECIDR_LOCAL_RUNNERS__LAYA__PORT`). A setting with a wider audience than sc-agent may have its own name; the project/phase is `SC_PROJECT` (ADR-SAG-0021).

Configuration files are JSON. TOML will be added when sc-config supports it (randlee/sc-runtime#152). `default.json`, `local.json` and the `SC_AGENT__` prefix are removed; nothing reads them. No migration is provided, because no release has users of the old layout.

**Key Principles:**

- A narrower location beats a wider one: local beats global.
- Within one location, a CLI's own file beats the shared file.
- sc ecosystem files in a repository live under `.sc/`, so a repository needs one `.gitignore` entry rather than one per tool.

### Rationale

One file per CLI keeps each CLI's settings separate and lets a new CLI be added without touching another CLI's file. The shared file gives one place for settings every CLI must agree on. The local location under `.sc/` lets each worktree differ, which matters because worktrees of one repository often work on different phases at the same time. Env vars named after the CLI keep one CLI's override from reaching another.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Consequences

**Positive:**

- A CLI added later gets its own global and local file without a schema change to any other file.
- A worktree can override any setting without editing the user's global files.

**Negative:**

- A setting can now come from seven places; `decidr doctor` reports a `setting | value | source` table.
- Existing `default.json` or `local.json` files are ignored and must be moved by hand.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting."

### Alternatives Considered

#### Alternative 1: Keep one shared file pair with a section per CLI

**Description:**

Keep `default.json` and `local.json` and give each CLI a section inside them.

**Pros:**

- One file to edit.

**Cons:**

- Every CLI's schema lives in one file, and adding a CLI changes it.
- `local.json` is a per-machine file in the same folder, not a repository layer.

**Why Rejected:**

It has no per-repository layer, and every CLI would share one file.

#### Alternative 2: Local files at `<worktree>/.sc-agent/`

**Description:**

Mirror the global folder name at the worktree root.

**Pros:**

- The same folder name in both locations.

**Cons:**

- Each sc tool would add its own dot folder at the repository root.

**Why Rejected:**

sc ecosystem files are consolidated under `.sc/`.

#### Alternative 3: Accept JSON, TOML and YAML

**Description:**

Read whichever of the three formats is present.

**Pros:**

- Users choose the format.

**Cons:**

- sc-config reads JSON only, and three formats need a rule for files that conflict.

**Why Rejected:**

One format now; TOML when sc-config supports it.
