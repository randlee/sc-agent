#!/usr/bin/env bash
# Sprint A-10 (bead agt-4p2.10): the `decide` CLI and the tree-sitter CLI (name not decided).
# Copies argument-parsing and output code from sc-agent-ref @ dce239a.
# Usage: bash docs/plans/phase-a/copy/a-10.sh   (TARGET=<dir> for a dry run)
#
# What this script does NOT do (the developer does it; see the bead design):
#   - It writes no Cargo.toml for crates/decide and adds no [[bin]]/[features] to crates/sc-treesitter/Cargo.toml.
#   - It writes no src/error.rs for decide and no decide test fixtures: they are new code.
#   - It does not replace JevError, ScAgentErrorResponse, Rev, the `extract` subcommand, `--rev`,
#     git_io, mimalloc or the `message()`/`value_parser_error`/`usage_error` mapping in copied ranges.
#   - It does not rename `extract` to `query` or turn its flags into the positional
#     `query <language> <query> <file>` form, and does not set the tree-sitter binary name.
#   - It does not fix the `&available[..take]` slice in read_line_bounded (indexing_slicing lint).
#   - It does not port the copied tests (they still assert the old error envelope / python+rust order).
#   - It copies nothing from sc-ai's command dispatch (sc_jev_host compose, envelopes, readbacks,
#     check.toml, ask, sessions): decide runs through the Decision trait instead.
. "$(dirname "$0")/lib.sh"

T=crates/sc-treesitter          # tree-sitter CLI lives in the sc-treesitter crate behind the `cli` feature
A=crates/sc-ai
H=crates/sc-jev-host/src/stream
D=crates/decide

# ---- tree-sitter CLI (crates/sc-treesitter/src/cli.rs, src/main.rs, tests/cli.rs) ----
copy_lines "$T/src/cli/mod.rs" 11 12   "$T/src/cli.rs"     # clap imports
copy_lines "$T/src/cli/mod.rs" 36 77   "$T/src/cli.rs"     # Cli, Command, ExtractArgs (language, query, captures)
copy_lines "$T/src/cli/mod.rs" 87 144  "$T/src/cli.rs"     # path arg, QueriesArgs, Captures, Parsed, parse()
copy_lines "$T/src/cli/mod.rs" 201 235 "$T/src/cli.rs"     # parse_captures, tests: cli_definition_is_consistent
copy_lines "$T/src/cli/mod.rs" 257 265 "$T/src/cli.rs"     # test: captures_are_trimmed_and_ordered
copy_lines "$T/src/main.rs" 8 9     "$T/src/main.rs"       # io / ExitCode imports
copy_lines "$T/src/main.rs" 25 35   "$T/src/main.rs"       # main()
copy_lines "$T/src/main.rs" 50 77   "$T/src/main.rs"       # run(): languages, queries dispatch
copy_lines "$T/src/main.rs" 80 83   "$T/src/main.rs"       # captures -> Option<Vec<&str>>
copy_lines "$T/src/main.rs" 98 103  "$T/src/main.rs"       # json_line()
copy_lines "$T/tests/cli.rs" 12 12   "$T/tests/cli.rs"     # use std::path::Path
copy_lines "$T/tests/cli.rs" 20 22   "$T/tests/cli.rs"     # binary()
copy_lines "$T/tests/cli.rs" 33 37   "$T/tests/cli.rs"     # run() (drop isolate_git / deadline)
copy_lines "$T/tests/cli.rs" 54 62   "$T/tests/cli.rs"     # error_json(), trim_newline()
copy_lines "$T/tests/cli.rs" 441 465 "$T/tests/cli.rs"     # languages + queries listing test

# ---- decide CLI (crates/decide) ----
copy_file  "$A/LICENSE" "$D/LICENSE"
copy_lines "$T/src/cli/mod.rs" 111 144 "$D/src/cli.rs"     # Parsed + parse() pattern (help/version vs error)
copy_lines "$A/src/cli.rs" 560 572 "$D/src/cli.rs"         # non_empty_text / non_empty_path value parsers
copy_lines "$A/src/cli.rs" 610 618 "$D/src/cli.rs"         # tests: cli_definition_is_consistent
copy_lines "$A/src/command.rs" 412 429 "$D/src/input.rs"   # request_bytes: file or `-` for stdin
copy_lines "$H/mod.rs" 797 845 "$D/src/input.rs"           # LineRead + read_line_bounded (JSONL batch)
copy_lines "$H/tests.rs" 7 51 "$D/src/input.rs"            # read_line_bounded tests (wrap in #[cfg(test)] mod)
copy_lines "$A/src/main.rs" 77 82 "$D/src/main.rs"         # print_stdout
copy_lines "$A/src/main.rs" 90 104 "$D/src/main.rs"        # emit: one stderr JSON line (remove .expect)
