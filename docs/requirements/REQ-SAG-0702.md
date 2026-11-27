# Requirement: A Run may override the send timeout and the worker count for its own calls only

**Status:** Approved  
**Version:** 0.7.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-25  
**Owner:** Rand Lee  

---

## REQ-SAG-0702: A Run may override the send timeout and the worker count for its own calls only

**Status:** Approved  

### Requirement Statement

Definitions:

- The send timeout is the deadline for one HTTP send of a decision request: connect, headers and body.
- The worker count is the number of requests of one batch that are sent at the same time.
- A Run is the run context a tool creates, uses for its calls and then disposes. `decidr send` creates one Run per command from its `--timeout-secs` and `--workers` options.
- An override is a send timeout or a worker count given to a Run.
- To clamp a value is to replace it with the nearest bound of a range when it falls outside that range.
- An invalid send timeout is a send timeout that is negative or not a number.
- An invalid worker count is a worker count below 1.
- The configured send timeout is 120 s unless configuration sets another value.
- A model's context is the largest request, in tokens, that the model accepts.
- The provider's error is the error value `decision.api_status` whose message is the provider's reply text.

**MUST statements:**

- A Run MUST accept an optional send timeout.
- A Run MUST accept an optional worker count.
- A Run's `timeout_secs` MUST replace the 120 s default send timeout for that Run.
- When a Run has an override, every call through that Run MUST use the override in place of the configured value.
- When a Run has no override, every call through that Run MUST use the configured value.
- When a Run is given an invalid send timeout, it MUST return an error.
- When a Run is given an invalid worker count, it MUST return an error.
- When a provider rejects a request as too large for the model's context, the call MUST return the provider's error.

**MUST NOT statements:**

- A Run MUST NOT clamp an override.
- An override MUST NOT change the configuration.
- A single call through a Run MUST NOT accept an override.

### Rationale

Different callers in one process need different parallelism and different patience. Without overrides both are fixed by configuration, so consumers refuse or ignore their own flags. The selected model's context is the only size rule for a request. Values that do not vary between the calls of one command are set once on the Run, so an override cannot leak into another Run. Error values rather than exceptions keep every Python result typed.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Rust unit test builds a Run with a 10 s send timeout and checks the transport timeout is 10 s.
- [ ] A Rust unit test builds a Run with `timeout_secs` of 30 s and checks its sends use 30 s, not 120 s.
- [ ] A Rust unit test builds a Run with a worker count of 2 and checks the batch runs 2 workers.
- [ ] A Rust unit test runs `decidr send --timeout-secs 10 --batch --workers 2` and checks the Run's send timeout is 10 s and its worker count is 2.
- [ ] A Rust unit test builds a Run with no override after one with overrides and checks it uses the configured send timeout and worker count.
- [ ] A Rust unit test gives a Run a negative send timeout and checks the result is an error value.
- [ ] A Rust unit test gives a Run a NaN send timeout and checks the result is an error value.
- [ ] A Rust unit test gives a Run a worker count of 0 and checks the result is an error value.
- [ ] A Rust unit test gives a Run a worker count of -1 and checks the result is an error value.
- [ ] A Python unit test builds `sc_agent.Run` with `timeout_secs` and `workers` and checks a batch through it uses both values.
- [ ] A Python unit test checks that `run.send` and `run.send_batch` have no `timeout_secs` or `workers` parameter.
- [ ] A Rust unit test has a provider reject a request as too large for the model's context and checks the call returns the provider's error.
- [ ] A Rust unit test gives a Run a worker count above the configured one and checks that many workers run.
- [ ] A Rust unit test checks the configuration is unchanged after a Run with overrides.
- [ ] A Rust unit test checks that the per-call send and batch signatures have no override parameter.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0701
- REQ-SAG-0601
- REQ-SAG-0013
