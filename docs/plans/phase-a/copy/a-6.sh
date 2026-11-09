#!/usr/bin/env bash
# Sprint A-6 (bead agt-4p2.6): sc-decision-systemone and sc-decision-local.
# Copies only the crate LICENSE files from sc-agent-ref@dce239a.
#
# What this script does NOT do (the developer does it; see the bead design):
#   - No source is copied. The old Jev client (crates/sc-jev/src/client/mod.rs, check.rs,
#     config.rs, jev.toml) is built on JudgeBackend, JevError, the canonical payload and the
#     sleeping busy retry, all removed; the request it builds is 6 lines and is rewritten
#     against the A-5 HttpRequest. The old repo has no local-daemon discovery code.
#   - Write new: both Cargo.toml files, sc-decision-systemone/src/{lib.rs,backend.rs,multipart.rs},
#     sc-decision-local/src/{lib.rs,daemon.rs,decider.rs,laya.rs,generic.rs,backend.rs,error.rs},
#     and the tests listed in the design.
#   - No renames, no Cargo or workspace edits.
. "$(dirname "$0")/lib.sh"

copy_file crates/sc-jev/LICENSE crates/sc-decision-systemone/LICENSE
copy_file crates/sc-jev/LICENSE crates/sc-decision-local/LICENSE
