# sc-treesitter — architecture

**Boundary:** `treesitter` (Phase A)  
**Depends on:** `sc-agent-types`, pinned `tree-sitter-*` grammar crates (features)  
**Does not depend on:** `sc-jev-check`, `sc-jev`, any judge crate

Tree-sitter-specific producer for the **`sc/subjects/1`** artifact ([`subjects-schema.md`](../sc-agent-types/subjects-schema.md)). Encapsulates `.scm` queries as crate assets; emits subjects JSON for injection into checks and agents.

## Grammars

Static, pinned, feature-flagged in `Cargo.toml` (`python` + `rust` default; optional `c-sharp`, `bash`, `toml`, `json`, markdown). **No** dynamic loading (`parser-directories`, user config dirs) in Phase A.

## Query library

Shipped under `crates/sc-treesitter/assets/queries/<language>/<purpose>.scm`.

- CLI: `--query branches` → `queries/python/branches.scm` (with `--language python`).
- `--query-file path.scm` for ad hoc runs; `query_sha256` always recorded.

Phase A ships at least `python/branches.scm` and `rust/match-arms.scm`.

## CLI (sc-ai-cli conventions)

- `sc-treesitter extract --language L --query Q [--captures …] [--rev HEAD] <path>` → subjects JSON on stdout.
- `sc-treesitter languages` — compiled grammars + versions.
- `sc-treesitter queries [--language L]` — asset index.

Reads file content at git rev via `git show <rev>:<path>`; repo root from `git rev-parse --show-toplevel`; records `blob_sha`.

## Library

`sc-treesitter` library target; binary is a thin wrapper (future single-binary feature can link it). Phase A: no other workspace crate depends on the library.

## Errors

Typed enum with fix text: `LANGUAGE_UNAVAILABLE`, `QUERY_INVALID`, `QUERY_NOT_FOUND`, `INPUT_MISSING`, `EXTRACTION_EMPTY`, `EXTRACTION_PARSE_ERROR`.

## Tests

Six `test_tree_sitter_*` cases from the former `extraction.md` list, plus `test_matches_mode_duplicate_captures` and `test_subjects_schema_roundtrip`. Producer fixture: committed `subjects.json` byte-for-byte reproduction from hook source + query.
