# Subjects artifact — `sc/subjects/1`

**Owner:** `sc-agent-types` (schema id + Rust types).  
**Producers:** `sc-treesitter`, future tools.  
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
    "grammar_version": "0.25.0",
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
| **Lines** | `start_line` / `end_line` are **1-based**; `end_line` inclusive |
| **id** | `{path}:{start_line}` for the first subject on that line; further subjects on the same line get `:1`, `:2`, … in `(start_byte, capture)` order |
| **text** | Half-open UTF-8 slice **`[start_byte, end_byte)`** from source bytes |
| **order** | Sort emitted subjects by `(start_byte, capture)` |
| **helpers** | Helper captures never appear in `subjects`. A helper capture is a capture name beginning with `_` (stored without `@`), for example `_fn` for `@_fn` in the query |
| **empty** | Zero subjects → `EXTRACTION_EMPTY` |
| **parse errors** | ERROR node inside a subject span → `EXTRACTION_PARSE_ERROR` (path + line) |
| **query mode** | Tree-sitter producers run **`matches`** only; record `"mode": "matches"` in `producer` |

### Same-line example

Two captures on line 42 → ids `path/to/file.py:42` and `path/to/file.py:42:1`.

## Fixture

Sprint **a-1** commits `tests/fixtures/jev/subjects-schema/v1/minimal.json` including
at least one same-line duplicate id case; validation in a-1 AC.
