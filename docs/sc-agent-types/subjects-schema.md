# Subjects artifact — `sc/subjects/1`

**Owner:** `sc-agent-types` (schema id + Rust types).  
**Producers:** `sc-treesitter`, future `syn-grep`, scripts, etc.  
**Consumers:** `sc-jev-check`, Phase B agents — **no parser crates in consumers**.

Normative JSON (Phase A example):

```json
{
  "schema": "sc/subjects/1",
  "source": {
    "path": "skills/atm-bd-lite/scripts/atm-bd-task-hook.py",
    "rev": "HEAD",
    "blob_sha": "<git blob sha>"
  },
  "producer": {
    "name": "sc-treesitter",
    "version": "0.1.0",
    "language": "python",
    "grammar": "tree-sitter-python",
    "grammar_version": "0.23.6",
    "query_sha256": "<hex>",
    "mode": "matches"
  },
  "subjects": [
    {
      "id": "skills/atm-bd-lite/scripts/atm-bd-task-hook.py:31",
      "capture": "arm",
      "text": "Transition.GIVE_BACK",
      "span": {
        "start_line": 31,
        "end_line": 31,
        "start_byte": 1180,
        "end_byte": 1200
      }
    }
  ]
}
```

## Rules (all producers)

| Rule | Requirement |
| --- | --- |
| **id** | `{path}:{start_line}`; `:n` suffix for same-line duplicates |
| **text** | Exact UTF-8 byte slice from source (not pretty-print / `unparse`) |
| **order** | Sort subjects by `(start_byte, capture)` |
| **helpers** | Capture names `@_…` never appear in `subjects` |
| **empty** | Zero subjects → producer error (`EXTRACTION_EMPTY`) |
| **parse errors** | ERROR node inside a subject span → `EXTRACTION_PARSE_ERROR` (path + line) |
| **query mode** | Tree-sitter producers use **`matches`**, not `captures` (duplicate-capture semantics differ); record `"mode": "matches"` in `producer` |

## Fixture

Sprint **a-1** commits `tests/fixtures/jev/subjects-schema/v1/minimal.json` validating against these rules (roundtrip + golden fields).
