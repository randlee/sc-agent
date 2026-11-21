# Phase H plan: land the starting code, close three gaps

Four sprints. The result of every sprint already exists and passes: `origin/ref/phase-h-gaps` (cefce2c), on top of the starting code `origin/ref/phase-h-start` (94503df). Reviewers can diff it to check each sprint's size and shape.

Measured at cefce2c: 2,054 source lines across crates/ (starting code: 1,914); `cargo test --workspace` 23 passed, 0 failed; `cargo clippy --workspace --all-targets -D warnings` exit 0; wheel pytest 4 passed. `cargo fmt --check` fails on the starting code and on cefce2c alike (no rustfmt.toml; the starting code was never formatted) — not a gate in this plan.

Order: h-0, then h-1 and h-2 in parallel, then h-3 (needs h-2).

Not planned (owner decides): the sitr API in the wheel and deleting sc-treesitter-py; category, retryable, SagError and the consumer template; comparing the FromLlm prompt with the Python tools.

## h-0: move crates/ to crates-ref/ and land the starting code

Branch `sprint/h-0-crates-ref-layout` → PR into `integrate/phase-h`. Difficulty fast. Requirements NONE; ADRs ADR-SAG-0004.

### Goal
The starting code replaces the old crates, with the old crates kept beside it as crates-ref/.

### Boundary (binding)
Change only these paths: `crates/`, `crates-ref/`, `Cargo.toml`, `Cargo.lock`. `Cargo.lock` changes only as cargo regenerates it.
Everything else is out of bounds, including a fix the compiler forces elsewhere: stop and send it to the lead.
No public item, dependency, test or behaviour beyond the Deliverables. crates-ref/ is never a reason to add code.

### Deliverables
1. `git mv` every crate in crates/ to crates-ref/ except sitr, sc-treesitter, sc-treesitter-py and sc-agent-testkit, which stay in crates/.
2. Copy crates/* from origin/ref/phase-h-start (94503df) into crates/, unchanged.
3. Root Cargo.toml: crates-ref/ is excluded from the workspace; [workspace.dependencies] names the crates now in crates/ (as in the starting code's Cargo.toml, plus the four kept crates).
4. Where a kept crate (sitr, sc-treesitter, sc-treesitter-py, sc-agent-testkit) does not build against the starting code, change only that crate's Cargo.toml or the failing call, and list each change in the PR body.

### Does Not Close
- the code changes of h-1 to h-3

### Starting point
The starting code is origin/ref/phase-h-start (94503df). The expected result of this sprint is in the reference branch origin/ref/phase-h-gaps (cefce2c); it is the size and shape to match, not code to copy blind.

### Acceptance Criteria
1. [ ] `git diff -M --stat <base>...HEAD -- crates-ref/` shows only renames.
2. [ ] `git diff origin/ref/phase-h-start HEAD -- crates/decidr crates/sc-agent-config crates/sc-agent-http crates/sc-agent-log crates/sc-agent-py crates/sc-agent-secrets crates/sc-agent-types crates/sc-decision-llm crates/sc-decision-local crates/sc-decision-runner crates/sc-decision-schema crates/sc-decision-systemone crates/sc-llm-provider` is empty.
3. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
4. [ ] `cargo build --workspace --all-targets`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
5. [ ] No test sleeps or depends on timing, load or thread order.
6. [ ] Changes inside the four kept crates are at most 30 added lines in total.

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-gaps (cefce2c).

## h-1: sc-agent-http: busy is any non-quota 429 or 5xx; a busy reply pauses the client

Branch `sprint/h-1-busy-pause` → PR into `sprint/h-0-crates-ref-layout`. Difficulty normal. Requirements REQ-SAG-0601, REQ-SAG-0602, REQ-SAG-0012; ADRs NONE.

### Goal
A busy reply is resent, and every sender on the same client waits out the pause.

### Boundary (binding)
Change only these paths: `crates/sc-agent-http/`. `Cargo.lock` changes only as cargo regenerates it.
Everything else is out of bounds, including a fix the compiler forces elsewhere: stop and send it to the lead.
No public item, dependency, test or behaviour beyond the Deliverables. crates-ref/ is never a reason to add code.

### Deliverables
1. Busy is status 429 or any 5xx whose body has no quota marker; the fixed BUSY list is removed. Quota replies are still returned at once.
2. Client holds one provider-wide not-before time. Every send attempt first waits (cancellably) until it. A busy reply extends it to now + max(Retry-After, the attempt's backoff) + up to 1/8 jitter. Network and invalid replies keep their per-request backoff.
3. `Client::paused_for(&self) -> Duration`: the wait left before the next send, zero when not paused.
4. Tests in crates/sc-agent-http/tests/client.rs: `every_5xx_is_busy_and_resent` (500, 501, 503, 599 each resent once, then 200), `busy_reply_pauses_every_sender_on_the_client` (Retry-After 3600: the cancelled send leaves `paused_for` above 3500 s), `retry_after_zero_still_waits_the_backoff_floor` (backoff 3600 s, Retry-After 0: same). `non_busy_5xx_is_decoded_not_resent` is removed.

### Does Not Close
- any other sc-agent-http change

### Starting point
The starting code is origin/ref/phase-h-start (94503df). The expected result of this sprint is in the reference branch origin/ref/phase-h-gaps (cefce2c); it is the size and shape to match, not code to copy blind.

### Acceptance Criteria
1. [ ] `cargo test -p sc-agent-http` passes, including the three named tests; the removed test is gone.
2. [ ] With Retry-After 0 and a nonzero backoff, the pause is at least the backoff (named test).
3. [ ] A quota 429 is still sent once (`quota_is_not_resent` passes unchanged).
4. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
5. [ ] `cargo build --workspace --all-targets`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
6. [ ] No test sleeps or depends on timing, load or thread order.
7. [ ] crates/sc-agent-http/src adds at most 40 lines and tests at most 35 (`git diff --numstat`).

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-gaps (cefce2c).

## h-2: sc-agent-log: general Logger and one record helper

Branch `sprint/h-2-log-general` → PR into `sprint/h-0-crates-ref-layout`. Difficulty fast. Requirements REQ-SAG-0008; ADRs ADR-SAG-0022.

### Goal
Callers get a general sc-observability Logger and write records through one helper.

### Boundary (binding)
Change only these paths: `crates/sc-agent-log/`. `Cargo.lock` changes only as cargo regenerates it.
Everything else is out of bounds, including a fix the compiler forces elsewhere: stop and send it to the lead.
No public item, dependency, test or behaviour beyond the Deliverables. crates-ref/ is never a reason to add code.

### Deliverables
1. `pub fn general(service: &str, log_root: &Path) -> Result<Arc<Logger>, Error>`: a Logger writing `<log_root>/logs/<service>.log.jsonl`, console off.
2. `pub fn record(logger: &Logger, target: &str, action: &str, fields: Map<String, Value>)`: one Info record; a failed write is ignored. `Session::row` uses it instead of building its own event.

### Does Not Close
- the wheel (h-3)

### Starting point
The starting code is origin/ref/phase-h-start (94503df). The expected result of this sprint is in the reference branch origin/ref/phase-h-gaps (cefce2c); it is the size and shape to match, not code to copy blind.

### Acceptance Criteria
1. [ ] `cargo test -p sc-agent-log` passes; session rows are written exactly as before (existing test unchanged).
2. [ ] h-3's pytest finds records written through `general` and `record` in `<log_root>/logs/<service>.log.jsonl`.
3. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
4. [ ] `cargo build --workspace --all-targets`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
5. [ ] No test sleeps or depends on timing, load or thread order.
6. [ ] crates/sc-agent-log/src adds at most 35 lines (`git diff --numstat`).

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-gaps (cefce2c).

## h-3: sc-agent-py: send, send_batch, CancelToken and the injected Logger

Branch `sprint/h-3-wheel` → PR into `sprint/h-2-log-general`. Difficulty normal. Requirements REQ-SAG-0018, REQ-SAG-0022, REQ-SAG-0016; ADRs ADR-SAG-0005.

### Goal
Python calls decisions through the wheel with errors as values, cancel, and its own Logger.

### Boundary (binding)
Change only these paths: `crates/sc-agent-py/`. `Cargo.lock` changes only as cargo regenerates it.
Everything else is out of bounds, including a fix the compiler forces elsewhere: stop and send it to the lead.
No public item, dependency, test or behaviour beyond the Deliverables. crates-ref/ is never a reason to add code.

### Deliverables
1. `logger(name)` returns `(Logger, None)` writing `<root>/logs/<name>.log.jsonl`, or `(None, error JSON)`. `<root>` is `SC_DECIDR_LOG_ROOT` when set (tests), else `~/.sc-agent`.
2. `send(request, model, project, agent_role, logger, provider=None, cancel=None)` returns one reply JSON; `send_batch(requests, ..., workers=8, cancel=None)` returns one reply per request in input order. A reply is `{"ok": true, "value"}` or `{"ok": false, "error": {"id", "message"}}`; nothing raises, panics included. `workers=0` gives an error reply per request.
3. Every call writes one `decidr.send` record per reply to the given Logger (model, ok, error id). Session rows go to `<root>/sessions/...` (REQ-SAG-0005).
4. `CancelToken()` with `cancel()` and `cancelled()`; passing it cancels unfinished requests.
5. The old `decide` function and its `log_root` argument are removed.
6. crates/sc-agent-py/tests/test_wheel.py: bad request JSON is an error value and logged; workers=0 is one error value per request and two records; an invalid project slug is an error value; CancelToken flips.

### Does Not Close
- the sitr API in the wheel and deleting sc-treesitter-py (not planned; owner decides)
- category, retryable, SagError and the consumer template (not planned; owner decides)

### Starting point
The starting code is origin/ref/phase-h-start (94503df). The expected result of this sprint is in the reference branch origin/ref/phase-h-gaps (cefce2c); it is the size and shape to match, not code to copy blind.

### Acceptance Criteria
1. [ ] `maturin develop -m crates/sc-agent-py/Cargo.toml && python -m pytest crates/sc-agent-py/tests` passes 4 tests.
2. [ ] No test touches a real API key or the keyring.
3. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
4. [ ] `cargo build --workspace --all-targets`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
5. [ ] No test sleeps or depends on timing, load or thread order.
6. [ ] crates/sc-agent-py/src adds at most 135 lines and tests at most 50 (`git diff --numstat`).

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-gaps (cefce2c).
