# Requirement: Configuration file locations and lookup order

**Status:** Proposed  
**Version:** 0.2.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0019: Configuration file locations and lookup order

**Status:** Proposed  

### Requirement Statement

Definitions: A CLI is an sc-agent command-line program, such as `decidr` or `sitr`; `<cli>` is its program name. The shared file is `sc-agent.json`. A CLI file is `<cli>.json`. The global folder is `~/.sc-agent/`, the user's home directory joined with `.sc-agent` on every platform. `<worktree>` is the root of the git work tree containing the folder the CLI runs in. The local folder is `<worktree>/.sc/sc-agent/`. A layer is one source of a setting's value. A typed error is an error detail (`ErrorDetail`, REQ-SAG-0016). The key path is the dotted path of a configuration key within its file, such as `runners.local.port`. The shared settings are the settings that apply to every CLI and that `sc-agent.json` holds. A model request is a request that asks a model for a reply, as `decidr send` does. The settings table has one row per setting and the columns `setting`, `value` and `source`. The source of a value is the layer it comes from, shown as the flag, the environment variable name, the file path or `default`. A missing-setting error is a typed error that names the setting and every layer that can set it.

**MUST statements:**

- For each setting, each CLI MUST use the value from the first of these seven layers, in this order, that sets it:
  1. the CLI flag;
  2. the environment variable (REQ-SAG-0020);
  3. `<cli>.json` in the local folder;
  4. `sc-agent.json` in the local folder;
  5. `<cli>.json` in the global folder;
  6. `sc-agent.json` in the global folder;
  7. the built-in default.
- When a required setting has no built-in default and no layer sets it, the CLI MUST fail with a missing-setting error.
- `sc-agent.json` MUST accept only settings that apply to every CLI.
- `<cli>.json` MUST accept that CLI's settings and the shared settings.
- `<cli>.json` in the local folder MUST accept the same keys as `<cli>.json` in the global folder.
- `sc-agent.json` in the local folder MUST accept the same keys as `sc-agent.json` in the global folder.
- When the CLI does not run inside a git work tree, layers 3 and 4 MUST be skipped.
- Configuration files MUST be JSON.
- Each CLI MUST have a `<cli> doctor` command (`decidr doctor`, `sitr doctor`).
- `<cli> doctor` MUST report the settings table.
- In each row of the settings table, the `source` cell MUST show the source of that row's value.
- `<cli> doctor` MUST report the path of every file in the seven layers.
- For each file in the seven layers, `<cli> doctor` MUST report whether it exists.
- When a CLI fails to load a configuration file, its error MUST name the file and the key path.

**MUST NOT statements:**

- A CLI MUST NOT read another CLI's `<cli>.json`.
- While it loads configuration, a CLI MUST NOT write to any configuration file.
- `<cli> doctor` MUST NOT send a model request.
- `<cli> doctor` MUST NOT write a file.

### Rationale

One file per CLI lets a CLI be added without changing another CLI's file. The shared file holds settings every CLI must agree on. The local folder lets worktrees of one repository hold different settings, such as different phases, and keeps sc files under `.sc/`.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting."

### Success Criteria

**Acceptance Criteria:**

- [ ] With `workers` set to 4 in the global `decidr.json` and 2 in the local `decidr.json`, decidr uses 2 and reports the local `decidr.json` as its source.
- [ ] With `project` set only in the global `sc-agent.json`, decidr and sitr both use it.
- [ ] With a setting in the local `sc-agent.json` and the global `decidr.json`, decidr uses the local `sc-agent.json` value.
- [ ] A flag value beats the same setting in the environment and in every file.
- [ ] Outside a git work tree, a `.sc/sc-agent/decidr.json` in the current folder is not read.
- [ ] A setting in `sitr.json` has no effect on decidr.
- [ ] With `"project": "phase-a"` in `<worktree>/.sc/sc-agent/decidr.json`, `decidr doctor` reports the row `project | phase-a | <worktree>/.sc/sc-agent/decidr.json`.
- [ ] With `"project": "phase-a"` only in the local `sc-agent.json`, `decidr doctor` and `sitr doctor` report the same `project` row.
- [ ] A malformed local `decidr.json` fails the load with an error naming that file.

### Dependencies

**Related:**

- REQ-SAG-0013
- REQ-SAG-0016
- REQ-SAG-0020
- REQ-SAG-0021
- ADR-SAG-0020
