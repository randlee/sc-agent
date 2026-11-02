# sc-jev-backends — requirements

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0500 | Haiku backend | a-5 |
| REQ-SAG-0501 | Luna backend + token count wire (feature 8); live probe optional | a-5 |
| REQ-SAG-0503 | Injectable system prompt (caller/config); no hardcoded production prose | a-5 |
| REQ-SAG-0504 | stop_reason / status → RESPONSE_INVALID | a-5 |
| REQ-SAG-0505 | LLM busy retry like Jev | a-5 |
| REQ-SAG-0502 | LLM oversize refusal | a-5 |

REQ-SAG-0400/0401 (Jev HTTP) belong to **`sc-jev`**, not this crate.

Each backend implements `JudgeBackend`.
