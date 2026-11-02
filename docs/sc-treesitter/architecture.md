# sc-treesitter — architecture

**Boundary:** `treesitter` (Phase A)  
**Depends on:** `sc-agent-types`, pinned `tree-sitter-*` grammar crates (features)  
**Does not depend on:** `sc-jev-check`, `sc-jev`, any judge crate

Tree-sitter producer for **`sc/subjects/1`** ([`subjects-schema.md`](../sc-agent-types/subjects-schema.md)).

## Grammars

Static, pinned, feature-flagged (`python` + `rust` default; optional grammars behind features). **No** dynamic loading in Phase A.

## Query library

`assets/queries/<language>/<purpose>.scm` — e.g. `--language python --query branches` → `queries/python/branches.scm`.

## CLI (sc-ai-cli conventions)

- `sc-treesitter extract --language L --query Q [--captures arm,refusal] [--rev HEAD] <path>` → subjects JSON on stdout. **`--captures`** is an allow-list only; execution always uses tree-sitter **`matches`** mode and records `producer.mode = "matches"`.
- `sc-treesitter languages` / `sc-treesitter queries [--language L]`

Errors on stderr as **`JevError`** codes (ADR-SAG-0001): `LANGUAGE_UNAVAILABLE`, `QUERY_INVALID`, `QUERY_NOT_FOUND`, `INPUT_MISSING`, plus shared `EXTRACTION_*` ids.

## Library + tests

Library target; binary is thin wrapper. Named tests listed in sprint **a-11** (six schema-rule tests + `test_matches_mode_duplicate_captures` + `test_subjects_schema_roundtrip`).
