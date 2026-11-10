# Anchors - phase-a

Constraints captured during planning. Each anchor is a testable statement written by the orchestrator and approved by the user; the user's verbatim words are kept beneath it as provenance. Every anchor is judged at the planning gate and again at the development gate. Edit `anchors.json`, not this file; this file is rendered.

Intent: `docs/plans/phase-a/intent.md`

| ID | Status | Summary | Stated |
| --- | --- | --- | --- |
| ANC-001 | locked | sc-decision-schema holds only minimum decision-API code | 2026-11-09T07:40:40Z |
| ANC-002 | retired | Python API offers every sc-ai CLI operation | 2026-11-08T21:07:35Z |
| ANC-003 | proposed | No requirements sourced from plan-anchor or judge-jev | 2026-11-09T07:24:54Z |
| ANC-004 | retired | Library code logs through injected sc-observability logger | 2026-11-09T00:26:57Z |
| ANC-005 | retired | One batch of Jev requests runs in parallel | 2026-11-08T21:07:35Z |
| ADR-SAG-0001 | locked | Every decision backend implements Backend; callers use Decision | 2026-11-09 |
| ADR-SAG-0004 | locked | No crate depends on a crate above it in the layering | 2026-11-09 |
| REQ-SAG-0002 | locked | Each crate returns its own error union carrying lower errors | 2026-11-09 |
| REQ-SAG-0006 | locked | No consumer-app or Jev-mock types in shared crates | 2026-11-09 |
| REQ-SAG-0009 | locked | Low-level crates build and run without tokio | 2026-11-09 |
| REQ-SAG-0010 | locked | No sleep calls in sc-agent source | 2026-11-09 |
| REQ-SAG-0201 | locked | Jev API types defined only in sc-decision-schema, built from the spec | 2026-11-09 |
| REQ-SAG-0601 | locked | Busy resend: configurable sends, holdoff in the queue | 2026-11-09 |
| REQ-SAG-0701 | locked | Batches run in parallel, fastest first, failures returned | 2026-11-09 |
| ANC-006 | locked | Python API offers every decide CLI operation | 2026-11-08T21:07:35Z |
| ANC-007 | locked | Existing sc-agent library code updated to write log records | 2026-11-09T00:26:57Z |

## ANC-001

Anchor: crates/sc-decision-schema must contain only the code necessary to send and receive 100% of the decision API, with nothing beyond that minimum.

> Has the absolute minimum code that is necessary to send/receive 100% of the decision api

Said by Rand Lee, planning session, 2026-11-09T07:40:40Z. Status: locked. Version 3.

Summary: sc-decision-schema holds only minimum decision-API code

Scope: adds or changes code in crates/sc-decision-schema

Deterministic check to pair with the development gate: every public item in crates/sc-decision-schema maps to a TypeSafe API request or response field, or to the decision trait

Excluded from (not judged; the cell passes as `excluded`):
- `agt-4p2.5`: ANC-001 governs code in crates/sc-decision-schema; A-5 adds no code to that crate.
- `agt-4p2.6`: ANC-001 governs code in crates/sc-decision-schema; A-6 adds sc-decision-systemone and sc-decision-local and no code to that crate.
- `agt-4p2.7`: ANC-001 governs code in crates/sc-decision-schema; A-7 adds no code to that crate.

Beads: anchor `agt-e49`, plan questions `agt-e49.1`, dev questions `agt-e49.2`

Fixtures:
- pass: Add to crates/sc-decision-schema only the TypeSafe request, response, answer and error types plus the Backend and Decision traits; the crate gets no transport, logging or CLI code.
- fail: sc-decision-schema also holds a response cache, a plan-anchor report formatter and an HTTP client.

## ANC-002

Anchor: Every operation available in the sc-ai CLI must also be available in the sc-agent Python API.

> I expect the python api may allow more flexibility than cli, NOT less.

Said by Rand Lee, planning session, 2026-11-08T21:07:35Z. Status: retired. Version 2.

Summary: Python API offers every sc-ai CLI operation

Scope: adds or changes an sc-ai command or the sc-agent Python API

Deterministic check to pair with the development gate: each sc-ai subcommand has a matching Python API function, checked by a test that lists both

Excluded from (not judged; the cell passes as `excluded`):
- `agt-4p2.3`: A-3 builds the sc-treesitter library and adds no CLI command; the tree-sitter CLI and its Python parity are sprint A-10.
- `agt-4p2.1`: sc-agent-types defines data types only; it adds no CLI command and no Python API function.
- `agt-4p2.4`: A-4 builds config and secrets crates; it adds no CLI command and no Python API function.
- `agt-4p2.5`: A-5 builds the HTTP transport crate; it adds no CLI command and no Python API function.
- `agt-4p2.6`: A-6 builds decision backends; it adds no CLI command and no Python API function.

Beads: anchor `agt-ug0`, plan questions `agt-ug0.1`, dev questions `agt-ug0.2`

## ANC-003

Anchor: No sc-agent REQ, NFR, ADR or bead must cite plan-anchor or judge-jev as the source of a requirement.

> I didn't give you plan-anchor or judge-jev requirements.

Said by Rand Lee, planning session, 2026-11-09T07:24:54Z. Status: proposed. Version 1.

Summary: No requirements sourced from plan-anchor or judge-jev

Scope: adds or changes a requirement, type or feature in sc-agent

## ANC-004

Anchor: sc-agent library code must log through an injected sc-observability logger.

> You need to go through the code and add logging and inject the logger everywhere that should have had it from day-1.

Said by Rand Lee, planning session, 2026-11-09T00:26:57Z. Status: retired. Version 2.

Summary: Library code logs through injected sc-observability logger

Scope: adds or changes code in an sc-agent library crate

## ANC-005

Anchor: sc-agent must run the Jev requests of one JSONL file, stdio stream or Python call in parallel.

> if I send 1000 jev requests in a single jsonl file/stdio/python call that the calls will be run in parallel w/ a degree of parallelism which has been shown to work.

Said by Rand Lee, planning session, 2026-11-08T21:07:35Z. Status: retired. Version 2.

Summary: One batch of Jev requests runs in parallel

Scope: adds or changes handling of batches of decision requests

## ADR-SAG-0001

Anchor: Every sc-agent decision backend must implement the Backend trait defined in the sc-decision-schema crate.

> Every decision backend (Jev, local Jev-compatible runners, Haiku, Luna) implements one sans-IO `Backend` trait in `sc-decision-schema`: encode a Jev API request into an HTTP request, and decode an HTTP response into a Jev API response.

Said by Rand Lee, record ADR-SAG-0001, 2026-11-09. Status: locked. Version 2.

Summary: Every decision backend implements Backend; callers use Decision

Scope: adds or changes a decision backend, the Backend or Decision trait, or sc-decision-runner

Excluded from (not judged; the cell passes as `excluded`):
- `agt-4p2.5`: A-5 builds the sc-agent-http transport; it implements no decision backend and does not define the Backend or Decision trait or sc-decision-runner.
- `agt-4p2.10`: A-10 builds the decide and tsq binaries and the Python API; it implements no decision backend and does not define the Backend or Decision trait or sc-decision-runner, it only calls the Decision trait.

Beads: anchor `agt-7mi`, plan questions `agt-7mi.1`, dev questions `agt-7mi.2`

Fixtures:
- pass: The Jev, local-runner, Haiku and Luna backends each implement the Backend trait from sc-decision-schema.
- fail: The Haiku backend defines its own LlmJudge trait and is called directly by the runner instead of through Backend.

## ADR-SAG-0004

Anchor: No sc-agent crate's Cargo.toml may depend on a crate in a higher layer, where the layers from the bottom are: types; config, secrets and log; http; decision schema; decision backends (sc-decision-local over sc-decision-systemone, sc-decision-llm over sc-llm-provider); runner.

> Layers from the bottom: `sc-agent-types`; `sc-agent-config`, `sc-agent-secrets` and `sc-agent-log`; `sc-agent-http`; `sc-decision-schema`; the decision backends `sc-decision-systemone`, `sc-decision-local` and `sc-decision-llm` (over `sc-llm-provider`); `sc-decision-runner`, which handles decision models only.

Said by Rand Lee, record ADR-SAG-0004, 2026-11-09. Status: locked. Version 6.

Summary: No crate depends on a crate above it in the layering

Scope: adds or changes a crate dependency in a Cargo.toml

Excluded from (not judged; the cell passes as `excluded`):
- `agt-4p2.3`: sc-treesitter stands outside the sc-agent layering (REQ-SAG-0301) and depends on no sc-agent crate.

Beads: anchor `agt-mud`, plan questions `agt-mud.1`, dev questions `agt-mud.2`

Fixtures:
- pass: sc-agent-http's Cargo.toml depends only on sc-agent-types, sc-agent-secrets and sc-agent-log, all lower layers; the only crate that depends on sc-decision-runner is decide, which sits above the runner.
- fail: sc-agent-types adds sc-decision-schema to its [dependencies] so it can name DecisionRequest.

## REQ-SAG-0002

Anchor: Every error returned by a public sc-agent function must carry the id and message of each lower-layer error that caused it.

> An error MUST carry the error id and message of every lower-layer error that caused it.

Said by Rand Lee, record REQ-SAG-0002, 2026-11-09. Status: locked. Version 2.

Summary: Each crate returns its own error union carrying lower errors

Scope: adds or changes an error type or a public function's return type

Beads: anchor `agt-t70`, plan questions `agt-t70.1`, dev questions `agt-t70.2`

Fixtures:
- pass: HttpError::Network wraps the ureq error and a test asserts the outer error's chain carries the ureq error's id and message.
- fail: The backend maps every lower-layer failure to a new error with the message "request failed", dropping the original error.

## REQ-SAG-0006

Anchor: No type, field or constant in the sc-agent-types or sc-decision-schema crates must be specific to plan-anchor, judge-jev or another application built on sc-agent.

> `sc-agent-types` and `sc-decision-schema` MUST NOT contain types specific to plan-anchor, judge-jev or any other consumer application.

Said by Rand Lee, record REQ-SAG-0006, 2026-11-09. Status: locked. Version 3.

Summary: No consumer-app or Jev-mock types in shared crates

Scope: adds or changes a type in sc-agent-types or sc-decision-schema

Excluded from (not judged; the cell passes as `excluded`):
- `agt-4p2.5`: REQ-SAG-0006 governs sc-agent-types and sc-decision-schema; A-5 changes neither (it builds sc-agent-http).
- `agt-4p2.6`: REQ-SAG-0006 governs sc-agent-types and sc-decision-schema; A-6 adds no type to either.
- `agt-4p2.7`: REQ-SAG-0006 governs sc-agent-types and sc-decision-schema; A-7 adds no type, field or constant to either.
- `agt-4p2.8`: REQ-SAG-0006 governs sc-agent-types and sc-decision-schema; A-8 builds sc-agent-log.
- `agt-4p2.4`: A-4 builds sc-agent-config and sc-agent-secrets and adds no type, field or constant to sc-agent-types or sc-decision-schema (VarName and the catalog types are A-1's).
- `agt-4p2.10`: A-10 builds decide and sc-agent-py and adds no type, field or constant to sc-agent-types or sc-decision-schema; it only uses them.

Beads: anchor `agt-qyd`, plan questions `agt-qyd.1`, dev questions `agt-qyd.2`

Fixtures:
- pass: Add the TypeSafe API request and response types to sc-decision-schema; no type, field or constant names plan-anchor or judge-jev.
- fail: sc-agent-types adds a PlanAnchorVerdict enum for the plan-anchor gate.

## REQ-SAG-0009

Anchor: No low-level sc-agent crate (types, config, secrets, log, http, decision schema, decision backends, runner) must depend on tokio with its default features.

> A program that does not use tokio MUST be able to use every low-level crate.

Said by Rand Lee, record REQ-SAG-0009, 2026-11-09. Status: locked. Version 2.

Summary: Low-level crates build and run without tokio

Scope: adds or changes dependencies of a low-level sc-agent crate

Excluded from (not judged; the cell passes as `excluded`):
- `agt-4p2.3`: sc-treesitter is not in REQ-SAG-0009's list of low-level crates (types, config, secrets, log, http, decision schema, decision backends, runner); it stands alone under REQ-SAG-0301.

Beads: anchor `agt-wth`, plan questions `agt-wth.1`, dev questions `agt-wth.2`

Fixtures:
- pass: `cargo tree -p sc-decision-runner` lists no tokio crate with default features.
- fail: sc-agent-http adds reqwest with its default tokio runtime as a required dependency.

## REQ-SAG-0010

Anchor: sc-agent source code must not call std::thread::sleep, tokio::time::sleep or any other sleep function.

> sc-agent source code MUST NOT call `std::thread::sleep`, `std::thread::sleep_until`, `tokio::time::sleep` or any other sleep function.

Said by Rand Lee, record REQ-SAG-0010, 2026-11-09. Status: locked. Version 3.

Summary: No sleep calls in sc-agent source

Scope: adds or changes Rust source in an sc-agent crate

Beads: anchor `agt-6dm`, plan questions `agt-6dm.1`, dev questions `agt-6dm.2`

Fixtures:
- pass: The sc-agent runner waits out holdoffs with CancelToken::wait_until and a clippy rule forbids sleep calls.
- fail: The sc-agent retry loop calls std::thread::sleep(Duration::from_secs(1)) between attempts.

## REQ-SAG-0201

Anchor: Every Rust type that maps to a field of the TypeSafe Jev API (POST /v1/systemone, GET /v1/models) must be defined in the sc-decision-schema crate.

> Every type that maps to the Jev API MUST be defined in `sc-decision-schema`.

Said by Rand Lee, record REQ-SAG-0201, 2026-11-09. Status: locked. Version 2.

Summary: Jev API types defined only in sc-decision-schema, built from the spec

Scope: adds or changes a type that maps to a TypeSafe API field

Excluded from (not judged; the cell passes as `excluded`):
- `agt-4p2.1`: sc-agent-types defines no type that maps to a TypeSafe API field; those are sprint A-2 (sc-decision-schema).
- `agt-4p2.10`: A-10 defines no type that maps to a TypeSafe API field; those types are sprint A-2 (sc-decision-schema) and decide only imports them.

Beads: anchor `agt-gc6`, plan questions `agt-gc6.1`, dev questions `agt-gc6.2`

Fixtures:
- pass: sc-decision-llm imports SystemOneRequest and SystemOneResponse from sc-decision-schema and defines no Jev-shaped type of its own.
- fail: sc-decision-systemone defines its own JevAnswer struct mirroring the TypeSafe choice answer.

## REQ-SAG-0601

Anchor: A request that receives an HTTP 429, 502 or 529 response must be resent until a configurable maximum number of sends, 5 by default, is reached.

> A request that receives a busy response MUST be resent unchanged.

Said by Rand Lee, record REQ-SAG-0601, 2026-11-09. Status: locked. Version 2.

Summary: Busy resend: configurable sends, holdoff in the queue

Scope: adds or changes busy-response handling, resend, holdoff or the runner queue

Excluded from (not judged; the cell passes as `excluded`):
- `agt-4p2.4`: A-4 delivers only the max_sends setting; resending is built in A-5 and A-9.
- `agt-4p2.7`: A-7 only returns decision.schema_mismatch; the 429/502/529 busy resend is built in A-5 and A-9.
- `agt-4p2.2`: Resending on 429/502/529 is built in A-5 and A-9; sc-decision-schema only encodes and decodes.
- `agt-4p2.6`: Resending on 429/502/529 is built in A-5 and A-9; the A-6 backends only encode and decode.
- `agt-4p2.10`: Resending on 429/502/529 is built in A-5 and A-9; decide and the Python API only pass through the runner and add no resend logic.

Beads: anchor `agt-ebd`, plan questions `agt-ebd.1`, dev questions `agt-ebd.2`

Fixtures:
- pass: A request answered 429 is resent unchanged until max_sends (default 5) is reached, then returns http.provider_busy.
- fail: A 429 response is returned to the caller at once and never resent.

## REQ-SAG-0701

Anchor: sc-decision-runner must send the requests of one batch in parallel across a configurable number of worker threads, 8 by default.

> `sc-decision-runner` MUST run the requests of one batch in parallel across workers.

Said by Rand Lee, record REQ-SAG-0701, 2026-11-09. Status: locked. Version 2.

Summary: Batches run in parallel, fastest first, failures returned

Scope: adds or changes batch handling in sc-decision-runner or decide

Excluded from (not judged; the cell passes as `excluded`):
- `agt-4p2.4`: A-4 delivers only the workers setting; the parallel batch run is built in A-9.

Beads: anchor `agt-ocy`, plan questions `agt-ocy.1`, dev questions `agt-ocy.2`

Fixtures:
- pass: sc-decision-runner sends a batch's requests across `workers` threads (default 8) and returns each result as it finishes.
- fail: The runner sends the batch's requests one after another on the caller's thread.

## ANC-006

Anchor: Every operation available in the decide CLI must also be available in the sc-agent Python API.

> I expect the python api may allow more flexibility than cli, NOT less.

Said by Rand Lee, planning session, 2026-11-08T21:07:35Z. Status: locked. Version 2.

Summary: Python API offers every decide CLI operation

Scope: adds or changes a decide subcommand or a function in the sc-agent Python bindings

Deterministic check to pair with the development gate: each decide subcommand has a matching Python API function, checked by a test that lists both

Excluded from (not judged; the cell passes as `excluded`):
- `agt-4p2.3`: A-3 builds the sc-treesitter library; it adds no decide CLI operation.
- `agt-4p2.1`: A-1 defines data types only; it adds no decide subcommand or Python binding function.

Beads: anchor `agt-frw`, plan questions `agt-frw.1`, dev questions `agt-frw.2`

Fixtures:
- pass: Each decide subcommand has a matching function in the sc-agent Python API, checked by a test that lists both.
- fail: decide gains a `models` subcommand with no Python equivalent.

## ANC-007

Anchor: The existing sc-agent library code must be updated so that it writes log records.

> You need to go through the code and add logging and inject the logger everywhere that should have had it from day-1.

Said by Rand Lee, planning session, 2026-11-09T00:26:57Z. Status: locked. Version 2.

Summary: Existing sc-agent library code updated to write log records

Scope: adds or changes code in an sc-agent library crate

Deterministic check to pair with the development gate: a test in each changed sc-agent library crate injects an in-memory GeneralLogger, runs the crate's operations and finds the log records they wrote

Excluded from (not judged; the cell passes as `excluded`):
- `agt-4p2.1`: sc-agent-types is the bottom ADR-SAG-0004 layer, below sc-agent-log, and holds data types with no I/O; it cannot take a GeneralLogger.
- `agt-4p2.2`: ANC-001 limits sc-decision-schema to the decision API types and traits with no logging code; its logging happens in the HTTP client and runner.
- `agt-4p2.4`: sc-agent-config and sc-agent-secrets share the ADR-SAG-0004 layer with sc-agent-log and cannot depend on it, so they cannot take a GeneralLogger; their callers log config and key-source events.
- `agt-4p2.3`: REQ-SAG-0301 makes sc-treesitter a stand-alone library that depends on no sc-agent crate, so it cannot take sc-agent's GeneralLogger; the tsq binary in A-3 writes its own tsq.log through sc-observability (A-3's REQ-SAG-0003 line).

Beads: anchor `agt-em9`, plan questions `agt-em9.1`, dev questions `agt-em9.2`

Fixtures:
- pass: sc-agent-http's existing request code is updated to write a log record for each request it sends, checked by a test that injects an in-memory GeneralLogger and reads the records back.
- fail: sc-agent-http's request code is left as it is and writes no log records; logging is deferred to a later phase.
