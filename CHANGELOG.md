# Changelog

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
