# sc-jev-check — requirements

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0300 | Extractors: `file`, `table-column`, `tree-sitter`, `command` | a-3 |
| REQ-SAG-0304 | `tree-sitter` + `.scm` queries; subject rules; python-branches gate | a-3 |
| REQ-SAG-0305 | `command` extractor | a-3 |
| REQ-SAG-0309 | `table-column` unescaped pipes | a-3 |
| REQ-SAG-0503 | LLM system prompt config (parse a-3; read a-6) | a-3 / a-6 |
| REQ-SAG-0600 | Sessions | a-6 |
| REQ-SAG-0601 | `run_check` / dry-run | a-6 |
| REQ-SAG-0602 | Equal-input record | a-6 |
| REQ-SAG-0603 | Multi-backend run | a-6 |
| REQ-SAG-0610 | Session logging via `SharedSessionLog` | a-6 |
| REQ-SAG-0611 | Log fields, no secrets | a-6 |

Normative extraction rules: [`extraction.md`](extraction.md).
