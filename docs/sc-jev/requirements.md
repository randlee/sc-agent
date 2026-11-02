# sc-jev — requirements

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0110 | **`sc-jev` owns 100% of TypeSafe Jev HTTP API** over the product lifetime; Phase A implements the check/judge slice only | a-4+ |
| REQ-SAG-0300 | Extractors: `file`, `table-column`, `delimited-regions` | a-3 |
| REQ-SAG-0304 | Multi-id delimited regions; optional `regions` filter; no in-crate AST | a-3 |
| REQ-SAG-0301 | Pre-send validation | a-3 |
| REQ-SAG-0302 | Repo root | a-3 |
| REQ-SAG-0303 | Question sets | a-3 |
| REQ-SAG-0400 | Jev API transport (injectable); byte-exact judge request | a-4 |
| REQ-SAG-0401 | Jev busy retry policy | a-4 |
| REQ-SAG-0600 | Sessions | a-6 |
| REQ-SAG-0601 | `run_check` / dry-run | a-6 |
| REQ-SAG-0602 | Equal-input record | a-6 |
| REQ-SAG-0603 | Multi-backend run | a-6 |
| REQ-SAG-0800 | PyO3 re-exports (via `sc-jev-py`) | a-8 |

Public library for `sc-ai` and Python; no argv parsing in this crate.
