# sc-agent-secrets — requirements

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0200 | env → keyring for `TYPESAFE_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` | a-2 |
| REQ-SAG-0201 | `ANTHROPIC_KEYCHAIN_SERVICE` optional override | a-2 |

Missing key → `KEY_MISSING` with fix text from `sc-agent-types`.
