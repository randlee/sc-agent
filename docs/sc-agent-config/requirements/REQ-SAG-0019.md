# Requirement: Configuration file locations and lookup order

**Status:** Approved  
**Version:** 0.7.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0019: Configuration file locations and lookup order

**Status:** Approved  

### Requirement Statement

Definitions: A CLI is an sc-agent command-line program that reads configuration, such as `decidr`; `<cli>` is its program name. `sitr` reads no configuration. The shared file is `sc-agent.json`. A CLI file is `<cli>.json`; its optional override file is `<cli>.local.json`. The global folder is `~/.sc-agent/`, the user's home directory joined with `.sc-agent` on every platform, relocated to the folder named by `SC_AGENT_HOME` when that variable is nonempty. An explicit `--config-dir` (or wheel `config_dir`) takes precedence over that variable. `<worktree>` is the root of the git work tree containing the folder the CLI runs in. The local folder is `<worktree>/.sc/sc-agent/`. A layer is one source of a setting's value. A typed error is an error value (REQ-SAG-0016). The key path is the dotted path of a configuration key within its file, such as `laya.port`. The shared settings are the defaults for configuration-reading CLIs that `sc-agent.json` holds. A model request is a request that asks a model for a reply, as `decidr send` does. The settings table has one row per setting and the columns `path`, `value` and `source`. The source of a value is the layer it comes from, shown as the flag, the environment variable name, the file path or `default`. A missing-setting error is a typed error that names the setting and every layer that can set it. A local endpoint is a keyless provider whose host and port come from configuration: laya's endpoint and the two decider endpoints (ADR-SAG-0019 decision 13). A built-in provider is a provider in sc-agent's Rust provider table. A model entry is the configuration object `models.<alias>`, with the optional string keys `provider` and `id`. The default model is the setting `model`: the selector a model request uses when the caller names none.

**MUST statements:**

- For each setting, each CLI MUST use the value from the first of these nine layers, in this order, that sets it:
  1. the CLI flag;
  2. the environment variable (REQ-SAG-0020);
  3. `<cli>.local.json` in the local folder;
  4. `<cli>.json` in the local folder;
  5. `sc-agent.json` in the local folder;
  6. `<cli>.local.json` in the global folder;
  7. `<cli>.json` in the global folder;
  8. `sc-agent.json` in the global folder;
  9. the built-in default.
- When a required setting has no built-in default and no layer sets it, the CLI MUST fail with a missing-setting error.
- `sc-agent.json` MUST accept the shared default for `project`.
- `sc-agent.json` MUST accept the shared default for `workers`.
- `sc-agent.json` MUST accept the shared default for `cache`.
- `sc-agent.json` MUST accept the shared default for `model`.
- `sc-agent.json` MUST accept model entries under `models`.
- `sc-agent.json` MUST accept the host and port of each local endpoint.
- `model` MUST be a string selector with the built-in default `jev`.
- A model request that names no model MUST use the `model` setting.
- A local endpoint MUST be configured by `<endpoint>.host` and `<endpoint>.port`, such as `laya.host` and `laya.port`.
- A model entry's `provider` MUST name a built-in provider.
- A model entry whose `provider` names no built-in provider MUST produce a typed error naming the file and key path.
- A model entry MUST be set only in a configuration file; no flag or environment variable sets a `models.<alias>` key.
- A model entry's `provider` MUST select the provider for its alias.
- A model entry's `id` MUST select the model id for its alias when its provider is not a local endpoint; a local endpoint's model keeps its fixed id.
- `cache` MUST be a boolean with the default `true` (REQ-SAG-0703).
- `decidr doctor` MUST report `cache`, `model`, every model entry and every local endpoint key in the settings table.
- `<cli>.json` MUST accept that CLI's settings.
- `<cli>.local.json` MUST accept that CLI's settings.
- `<cli>.json` MUST accept the shared settings.
- `<cli>.local.json` MUST accept the shared settings.
- An unknown file key MUST produce a typed error naming that file and key path.
- `<cli>.json` in the local folder MUST accept the same keys as `<cli>.json` in the global folder.
- `sc-agent.json` in the local folder MUST accept the same keys as `sc-agent.json` in the global folder.
- When the CLI does not run inside a git work tree, layers 3 through 5 MUST be skipped.
- Each configuration file MUST use JSON format.
- Each CLI MUST have a `<cli> doctor` command, such as `decidr doctor`.
- `<cli> doctor` MUST report the settings table.
- In each row of the settings table, the `source` cell MUST show the source of that row's value.
- `<cli> doctor` MUST report the path of every file in the nine layers.
- For each file in the nine layers, `<cli> doctor` MUST report whether it exists.
- When a CLI fails to load a configuration file, its error MUST name the file, and the key path when the failure is at a key.

**MUST NOT statements:**

- A CLI MUST NOT read another CLI's `<cli>.json`.
- While it loads configuration, a CLI MUST NOT write to any configuration file.
- `<cli> doctor` MUST NOT send a model request.
- `<cli> doctor` MUST NOT write a file.
- Configuration MUST NOT add a provider, a credential name or a remote base address.

### Rationale

One file per CLI lets a CLI be added without changing another CLI's file. The shared file holds shared defaults that a CLI file may override. The local folder lets worktrees of one repository hold different settings, such as different phases, and keeps sc files under `.sc/`.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): global model configuration `models.<alias> = {provider, id}` overrides the built-in table; configuration may only choose among built-in providers and set ids; laya and decider are local endpoints configured by host and port; `cache` is a configuration key (REQ-SAG-0703).

Owner ruling 2026-11-24 (supersedes the optional or configured local model id and the omitted `model` above): laya and decider have fixed model ids in the model table, like every other model. laya requires `laya:en`. decider has two rows, `decider-2b-v11` (port 18000) and `decider-4b-v2.1` (port 18001), both released 2026-09-24, and `decider` resolves by the same rule as `haiku`. The decider servers accept and ignore `model`, so every request carries the resolved id. Cache keys always use the resolved fixed id. A model entry does not supply a local model's id.

Owner ruling 2026-11-24 (plan review): with no `--model`, the model is `jev`, and the default is overridable by a configuration variable. The setting is `model`, built-in default `jev`, environment variable `DECIDR_MODEL` (REQ-SAG-0020).

### Success Criteria

**Acceptance Criteria:**

- [ ] In either folder, `<cli>.local.json` overrides `<cli>.json`; any local-folder value overrides every global-folder value.
- [ ] With `SC_AGENT_HOME` set, global files are read from that folder and doctor reports those paths; an explicit configuration directory overrides it.

- [ ] With `workers` set to 4 in the global `decidr.json` and 2 in the local `decidr.json`, decidr uses 2 and reports the local `decidr.json` as its source.
- [ ] With `project` set only in the global `sc-agent.json`, decidr uses it.
- [ ] With a setting in the local `sc-agent.json` and the global `decidr.json`, decidr uses the local `sc-agent.json` value.
- [ ] A flag value beats the same setting in the environment and in every file.
- [ ] Outside a git work tree, a `.sc/sc-agent/decidr.json` in the current folder is not read.
- [ ] With `"project": "phase-a"` in `<worktree>/.sc/sc-agent/decidr.json`, `decidr doctor` reports the row `project | phase-a | <worktree>/.sc/sc-agent/decidr.json`.
- [ ] With `"project": "phase-a"` only in the local `sc-agent.json`, `decidr doctor` reports the `project` row with that file as its source.
- [ ] A malformed local `decidr.json` fails the load with an error naming that file.
- [ ] With `"models": {"luna": {"provider": "nous"}}` in the global `sc-agent.json`, `luna` resolves to the `nous` provider, and `decidr doctor` reports the row `models.luna.provider | nous | <that file>`.
- [ ] A model entry whose `provider` is not a built-in provider fails the load with a typed error naming the file and `models.<alias>.provider`.
- [ ] With a decider endpoint's host and port set, `decidr doctor` reports both endpoint rows and `cache | true | default`.
- [ ] With `"models": {"laya": {"id": "x"}}` in the global `sc-agent.json`, a laya request still carries `laya:en`.
- [ ] With no `model` set in any layer, `decidr doctor` reports `model | jev | default`, and `decidr send` with no `--model` resolves `jev`; with `"model": "haiku"` in the global `sc-agent.json`, `decidr send` with no `--model` resolves `haiku`.

### Dependencies

**Related:**

- REQ-SAG-0013
- REQ-SAG-0016
- REQ-SAG-0020
- REQ-SAG-0021
- ADR-SAG-0020
