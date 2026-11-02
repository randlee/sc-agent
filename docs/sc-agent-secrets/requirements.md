# sc-agent-secrets — requirements

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0200 | `resolve_key(id, repo_root)` env → `.env` → keyring | a-2 |
| REQ-SAG-0201 | `ANTHROPIC_KEYCHAIN_SERVICE` optional override | a-2 |
| REQ-SAG-0202 | Repo-root `.env`; process env wins | a-2 |

Missing key → `KEY_MISSING` with fix text from `sc-agent-types`.
