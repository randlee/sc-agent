#!/usr/bin/env bash
# Sprint A-2 (bead agt-4p2.2): sc-decision-schema.
# Copies the few old pieces worth keeping from sc-agent-ref@dce239a into crates/sc-decision-schema/.
#
# What this script does NOT do (the developer does it; see the bead design):
#   - src/map.rs: add the module header and `use` lines (std::collections::BTreeSet, std::fmt,
#     std::marker::PhantomData, serde::de::{self, MapAccess, Visitor}, serde::ser::SerializeMap,
#     serde::{Deserialize, Deserializer, Serialize, Serializer}); change try_from_pairs to return
#     DecisionError::DuplicateKey { key } instead of JevError::input_invalid; drop keys() and
#     is_empty() unless used; fix the doc-test path (sc_decision_schema::OrderedMap).
#   - src/digest.rs: the copied of_text body is a fragment. Rewrite it as crate-private
#     QuestionDigest::of_text(&str) -> Result<QuestionDigest, sc_sha::ShaError>; delete the
#     unreachable!() (REQ-SAG-0002: no panics). question_digests maps ShaError to
#     DecisionError::Encode; there is no DecisionError::Digest variant.
#   - Write everything else new: Cargo.toml, lib.rs, request.rs, response.rs, models.rs,
#     api_error.rs, decision.rs, backend.rs, error.rs, tests. No other old file is copied:
#     the old Jev types disagree with openapi.json and the canonical payload is rejected by
#     ADR-SAG-0001.
#   - No renames beyond the target paths below; no Cargo or workspace edits.
. "$(dirname "$0")/lib.sh"

C=crates/sc-decision-schema
copy_file  crates/sc-decision-schema/LICENSE                     $C/LICENSE
copy_lines crates/sc-decision-schema/src/jev/v1.rs   205 341     $C/src/map.rs
copy_lines crates/sc-agent-types/src/ids.rs          838 847     $C/src/digest.rs
