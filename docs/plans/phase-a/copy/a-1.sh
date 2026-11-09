#!/usr/bin/env bash
# Sprint A-1 (bead agt-4p2.1): copy the sc-agent-types starting code from sc-agent-ref@dce239a.
# Run: bash docs/plans/phase-a/copy/a-1.sh   (TARGET=<dir> to dry-run elsewhere)
#
# This script only copies text. It does NOT do any of the following; the developer does them
# (see the bead design for the exact target code):
#   - write Cargo.toml, README.md, src/lib.rs, src/error.rs (TypesError) or the workspace [workspace.lints];
#   - rewrite CancelToken onto Arc<Inner{cancelled, lock, wake}> and add wait_until/Wait (no sleep);
#   - turn UnknownModel into ModelId(Cow<'static, str>), delete the ModelId variant enum, known(), family(),
#     ModelFamily, ModelDescriptor::model() (expect), the family column and the old doc text;
#   - rename ModelAliases to Catalog, re-key it by ShortName, drop Entry.source, ModelSource, ResolvedModel,
#     source(), resolved(), with_override(), overlay_overrides(), check_family(), aliases();
#   - rename with_file_model to with_config_model(ModelId, ShortName, ReleaseDate);
#   - turn check_date into ReleaseDate's FromStr with .get() instead of bytes[4]/bytes[7] indexing;
#   - rename BackendKeyId to Provider (six variants) and drop env_var(); make a second copy into ShortName (nine);
#   - add ModelChoice and Catalog::resolve; replace every JevError with a TypesError variant;
#   - port the tests to the new names (they still use ModelId::Jev1_14_0, ModelSource, BackendId, ...).
. "$(dirname "$0")/lib.sh"

R=crates/sc-agent-types
T=crates/sc-agent-types

copy_file  $R/LICENSE                         $T/LICENSE
copy_file  $R/src/cancel.rs                   $T/src/cancel.rs
# ModelId newtype (from UnknownModel), Display, FromStr, serde
copy_lines $R/src/models.rs   11  35          $T/src/model.rs
copy_lines $R/src/models.rs  144 180          $T/src/model.rs
# ModelDescriptor, MODELS, row()
copy_lines $R/src/models.rs  198 304          $T/src/model.rs
# ModelAliases -> Catalog: struct, builtin, alias_of, known_models, latest; with_file_model
copy_lines $R/src/models.rs  401 473          $T/src/catalog.rs
copy_lines $R/src/models.rs  508 548          $T/src/catalog.rs
# check_date -> ReleaseDate
copy_lines $R/src/models.rs  623 646          $T/src/release_date.rs
# BackendKeyId -> Provider, and the same shape for ShortName
copy_lines $R/src/ids.rs     261 337          $T/src/provider.rs
copy_lines $R/src/ids.rs     261 337          $T/src/short_name.rs
# SessionId + strict parse_ulid
copy_lines $R/src/ids.rs     180 259          $T/src/session_id.rs
copy_lines $R/src/ids.rs    1369 1377         $T/src/session_id.rs
# Tests
copy_lines $R/tests/models.rs   1  32         $T/tests/catalog.rs
copy_lines $R/tests/models.rs  52 123         $T/tests/catalog.rs
copy_lines $R/tests/models.rs 149 171         $T/tests/catalog.rs
copy_lines $R/tests/models.rs 191 208         $T/tests/catalog.rs
copy_lines $R/tests/ids.rs     67  85         $T/tests/names.rs
copy_lines $R/tests/ids.rs    252 282         $T/tests/names.rs
