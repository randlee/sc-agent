# sc-ai — requirements

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0700 | `sc-ai jev` subcommands, JSON envelope | a-7 |
| REQ-SAG-0701 | Typed stderr errors, exit 2 | a-7 |
| REQ-SAG-0702 | Readback pairs | a-7 |
| REQ-SAG-0703 | Install documentation | a-7 |
| REQ-SAG-0710 | Integration test: versioned sc-ai-cli envelope; Jev answer + `session_id` in `data` | a-7 |
| REQ-SAG-0711 | `sc-observability = "1.5.0"` + v2 `LoggerBuilder` adapter | a-7 |
| NFR-SAG-0002 | No secrets in output | a-7 |
| NFR-SAG-0003 | Document default log root | a-7 |

Phase A: `sc-ai jev` check parity only. Phase B: more Jev API + API-direct Claude/Codex (REQ-SAG-0900/0901); never vendor CLI wrappers.
