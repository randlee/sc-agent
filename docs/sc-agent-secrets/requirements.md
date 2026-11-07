# sc-agent-secrets — requirements

**Sprint** = the sprint that implements the requirement. The owning `req:`
acceptance root, when it differs, is shown as "root a-N"; the closure table in
[`plans/phase-a/plan.md`](../plans/phase-a/plan.md#requirement-closure-integration-req-roots) is authoritative.

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0200 | `resolve_key(id, repo_root)` env → `.env` → keyring | a-2 |
| REQ-SAG-0201 | `ANTHROPIC_KEYCHAIN_SERVICE` optional override | a-2 |
| REQ-SAG-0202 | Repo-root `.env`; process env wins | a-2 |

Missing key → `KEY_MISSING` with fix text from `sc-agent-types`.
