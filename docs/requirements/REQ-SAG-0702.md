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
- An invalid send timeout is a send timeout that is negative or not a number.
- An invalid worker count is a worker count below 1.
- The configured send timeout is 120 s unless configuration sets another value (REQ-SAG-0015).
- A model's context is the largest request, in tokens, that the model accepts.
- The fixed send count is the number of sends defined in REQ-SAG-0601.
- A batch result is the result of one request of a batch, carrying that request's input position.
- The batch callback is the function a caller passes with a batch to receive results while the batch runs.

**MUST statements:**

- A Run MUST accept an optional send timeout.
- A Run MUST accept an optional worker count.
- A Run's `timeout_secs` MUST replace the 120 s default send timeout for that Run.
- When a Run has an override, every call through that Run MUST use the override in place of the configured value.
- When a Run has no override, every call through that Run MUST use the configured value.
- When a Run is given an invalid send timeout, it MUST return the error value `decidr.invalid_request_timeout`.
- When a Run is given an invalid worker count, it MUST return the error value `decidr.invalid_workers`.
- A request MUST fit the selected model's context.
- When a send exceeds the send timeout, the Run MUST resend it until it has made 5 sends in all, the first included.
- A batch MUST return one batch result per request.
- The batch callback MUST receive each result when that result finishes.

**MUST NOT statements:**

- A Run MUST NOT clamp an override.
- An override MUST NOT change the configuration.
- A single call through a Run MUST NOT accept an override.

### Rationale

Different callers in one process need different parallelism and different patience. Without overrides both are fixed by configuration, so consumers refuse or ignore their own flags. The selected model's context is the only size rule for a request. Values that do not vary between the calls of one command are set once on the Run, so an override cannot leak into another Run. Error values rather than exceptions keep every Python result typed.

Owner ruling (2026-11-21), Rand Lee: "I do not require results in request order."

### Success Criteria

**Acceptance Criteria:**

- [ ] A Rust unit test builds a Run with a 10 s send timeout and checks the transport timeout is 10 s.
- [ ] A Rust unit test builds a Run with `timeout_secs` of 30 s and checks its sends use 30 s, not 120 s.
- [ ] A Rust unit test builds a Run with a worker count of 2 and checks the batch runs 2 workers.
- [ ] A Rust unit test runs `decidr send --timeout-secs 10 --batch --workers 2` and checks the Run's send timeout is 10 s and its worker count is 2.
- [ ] A Rust unit test builds a Run with no override after one with overrides and checks it uses the configured send timeout and worker count.
- [ ] A Rust unit test gives a Run a negative send timeout and checks the result is the error value `decidr.invalid_request_timeout`.
- [ ] A Rust unit test gives a Run a NaN send timeout and checks the result is the error value `decidr.invalid_request_timeout`.
- [ ] A Rust unit test gives a Run a worker count of 0 and checks the result is the error value `decidr.invalid_workers`.
- [ ] A Rust unit test gives a Run a worker count of -1 and checks the result is the error value `decidr.invalid_workers`.
- [ ] A Python unit test builds `sc_agent.Run` with `timeout_secs` and `workers` and checks a batch through it uses both values.
- [ ] A Python unit test checks that `run.send` and `run.send_batch` have no `timeout_secs` or `workers` parameter.
- [ ] A Rust unit test sends a request larger than the selected model's context and checks the result is an error value and no send is made.
- [ ] A Rust unit test with a loopback runner that never answers checks the result has the error id `http.network` and `attempts` equal to the fixed send count.
- [ ] A Rust unit test sends a batch and checks it returns one result per request, each with its input position.
- [ ] A Rust unit test sends a batch whose requests finish out of order and checks the callback sees finish order.
- [ ] A Rust unit test gives a Run a worker count above the configured one and checks that many workers run.
- [ ] A Rust unit test checks the configuration is unchanged after a Run with overrides.
- [ ] A Rust unit test checks that the per-call send and batch signatures have no override parameter.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0701
- REQ-SAG-0601
- REQ-SAG-0013
- REQ-SAG-0015
