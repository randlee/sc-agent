# Changelog

## Unreleased

## 0.3.1

- Replace the former seventeen-crate workspace with five crates: sc-agent, decidr, sc-agent-py, sc-treesitter and sitr.
- Ship one `sc-decidr` wheel with the decision and sitr APIs under `sc_agent`; sitr is no longer a separate wheel.
- Return tagged success or error values from the generated client instead of raising exceptions.
- Share one native sc-observability Logger between each consumer and its wheel calls.
- Preserve session records up to 1 MiB and support `--version` on both decidr and sitr.
- Defer crates.io publication until sc-config is released; wheel and binary distribution remain enabled.

### Consumer migration

- For plan-anchor and judge-jev, render the shared `consumer.py.j2` template from the installed, pinned sc-decidr wheel with the consumer's `client_name` and `sitr` settings. Handle returned errors instead of catching client exceptions.
- Obtain the logger handle from `sc_agent.logger`'s success value and pass that same handle as `general_log` to decision, batch and sitr calls; use the wheel's logging API for consumer events.
- Replace `CancelToken` with `sc_agent.Cancel` and call its `cancel()` method to stop work.
- Use `jev` instead of `jev-latest` as the Jev model name; an explicit version such as `jev-1.14.0` pins the model.

## 0.2.8

- Let consumers stop shared batch submission with an optional result predicate, draining submitted jobs before executor shutdown.
- Close test subprocess stdin explicitly and bound Rust CLI and repository-identity child processes with the shared child guard.
- Build release and dry-run wheels with the locked Cargo dependency set.

### Breaking (Rust API)

- `ErrorId` implementors must supply `base_category()`; structured errors expose category, status, attempts, and retry-after details where available.
- Error details are boxed in `DecisionError::Backend.detail`, `DecisionError::SchemaMismatch.detail`, `LogError::InvalidName.cause`, `LogError::RowsLost.last`, `SecretsError::DotenvRead.source`, `SecretsError::KeyringRead.source`, and `ParseCause::Redacted`. Optional fields retain their `Option` wrapper. `HttpError::Network.cause` was already boxed.
- ADR-SAG-0019 enlarged `ErrorDetail`; boxing avoids Clippy's `result_large_err` and follows the existing `Box<ProviderReply>` pattern. Owner ruling: `agt-phase-b.4`. JSON output and the Python wheel are unchanged.

### Consumer migration

- Python 3.11 or newer is required. Install `sc-decidr` and `sitr` at the same version when using the optional tree-sitter section of the generated consumer.
- `send_batch` returns results in input order; `on_result` receives them as they finish. Use the structured error category and `retryable` fields instead of interpreting error messages.

## 0.2.7 (prerelease)

- Preserve typed answers, native send counts and error wire details in shared batch results.
- Validate fixture answers against their questions and redact serialized batch records.
- Recheck remote-judging consent in the selected repository for each call.

## 0.2.6 (prerelease)

- Share Jev batch settings, validation, fixture handling, streaming and result records in the generated consumer.
- Parse and validate typed NOUL, choice, score and box answers through the wheel.
- Isolate consumer settings test modules and make the sitr closed-stdout test deterministic.

## 0.2.5 (prerelease)

- Report missing optional sitr dependencies from generated consumer extraction.
- Identify the source wheel template in generated clients and retain generic shared error defaults.
- Publish integration run pages and a report index.

## 0.2.4 (prerelease)

- Generated consumers use one raisable ClientError for wheel and local errors, with the common error envelope and unchanged Rust messages and recovery text.
- Remove the redundant generated call/call_batch transport APIs; use call_result/call_batch_results.
- Complete Rust recovery guidance and log each failed runner result once with native id, category, provider, status, message and action. Missing-provider errors name credential variables; authentication messages withhold response bodies.

## 0.2.3 (prerelease)

- Preserve quota classification for context-overflow-shaped responses with a quota HTTP status.
- Let generated consumers map `decision.context_exceeded` through the optional `context_exceeded` key, falling back to its stored category code.

## 0.2.2 (prerelease)
