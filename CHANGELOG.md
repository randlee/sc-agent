# Changelog

## Unreleased

## 0.2.8

- Let consumers stop shared batch submission with an optional result predicate, draining submitted jobs before executor shutdown.
- Close test subprocess stdin explicitly and bound Rust CLI and repository-identity child processes with the shared child guard.
- Build release and dry-run wheels with the locked Cargo dependency set.

## 0.2.7 (unreleased)

- Preserve typed answers, native send counts and error wire details in shared batch results.
- Validate fixture answers against their questions and redact serialized batch records.
- Recheck remote-judging consent in the selected repository for each call.

## 0.2.6 (unreleased)

- Share Jev batch settings, validation, fixture handling, streaming and result records in the generated consumer.
- Parse and validate typed NOUL, choice, score and box answers through the wheel.
- Isolate consumer settings test modules and make the sitr closed-stdout test deterministic.

## 0.2.5 (unreleased)

- Report missing optional sitr dependencies from generated consumer extraction.
- Identify the source wheel template in generated clients and retain generic shared error defaults.
- Publish integration run pages and a report index.

## 0.2.4 (unreleased)

- Generated consumers use one raisable ClientError for wheel and local errors, with the common error envelope and unchanged Rust messages and recovery text.
- Remove the redundant generated call/call_batch transport APIs; use call_result/call_batch_results.
- Complete Rust recovery guidance and log each failed runner result once with native id, category, provider, status, message and action. Missing-provider errors name credential variables; authentication messages withhold response bodies.

## 0.2.3 (unreleased)

- Preserve quota classification for context-overflow-shaped responses with a quota HTTP status.
- Let generated consumers map `decision.context_exceeded` through the optional `context_exceeded` key, falling back to its stored category code.

## 0.2.2 (unreleased)

### Breaking (Rust API)

- `ErrorId` implementors must supply `base_category()`; structured errors expose category, status, attempts, and retry-after details where available.
- Error details are boxed in `DecisionError::Backend.detail`, `DecisionError::SchemaMismatch.detail`, `LogError::InvalidName.cause`, `LogError::RowsLost.last`, `SecretsError::DotenvRead.source`, `SecretsError::KeyringRead.source`, and `ParseCause::Redacted`. Optional fields retain their `Option` wrapper. `HttpError::Network.cause` was already boxed.
- ADR-SAG-0019 enlarged `ErrorDetail`; boxing avoids Clippy's `result_large_err` and follows the existing `Box<ProviderReply>` pattern. Owner ruling: `agt-phase-b.4`. JSON output and the Python wheel are unchanged.

### Consumer migration

- Python 3.11 or newer is required. Install `sc-decidr` and `sitr` at the same version when using the optional tree-sitter section of the generated consumer.
- `send_batch` returns results in input order; `on_result` receives them as they finish. Use the structured error category and `retryable` fields instead of interpreting error messages.
