# sc-jev-backends — requirements

**Sprint** = the sprint that implements the requirement. The owning `req:`
acceptance root, when it differs, is shown as "root a-N"; the closure table in
[`plans/phase-a/plan.md`](../plans/phase-a/plan.md#requirement-closure-integration-req-roots) is authoritative.

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0500 | Haiku backend | a-5 |
| REQ-SAG-0501 | Luna backend + token count wire (feature 8); live probe optional | a-5 |
| REQ-SAG-0503 | Injectable system prompt (loaded by facade a-6); wire proof in a-5 under REQ-SAG-0500 | a-6 / a-5 (root a-7) |
| REQ-SAG-0504 | stop_reason / status → RESPONSE_INVALID | a-5 |
| REQ-SAG-0505 | LLM busy retry like Jev | a-5 |
| REQ-SAG-0502 | LLM oversize refusal | a-5 |

REQ-SAG-0400/0401 (Jev HTTP) belong to **`sc-jev`**, not this crate.

Each backend implements `JudgeBackend`.

## Phase A.1 (r-3)

The transport, busy retry (REQ-SAG-0505 as amended: absent or malformed
`Retry-After` backs off 1, 2, 4 s), caps and redaction are
[`sc-agent-http`](../sc-agent-http/requirements.md) (REQ-SAG-0402). This crate
keeps the wire builders and reply parsers (ADR-SAG-0019).
