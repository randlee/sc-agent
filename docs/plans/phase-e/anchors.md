# Anchors - docs/plans/phase-e.jsonl

Constraints captured during planning. Each anchor is a testable statement written by the orchestrator and approved by the user; the user's verbatim words are kept beneath it as provenance. Every anchor is judged at the planning gate and again at the development gate. Edit `anchors.json`, not this file; this file is rendered.

Intent: `docs/plans/phase-e/intent.md`

| ID | Status | Summary | Stated |
| --- | --- | --- | --- |
| ANC-001 | locked | No second Logger around an injected logger (ban python_logger) | 2026-11-18 |

## ANC-001

Anchor: When a caller passes its own sc_observability.Logger into a wheel call (general_log= or session_log=), sc-agent MUST NOT build a second sc-observability Logger with LoggerBuilder for that log, as the python_logger function in crates/sc-agent-py does.

> we created a oversized set of sprints expressly to normalize config and logging around sc-observability logger (injected).  why do you keep adding more loggers to the design? ... add a new anchor banning python_logger ... lock this

Said by Rand Lee, phase-e/f/g planning session, 2026-11-18. Status: locked. Version 4.

Summary: No second Logger around an injected logger (ban python_logger)

Scope: touches crates/sc-agent-log, crates/sc-agent-py, crates/sc-treesitter-py or the sc-decidr generated client, or any code path that accepts an injected logger (general_log=, session_log=)

Deterministic check to pair with the development gate: git grep -n 'fn python_logger' crates returns nothing, and no LoggerBuilder call in crates/sc-agent-py, crates/sc-treesitter-py or crates/sc-agent-log is reached from a path that received an injected Python Logger

Beads: anchor `agt-bllk`, plan questions `agt-bllk.1`, dev questions `agt-bllk.2`

Fixtures:
- pass: python_logger is deleted: `git grep -n 'fn python_logger' crates` returns nothing, and `git grep -n LoggerBuilder crates/sc-agent-py crates/sc-treesitter-py` returns nothing; the sitr wheel's general_log= hands each record straight to the caller's Logger.log(LogEvent), shown by tests/test_sitr_logging.py::test_injected_logger_receives_every_record passing.
- fail: e-3 moves python_logger into a shared sc-agent-pylog crate so both wheels wrap the injected Logger in their own sc-observability Logger the same way.
