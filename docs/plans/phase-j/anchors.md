# Anchors - phase-j

Constraints captured during planning. Each anchor is a testable statement written by the orchestrator and approved by the user; the user's verbatim words are kept beneath it as provenance. Every anchor is judged at the planning gate and again at the development gate. Edit `anchors.json`, not this file; this file is rendered.

Intent: `docs/plans/phase-j/intent.md`

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

## ANC-001

Anchor: The decidr model name `jev` must select the newest Jev model in sc-agent's model table unless a config file sets another default, so callers get the latest Jev without a code change.

> I want 'jev' to select latest, config can override default.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 1.

Summary: C01 The decidr model name `jev` must select the newest Jev model

Scope: adds or changes model resolution, the model table or the jev alias in sc-agent or decidr

Deterministic check to pair with the development gate: unit test: resolve("jev") == first Jev row of the date-sorted table; with config default set, == configured id

## ANC-002

Anchor: Until SQLite schema 1.0, sc-agent, judge-jev and plan-anchor must read only the record schema version they write and contain no reader for an older version, so read and write stay in step.

> We need to read what we are writing, no back compat until SQLite 1.0

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 1.

Summary: C02 Until SQLite schema 1.0, sc-agent, judge-jev and plan-anchor

Scope: adds or changes code that reads batch result lines, session rows or run records

Deterministic check to pair with the development gate: `rg -n 'jev-batch-result/1\

## ANC-003

Anchor: When phase j removes a backward-compatible reader, its code and its tests must both be deleted, because backward-compatible reads are baselined later in SQLite.

> remove code & tests, don't just remove. We will baseline backward compatible reads in SQLite.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 1.

Summary: C03 When phase j removes a backward-compatible reader, its code 

Scope: removes or changes a reader of an earlier record schema version, or its tests

Deterministic check to pair with the development gate: the sprint diff deletes the reader's tests and fixtures; `rg` finds no skip/xfail/#[ignore] mentioning the removed reader

## ANC-004

Anchor: sc-agent must impose no request input size limit of its own beyond the selected model's context, so input limits are not over-specified.

> These limits are clearly over-specified. Input is limited by model context.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 1.

Summary: C04 sc-agent must impose no request input size limit of its own 

Scope: adds or changes a request size check, a record size check or handling of an sc-observability size error

Deterministic check to pair with the development gate: `rg -n '8 \* 1024 \* 1024\

## ANC-005

Anchor: In production runs, every logger sc-agent creates by default, not injected by a caller, must write under the configured log root with a fixed folder and file name, so tools can find the logs.

> I mandated - that all default loggers created in production have root/folder/file naming conventions specified.

Said by Rand Lee, planning session; session 2026-11-26; also memory `owner-logger-mandate`, 2026-11-26. Status: locked. Version 1.

Summary: C05 In production runs, every logger sc-agent creates by default

Scope: adds or changes creation of a default sc-observability Logger in sc-agent, decidr or the client template

Deterministic check to pair with the development gate: test: default_logger(name) path == `<log_root>/logs/<name>.log.jsonl`; session sink path == `<log_root>/sessions/<repo-id>/<role>/<project>/sessions.jsonl`

## ANC-006

Anchor: sc-agent must not change a logger the caller injects, so tests keep control of injected loggers.

> Injected loggers are not changed (this was specifically set for ease of testing)

Said by Rand Lee, planning session; session 2026-11-26; memory `owner-logger-mandate`, 2026-11-26. Status: locked. Version 1.

Summary: C06 sc-agent must not change a logger the caller injects, so tes

Scope: adds or changes code that receives, wraps, flushes or closes an injected sc-observability Logger

Deterministic check to pair with the development gate: test: Run with injected Logger never calls shutdown/rebuild on it, and its root/sinks are unchanged after close; `rg LoggerBuilder` only on the default path

## ANC-007

Anchor: Phase j's non-test Rust code, as counted by scripts/gate/rewrite.py, must total at most 2,500 lines at every sprint end, so the phase stays small.

> 2.5k phase cap is what matters

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 1.

Summary: C07 Phase j's non-test Rust code, as counted by scripts/gate/rew

Scope: adds or changes non-test Rust source in the sc-agent repository

Deterministic check to pair with the development gate: `git diff --numstat <phase-j base>..<phase-j tip> -- 'crates/**/*.rs'`, tests excluded, net added ≤ 2500

## ANC-008

Anchor: In phase-j code, related values passed to several methods must be grouped in one struct that replaces those repeated parameters, so encapsulation is efficient and repeated code stays minimal as the architecture scales.

> Related values are strategically grouped in one struct to remove repeated method parameters, for efficient encapsulation and to minimize repeated code as the architecture scales.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 1.

Summary: C08 In phase-j code, related values passed to several methods mu

Scope: adds or changes function or method signatures in phase-j code

Deterministic check to pair with the development gate: review of changed pub fn signatures: no function takes run_id/tool/tool_version/cache/model/provider as separate params

## ANC-009

Anchor: The sc-agent model table must accept extra model rows from config in addition to its hard-coded rows, so a new model works without waiting for a release.

> hard-coded rows will get stale quickly and often

Said by Rand Lee, planning session; PRD D15, owner 2026-11-24, 2026-11-26. Status: locked. Version 1.

Summary: C10 The sc-agent model table must accept extra model rows from c

Scope: adds or changes the model table or model configuration in sc-agent

Deterministic check to pair with the development gate: test: a config file with an extra model row makes `decidr models` list it, and a send with that id resolves

## ANC-010

Anchor: Phase-j code must not copy and paste logic between files or tools, so the code base does not grow unnecessarily.

> For years any copy/paste was a big red flag that I was growing the code base unnecessarily.

Said by Rand Lee, planning session; PRD D20, owner 2026-11-24, 2026-11-26. Status: locked. Version 1.

Summary: C11 Phase-j code must not copy and paste logic between files or 

Scope: adds or changes code in judge-jev, plan-anchor or the client template that both tools use

Deterministic check to pair with the development gate: a duplicate-code scan (e.g. jscpd/`rg` of shared helper bodies) across judge-jev and plan-anchor shows no new duplicated block

## ANC-011

Anchor: Every operation the decidr CLI offers must also be callable from the sc-agent Python API, so the Python API is never less flexible than the CLI.

> I expect the python api may allow more flexibility than cli, NOT less.

Said by Rand Lee, planning session; phase-a ANC-006 quote, 2026-11-26. Status: locked. Version 1.

Summary: C14 Every operation the decidr CLI offers must also be callable 

Scope: adds or changes a decidr subcommand or a function in the sc-agent Python bindings

Deterministic check to pair with the development gate: for each `decidr` subcommand touched in phase j (models, send, batch), a Python binding exists and has a test

## ANC-012

Anchor: sc-agent's decision providers must implement one decision trait, with one implementation per wire format (Jev TypeSafe systemone, OpenAI-compatible chat/completions, Anthropic-compatible Messages), and every provider that speaks a wire format must use that format's implementation, so adding a provider needs no new code or hard-coding.

> there should be implementations of the decison trait for jev, openai-sdk, anthropic-sdk and compatible providers.  we will also need a llm-trait or similar to enable json-in/json-out llm behavior for open-ai-sdk, anthropic-sdk and compatible providers.  i.e. all providers w/ compatible api should use the same impelemntation.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 1.

Summary: one decision trait, one implementation per wire format

Scope: adds or changes decision providers, provider dispatch or the provider table in sc-agent

Deterministic check to pair with the development gate: grep: one decision trait; one impl per wire format; no match on wire strings outside those impls; adding a compatible provider is a config/table row only

## ANC-013

Anchor: sc-agent must provide an LLM trait for JSON-in/JSON-out calls, with one implementation per wire format (OpenAI-compatible, Anthropic-compatible) shared by every provider speaking that format, so callers reach any LLM through one interface.

> we will also need a llm-trait or similar to enable json-in/json-out llm behavior for open-ai-sdk, anthropic-sdk and compatible providers.  i.e. all providers w/ compatible api should use the same impelemntation.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 1.

Summary: LLM trait, JSON in/out, one impl per wire format

Scope: adds or changes LLM (non-Jev) calls in sc-agent

Deterministic check to pair with the development gate: grep: one LLM trait with one impl per wire format; decision-over-LLM uses it; test sends JSON and receives schema-valid JSON for each wire format

## ANC-014

Anchor: When an LLM answers a decision request in place of Jev, sc-agent must send a built-in default system prompt, and the config settings decidr.luna-system-prompt and decidr.haiku-system-prompt must each override that prompt for their model, so prompts can be tested without code changes.

> make sure config allows settings for decidr.luna-system-prompt and decidr.haiku-system-prompt as alternatives that can be overridden from config when we do more extensive testing w/out code changes.

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 1.

Summary: LLM-as-Jev system prompt; luna/haiku config overrides

Scope: adds or changes how an LLM is prompted to answer a decision request, or decidr config keys

Deterministic check to pair with the development gate: test: request to luna/haiku carries a system message equal to the default; with decidr.luna-system-prompt / decidr.haiku-system-prompt set in config it equals the configured text

## ANC-015

Anchor: A rule that applies to every decidr-based tool (logging, model resolution and the like) must be specified only in sc-agent's records; a derivative tool's records (judge-jev, plan-anchor and others) must not restate it and give only the tool name and the tool's own behaviour, so each rule has one source.

> judge-jev and plan-anchor simply supply tool name and follow the rules established for all decidr based tools

Said by Rand Lee, planning session; session, 2026-11-26; also: 'same thing for REQ-JEV-0009 (this is sc-agent requirement) and applies to all tools', 2026-11-26. Status: locked. Version 1.

Summary: decidr-wide rules live only in sc-agent

Scope: adds or changes REQ/NFR/ADR records of judge-jev, plan-anchor or another sc-agent-based tool

Deterministic check to pair with the development gate: no derivative-tool record states log placement, model resolution, consent, retries or result-line fields; each points to the sc-agent record

## ANC-016

Anchor: sc-agent's records must define the logger-injection path, and a derivative tool's production code must not use it, only its tests, so production logs always follow sc-agent's default rules.

> judge-jev/plan-anchor and other derivative tools should defer to the default tool rules and not use injection path for production code (test only).

Said by Rand Lee, planning session; session, 2026-11-26, 2026-11-26. Status: locked. Version 1.

Summary: logger injection is test-only for derivative tools

Scope: adds or changes logger construction or injection in sc-agent or a derivative tool

Deterministic check to pair with the development gate: grep derivative tools' non-test code for logger injection: none; one sc-agent record defines the injection path
