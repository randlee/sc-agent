# sc-jev-check — requirements

**Sprint** = the sprint that implements the requirement. The owning `req:`
acceptance root, when it differs, is shown as "root a-N"; the closure table in
[`plans/phase-a/plan.md`](../plans/phase-a/plan.md#requirement-closure-integration-req-roots) is authoritative.

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0300 | Extractors: `file`, `table-column`, `subjects`, `command` | a-3 |
| REQ-SAG-0311 | `subjects` input: validate `sc/subjects/1`, map ids/text | a-3 |
| REQ-SAG-0305 | `command` stdout = subjects schema | a-3 |
| REQ-SAG-0309 | `table-column` unescaped pipes | a-3 |
| REQ-SAG-0301 | Refuse send on config/input errors | a-3 |
| REQ-SAG-0302 | Repo-root resolution | a-3 |
| REQ-SAG-0303 | Question-set schema | a-3 |
| REQ-SAG-0503 | LLM system prompt load + inject (parse field a-3; read/inject a-6) | a-6 (root a-7) |
| REQ-SAG-0600 | Sessions | a-6 (root a-7) |
| REQ-SAG-0601 | `run_check` / dry-run; `prepare`/`run` with `RunConfig::dry_run` (r-2) | a-6 (root a-7), r-2 |
| REQ-SAG-0602 | Equal-input record | a-6 |
| REQ-SAG-0603 | Multi-backend run | a-6 (root a-7) |
| REQ-SAG-0610 | Session logging via `SharedSessionLog` | a-6 (root a-8) |
| REQ-SAG-0611 | Log fields, no secrets | a-6 (root a-8) |
| REQ-SAG-0312 | `command` input `path` field substituted for `{path}` | r-2 |
| REQ-SAG-0313 | Command producer stderr error response re-raised with its code | r-2 |
| REQ-SAG-0314 | Request byte cap owned by each backend; dry-run reports `over_limit` | r-2 |
| REQ-SAG-0607 | Stateless `FileSessionStore`, no-lock concurrency model, one write per run | r-2 |
| REQ-SAG-0620 | One engine API (`prepare` → `run`) and one `RunResponse` | r-2 |

Consumer boundary: [`extraction.md`](extraction.md). Subjects schema: [`subjects-schema.md`](../sc-agent-types/subjects-schema.md).
