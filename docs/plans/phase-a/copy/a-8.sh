#!/usr/bin/env bash
# Sprint A-8 (bead agt-4p2.8): copy the sc-agent-log starting code from sc-agent-ref@dce239a.
# Run: bash docs/plans/phase-a/copy/a-8.sh   (TARGET=<dir> to dry-run elsewhere)
#
# This script only copies text. It does NOT do any of the following; the developer does them
# (the bead design has the exact target code):
#   - write Cargo.toml, README.md, src/lib.rs, src/error.rs (LogError), src/identity.rs, src/loggers.rs;
#   - writer.rs: drop LoggerInner.pid/is_local, the fork `blocking` helper, `harden_log_files` and
#     `logs_dir`/`file_name` fields; make Drop flush+shutdown directly; rename LoggerInner to Writer;
#     replace every JevError with a LogError variant; take_lost_rows returns Option<LostRows>, not text;
#     start_logger takes a LoggerConfig and an optional extra sink, and maps errors to LogError::Start;
#     logger_config: drop the serde LevelFilter hack (take a LevelFilter), keep console off;
#   - general.rs: keep only the startup retention pass of prepare_log_files (JsonlFileSink::open on the
#     active file under the sc-observability default policy); delete create_private_dirs/0600 handling;
#   - session.rs: turn ObservabilityAdapter::log_event into SessionLogger::event(): fixed target
#     "sc_agent.session", action "session.row", correlation_id/request_id None (drop envelope_id! and its
#     debug_assert), level Info, fields = identity fields + caller payload; delete the registry,
#     Operation, level_of, action_name and every GeneralLogAdapter item;
#   - slug.rs: rename ProjectLabel to Slug and replace its rule with the REQ-SAG-0004 slug rule
#     (non-empty, only a-z 0-9 . _ -), error LogError::InvalidSlug; drop MAX_BYTES and the doc example;
#   - repo.rs: keep only the git() Command builder; drop the #[expect(disallowed_methods)] reason text
#     about revisions; add origin_url(), normalize_origin(), local_repo() (new code);
#   - tests/writer.rs: port the four tests to SessionLogger/GeneralLogger; the rotation test moves to the
#     general logger; add the session "never rotates" test.
. "$(dirname "$0")/lib.sh"

H=crates/sc-jev-host
TY=crates/sc-agent-types
T=crates/sc-agent-log

copy_file  $H/LICENSE                          $T/LICENSE
# Writer: one running sc-observability logger, flush, lost-row count, flush+shutdown on drop
copy_lines $H/src/obs.rs   107 209             $T/src/writer.rs
# File-only logger configuration and start
copy_lines $H/src/obs.rs   418 455             $T/src/writer.rs
# Startup retention pass for the general log (sc-observability's own JsonlFileSink::open)
copy_lines $H/src/obs.rs   486 528             $T/src/general.rs
# Session row -> sc-observability LogEvent
copy_lines $H/src/obs.rs   310 349             $T/src/session.rs
# Slug newtype shape (from ProjectLabel)
copy_lines $TY/src/ids.rs 1164 1245            $T/src/slug.rs
# argv-only git Command builder
copy_lines $TY/src/git.rs   88 109             $T/src/repo.rs
# Tests: unwritable helper, lost rows reported once, healthy writer, startup rotation
copy_lines $H/tests/obs.rs 202 283             $T/tests/writer.rs
