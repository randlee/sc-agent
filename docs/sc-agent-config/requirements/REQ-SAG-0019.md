# Requirement: Configuration file locations and lookup order

**Status:** Approved  
**Version:** 0.8.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0019: Configuration file locations and lookup order

**Status:** Approved  

### Requirement Statement

Definitions: A CLI is an sc-agent command-line program that reads configuration, such as `decidr`; `<cli>` is its program name. `sitr` reads no configuration. The shared file is `sc-agent.json`. A CLI file is `<cli>.json` or its optional override file `<cli>.local.json`. The global folder is `~/.sc-agent/`, the user's home directory joined with `.sc-agent` on every platform, relocated to the folder named by `SC_AGENT_HOME` when that variable is nonempty. An explicit `--config-dir` (or wheel `config_dir`) takes precedence over that variable. `<worktree>` is the root of the git work tree containing the folder the CLI runs in. The local folder is `<worktree>/.sc/sc-agent/`. A layer is one source of a setting's value. The local-folder layers are layers 3, 4 and 5. A typed error is an error value (REQ-SAG-0016). The key path is the dotted path of a configuration key within its file, such as `laya.port`. The shared settings are the defaults for configuration-reading CLIs that `sc-agent.json` holds. A model request is a request that asks a model for a reply, as `decidr send` does. The settings table has one row per setting and the columns `path`, `value` and `source`. The source of a value is the layer it comes from, shown as the flag, the environment variable name, the file path or `default`. A missing-setting error is a typed error that names the setting and every layer that can set it. A local endpoint is a keyless provider that runs a local model, such as laya and decider; its address is its host and port. A paid API is a remote model provider that requires an API key; its key is sent only to that provider's built-in base address. The failure location of a load error is the file path and the key path. A model entry is the configuration object `models.<alias>`; it adds a model row named `<alias>`.

**MUST statements:**

- When a required setting has no built-in default and no layer sets it, the CLI MUST return a missing-setting error.
- Each configuration file MUST accept the shared settings.
- A CLI file MUST accept that CLI's settings.
- An unknown key in a configuration file MUST cause a typed load error.
- The `cache` setting MUST default to `true`.
- A model entry MUST add one row to the model table.
- A model entry MUST override the model its name selects.
- A local endpoint's address MUST be settable in configuration.
- When the CLI does not run inside a git work tree, it MUST skip the local-folder layers.
- Each CLI's `<cli> doctor` command, such as `decidr doctor`, MUST report the settings table.
- In each row of the settings table, the `source` cell MUST show the source of that row's value.
- `<cli> doctor` MUST report the path of every file in the nine layers.
- For each file in the nine layers, `<cli> doctor` MUST report whether it exists.
- When a CLI fails to load a configuration file, its error MUST name the failure location.

**MUST NOT statements:**

- A CLI MUST NOT read another CLI's `<cli>.json`.
- While it loads configuration, a CLI MUST NOT write to any configuration file.
- `<cli> doctor` MUST NOT send a model request.
- `<cli> doctor` MUST NOT write a file.
- Configuration MUST NOT change a paid API's base address.

### Rationale

One file per CLI lets a CLI be added without changing another CLI's file. The shared file holds shared defaults that a CLI file may override. The local folder lets worktrees of one repository hold different settings, such as different phases, and keeps sc files under `.sc/`. A settable local endpoint host lets a local model run on another computer. Configuration may add model rows because built-in rows go stale; it may not redirect a paid API's key.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] CLI run (`decidr doctor`): with `SC_AGENT_HOME` set, global files are read from that folder and doctor reports those paths; an explicit configuration directory overrides it.
- [ ] CLI run (`decidr doctor`): with `workers` set to 4 in the global `decidr.json` and 2 in the local `decidr.json`, decidr uses 2 and reports the local `decidr.json` as its source.
- [ ] Unit test (shared settings): with `project` set only in the global `sc-agent.json`, decidr uses it.
- [ ] Unit test (missing setting): with a required setting set in no layer, the load fails with a missing-setting error.
- [ ] Unit test (CLI file): a `decidr` setting in `decidr.local.json` is accepted.
- [ ] Unit test (unknown key): an unknown key in the local `decidr.json` fails the load with a typed error naming that file and key path.
- [ ] Unit test (work tree): outside a git work tree, a `.sc/sc-agent/decidr.json` in the current folder is not read.
- [ ] CLI run (`decidr doctor`): with `"project": "phase-a"` in `<worktree>/.sc/sc-agent/decidr.json`, `decidr doctor` reports the row `project | phase-a | <worktree>/.sc/sc-agent/decidr.json`.
- [ ] CLI run (`decidr doctor`): with `"project": "phase-a"` only in the local `sc-agent.json`, `decidr doctor` reports the `project` row with that file as its source.
- [ ] CLI run (`decidr doctor`): doctor lists the path of each of the nine layers' files and whether each exists.
- [ ] Unit test (load error): a malformed local `decidr.json` fails the load with an error naming that file.
- [ ] Unit test (other CLI): a key in a `sitr.json` beside `decidr.json` does not reach decidr's settings.
- [ ] Unit test (no write): loading configuration leaves every configuration file's bytes and modification time unchanged.
- [ ] Unit test (doctor offline): `decidr doctor` run through its Rust entry point with a provider stub that fails on any request completes without calling the stub.
- [ ] Unit test (doctor no write): `decidr doctor` run in an empty temporary home and work tree leaves both directory listings unchanged.
- [ ] Unit test (model entry): with `"models": {"luna": {"provider": "nous", "id": "openai/gpt-6.1-luna", "released": "2026-11-20"}}` in the global `sc-agent.json`, the model table has a `luna` row with id `openai/gpt-6.1-luna` and provider `nous`.
- [ ] Unit test (paid API base address): a model entry whose `provider` is not a built-in provider fails the load with a typed error.
- [ ] Unit test (local endpoint): with `laya.host` set to another computer's address in the global `sc-agent.json`, the resolved laya endpoint uses that host.
- [ ] Unit test (cache default): with no `cache` set in any layer, the resolved `cache` setting is `true`.

### Dependencies

**Related:**

- REQ-SAG-0013
- REQ-SAG-0016
- REQ-SAG-0020
- REQ-SAG-0021
- ADR-SAG-0020
