# Requirement: Configuration file locations and lookup order

**Status:** Proposed  
**Version:** 0.1.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  

---

## REQ-SAG-0019: Configuration file locations and lookup order

**Status:** Proposed  

### Requirement Statement

Definitions: A CLI is an sc-agent command-line program, such as `decidr` or `sitr`; `<cli>` is its program name. The shared file is `sc-agent.json`. A CLI file is `<cli>.json`. The global folder is `~/.sc-agent/`, the user's home directory joined with `.sc-agent` on every platform. `<worktree>` is the root of the git work tree containing the folder the CLI runs in. The local folder is `<worktree>/.sc/sc-agent/`. A layer is one source of a setting's value.

**MUST statements:**

- Each CLI MUST read its settings from these seven layers, and the value MUST come from the first layer, in this order, that sets it:
  1. the CLI flag;
  2. the environment variable (REQ-SAG-0020);
  3. `<cli>.json` in the local folder;
  4. `sc-agent.json` in the local folder;
  5. `<cli>.json` in the global folder;
  6. `sc-agent.json` in the global folder;
  7. the built-in default.
- A required setting with no built-in default that no layer sets MUST fail with a typed error naming the setting and every layer that can set it.
- `sc-agent.json` MUST accept only settings that apply to every CLI, and `<cli>.json` MUST accept that CLI's settings and the shared settings.
- A file in the local folder MUST accept the same keys as the file of the same name in the global folder.
- When the CLI does not run inside a git work tree, layers 3 and 4 MUST be skipped.
- Configuration files MUST be JSON.
- `<cli> doctor` MUST report, for every setting, its value and the layer that supplied it, and the path of every file in the seven layers with whether it exists.
- A failed load MUST name the file and the key path in its error.

**MUST NOT statements:**

- A CLI MUST NOT read `default.json` or `local.json`.
- A CLI MUST NOT read another CLI's `<cli>.json`.
- A CLI MUST NOT write to any configuration file while it loads configuration.

### Rationale

One file per CLI lets a CLI be added without changing another CLI's file. The shared file holds settings every CLI must agree on. The local folder lets worktrees of one repository hold different settings, such as different phases, and keeps sc files under `.sc/`.

### Success Criteria

**Acceptance Criteria:**

- [ ] With `workers` set to 4 in the global `decidr.json` and 2 in the local `decidr.json`, decidr uses 2 and reports the local `decidr.json` as its source.
- [ ] With `log_root` set only in the global `sc-agent.json`, decidr and sitr both use it.
- [ ] With a setting in the local `sc-agent.json` and the global `decidr.json`, decidr uses the local `sc-agent.json` value.
- [ ] A flag value beats the same setting in the environment and in every file.
- [ ] Outside a git work tree, a `.sc/sc-agent/decidr.json` in the current folder is not read.
- [ ] A `~/.sc-agent/default.json` that sets `workers` has no effect.
- [ ] A setting in `sitr.json` has no effect on decidr.
- [ ] A malformed local `decidr.json` fails the load with an error naming that file.

### Dependencies

**Related:**

- REQ-SAG-0013
- REQ-SAG-0020
- REQ-SAG-0021
- ADR-SAG-0020
