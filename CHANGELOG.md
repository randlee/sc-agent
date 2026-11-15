# Changelog

## 0.2.0 (unreleased)

### Breaking (Rust API)

- Error details are boxed in `DecisionError::Backend.detail`, `DecisionError::SchemaMismatch.detail`, `LogError::InvalidName.cause`, `LogError::RowsLost.last`, `SecretsError::DotenvRead.source`, `SecretsError::KeyringRead.source`, and `ParseCause::Redacted`. Optional fields retain their `Option` wrapper. `HttpError::Network.cause` was already boxed.
- ADR-SAG-0019 enlarged `ErrorDetail`; boxing avoids Clippy's `result_large_err` and follows the existing `Box<ProviderReply>` pattern. Owner ruling: `agt-phase-b.4`. JSON output and the Python wheel are unchanged.
