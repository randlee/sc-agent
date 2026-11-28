# Anchors - phase-m

Constraints captured during planning. Each anchor is a testable statement written by the orchestrator and approved by the user; the user's verbatim words are kept beneath it as provenance. Every anchor is judged at the planning gate and again at the development gate. Edit `anchors.json`, not this file; this file is rendered.

Intent: `docs/plans/phase-m/intent.md`

| ID | Status | Summary | Stated |
| --- | --- | --- | --- |
| ANC-001 | locked | C01 The decidr model name `jev` must select the newest Jev model | 2026-11-26 |
| ANC-002 | locked | C02 Until SQLite schema 1.0, sc-agent, judge-jev and plan-anchor | 2026-11-26 |
| ANC-003 | locked | C03 When phase j removes a backward-compatible reader, its code  | 2026-11-26 |
| ANC-004 | locked | C04 sc-agent must impose no request input size limit of its own  | 2026-11-26 |
| ANC-005 | locked | C05 In production runs, every logger sc-agent creates by default | 2026-11-26 |
| ANC-006 | locked | C06 sc-agent must not change a logger the caller injects, so tes | 2026-11-26 |
| ANC-007 | locked | C07 Phase j's non-test Rust code, as counted by scripts/gate/rew | 2026-11-26 |
| ANC-008 | locked | C08 In phase-j code, related values passed to several methods mu | 2026-11-26 |
| ANC-009 | locked | C10 The sc-agent model table must accept extra model rows from c | 2026-11-26 |
| ANC-010 | locked | C11 Phase-j code must not copy and paste logic between files or  | 2026-11-26 |
| ANC-011 | locked | C14 Every operation the decidr CLI offers must also be callable  | 2026-11-26 |
| ANC-012 | locked | one decision trait, one implementation per wire format | 2026-11-26 |
| ANC-013 | locked | LLM trait, JSON in/out, one impl per wire format | 2026-11-26 |
| ANC-014 | locked | LLM-as-Jev system prompt; luna/haiku config overrides | 2026-11-26 |
| ANC-015 | locked | decidr-wide rules live only in sc-agent | 2026-11-26 |
| ANC-016 | locked | logger injection is test-only for derivative tools | 2026-11-26 |
| ANC-017 | locked | no flaky tests | 2026-11-27 |
| ANC-018 | locked | live integration = unit tests run live, manually | 2026-11-27 |
| ANC-019 | locked | Python 3.11+ | 2026-11-27 |
| ANC-020 | locked | no second logger when one is injected | 2026-11-27 |
| ANC-021 | locked | Shared code sits in the lowest layer using it | 2026-11-27 |
| ANC-022 | locked | Local model URL and id come only from config | 2026-11-27 |
| ANC-023 | locked | Session rows go to SQLite, not sessions.jsonl | 2026-11-27 |
| ANC-024 | locked | General log follows sc-observability retention | 2026-11-27 |
| ANC-025 | locked | sc-agent-schema holds structs and SQL text only | 2026-11-27 |
| ANC-026 | locked | Core table SQL defined only in sc-agent-schema | 2026-11-27 |
| ANC-027 | locked | Requests stored once; replies in responses table | 2026-11-27 |
| ANC-028 | locked | Cache key is request sha256 and model, not run_id | 2026-11-27 |
| ANC-029 | locked | --resume lives in sc-llm | 2026-11-27 |
| ANC-030 | locked | Retry and holdoff take provider rules as parameters | 2026-11-27 |
| ANC-031 | locked | Tool results are stored in SQLite | 2026-11-27 |
| ANC-032 | locked | Tool tables versioned apart; none in sc-agent-py | 2026-11-27 |

## ANC-001

Anchor: The decidr model name `jev` must select the newest Jev model in sc-agent's model table unless a config file sets another default, so callers get the latest Jev without a code change.

> I want 'jev' to select latest, config can override default.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 2.

Summary: C01 The decidr model name `jev` must select the newest Jev model

Scope: adds or changes model resolution, the model table or the jev alias in sc-agent or decidr

Deterministic check to pair with the development gate: unit test: resolve("jev") == first Jev row of the date-sorted table; with config default set, == configured id

Beads: anchor `agt-jzag`, plan questions `agt-jzag.1`, dev questions `agt-jzag.2`

Fixtures:
- pass: With no config default set, `--model jev` resolves to the newest Jev row in the model table.
- fail: The jev alias is pinned in code to jev-2025-06, and a code change is needed to move it.

## ANC-002

Anchor: Until SQLite schema 1.0, sc-agent, judge-jev and plan-anchor must read only the record schema version they write and contain no reader for an older version, so read and write stay in step.

> We need to read what we are writing, no back compat until SQLite 1.0

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 2.

Summary: C02 Until SQLite schema 1.0, sc-agent, judge-jev and plan-anchor

Scope: adds or changes code that reads batch result lines, session rows or run records

Deterministic check to pair with the development gate: `rg -n 'jev-batch-result/1\

Beads: anchor `agt-8y0m`, plan questions `agt-8y0m.1`, dev questions `agt-8y0m.2`

Fixtures:
- pass: The session reader accepts only the schema version the writer emits and errors on any other version.
- fail: A v1 fallback reader is added so old batch result lines still load.

## ANC-003

Anchor: When a backward-compatible reader is removed, its code and its tests must both be deleted, because backward-compatible reads are baselined later in SQLite.

> remove code & tests, don't just remove. We will baseline backward compatible reads in SQLite.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 3.

Summary: C03 When phase j removes a backward-compatible reader, its code 

Scope: removes or changes a reader of an earlier record schema version, or its tests

Deterministic check to pair with the development gate: the sprint diff deletes the reader's tests and fixtures; `rg` finds no skip/xfail/#[ignore] mentioning the removed reader

Beads: anchor `agt-tvqh`, plan questions `agt-tvqh.1`, dev questions `agt-tvqh.2`

Fixtures:
- pass: The v1 run-record reader is deleted together with its tests and fixtures.
- fail: The v1 reader is removed but its tests are kept and marked #[ignore].

## ANC-004

Anchor: sc-agent must impose no request input size limit of its own beyond the selected model's context, so input limits are not over-specified.

> These limits are clearly over-specified. Input is limited by model context.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 2.

Summary: C04 sc-agent must impose no request input size limit of its own 

Scope: adds or changes a request size check, a record size check or handling of an sc-observability size error

Deterministic check to pair with the development gate: `rg -n '8 \* 1024 \* 1024\

Beads: anchor `agt-bouw`, plan questions `agt-bouw.1`, dev questions `agt-bouw.2`

Fixtures:
- pass: Requests are sent at any size up to the model's context window; sc-agent adds no byte cap.
- fail: sc-agent rejects any request body over 64 KiB before sending it.

## ANC-005

Anchor: In production runs, every logger sc-agent creates by default, not injected by a caller, must write under the configured log root with a fixed folder and file name, so tools can find the logs.

> I mandated - that all default loggers created in production have root/folder/file naming conventions specified.

Said by Rand Lee, planning session; session 2026-11-26; also memory `owner-logger-mandate`, 2026-11-26. Status: locked. Version 2.

Summary: C05 In production runs, every logger sc-agent creates by default

Scope: adds or changes creation of a default sc-observability Logger in sc-agent, decidr or the client template

Deterministic check to pair with the development gate: test: default_logger(name) path == `<log_root>/logs/<name>.log.jsonl`; session sink path == `<log_root>/sessions/<repo-id>/<role>/<project>/sessions.jsonl`

Beads: anchor `agt-2mm1`, plan questions `agt-2mm1.1`, dev questions `agt-2mm1.2`

Fixtures:
- pass: The default logger writes to <log_root>/sc-agent/general.log, a fixed folder and file name.
- fail: The default logger writes to a temp file with a random name in the working directory.

## ANC-006

Anchor: sc-agent must not change a logger the caller injects, so tests keep control of injected loggers.

> Injected loggers are not changed (this was specifically set for ease of testing)

Said by Rand Lee, planning session; session 2026-11-26; memory `owner-logger-mandate`, 2026-11-26. Status: locked. Version 2.

Summary: C06 sc-agent must not change a logger the caller injects, so tes

Scope: adds or changes code that receives, wraps, flushes or closes an injected sc-observability Logger

Deterministic check to pair with the development gate: test: Run with injected Logger never calls shutdown/rebuild on it, and its root/sinks are unchanged after close; `rg LoggerBuilder` only on the default path

Beads: anchor `agt-o43c`, plan questions `agt-o43c.1`, dev questions `agt-o43c.2`

Fixtures:
- pass: An injected logger is used as given; sc-agent does not change its level, sink or flush policy.
- fail: sc-agent sets the injected logger's level to INFO and wraps it in its own sink.

## ANC-007

Anchor: Phase m's non-test Rust code, as counted by scripts/gate/rewrite.py, must total at most 2,590 lines at every sprint end, so the phase stays small.

> 2.5k phase cap is what matters

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 3.

Summary: C07 Phase j's non-test Rust code, as counted by scripts/gate/rew

Scope: adds or changes non-test Rust source in the sc-agent repository

Deterministic check to pair with the development gate: `git diff --numstat <phase-j base>..<phase-j tip> -- 'crates/**/*.rs'`, tests excluded, net added ≤ 2580

Beads: anchor `agt-ctuj`, plan questions `agt-ctuj.1`, dev questions `agt-ctuj.2`

Fixtures:
- pass: At sprint end scripts/gate/rewrite.py reports 2,540 of 2,590 lines.
- fail: The sprint ends with rewrite.py reporting 2,710 lines against the 2,590 limit.

## ANC-008

Anchor: In sc-agent code, related values passed to several methods must be grouped in one struct that replaces those repeated parameters, so encapsulation is efficient and repeated code stays minimal as the architecture scales.

> Related values are strategically grouped in one struct to remove repeated method parameters, for efficient encapsulation and to minimize repeated code as the architecture scales.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 3.

Summary: C08 In phase-j code, related values passed to several methods mu

Scope: adds or changes function or method signatures in sc-agent code

Deterministic check to pair with the development gate: review of changed pub fn signatures: no function takes run_id/tool/tool_version/cache/model/provider as separate params

Beads: anchor `agt-jorx`, plan questions `agt-jorx.1`, dev questions `agt-jorx.2`

Fixtures:
- pass: The five methods that took endpoint, model and timeout now take one `ProviderTarget` struct.
- fail: Each new method repeats the same endpoint, model, timeout and retry parameters.

## ANC-009

Anchor: The sc-agent model table must accept extra model rows from config in addition to its hard-coded rows, so a new model works without waiting for a release.

> hard-coded rows will get stale quickly and often

Said by Rand Lee, planning session; PRD D15, owner 2026-11-24, 2026-11-26. Status: locked. Version 2.

Summary: C10 The sc-agent model table must accept extra model rows from c

Scope: adds or changes the model table or model configuration in sc-agent

Deterministic check to pair with the development gate: test: a config file with an extra model row makes `decidr models` list it, and a send with that id resolves

Beads: anchor `agt-1quf`, plan questions `agt-1quf.1`, dev questions `agt-1quf.2`

Fixtures:
- pass: A model row added under [models] in the config file is usable alongside the built-in rows.
- fail: Adding a model requires editing the hard-coded model table and cutting a release.

## ANC-010

Anchor: sc-agent, judge-jev and plan-anchor code must not copy and paste logic between files or tools, so the code base does not grow unnecessarily.

> For years any copy/paste was a big red flag that I was growing the code base unnecessarily.

Said by Rand Lee, planning session; PRD D20, owner 2026-11-24, 2026-11-26. Status: locked. Version 3.

Summary: C11 Phase-j code must not copy and paste logic between files or 

Scope: adds or changes code in judge-jev, plan-anchor or the client template that both tools use

Deterministic check to pair with the development gate: a duplicate-code scan (e.g. jscpd/`rg` of shared helper bodies) across judge-jev and plan-anchor shows no new duplicated block

Beads: anchor `agt-43sg`, plan questions `agt-43sg.1`, dev questions `agt-43sg.2`

Fixtures:
- pass: judge-jev and plan-anchor both call the shared client generated from the template.
- fail: plan-anchor gets a copied and pasted version of judge-jev's result parser.

## ANC-011

Anchor: Every operation the decidr CLI offers must also be callable from the sc-agent Python API, so the Python API is never less flexible than the CLI.

> I expect the python api may allow more flexibility than cli, NOT less.

Said by Rand Lee, planning session; phase-a ANC-006 quote, 2026-11-26. Status: locked. Version 2.

Summary: C14 Every operation the decidr CLI offers must also be callable 

Scope: adds or changes a decidr subcommand or a function in the sc-agent Python bindings

Deterministic check to pair with the development gate: for each `decidr` subcommand touched in phase j (models, send, batch), a Python binding exists and has a test

Beads: anchor `agt-sa4v`, plan questions `agt-sa4v.1`, dev questions `agt-sa4v.2`

Fixtures:
- pass: The new `decidr batch` subcommand is also exposed as `sc_agent.batch()` in the Python API.
- fail: `decidr cache clear` is added to the CLI with no Python API equivalent.

## ANC-012

Anchor: sc-agent's decision providers must implement one decision trait, with one implementation per wire format (Jev TypeSafe systemone, OpenAI-compatible chat/completions, Anthropic-compatible Messages), and every provider that speaks a wire format must use that format's implementation, so adding a provider needs no new code or hard-coding.

> there should be implementations of the decison trait for jev, openai-sdk, anthropic-sdk and compatible providers.  we will also need a llm-trait or similar to enable json-in/json-out llm behavior for open-ai-sdk, anthropic-sdk and compatible providers.  i.e. all providers w/ compatible api should use the same impelemntation.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 2.

Summary: one decision trait, one implementation per wire format

Scope: adds or changes decision providers, provider dispatch or the provider table in sc-agent

Deterministic check to pair with the development gate: grep: one decision trait; one impl per wire format; no match on wire strings outside those impls; adding a compatible provider is a config/table row only

Beads: anchor `agt-et87`, plan questions `agt-et87.1`, dev questions `agt-et87.2`

Fixtures:
- pass: The new provider is a row naming the OpenAI-compatible wire format and reuses that implementation.
- fail: A dedicated Together decision provider is written that duplicates the OpenAI-compatible request code.

## ANC-013

Anchor: sc-agent must provide an LLM trait for JSON-in/JSON-out calls, with one implementation per wire format (OpenAI-compatible, Anthropic-compatible) shared by every provider speaking that format, so callers reach any LLM through one interface.

> we will also need a llm-trait or similar to enable json-in/json-out llm behavior for open-ai-sdk, anthropic-sdk and compatible providers.  i.e. all providers w/ compatible api should use the same impelemntation.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 2.

Summary: LLM trait, JSON in/out, one impl per wire format

Scope: adds or changes LLM (non-Jev) calls in sc-agent

Deterministic check to pair with the development gate: grep: one LLM trait with one impl per wire format; decision-over-LLM uses it; test sends JSON and receives schema-valid JSON for each wire format

Beads: anchor `agt-jod5`, plan questions `agt-jod5.1`, dev questions `agt-jod5.2`

Fixtures:
- pass: LLM calls go through the `Llm` trait, and the OpenRouter provider uses the shared OpenAI-compatible implementation.
- fail: Each LLM provider gets its own call function with its own JSON request building.

## ANC-014

Anchor: When an LLM answers a decision request in place of Jev, sc-agent must send a built-in default system prompt, and the config settings decidr.luna-system-prompt and decidr.haiku-system-prompt must each override that prompt for their model, so prompts can be tested without code changes.

> make sure config allows settings for decidr.luna-system-prompt and decidr.haiku-system-prompt as alternatives that can be overridden from config when we do more extensive testing w/out code changes.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 2.

Summary: LLM-as-Jev system prompt; luna/haiku config overrides

Scope: adds or changes how an LLM is prompted to answer a decision request, or decidr config keys

Deterministic check to pair with the development gate: test: request to luna/haiku carries a system message equal to the default; with decidr.luna-system-prompt / decidr.haiku-system-prompt set in config it equals the configured text

Beads: anchor `agt-pahi`, plan questions `agt-pahi.1`, dev questions `agt-pahi.2`

Fixtures:
- pass: Luna answering a decision request gets the built-in system prompt unless decidr.luna-system-prompt overrides it.
- fail: The haiku system prompt is a string constant with no config override.

## ANC-015

Anchor: A rule that applies to every decidr-based tool (logging, model resolution and the like) must be specified only in sc-agent's records; a derivative tool's records (judge-jev, plan-anchor and others) must not restate it and give only the tool name and the tool's own behaviour, so each rule has one source.

> judge-jev and plan-anchor simply supply tool name and follow the rules established for all decidr based tools

Said by Rand Lee, planning session; session, 2026-11-26; also: 'same thing for REQ-JEV-0009 (this is sc-agent requirement) and applies to all tools', 2026-11-26. Status: locked. Version 2.

Summary: decidr-wide rules live only in sc-agent

Scope: adds or changes REQ/NFR/ADR records of judge-jev, plan-anchor or another sc-agent-based tool

Deterministic check to pair with the development gate: no derivative-tool record states log placement, model resolution, consent, retries or result-line fields; each points to the sc-agent record

Beads: anchor `agt-fokq`, plan questions `agt-fokq.1`, dev questions `agt-fokq.2`

Fixtures:
- pass: judge-jev's records name the tool and its own behaviour and point to sc-agent for the logging rule.
- fail: judge-jev's REQ restates sc-agent's log-root and model-resolution rules in full.

## ANC-016

Anchor: sc-agent's records must define the logger-injection path, and a derivative tool's production code must not use it, only its tests, so production logs always follow sc-agent's default rules.

> judge-jev/plan-anchor and other derivative tools should defer to the default tool rules and not use injection path for production code (test only).

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 2.

Summary: logger injection is test-only for derivative tools

Scope: adds or changes logger construction or injection in sc-agent or a derivative tool

Deterministic check to pair with the development gate: grep derivative tools' non-test code for logger injection: none; one sc-agent record defines the injection path

Beads: anchor `agt-mww2`, plan questions `agt-mww2.1`, dev questions `agt-mww2.2`

Fixtures:
- pass: plan-anchor's production code uses the default logger; only its tests inject a logger.
- fail: plan-anchor's CLI injects its own logger in production runs.

## ANC-017

Anchor: sc-agent tests must not depend on races, wall-clock timing, sleeps, event-loop ordering or system load; a test found to depend on one must be redesigned to drive its logic with controlled inputs.

> Flaky tests are prohibited by design. Any flaky test identified (one that relies on a race, timing, sleeps, event-loop ordering or load) must be re-designed to be deterministic by driving the logic with controlled inputs.

Said by Rand Lee, carried forward 2026-11-27 (record clean-up layer 1), 2026-11-27. Status: locked. Version 2.

Summary: no flaky tests

Scope: adds or changes a test under crates/*/tests, crates/*/src test modules, tests/ or crates/sc-agent-py/tests

Deterministic check to pair with the development gate: review changed tests for sleep, wall-clock assertions, poll-until-timeout and unsynchronized thread ordering; a failure-only backstop that fails with captured output is allowed

Beads: anchor `agt-lnby`, plan questions `agt-lnby.1`, dev questions `agt-lnby.2`

Fixtures:
- pass: The new holdoff test drives its deadline with an injected clock and an explicit channel handshake.
- fail: The test sleeps 200 ms and then asserts the worker has finished.

## ANC-018

Anchor: sc-agent's integration tests must be its unit tests run against live agents, triggered manually.

> I expect integration tests to be build as unit tests (ideally tests that already exist using mocks).  they are simply run w/ live agents and triggered manually.

Said by Rand Lee, carried forward 2026-11-27 (record clean-up layer 1), 2026-11-27. Status: locked. Version 4.

Summary: live integration = unit tests run live, manually

Scope: adds or changes a live-provider test, a test selection for live runs, or a CI workflow under .github/workflows

Deterministic check to pair with the development gate: grep .github/workflows for live-provider test jobs triggered on push, pull_request or schedule; each live test name matches an existing mocked unit test

Beads: anchor `agt-y7m6`, plan questions `agt-y7m6.1`, dev questions `agt-y7m6.2`

Fixtures:
- pass: The live job reruns the existing unit tests against real providers and is started with workflow_dispatch only.
- fail: A new live_integration suite runs on every pull request.

## ANC-019

Anchor: sc-agent must require Python 3.11 or newer.

> sc-agent, judge-jev, prompt-anchor all must be 3.11+

Said by Rand Lee, carried forward 2026-11-27 (record clean-up layer 1), 2026-11-27. Status: locked. Version 3.

Summary: Python 3.11+

Scope: edits pyproject.toml requires-python, Python version checks, CI Python matrices or script interpreter guards

Deterministic check to pair with the development gate: grep requires-python in every sc-agent pyproject.toml reads >=3.11

Beads: anchor `agt-e0lk`, plan questions `agt-e0lk.1`, dev questions `agt-e0lk.2`

Fixtures:
- pass: pyproject.toml sets requires-python = ">=3.11".
- fail: The wheel now supports Python 3.9.

## ANC-020

Anchor: When a caller passes its own sc_observability Logger into an sc-agent wheel call, sc-agent must not build a second Logger for that log.

> we created a oversized set of sprints expressly to normalize config and logging around sc-observability logger (injected).  why do you keep adding more loggers to the design? ... add a new anchor banning python_logger ... lock this

Said by Rand Lee, carried forward 2026-11-27 (record clean-up layer 1), 2026-11-27. Status: locked. Version 2.

Summary: no second logger when one is injected

Scope: touches crates/sc-agent-py or any code path that accepts an injected logger (general_log=, session_log=)

Deterministic check to pair with the development gate: no LoggerBuilder call in crates/sc-agent-py runs on a path where the caller supplied a logger

Beads: anchor `agt-g3pq`, plan questions `agt-g3pq.1`, dev questions `agt-g3pq.2`

Fixtures:
- pass: When general_log= is given, the wheel writes to that logger and builds no other.
- fail: The wheel builds its own Logger with LoggerBuilder even when the caller passed session_log=.

## ANC-021

Anchor: Every phase-m sprint bead names the crate and layer each change lands in, and that layer is the lowest layer whose crates all use the change.

> You need to be able to push shared methods down and move differences up

Said by Rand Lee, phase-m planning session, 2026-11-27; layer rule of ADR-SAG-0004 v0.5.0, 2026-11-27. Status: locked. Version 2.

Summary: Shared code sits in the lowest layer using it

Scope: adds or changes code in any crate under crates/

Beads: anchor `agt-s6th`, plan questions `agt-s6th.1`, dev questions `agt-s6th.2`

Fixtures:
- pass: The bead puts the retry helper in sc-agent-types, layer 1, because both sc-anthropic and sc-openai use it.
- fail: The bead puts a helper used by both providers in sc-openai and copies it into sc-anthropic.

## ANC-022

Anchor: sc-agent has no hard-coded row for a local model: a local model's URL and model id come only from config.

> local models come from config. Both ip & model string.

Said by Rand Lee, phase-m planning session, 2026-11-27, 2026-11-27. Status: locked. Version 2.

Summary: Local model URL and id come only from config

Scope: adds or changes the model table, model configuration or local-model handling

Beads: anchor `agt-czru`, plan questions `agt-czru.1`, dev questions `agt-czru.2`

Fixtures:
- pass: A local model is reached only through a config row giving its URL and model id.
- fail: The model table gains a hard-coded row for llama3 at http://localhost:11434.

## ANC-023

Anchor: sc-agent writes each session row to the repository's SQLite database through `sc-agent-sqlite`; no code writes `sessions.jsonl`.

> I want session logs written to SQLite, jsonl was just a quick item for mvp

Said by Rand Lee, phase-m planning session, 2026-11-27, 2026-11-27. Status: locked. Version 2.

Summary: Session rows go to SQLite, not sessions.jsonl

Scope: adds or changes code that writes or reads session rows

Beads: anchor `agt-rdga`, plan questions `agt-rdga.1`, dev questions `agt-rdga.2`

Fixtures:
- pass: Session rows are inserted through sc-agent-sqlite into the repository database, and sessions.jsonl is no longer written.
- fail: Each session is appended to sessions.jsonl as before.

## ANC-024

Anchor: The general log is written through sc-observability under its rotation and deletion policy; sc-agent adds no retention rule of its own.

> general logs follow sc-obs rotation/deletion guidelines

Said by Rand Lee, phase-m planning session, 2026-11-27, 2026-11-27. Status: locked. Version 2.

Summary: General log follows sc-observability retention

Scope: adds or changes general-log writing, rotation or deletion

Beads: anchor `agt-69i3`, plan questions `agt-69i3.1`, dev questions `agt-69i3.2`

Fixtures:
- pass: General-log rotation and deletion are left to sc-observability's policy.
- fail: sc-agent deletes general logs older than 7 days with its own cleanup task.

## ANC-025

Anchor: `sc-agent-schema` contains only struct definitions, literal SQL text and convenience methods on those structs; it has no SQLite dependency and no upgrade code.

> I consider schema to hold schema (structs and literal sql text) only. Exceptions would be extension convenience methods/transforms and schema versioning/upgrade code.

Said by Rand Lee, phase-m planning session, 2026-11-27; Rand, 2026-11-27 23:44, on this candidate: "6. Remove upgrade", 2026-11-27. Status: locked. Version 2.

Summary: sc-agent-schema holds structs and SQL text only

Scope: adds or changes any file under crates/sc-agent-schema

Beads: anchor `agt-3fwl`, plan questions `agt-3fwl.1`, dev questions `agt-3fwl.2`

Fixtures:
- pass: sc-agent-schema adds the `Session` struct and its CREATE TABLE text, with no rusqlite dependency.
- fail: sc-agent-schema gains a rusqlite dependency and a migrate_v1_to_v2 function.

## ANC-026

Anchor: Every core table's SQL is defined in `sc-agent-schema` and nowhere else.

> SQLite schema is in sc-agent-schema.

Said by Rand Lee, phase-m planning session, 2026-11-27, 2026-11-27. Status: locked. Version 2.

Summary: Core table SQL defined only in sc-agent-schema

Scope: adds or changes SQL table definitions anywhere in the workspace

Beads: anchor `agt-lw7b`, plan questions `agt-lw7b.1`, dev questions `agt-lw7b.2`

Fixtures:
- pass: The responses table's CREATE TABLE text is added in sc-agent-schema and used from there.
- fail: sc-agent-sqlite defines its own CREATE TABLE for sessions.

## ANC-027

Anchor: The session schema stores a request once in `messages`, keyed by sha256, and each model reply as a row in `responses` that references it.

> We likely want message+sha256 and response+model in separate tables as this is one->many

Said by Rand Lee, phase-m planning session, 2026-11-27, 2026-11-27. Status: locked. Version 2.

Summary: Requests stored once; replies in responses table

Scope: adds or changes the session tables or code that stores a request or a model reply

Beads: anchor `agt-kkrv`, plan questions `agt-kkrv.1`, dev questions `agt-kkrv.2`

Fixtures:
- pass: A repeated request is stored once in messages by sha256, and each reply is a responses row referencing it.
- fail: Each session row stores the full request text alongside its single reply.

## ANC-028

Anchor: A cache hit is decided by the request sha256 and the model alone; `run_id` is not part of the cache key.

> run-id has nothing to do with caching. run is a collection of queries of which some may be cached (if caching enabled)

Said by Rand Lee, phase-m planning session, 2026-11-27, 2026-11-27. Status: locked. Version 2.

Summary: Cache key is request sha256 and model, not run_id

Scope: adds or changes the result cache, cache keys or run ids

Beads: anchor `agt-0fau`, plan questions `agt-0fau.1`, dev questions `agt-0fau.2`

Fixtures:
- pass: The cache is looked up by (request_sha256, model) only.
- fail: The cache key includes run_id, so a new run never hits the cache.

## ANC-029

Anchor: `--resume` is implemented in `sc-llm`; resuming a decision session continues it as an LLM session.

> Refers to sc-llm behavior. —resume on decision session would convert it to llm-session

Said by Rand Lee, phase-m planning session, 2026-11-27, 2026-11-27. Status: locked. Version 2.

Summary: --resume lives in sc-llm

Scope: adds or changes --resume or session resumption

Beads: anchor `agt-x65j`, plan questions `agt-x65j.1`, dev questions `agt-x65j.2`

Fixtures:
- pass: `--resume` is handled in sc-llm and turns a decision session into an LLM session.
- fail: `--resume` is implemented in sc-decision-runner and keeps the session a decision session.

## ANC-030

Anchor: Retry and holdoff code takes its waits, limits and busy and quota rules as parameters from the calling layer, and contains no provider name, host or status-code table.

> retry and holdoff are pushed down as far as possible w/ info/data patameterized coming from implementation.

Said by Rand Lee, phase-m planning session, 2026-11-27, 2026-11-27. Status: locked. Version 3.

Summary: Retry and holdoff take provider rules as parameters

Scope: adds or changes retry, resend, backoff or holdoff code

Beads: anchor `agt-2sum`, plan questions `agt-2sum.1`, dev questions `agt-2sum.2`

Fixtures:
- pass: The retry helper takes its waits, limits and busy and quota rules as parameters from the provider crate and contains no provider name, host or status-code table.
- fail: The retry helper hard-codes 429 and 529 handling for api.anthropic.com.

## ANC-031

Anchor: judge-jev, plan-anchor and every other sc-agent-based tool store their results in the repository's SQLite database, not in JSON or JSONL result files.

> At the end of the day, I want all the 'results' from plan-anchor, judge-jev or any other app to save data to SQLite.

Said by Rand Lee, phase-m planning session, 2026-11-27, 2026-11-27. Status: locked. Version 2.

Summary: Tool results are stored in SQLite

Scope: adds or changes how judge-jev, plan-anchor or another sc-agent-based tool stores results

Beads: anchor `agt-yz3r`, plan questions `agt-yz3r.1`, dev questions `agt-yz3r.2`

Fixtures:
- pass: judge-jev writes its quality results to the repository SQLite database.
- fail: plan-anchor writes each gate run to anchor-runs/<run>.json as its result store.

## ANC-032

Anchor: Each tool's tables carry their own schema version, independent of the core schema version; no table definition is in the wheel crate `sc-agent-py`.

> Also schema is not encoded at individual wheel layer. Schema is versioned for app tables independently.

Said by Rand Lee, phase-m planning session, 2026-11-27, 2026-11-27. Status: locked. Version 2.

Summary: Tool tables versioned apart; none in sc-agent-py

Scope: adds or changes a tool's table definitions, schema versions, or crates/sc-agent-py

Beads: anchor `agt-ws1i`, plan questions `agt-ws1i.1`, dev questions `agt-ws1i.2`

Fixtures:
- pass: judge-jev's tables have their own schema version, separate from the core schema version.
- fail: sc-agent-py defines the judge-jev result table and versions it with the core schema.
