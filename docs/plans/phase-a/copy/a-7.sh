#!/usr/bin/env bash
# Sprint A-7 (bead agt-4p2.7): copy the sc-llm-provider and sc-decision-llm starting code from
# sc-agent-ref@dce239a (old sc-jev-backends and sc-decision-schema/src/llm).
# Run: bash docs/plans/phase-a/copy/a-7.sh   (TARGET=<dir> to dry-run elsewhere)
#
# This script only copies text. It does NOT do any of the following; the developer does them
# (see the bead design for the exact target code):
#   - write either crate's Cargo.toml, README.md, src/lib.rs or src/error.rs (LlmError, DecisionLlmError);
#   - write LlmRequest/LlmResponse/Message/Role/Output/Usage, LlmClient::{encode, decode} and the
#     OpenAI-compatible Chat Completions encoder/decoder (the old Luna code spoke the Responses API);
#   - turn the old provider_document()/message_body() (raw payload text + caller system prompt) into
#     JSON-schema requests: Anthropic output_config.format, Chat Completions response_format;
#   - drop recovered_digest, WireFault, count_body/parse_count (token-count call), provider_meta,
#     VendorReply.non_text_blocks, the 32 000-token / byte limits (validate.rs) and the macro-built
#     AnthropicHaikuBackend/OpenAiLunaBackend types; replace encode()'s expect() with LlmError::Encode;
#   - replace every JevError with LlmError / DecisionLlmError; ResolvedApiKey -> sc_agent_secrets::ApiKey;
#   - sc-decision-llm: write the built-in system prompt, the per-question-type answer schema
#     (noul, choice, score, box), the SystemOneRequest -> LlmRequest translation and the
#     model JSON -> SystemOneResponse translation; drop the canonical payload, ModelFamily and the
#     variants/SchemaFamily dispatch; the old answer check covered choice only;
#   - port the copied test helpers (they build JudgeRequest / BackendId) and write the tests.
. "$(dirname "$0")/lib.sh"

B=crates/sc-jev-backends
D=crates/sc-decision-schema
P=crates/sc-llm-provider
L=crates/sc-decision-llm

copy_file  $B/LICENSE                                  $P/LICENSE
copy_file  $B/LICENSE                                  $L/LICENSE

# --- sc-llm-provider ---
# Hosts, paths, anthropic-version, Haiku max_tokens default
copy_lines $B/src/wire.rs             13   30          $P/src/endpoints.rs
# Non-secret headers and the credential header per wire
copy_lines $B/src/wire.rs            200  223          $P/src/endpoints.rs
# Reply envelope skeleton, Anthropic text gathering, truncation, usage counters
copy_lines $B/src/wire.rs            252  289          $P/src/decode.rs
copy_lines $B/src/wire.rs            302  339          $P/src/decode.rs
copy_lines $B/src/wire.rs            386  423          $P/src/decode.rs
# HttpRequest assembly (headers + secret) for one provider call
copy_lines $B/src/backend.rs         325  336          $P/src/client.rs
# Test reply builders (generic, Anthropic) and the truncation test
copy_lines $B/tests/llm.rs            51   68          $P/tests/decode.rs
copy_lines $B/tests/llm.rs            87  101          $P/tests/decode.rs
copy_lines $B/tests/llm.rs           498  533          $P/tests/decode.rs

# --- sc-decision-llm ---
# Model reply text -> answers object; ids-differ error
copy_lines $D/src/llm/v1.rs           20   63          $L/src/answers.rs
# Old per-answer check (choice + confidence + probabilities keyed by the criteria)
copy_lines $D/src/variants.rs         51  120          $L/src/answers.rs
# Old answer fixture used by the tests
copy_lines $B/tests/llm.rs            74   85          $L/tests/translate.rs
