# Phase H plan: land the starting code, close three gaps

Four sprints. Every sprint's result already exists and passes: `origin/ref/phase-h-result` (e308c0b), one commit per sprint on top of integrate/phase-h 2da3049: h-0 b6641d3, h-1 1cdda9e, h-2 e9bc558, h-3 e308c0b. To check a sprint's scope, fence and size, diff its commit against its parent.

Measured at e308c0b (macOS):
- crates/ (the active workspace): 5,146 Rust source lines and 1,201 test lines (.rs and .py under tests/). Of the source, 2,402 are the three kept crates (sitr 565, sc-treesitter 1,822, sc-treesitter-py 15) and 2,744 are the starting code plus h-1 to h-3. Before h-0, crates/ held 31,085 source and 22,027 test lines.
- crates-ref/ (old code, outside the workspace, not built): 47,337 Rust lines.
- `cargo build --workspace --all-targets` passes; `cargo test --workspace --all-features` 74 passed, 0 failed; `cargo clippy --workspace --all-targets --all-features -- -D warnings` exit 0; on Python 3.11 with maturin 1.14.1, both wheels build as cp311-abi3 with Requires-Python >=3.11 (sc-decidr, sitr) and pytest passes 9 (sc-agent-py 5, sc-treesitter-py 4).
- `cargo fmt --check --all` exits 0: h-0 runs `cargo fmt --all` on the starting code (never formatted before), and every sprint keeps it formatted. Line counts and caps are after rustfmt.

Order: h-0, then h-1 and h-2 in parallel, then h-3 (needs h-2).

Not planned (owner decides): REQ-SAG-0011 (KeyboardInterrupt cancels the CancelToken); REQ-SAG-0601's 90-second budget, ProviderBusy reason and queue holdoff (waits hold a worker); REQ-SAG-0016's error detail fields; the sitr API in the sc-agent wheel and deleting sc-treesitter-py; category, retryable, SagError and the consumer template; comparing the FromLlm prompt with the Python tools; turning a wrong-type Python argument into an error value instead of TypeError.

Cited REQ/NFR/ADR records say what each sprint touches. Where an older record conflicts with the owner's later logging rulings, the rulings win; a citation is not a reason to add code.

## h-0: move crates/ to crates-ref/ and land the starting code

Branch `sprint/h-0-crates-ref-layout` → PR into `integrate/phase-h`. Difficulty fast. Requirements NFR-SAG-0003; ADRs ADR-SAG-0004, ADR-SAG-0019, ADR-SAG-0005.

### Goal
The starting code replaces the old crates, with the old crates kept beside it as crates-ref/.

### Boundary (binding)
Change only these paths: `crates/`, `crates-ref/`, `Cargo.toml`, `clippy.toml`, `Cargo.lock`. `Cargo.lock` changes only as cargo regenerates it.
Everything else is out of bounds, including a fix the compiler forces elsewhere: stop and send it to the lead.
No public item, dependency, test or behaviour beyond the Deliverables. crates-ref/ is never a reason to add code.

### Deliverables
1. `git mv` every crate in crates/ to crates-ref/ except sitr, sc-treesitter and sc-treesitter-py, which stay in crates/. sc-agent-testkit moves too: it needs the old sc-agent-types.
2. Copy crates/* from origin/ref/phase-h-start (94503df) into crates/, unchanged except crates/sc-agent-py: pyo3 `abi3-py311` (not `abi3-py39`) and a new pyproject.toml (name `sc-decidr`, version 0.2.8, `requires-python = ">=3.11"`, license MIT, maturin `module-name = "sc_agent"`), so it matches `[python].floor` (3.11) in floors.toml (NFR-SAG-0003, ADR-SAG-0019). Root Cargo.toml and clippy.toml are the starting code's, plus `exclude = ["crates-ref"]` and [workspace.dependencies] entries for the three kept crates. Then run `cargo fmt --all`: the starting code was never formatted, and the repository lint runs `cargo fmt --check`.
3. Exactly these kept-crate changes, so the workspace builds: sitr drops its sc-agent-testkit dev-dependency and its CLI test runs the binary with `Command::output()`; sc-treesitter-py drops pyo3's `extension-module` feature (it unified into sc-agent-py and broke its link). Every pyo3 dependency stays at `abi3-py311`, so every maturin version picks the same floor. Any other change needed is a stop: report it to the lead.

### Does Not Close
- the code changes of h-1 to h-3

### Starting point
The starting code is origin/ref/phase-h-start (94503df). This sprint's expected result is its commit in origin/ref/phase-h-result (e308c0b; one commit per sprint: h-0 b6641d3, h-1 1cdda9e, h-2 e9bc558, h-3 e308c0b); it is the size and shape to match. Reviewers diff that commit against its parent.

### Acceptance Criteria
1. [ ] For every moved crate X, `git rev-parse <base>:crates/X` equals `git rev-parse HEAD:crates-ref/X` (moved byte for byte).
2. [ ] On Python 3.11 with maturin 1.14.1 or later, `maturin build -m crates/<crate>/Cargo.toml` gives `sc_decidr-0.2.8-cp311-abi3-*.whl` and `sitr-0.2.8-cp311-abi3-*.whl`, each with METADATA `Requires-Python: >=3.11`; installed, `python -m pytest crates/sc-treesitter-py/tests` passes 4 tests.
3. [ ] Formatting aside, the starting code is unchanged: in a scratch worktree of origin/ref/phase-h-start, run `cargo fmt --all`, then `git diff HEAD_OF_THIS_PR -- crates/decidr crates/sc-agent-config crates/sc-agent-http crates/sc-agent-log crates/sc-agent-py crates/sc-agent-secrets crates/sc-agent-types crates/sc-decision-llm crates/sc-decision-local crates/sc-decision-runner crates/sc-decision-schema crates/sc-decision-systemone crates/sc-llm-provider ':!crates/sc-agent-py/Cargo.toml' ':!crates/sc-agent-py/pyproject.toml'` there is empty. The diff of crates/sc-agent-py/Cargo.toml against the starting code shows only `abi3-py39` replaced by `abi3-py311`, and crates/sc-agent-py/pyproject.toml is new with exactly the fields in deliverable 2.
4. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
5. [ ] `cargo fmt --check --all`, `cargo build --workspace --all-targets`, `cargo test --workspace --all-features` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` exit 0 (the repository's test and lint commands).
6. [ ] No test sleeps or depends on timing, load or thread order.
7. [ ] Changes inside the three kept crates are at most 5 added lines in total (reference: 2).

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-result (e308c0b; one commit per sprint: h-0 b6641d3, h-1 1cdda9e, h-2 e9bc558, h-3 e308c0b).

## h-1: sc-agent-http: busy replies are resent with REQ-SAG-0601 holdoffs; a busy reply pauses the client

Branch `sprint/h-1-busy-pause` → PR into `sprint/h-0-crates-ref-layout`. Difficulty normal. Requirements REQ-SAG-0601, REQ-SAG-0602, REQ-SAG-0012; ADRs NONE.

### Goal
A busy reply is resent after the holdoff REQ-SAG-0601 sets, and every sender on the same client waits it out.

### Boundary (binding)
Change only these paths: `crates/sc-agent-http/`. `Cargo.lock` changes only as cargo regenerates it.
Everything else is out of bounds, including a fix the compiler forces elsewhere: stop and send it to the lead.
No public item, dependency, test or behaviour beyond the Deliverables. crates-ref/ is never a reason to add code.

### Deliverables
1. Busy is status 429 or any 5xx whose body has no quota marker; the fixed BUSY list is removed. Quota replies are still returned at once.
2. `pub fn retry_after(text: &str) -> Option<Duration>`: valid only as 1 to 32 ASCII characters of digits with an optional fractional part; anything else is `None`. Parsed exactly with integers (no floating point), unclamped: whole seconds past u64::MAX saturate to u64::MAX; a fraction finer than 1 ns rounds up to the next nanosecond.
3. `pub fn holdoff(backoff, attempt, retry_after) -> Duration`: a valid Retry-After clamped to `RETRY_AFTER_MIN` (250 ms) ..= `HOLDOFF_MAX` (30 s); without one, `backoff` doubled per attempt (1, 2, 4, 8 s with the default 1 s) and capped at 30 s. Every wait adds up to `JITTER_MAX` (250 ms) of jitter.
4. Client holds one provider-wide not-before time. A busy reply extends it to now + holdoff + jitter; network and invalid replies wait holdoff + jitter (no Retry-After) for that request only. Every send attempt first waits (cancellably) until the not-before time, re-reading it after each wait because another sender may have extended it. `Client::not_before(&self) -> Option<Instant>` returns it. `ProviderBusy.retry_after_seconds` is the last valid Retry-After, unclamped, rounded up to whole seconds (saturating at u64::MAX).
5. Tests in crates/sc-agent-http/tests/client.rs: `every_5xx_is_busy_and_resent` (500, 501, 503, 599 each resent once, then 200); `retry_after_is_valid_only_as_digits_with_an_optional_fraction` (3, 2.5, 0 valid; u64::MAX and 32 nines -> u64::MAX s; 2.0000000000000001 -> 2 s + 1 ns; empty, abc, -1, `1.`, `.5`, leading space, 1e3, 33 digits invalid); `holdoff_clamps_retry_after_and_doubles_the_backoff` (3 -> 3 s, 99 -> 30 s, 0 -> 250 ms, u64::MAX -> 30 s; no Retry-After: 1, 2, 4, 8, 16, 30, 30 s); `busy_reply_pauses_the_client_for_its_holdoff` (Retry-After 99: `before + 30 s <= not_before() <= after + 30.25 s`, with `before` and `after` taken around the send); `provider_busy_reports_the_retry_after_unclamped_and_rounded_up` (one send each; Retry-After 2.5 -> Some(3), 2.0000000000000001 -> Some(3), 99 -> Some(99), u64::MAX -> Some(u64::MAX), all `attempts: 1`). None of these depends on elapsed time. `non_busy_5xx_is_decoded_not_resent` is removed.

### Does Not Close
- REQ-SAG-0601's queue holdoff: a held-off request waits on its worker thread, not in the queue (a busy pause stops every sender on the client anyway); not planned; owner decides
- REQ-SAG-0601's 90-second budget per request and the ProviderBusy reason field (`sends` or `budget`): not in the starting code, not planned; owner decides
- any other sc-agent-http change

### Starting point
The starting code is origin/ref/phase-h-start (94503df). This sprint's expected result is its commit in origin/ref/phase-h-result (e308c0b; one commit per sprint: h-0 b6641d3, h-1 1cdda9e, h-2 e9bc558, h-3 e308c0b); it is the size and shape to match. Reviewers diff that commit against its parent.

### Acceptance Criteria
1. [ ] `cargo test -p sc-agent-http` passes, including the five named tests; the removed test is gone.
2. [ ] A quota 429 is still sent once (`quota_is_not_resent` passes unchanged).
3. [ ] `grep -rn 'sleep(' crates` finds nothing.
4. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
5. [ ] `cargo fmt --check --all`, `cargo build --workspace --all-targets`, `cargo test --workspace --all-features` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` exit 0 (the repository's test and lint commands).
6. [ ] No test sleeps or depends on timing, load or thread order.
7. [ ] crates/sc-agent-http/src adds at most 97 lines and tests at most 107 (`git diff --numstat`, after rustfmt; reference 92 and 102).

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-result (e308c0b; one commit per sprint: h-0 b6641d3, h-1 1cdda9e, h-2 e9bc558, h-3 e308c0b).

## h-2: sc-agent-log: general Logger and one record helper

Branch `sprint/h-2-log-general` → PR into `sprint/h-0-crates-ref-layout`. Difficulty fast. Requirements REQ-SAG-0008, REQ-SAG-0005, REQ-SAG-0004; ADRs ADR-SAG-0021, ADR-SAG-0002, ADR-SAG-0004.

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
The starting code is origin/ref/phase-h-start (94503df). This sprint's expected result is its commit in origin/ref/phase-h-result (e308c0b; one commit per sprint: h-0 b6641d3, h-1 1cdda9e, h-2 e9bc558, h-3 e308c0b); it is the size and shape to match. Reviewers diff that commit against its parent.

### Acceptance Criteria
1. [ ] `cargo test -p sc-agent-log` passes both new tests.
2. [ ] Rows unchanged: with crates/sc-agent-log/src from h-0, `cargo test -p sc-agent-log --test session_row` passes.
3. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
4. [ ] `cargo fmt --check --all`, `cargo build --workspace --all-targets`, `cargo test --workspace --all-features` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` exit 0 (the repository's test and lint commands).
5. [ ] No test sleeps or depends on timing, load or thread order.
6. [ ] crates/sc-agent-log/src adds at most 47 lines and tests at most 85 (`git diff --numstat`, after rustfmt; reference 42 and 80).

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-result (e308c0b; one commit per sprint: h-0 b6641d3, h-1 1cdda9e, h-2 e9bc558, h-3 e308c0b).

## h-3: sc-agent-py: send, send_batch, CancelToken and the injected Logger

Branch `sprint/h-3-wheel` → PR into `sprint/h-2-log-general`. Difficulty normal. Requirements REQ-SAG-0018, REQ-SAG-0022, REQ-SAG-0016, REQ-SAG-0005, REQ-SAG-0003, REQ-SAG-0004, REQ-SAG-0008; ADRs ADR-SAG-0005, ADR-SAG-0021, ADR-SAG-0019, ADR-SAG-0004, ADR-SAG-0020.

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
- REQ-SAG-0016's error detail (status, attempts, retry_after_ms, category, retryable): a reply error is `{id, message}`; not planned; owner decides
- REQ-SAG-0011 (a KeyboardInterrupt cancels the CancelToken): not planned; the CancelToken is passed explicitly; owner decides
- the sitr API in the wheel and deleting sc-treesitter-py (not planned; owner decides)
- category, retryable, SagError and the consumer template (not planned; owner decides)

### Starting point
The starting code is origin/ref/phase-h-start (94503df). This sprint's expected result is its commit in origin/ref/phase-h-result (e308c0b; one commit per sprint: h-0 b6641d3, h-1 1cdda9e, h-2 e9bc558, h-3 e308c0b); it is the size and shape to match. Reviewers diff that commit against its parent.

### Acceptance Criteria
1. [ ] On Python 3.11, `maturin develop -m crates/sc-agent-py/Cargo.toml && python -m pytest crates/sc-agent-py/tests` passes 5 tests.
2. [ ] No test touches a real API key or the keyring.
3. [ ] `git diff --name-only <base>...HEAD` lists only the owned paths and `Cargo.lock`.
4. [ ] `cargo fmt --check --all`, `cargo build --workspace --all-targets`, `cargo test --workspace --all-features` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` exit 0 (the repository's test and lint commands).
5. [ ] No test sleeps or depends on timing, load or thread order.
6. [ ] crates/sc-agent-py/src adds at most 185 lines and tests at most 60 (`git diff --numstat`, after rustfmt; reference 177 and 53).

### Verification
Sanity and QA re-run every command above at the PR head and compare the behaviour with the reference result origin/ref/phase-h-result (e308c0b; one commit per sprint: h-0 b6641d3, h-1 1cdda9e, h-2 e9bc558, h-3 e308c0b).
