#!/usr/bin/env bash
# Sprint A-10 (bead agt-4p2.10): the `decidr` CLI and library (the sc-agent-py crate is new code).
# Copies argument-parsing and output code from sc-agent-ref @ dce239a.
# Usage: bash docs/plans/phase-a/copy/a-10.sh   (TARGET=<dir> for a dry run)
#
# What this script does NOT do (the developer does it; see the bead design):
#   - It writes no Cargo.toml for crates/decidr or crates/sc-agent-py.
#   - It writes no src/error.rs for decidr and no decidr test fixtures: they are new code.
#   - It does not replace JevError, ScAgentErrorResponse, mimalloc or the
#     `message()`/`value_parser_error`/`usage_error` mapping in copied ranges.
#   - It copies nothing for the sitr binary: those rows moved to Sprint A-3 (a-3.sh).
#   - It does not fix the `&available[..take]` slice in read_line_bounded (indexing_slicing lint).
#   - It does not port the copied tests (they still assert the old error envelope).
#   - It copies nothing from sc-ai's command dispatch (sc_jev_host compose, envelopes, readbacks,
#     check.toml, ask, sessions): decidr runs through the Decision trait instead.
. "$(dirname "$0")/lib.sh"

T=crates/sc-treesitter          # source of the Parsed/parse() pattern (the sitr rows are in a-3.sh)
A=crates/sc-ai
H=crates/sc-jev-host/src/stream
D=crates/decidr

# ---- decidr CLI (crates/decidr) ----
copy_file  "$A/LICENSE" "$D/LICENSE"
copy_lines "$T/src/cli/mod.rs" 111 144 "$D/src/cli.rs"     # Parsed + parse() pattern (help/version vs error)
copy_lines "$A/src/cli.rs" 560 572 "$D/src/cli.rs"         # non_empty_text / non_empty_path value parsers
copy_lines "$A/src/cli.rs" 610 618 "$D/src/cli.rs"         # tests: cli_definition_is_consistent
copy_lines "$A/src/command.rs" 412 429 "$D/src/input.rs"   # request_bytes: file or `-` for stdin
copy_lines "$H/mod.rs" 797 845 "$D/src/input.rs"           # LineRead + read_line_bounded (JSONL batch)
copy_lines "$H/tests.rs" 7 51 "$D/src/input.rs"            # read_line_bounded tests (wrap in #[cfg(test)] mod)
copy_lines "$A/src/main.rs" 77 82 "$D/src/main.rs"         # print_stdout
copy_lines "$A/src/main.rs" 90 104 "$D/src/main.rs"        # emit: one stderr JSON line (remove .expect)
