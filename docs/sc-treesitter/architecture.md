# sc-treesitter — architecture

**Boundary:** `treesitter` (Phase A)  
**Depends on:** `sc-agent-types`, `mimalloc`, `serde_json`, `tree-sitter`, and the pinned grammar crates `tree-sitter-python` / `tree-sitter-rust` (optional, features `python` / `rust`); matches `boundaries/sc-treesitter/treesitter.toml`  
**Does not depend on:** `sc-jev-check`, `sc-jev`, `sc-jev-backends`, `sc-agent-secrets`, any HTTP crate
**Ships:** library plus the `sc-treesitter` binary (second released binary next to `sc-ai`)

Tree-sitter producer for **`sc/subjects/1`** ([`subjects-schema.md`](../sc-agent-types/subjects-schema.md)).

## Grammars

Static, pinned, feature-flagged (`python` + `rust` default; optional grammars behind features). **No** dynamic loading in Phase A.

## Query library

`assets/queries/<language>/<purpose>.scm` — e.g. `--language python --query branches` → `queries/python/branches.scm`.

## CLI (sc-ai-cli conventions)

- `sc-treesitter extract --language L --query Q [--captures arm,refusal] [--rev HEAD] <path>` → subjects JSON on stdout. **`--captures`** is an allow-list only; execution always uses tree-sitter **`matches`** mode and records `producer.mode = "matches"`.
- `sc-treesitter languages` / `sc-treesitter queries [--language L]`

**Helper captures.** Capture names beginning with `_` (stored without `@`)
are helper captures: queries use them for predicates and they are never
emitted as subjects ([`subjects-schema.md`](../sc-agent-types/subjects-schema.md) rule **helpers**).

Errors on stderr as **`JevError`** codes (ADR-SAG-0001): `LANGUAGE_UNAVAILABLE`, `QUERY_INVALID`, `QUERY_NOT_FOUND`, `INPUT_MISSING`, plus shared `EXTRACTION_*` ids.

## Library API notes (a-10)

Recorded by the Phase A phase-end hardening (a-10):

- `sc_treesitter::languages()` returns
  `impl ExactSizeIterator<Item = &'static LanguageInfo>` (was
  `&'static [LanguageInfo]`). Both come from the one cfg-gated grammar table.
- `sc_treesitter::queries(language)` returns
  `Result<Vec<&'static QueryInfo>, JevError>` (was `Vec<QueryInfo>` by
  value); an uncompiled `language` is `LANGUAGE_UNAVAILABLE`.
- `Rev::parse` rejects a revision that starts with `-` as `INPUT_INVALID`,
  so `--rev=--output=<file>` never reaches git. Git reads use
  `rev-parse --verify --end-of-options` and the resolved object id.
- `--rev WORKTREE` hashes the bytes it returns with
  `git hash-object -t blob --stdin --no-filters`, so `blob_sha` is the blob id
  of exactly those bytes on every platform.
- Reads are capped at 8 MiB (`INPUT_TOO_LARGE`), matching `sc-jev-check`.
- Failure codes from git reads and argument parsing use the existing
  `INPUT_INVALID`, `INPUT_MISSING` and `EXTRACTION_FAILED` ids (ADR-SAG-0029,
  error-code dispositions). No new ids.

## Library + tests

Library target; binary is thin wrapper. Named tests listed in sprint **a-11** (six schema-rule tests + `test_matches_mode_duplicate_captures` + `test_subjects_schema_roundtrip`).
