# Phase H plan: land the starting code, close three gaps

Four sprints. Every sprint's result already exists and passes: `origin/ref/phase-h-result` (7a3aab3), one commit per sprint on top of integrate/phase-h 2da3049: h-0 d9bfa54, h-1 ae0990c, h-2 1099913, h-3 7a3aab3. To check a sprint's scope, fence and size, diff its commit against its parent.

Measured at 7a3aab3 (macOS):
- crates/ (the active workspace): 4,487 Rust source lines and 899 test lines (.rs and .py under tests/). Of the source, 2,406 are the three kept crates (sitr 565, sc-treesitter 1,826, sc-treesitter-py 15) and 2,081 are the starting code plus h-1 to h-3. Before h-0, crates/ held 31,085 source and 22,027 test lines.
- crates-ref/ (old code, outside the workspace, not built): 47,337 Rust lines.
- `cargo build --workspace --all-targets` passes; `cargo test --workspace` 73 passed, 0 failed; `cargo clippy --workspace --all-targets -- -D warnings` exit 0; with maturin 1.14.1, both wheels build and pytest passes 9 (sc-agent-py 5, sc-treesitter-py 4).
- `cargo fmt --check` fails on the starting code and on this result alike (no rustfmt.toml; the starting code was never formatted). Not a gate in this plan.

Order: h-0, then h-1 and h-2 in parallel, then h-3 (needs h-2).

Not planned (owner decides): the sitr API in the sc-agent wheel and deleting sc-treesitter-py; category, retryable, SagError and the consumer template; comparing the FromLlm prompt with the Python tools; turning a wrong-type Python argument into an error value instead of TypeError.

## h-0: move crates/ to crates-ref/ and land the starting code

Branch `sprint/h-0-crates-ref-layout` → PR into `integrate/phase-h`. Difficulty fast. Requirements NONE; ADRs ADR-SAG-0004.

### Goal
The starting code replaces the old crates, with the old crates kept beside it as crates-ref/.

### Boundary (binding)
Change only these paths: `crates/`, `crates-ref/`, `Cargo.toml`, `clippy.toml`, `Cargo.lock`. `Cargo.lock` changes only as cargo regenerates it.
Everything else is out of bounds, including a fix the compiler forces elsewhere: stop and send it to the lead.
No public item, dependency, test or behaviour beyond the Deliverables. crates-ref/ is never a reason to add code.

### Deliverables
1. `git mv` every crate in crates/ to crates-ref/ except sitr, sc-treesitter and sc-treesitter-py, which stay in crates/. sc-agent-testkit moves too: it needs the old sc-agent-types.
2. Copy crates/* from origin/ref/phase-h-start (94503df) into crates/, unchanged. Root Cargo.toml and clippy.toml are the starting code's, plus `exclude = ["crates-ref"]` and [workspace.dependencies] entries for the three kept crates.
3. Exactly these kept-crate changes, so the workspace builds: sitr drops its sc-agent-testkit dev-dependency and its CLI test runs the binary with `Command::output()`; sc-treesitter reads a Python str with `to_cow` (the workspace's abi3 floor is py39, which lacks `to_str`); sc-treesitter-py drops pyo3's `extension-module` feature (it unified into sc-agent-py and broke its link); sc-treesitter and sc-treesitter-py use pyo3 `abi3-py39`, the starting wheel's floor, so every maturin version picks one floor for the workspace. Any other change needed is a stop: report it to the lead.

### Does Not Close
- the code changes of h-1 to h-3

### Starting point
The starting code is origin/ref/phase-h-start (94503df). This sprint's expected result is its commit in origin/ref/phase-h-result (7a3aab3; one commit per sprint: h-0 d9bfa54, h-1 ae0990c, h-2 1099913, h-3 7a3aab3); it is the size and shape to match. Reviewers diff that commit against its parent.

### Acceptance Criteria
1. [ ] For every moved crate X, `git rev-parse <base>:crates/X` equals `git rev-parse HEAD:crates-ref/X` (moved byte for byte).
2. [ ] `maturin develop -m crates/sc-treesitter-py/Cargo.toml` (maturin 1.14.1 or later) then `python -m pytest crates/sc-treesitter-py/tests` passes 4 tests; `maturin develop -m crates/sc-agent-py/Cargo.toml` builds.
3. [ ] `git diff origin/ref/phase-h-start HEAD -- crates/decidr crates/sc-agent-config crates/sc-agent-http crates/sc-agent-log crates/sc-agent-py crates/sc-agent-secrets crates/sc-agent-types crates/sc-decision-llm crates/sc-decision-local crates/sc-decision-runner crates/sc-decision-schema crates/sc-decision-systemone crates/sc-llm-provider` is empty.
4. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
5. [ ] `cargo build --workspace --all-targets`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
6. [ ] No test sleeps or depends on timing, load or thread order.
7. [ ] Changes inside the three kept crates are at most 15 added lines in total (reference: 10).

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-result (7a3aab3; one commit per sprint: h-0 d9bfa54, h-1 ae0990c, h-2 1099913, h-3 7a3aab3).

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
2. Client holds one provider-wide not-before time. Every send attempt first waits (cancellably) until it. A busy reply extends it to now + min(max(Retry-After, the attempt's backoff) + up to 1/8 jitter, `MAX_PAUSE`). Network and invalid replies keep their per-request backoff.
3. `pub const MAX_PAUSE: Duration` = 24 hours: the whole pause, jitter included, never exceeds it, so no Retry-After value (up to u64::MAX seconds) can overflow an Instant or panic.
4. The wait before a send re-reads the not-before time after every wait, because another sender may have extended it.
5. `Client::not_before(&self) -> Option<Instant>`: the pause end, `None` when never paused.
6. Tests in crates/sc-agent-http/tests/client.rs: `every_5xx_is_busy_and_resent` (500, 501, 503, 599 each resent once, then 200), `busy_reply_pauses_the_client_for_retry_after` (Retry-After 3600: after the cancelled send, `not_before() >= Some(before + 3600 s)` with `before` taken before the send), `retry_after_zero_still_waits_the_backoff_floor` (backoff 3600 s, Retry-After 0: same). `huge_retry_after_is_capped_not_a_panic` (Retry-After u64::MAX: the send returns Cancelled and `before + MAX_PAUSE <= not_before() <= after + MAX_PAUSE`, with `before` and `after` taken around the send). These compare Instants, so no delay can fail them. `non_busy_5xx_is_decoded_not_resent` is removed.

### Does Not Close
- any other sc-agent-http change

### Starting point
The starting code is origin/ref/phase-h-start (94503df). This sprint's expected result is its commit in origin/ref/phase-h-result (7a3aab3; one commit per sprint: h-0 d9bfa54, h-1 ae0990c, h-2 1099913, h-3 7a3aab3); it is the size and shape to match. Reviewers diff that commit against its parent.

### Acceptance Criteria
1. [ ] `cargo test -p sc-agent-http` passes, including the three named tests; the removed test is gone.
2. [ ] With Retry-After 0 and a nonzero backoff, the pause is at least the backoff (named test).
3. [ ] A quota 429 is still sent once (`quota_is_not_resent` passes unchanged).
4. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
5. [ ] `cargo build --workspace --all-targets`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
6. [ ] No test sleeps or depends on timing, load or thread order.
7. [ ] crates/sc-agent-http/src adds at most 55 lines and tests at most 50 (`git diff --numstat`; reference 53 and 46).

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-result (7a3aab3; one commit per sprint: h-0 d9bfa54, h-1 ae0990c, h-2 1099913, h-3 7a3aab3).

## h-2: sc-agent-log: general Logger and one record helper

Branch `sprint/h-2-log-general` → PR into `sprint/h-0-crates-ref-layout`. Difficulty fast. Requirements REQ-SAG-0008, REQ-SAG-0005; ADRs ADR-SAG-0021.

### Goal
Callers get a general sc-observability Logger and write records through one helper.

### Boundary (binding)
Change only these paths: `crates/sc-agent-log/`. `Cargo.lock` changes only as cargo regenerates it.
Everything else is out of bounds, including a fix the compiler forces elsewhere: stop and send it to the lead.
No public item, dependency, test or behaviour beyond the Deliverables. crates-ref/ is never a reason to add code.

### Deliverables
1. `pub fn general(service: &str, log_root: &Path) -> Result<Arc<Logger>, Error>`: a Logger writing `<log_root>/logs/<service>.log.jsonl`, console off.
2. `pub fn record(logger: &Logger, target: &str, action: &str, fields: Map<String, Value>)`: one Info record; a failed write is ignored. `Session::row` uses it instead of building its own event.
3. Tests (dev-dependency tempfile): crates/sc-agent-log/tests/records.rs `general_logger_writes_records_to_the_service_log` (one record with its target, action and fields in `<root>/logs/t.log.jsonl`); crates/sc-agent-log/tests/session_row.rs `session_row_carries_repo_project_and_role` (one row with target, action, repo, project, agent role and position in the session file), importing only `Session`.

### Does Not Close
- the wheel (h-3)

### Starting point
The starting code is origin/ref/phase-h-start (94503df). This sprint's expected result is its commit in origin/ref/phase-h-result (7a3aab3; one commit per sprint: h-0 d9bfa54, h-1 ae0990c, h-2 1099913, h-3 7a3aab3); it is the size and shape to match. Reviewers diff that commit against its parent.

### Acceptance Criteria
1. [ ] `cargo test -p sc-agent-log` passes both new tests.
2. [ ] Rows unchanged: with crates/sc-agent-log/src from h-0, `cargo test -p sc-agent-log --test session_row` passes.
3. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
4. [ ] `cargo build --workspace --all-targets`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
5. [ ] No test sleeps or depends on timing, load or thread order.
6. [ ] crates/sc-agent-log/src adds at most 35 lines and tests at most 60 (reference 32 and 58).

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-result (7a3aab3; one commit per sprint: h-0 d9bfa54, h-1 ae0990c, h-2 1099913, h-3 7a3aab3).

## h-3: sc-agent-py: send, send_batch, CancelToken and the injected Logger

Branch `sprint/h-3-wheel` → PR into `sprint/h-2-log-general`. Difficulty normal. Requirements REQ-SAG-0018, REQ-SAG-0022, REQ-SAG-0016, REQ-SAG-0005; ADRs ADR-SAG-0005, ADR-SAG-0021.

### Goal
Python calls decisions through the wheel with errors as values, cancel, and its own Logger.

### Boundary (binding)
Change only these paths: `crates/sc-agent-py/`. `Cargo.lock` changes only as cargo regenerates it.
Everything else is out of bounds, including a fix the compiler forces elsewhere: stop and send it to the lead.
No public item, dependency, test or behaviour beyond the Deliverables. crates-ref/ is never a reason to add code.

### Deliverables
1. `logger(name)` returns `(Logger, None)` writing `<root>/logs/<name>.log.jsonl`, or `(None, error JSON)`. `<root>` is `SC_DECIDR_LOG_ROOT` when set (tests), else `~/.sc-agent` from `std::env::home_dir()`; no absolute home directory is an error value, never a relative path.
2. `send(request, model, project, agent_role, logger, provider=None, cancel=None)` returns one reply JSON; `send_batch(requests, ..., workers=8, cancel=None)` returns one reply per request in input order. A reply is `{"ok": true, "value"}` or `{"ok": false, "error": {"id", "message"}}`; nothing raises, panics included. `workers=0` gives an error reply per request. An argument of the wrong Python type (for example `None` for a string) is Python's own TypeError, raised before the call starts.
3. Every call writes one `decidr.send` record per reply to the given Logger (model, ok, error id). Session rows go to `<root>/sessions/...` (REQ-SAG-0005).
4. `CancelToken()` with `cancel()` and `cancelled()`; passing it cancels unfinished requests.
5. The old `decide` function and its `log_root` argument are removed. crates/sc-agent-py/Cargo.toml gains `sc-observability = "=1.5.0"` (the Logger type); no other dependency.
6. An empty batch returns no replies and writes no records, before any provider or key lookup.
7. crates/sc-agent-py/tests/test_wheel.py: bad request JSON is an error value and logged; workers=0 is one error value per request and two records; an empty batch with default workers is `[]`; an invalid project slug is an error value; CancelToken flips.

### Does Not Close
- the sitr API in the wheel and deleting sc-treesitter-py (not planned; owner decides)
- category, retryable, SagError and the consumer template (not planned; owner decides)

### Starting point
The starting code is origin/ref/phase-h-start (94503df). This sprint's expected result is its commit in origin/ref/phase-h-result (7a3aab3; one commit per sprint: h-0 d9bfa54, h-1 ae0990c, h-2 1099913, h-3 7a3aab3); it is the size and shape to match. Reviewers diff that commit against its parent.

### Acceptance Criteria
1. [ ] `maturin develop -m crates/sc-agent-py/Cargo.toml && python -m pytest crates/sc-agent-py/tests` passes 5 tests.
2. [ ] No test touches a real API key or the keyring.
3. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
4. [ ] `cargo build --workspace --all-targets`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
5. [ ] No test sleeps or depends on timing, load or thread order.
6. [ ] crates/sc-agent-py/src adds at most 140 lines and tests at most 60 (reference 133 and 53).

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-result (7a3aab3; one commit per sprint: h-0 d9bfa54, h-1 ae0990c, h-2 1099913, h-3 7a3aab3).
