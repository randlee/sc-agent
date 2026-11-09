#!/usr/bin/env bash
# Sprint A-3 (bead agt-4p2.3): sc-treesitter library.
# Copies the old sc-treesitter library code (sc-agent-ref @ dce239a) into crates/sc-treesitter/.
# Usage: bash docs/plans/phase-a/copy/a-3.sh   (TARGET=<dir> for a dry run)
#
# What this script does NOT do (the developer does it; see the bead design):
#   - It writes no Cargo.toml, README.md, src/error.rs, src/python.rs or fixtures: they are new code.
#   - It does not remove `use sc_agent_types::...` lines inside copied ranges, the JevError calls,
#     PRODUCER_NAME, the u32 narrowing, the two debug_assert! blocks or the `capture_names[index]` index.
#   - It does not rename `extract` to `run_query`, the grammar features `python`/`rust` to
#     `grammar-python`/`grammar-rust`, or `subjects_to_json_bytes` to `to_json_bytes`.
#   - It does not rewrite the copied tests: they still call the old `run(..., path, ...)` helper,
#     `error.code()` strings and SubjectsDocument accessors; they must be ported to the new API.
#   - It does not strip doc comments that mention sc/subjects/1, query_sha256, Phase A or git.
#   - It copies nothing for the CLI binary (cli.rs, main.rs, tests/cli.rs): that is Sprint A-10 (a-10.sh).
#   - It adds no C# or Go grammar rows or queries (second wave).
. "$(dirname "$0")/lib.sh"

S=crates/sc-treesitter          # source crate in sc-agent-ref
D=crates/sc-treesitter          # target crate in this repo

copy_file "$S/LICENSE" "$D/LICENSE"

# Grammar table: LanguageInfo, Grammar, python/rust loaders and rows, languages(), language(), lookup(), all().
copy_lines "$S/src/grammar.rs" 12 100 "$D/src/grammar.rs"

# Query assets: QueryInfo, PYTHON/RUST consts, queries(), query_source().
copy_lines "$S/src/query.rs" 9 75 "$D/src/query.rs"

# Query run core -> src/run.rs: imports, RawCapture, extract() body (rewritten to run_query),
# capture_allowed, inclusive_lines, error_line_inside.
copy_lines "$S/src/extract.rs" 7 7     "$D/src/run.rs"
copy_lines "$S/src/extract.rs" 132 138 "$D/src/run.rs"
copy_lines "$S/src/extract.rs" 176 252 "$D/src/run.rs"
copy_lines "$S/src/extract.rs" 273 280 "$D/src/run.rs"
copy_lines "$S/src/extract.rs" 293 312 "$D/src/run.rs"
copy_lines "$S/src/extract.rs" 326 359 "$D/src/run.rs"
# Unit tests (to port): rust match arms, same-line determinism, parse error, unknown language,
# duplicate captures, empty result, invalid query, MISSING node, clean source.
copy_lines "$S/src/extract.rs" 361 363 "$D/src/run.rs"
copy_lines "$S/src/extract.rs" 494 523 "$D/src/run.rs"
copy_lines "$S/src/extract.rs" 552 611 "$D/src/run.rs"
copy_lines "$S/src/extract.rs" 639 658 "$D/src/run.rs"
copy_lines "$S/src/extract.rs" 679 702 "$D/src/run.rs"
copy_lines "$S/src/extract.rs" 734 734 "$D/src/run.rs"

# Crate root: module wiring and re-exports, then the pretty-JSON encoder (render.rs) folded in.
copy_lines "$S/src/lib.rs" 34 59 "$D/src/lib.rs"
copy_lines "$S/src/render.rs" 6 21 "$D/src/lib.rs"

# Predefined queries (first wave). python/branches.scm: only the case_clause half (lines 2-5);
# the Refusal/lite-gate half is not copied.
copy_file  "$S/assets/queries/rust/match-arms.scm" "$D/assets/queries/rust/match-arms.scm"
copy_lines "$S/assets/queries/python/branches.scm" 2 5 "$D/assets/queries/python/branches.scm"
