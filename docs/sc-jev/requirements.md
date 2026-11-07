# sc-jev — requirements

**Sprint** = the sprint that implements the requirement. The owning `req:`
acceptance root, when it differs, is shown as "root a-N"; the closure table in
[`plans/phase-a/plan.md`](../plans/phase-a/plan.md#requirement-closure-integration-req-roots) is authoritative.

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0110 | **`sc-jev` owns 100% of TypeSafe Jev HTTP**; Phase A check/judge slice | a-4+ |
| REQ-SAG-0400 | Injectable transport at construction; byte-exact judge body | a-4 |
| REQ-SAG-0401 | Jev busy retry policy | a-4 |

Public HTTP library; no argv parsing, no `check.toml`, no extractors.

## Phase A.1 (r-3)

Transport, busy retry (REQ-SAG-0505 as amended), caps and redaction moved to
[`sc-agent-http`](../sc-agent-http/requirements.md) (REQ-SAG-0402).
REQ-SAG-0400 is kept: the `sc_agent_http::Transport` is injected at
construction and the body stays byte-exact. REQ-SAG-0401 is now the shared
`BusyPolicy` with the packaged `attempts = 4`.
