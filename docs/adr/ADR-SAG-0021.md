# Decision: General logs and session logs live in separate trees, and the session identity is a fixed agent role plus a required project/phase

**Status:** Proposed  
**Version:** 0.1.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-18  

---

## ADR-SAG-0021: General logs and session logs live in separate trees, and the session identity is a fixed agent role plus a required project/phase

**Status:** Proposed  

### Related Documents

**Requirements:**

- REQ-SAG-0003
- REQ-SAG-0004
- REQ-SAG-0005
- REQ-SAG-0021

**Decisions:**

- ADR-SAG-0002
- ADR-SAG-0019
- ADR-SAG-0020

### Context

Session logs are the long-term record the owner analyses and will later load into SQLite and sync to a server (ADR-SAG-0002). They must be separable by repository, agent role and project/phase.

Before this decision, the general logs (`<log_root>/logs/<cli>.log.jsonl`) and the session logs (`<log_root>/logs/<repo>/<agent_role>/<project>/`) shared the `logs/` folder. The generated consumer clients passed the two identity fields the wrong way round. plan-anchor passed `project` `plan-anchor` with `agent_role` `judge` or `tournament`, and judge-jev passed `project` `judge-jev` with `agent_role` `quality`. As a result, every session of one tool in one repository landed in one file whatever phase it ran under, which breaks REQ-SAG-0004's definitions. Agents work by phase, so the project/phase is the main way their sessions are grouped.

**Problem Statement:**

Session logs must never mix with general logs, and every session must be filed under the agent that made it and the project/phase it belongs to.

### Decision

Logs live under `<log_root>`, which defaults to `~/.sc-agent` and is set by `log_root` in the shared configuration (ADR-SAG-0020).

- Each CLI's general log is `<log_root>/logs/<cli>/<cli>.log.jsonl`, for example `logs/decidr/decidr.log.jsonl`. Each CLI writes only its own general log.
- Session logs are `<log_root>/sessions/<repo>/<agent_role>/<project>/sessions.jsonl`. `sessions/` sits beside `logs/`, not under it, and holds nothing but session logs.

The CLIs and the logs each one writes are these, and no others:

| CLI | General log | Session logs |
|---|---|---|
| `decidr` | `<log_root>/logs/decidr/decidr.log.jsonl` | `<log_root>/sessions/<repo>/<agent_role>/<project>/sessions.jsonl` |
| `sitr` | `<log_root>/logs/sitr/sitr.log.jsonl` | none |

The logs are written by the Rust code. A PyO3 binding is not a CLI and has no row: a caller of the binding may inject its own general and session loggers (REQ-SAG-0008), and with none injected the binding writes the default logs of the CLI it binds, at the paths in that CLI's row.

Every CLI has a row, and every cell is filled; a CLI that writes no log of a kind says `none`. A plan that adds a CLI, or changes a log a CLI writes, is not approved until it includes a new version of this ADR with that row.

The session identity is fixed as follows.

- `agent_role` is the named agent or app that makes the request, for example `plan-anchor`, `judge-jev`, `lai` or `sanity`. It is required. A generated consumer client hardcodes it to its product name; it is not a user setting.
- `project` is the project/phase name, for example `sc-agent-phase-b`. It is required, and a session without one is refused with a typed error that names every place it can be set. It is resolved by the seven-layer lookup of ADR-SAG-0020: the `--project` flag, the `SC_PROJECT` environment variable, the local `<cli>.json`, the local `sc-agent.json`, the global `<cli>.json`, then the global `sc-agent.json`. It has no default.
- In code the setting is named `project`. Every CLI usage text, help text and error describes it as "project/phase".

**Key Principles:**

- Session logs and general logs are never written to the same folder tree.
- A session is never filed under a guessed project/phase.

### Rationale

A separate `sessions/` tree lets the owner's tools, and the later SQLite load, read every session without skipping general logs. A per-CLI folder under `logs/` keeps each CLI's diagnostics apart as more CLIs are added. Hardcoding the agent role in the generated client stops an app from filing its sessions under another agent's name. Requiring the project/phase stops sessions of different phases from merging into one file. `SC_PROJECT` is not prefixed with a CLI name because the project/phase applies to every CLI and to tools outside sc-agent.

### Consequences

**Positive:**

- `sessions/<repo>/<agent_role>/<project>/` selects one agent's work in one phase with a path glob.
- Adding a CLI adds one folder under `logs/` and nothing under `sessions/`.

**Negative:**

- Every caller must supply a project/phase before its first request.
- Session files written under the earlier layout are not moved by sc-agent.

### Alternatives Considered

#### Alternative 1: Keep session logs under `logs/`

**Description:**

Keep `<log_root>/logs/<repo>/<agent_role>/<project>/` beside the general log files.

**Pros:**

- No path change.

**Cons:**

- Session and general logs mix in one tree.

**Why Rejected:**

Session and general logs must not mix.

#### Alternative 2: Default the project/phase

**Description:**

File sessions without a project/phase under a fixed name such as `unassigned`.

**Pros:**

- Existing scripts keep working.

**Cons:**

- Sessions of different phases merge into one file.

**Why Rejected:**

A session is never filed under a guessed project/phase.

#### Alternative 3: Name the variable `DECIDR_PROJECT`

**Description:**

Name the project/phase variable after the decidr CLI, like other settings.

**Pros:**

- Matches the per-CLI naming rule.

**Cons:**

- Each CLI would need its own variable for one shared value.

**Why Rejected:**

The project/phase applies to every CLI and to tools outside sc-agent.
