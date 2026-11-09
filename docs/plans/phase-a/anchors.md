# Anchors - phase-a

Constraints captured during planning. Each anchor is a testable statement written by the orchestrator and approved by the user; the user's verbatim words are kept beneath it as provenance. Every anchor is judged at the planning gate and again at the development gate. Edit `anchors.json`, not this file; this file is rendered.

Intent: `docs/plans/phase-a/intent.md`

| ID | Status | Summary | Stated |
| --- | --- | --- | --- |
| ANC-001 | locked | sc-decision-schema holds only minimum decision-API code | 2026-11-09T07:40:40Z |
| ANC-002 | locked | Python API offers every sc-ai CLI operation | 2026-11-08T21:07:35Z |
| ANC-003 | proposed | No requirements sourced from plan-anchor or judge-jev | 2026-11-09T07:24:54Z |
| ANC-004 | proposed | Library code logs through injected sc-observability logger | 2026-11-09T00:26:57Z |
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

## ANC-001

Anchor: crates/sc-decision-schema must contain only the code necessary to send and receive 100% of the decision API, with nothing beyond that minimum.

> Has the absolute minimum code that is necessary to send/receive 100% of the decision api

Said by Rand Lee, planning session, 2026-11-09T07:40:40Z. Status: locked. Version 1.

Summary: sc-decision-schema holds only minimum decision-API code

Scope: adds or changes code in crates/sc-decision-schema

Deterministic check to pair with the development gate: every public item in crates/sc-decision-schema maps to a TypeSafe API request or response field, or to the decision trait

Beads: anchor `agt-e49`, plan questions `agt-e49.1`, dev questions `agt-e49.2`

## ANC-002

Anchor: Every operation available in the sc-ai CLI must also be available in the sc-agent Python API.

> I expect the python api may allow more flexibility than cli, NOT less.

Said by Rand Lee, planning session, 2026-11-08T21:07:35Z. Status: locked. Version 1.

Summary: Python API offers every sc-ai CLI operation

Scope: adds or changes an sc-ai command or the sc-agent Python API

Deterministic check to pair with the development gate: each sc-ai subcommand has a matching Python API function, checked by a test that lists both

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

Said by Rand Lee, planning session, 2026-11-09T00:26:57Z. Status: proposed. Version 1.

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

Said by Rand Lee, record ADR-SAG-0001, 2026-11-09. Status: locked. Version 1.

Summary: Every decision backend implements Backend; callers use Decision

Scope: adds or changes a decision backend, the Backend or Decision trait, or sc-decision-runner

Beads: anchor `agt-7mi`, plan questions `agt-7mi.1`, dev questions `agt-7mi.2`

## ADR-SAG-0004

Anchor: No sc-agent crate's Cargo.toml must depend on a crate in a higher layer, where the layers from the bottom are: types; config, secrets and log; http; decision schema; decision backends; runner.

> Layers from the bottom: `sc-agent-types`; `sc-agent-config`, `sc-agent-secrets` and `sc-agent-log`; `sc-agent-http`; `sc-decision-schema`; the decision backends `sc-decision-systemone`, `sc-decision-local` and `sc-decision-llm` (over `sc-llm-provider`); `sc-decision-runner`, which handles decision models only.

Said by Rand Lee, record ADR-SAG-0004, 2026-11-09. Status: locked. Version 1.

Summary: No crate depends on a crate above it in the layering

Scope: adds or changes a crate dependency in a Cargo.toml

Beads: anchor `agt-mud`, plan questions `agt-mud.1`, dev questions `agt-mud.2`

## REQ-SAG-0002

Anchor: Every error returned by a public sc-agent function must carry the id and message of each lower-layer error that caused it.

> An error MUST carry the error id and message of every lower-layer error that caused it.

Said by Rand Lee, record REQ-SAG-0002, 2026-11-09. Status: locked. Version 1.

Summary: Each crate returns its own error union carrying lower errors

Scope: adds or changes an error type or a public function's return type

Beads: anchor `agt-t70`, plan questions `agt-t70.1`, dev questions `agt-t70.2`

## REQ-SAG-0006

Anchor: No type, field or constant in the sc-agent-types or sc-decision-schema crates must be specific to plan-anchor, judge-jev or another application built on sc-agent.

> `sc-agent-types` and `sc-decision-schema` MUST NOT contain types specific to plan-anchor, judge-jev or any other consumer application.

Said by Rand Lee, record REQ-SAG-0006, 2026-11-09. Status: locked. Version 1.

Summary: No consumer-app or Jev-mock types in shared crates

Scope: adds or changes a type in sc-agent-types or sc-decision-schema

Beads: anchor `agt-qyd`, plan questions `agt-qyd.1`, dev questions `agt-qyd.2`

## REQ-SAG-0009

Anchor: No low-level sc-agent crate (types, config, secrets, log, http, decision schema, decision backends, runner) must depend on tokio with its default features.

> A program that does not use tokio MUST be able to use every low-level crate.

Said by Rand Lee, record REQ-SAG-0009, 2026-11-09. Status: locked. Version 1.

Summary: Low-level crates build and run without tokio

Scope: adds or changes dependencies of a low-level sc-agent crate

Beads: anchor `agt-wth`, plan questions `agt-wth.1`, dev questions `agt-wth.2`

## REQ-SAG-0010

Anchor: sc-agent source code must not call std::thread::sleep, tokio::time::sleep or any other sleep function.

> sc-agent source code MUST NOT call `std::thread::sleep`, `std::thread::sleep_until`, `tokio::time::sleep` or any other sleep function.

Said by Rand Lee, record REQ-SAG-0010, 2026-11-09. Status: locked. Version 1.

Summary: No sleep calls in sc-agent source

Scope: adds or changes Rust source in an sc-agent crate

Beads: anchor `agt-6dm`, plan questions `agt-6dm.1`, dev questions `agt-6dm.2`

## REQ-SAG-0201

Anchor: Every Rust type that maps to a field of the TypeSafe Jev API (POST /v1/systemone, GET /v1/models) must be defined in the sc-decision-schema crate.

> Every type that maps to the Jev API MUST be defined in `sc-decision-schema`.

Said by Rand Lee, record REQ-SAG-0201, 2026-11-09. Status: locked. Version 1.

Summary: Jev API types defined only in sc-decision-schema, built from the spec

Scope: adds or changes a type that maps to a TypeSafe API field

Beads: anchor `agt-gc6`, plan questions `agt-gc6.1`, dev questions `agt-gc6.2`

## REQ-SAG-0601

Anchor: A request that receives an HTTP 429, 502 or 529 response must be resent until a configurable maximum number of sends, 5 by default, is reached.

> A request that receives a busy response MUST be resent unchanged.

Said by Rand Lee, record REQ-SAG-0601, 2026-11-09. Status: locked. Version 1.

Summary: Busy resend: configurable sends, holdoff in the queue

Scope: adds or changes busy-response handling, resend, holdoff or the runner queue

Beads: anchor `agt-ebd`, plan questions `agt-ebd.1`, dev questions `agt-ebd.2`

## REQ-SAG-0701

Anchor: sc-decision-runner must send the requests of one batch in parallel across a configurable number of worker threads, 8 by default.

> `sc-decision-runner` MUST run the requests of one batch in parallel across workers.

Said by Rand Lee, record REQ-SAG-0701, 2026-11-09. Status: locked. Version 1.

Summary: Batches run in parallel, fastest first, failures returned

Scope: adds or changes batch handling in sc-decision-runner or decide

Beads: anchor `agt-ocy`, plan questions `agt-ocy.1`, dev questions `agt-ocy.2`
