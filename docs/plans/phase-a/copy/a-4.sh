#!/usr/bin/env bash
# Sprint A-4 (bead agt-4p2.4): copy the sc-agent-secrets starting code from sc-agent-ref@dce239a.
# sc-agent-config is new code (no old equivalent: the old models.toml reader was TOML in sc-jev-host);
# only its LICENSE is copied.
# Run: bash docs/plans/phase-a/copy/a-4.sh   (TARGET=<dir> to dry-run elsewhere)
#
# This script only copies text. It does NOT do any of the following; the developer does them
# (see the bead design for the exact target code):
#   - write either crate's Cargo.toml, README.md, src/lib.rs or src/error.rs (SecretsError, AgentConfigError);
#   - write sc-agent-config's AgentConfig/ExtraModel/LocalRunner types, load() and catalog();
#   - add the OPENROUTER/TOGETHER/NOUS variable constants and env_var(Provider);
#   - rename ResolvedApiKey to ApiKey; change try_new(BackendKeyId, String) to try_new(&str var, String);
#     drop redact_error() and JevError::map_detail; replace JevError with SecretsError::{KeyMissing, KeyMalformed};
#   - drop the ANTHROPIC_KEYCHAIN_SERVICE constant, dotenv, keyring, whoami and every *_logged function;
#   - write resolve(Provider, &impl EnvSource) and resolve_var(&str, &impl EnvSource);
#   - port the copied unit tests from BackendKeyId::ALL / JevError to Provider::ALL / SecretsError,
#     and drop redact_error_covers_every_text_field (not copied).
. "$(dirname "$0")/lib.sh"

S=crates/sc-agent-secrets
T=crates/sc-agent-secrets
C=crates/sc-agent-config

copy_file  $S/LICENSE                                   $T/LICENSE
copy_file  $S/LICENSE                                   $C/LICENSE
# Credential variable names (REQ-SAG-0401)
copy_lines $S/src/resolve.rs        17  24              $T/src/vars.rs
# EnvSource trait + ProcessEnv
copy_lines $S/src/resolve.rs        34  42              $T/src/env.rs
copy_lines $S/src/platform.rs       15  31              $T/src/env.rs
# Sealed key (was sc-agent-types ResolvedApiKey) and its unit tests (not the redact_error test)
copy_lines crates/sc-agent-types/src/secret.rs   9 134  $T/src/api_key.rs
copy_lines crates/sc-agent-types/src/secret.rs 136 194  $T/src/api_key.rs
# MapEnv test double for the integration tests
copy_lines $S/src/resolve.rs       305 348              $T/tests/resolve.rs
