# j-3 plan: session log, run identity, result cache, session-row reader

Base: sprint/j-2 @9e36bfeb (planned end state of docs/plans/phase-j/j-2-plan.md: `Run`, `RunArgs`, `Options` without `provider`, config/load/doctor taking `tool`, PyO3 `PyRun`/`PyLogger`; j-2 end about 2,421/2,580 with restructure counted 0). Line refs are file:line at that commit (Rust `Run` built; PyO3 and tests still to land from j-2, so PyO3 refs are to the j-2 plan). Written by reading the code and the records on origin/plan/phase-j (REQ-SAG-0002/0003/0004/0005/0011/0017/0703, ADR-SAG-0019 decision 14, ADR-SAG-0021 Session log/Paths/Session identity). The bead's old prose is intent only.

## Goal

Rand (ANC-004, ANC-005, ANC-008; owner rulings 2026-11-24): "These limits are clearly over-specified. Input is limited by model context."; "all default loggers created in production have root/folder/file naming conventions"; "Related values are strategically grouped in one struct to remove repeated method parameters"; creating a Run mints `run_id`, Rust appends `{run_id, session_id, cached}` to `runs.jsonl`, an unchanged request to the same model is answered from the newest successful session row. Measurable: every query sent to a model adds one row to `sessions.jsonl` and one link to `runs.jsonl` (REQ-SAG-0005); a resend makes no model call and writes no row, only a `cached: true` link (REQ-SAG-0703); no `SESSION_APPEND`, no hand-rolled `OpenOptions` append; restructuring adds 0 net lines; phase total <= 2,580.

## Layout

| file | holds |
|---|---|
| crates/sc-agent/src/lib.rs | all sc-agent items below (no session.rs; providers untouched): `Options.tool_version`, `RunArgs.{tool_version,cache}`, `Settings.cache`, `Run.run_id`, `Exchange`, `session_dir`, `session_rows`, private `event`/`append`/`rows`/`sha`, the error-category arm |
| crates/sc-agent-py/src/lib.rs | `PyRun` keywords `tool_version`, `cache`; getter `run_id`; `#[pyfunction] session_rows` |
| crates/sc-agent-py/python/sc_agent/{__init__.py,__init__.pyi,_native.pyi} | `session_rows` export and stub; `Run` stub gains `tool_version`, `cache`, `run_id` |
| crates/decidr/src/main.rs | `tool_version` in its `RunArgs` |
| crates/sc-agent-py/python/sc_agent/templates/consumer.py.j2, crates/sc-agent-py/README.md, scripts/check_consumer_template.py | only the `tool_version` wiring (render variable, `TOOL_VERSION`, two call-site arguments, render command, check vars) |
| crates/sc-agent-py/src/lib.rs (sitr) | the four sitr pyfunctions gain `tool=` |
| crates/sc-agent/Cargo.toml | no new dependency (sc-observability, ulid, sc-sha already linked) |
| tests: crates/sc-agent/tests/{support/mod.rs,records.rs,sending.rs,wires.rs,config_and_values.rs,cache.rs (new)}, crates/decidr/tests/cli.rs, tests/contract/{test_session.py (new),test_contract.py} | see Done checks |

## Deliverables
1. J3-S01: Options.tool_version
2. J3-S02: RunArgs.tool_version and RunArgs.cache
3. J3-S03: Run::from_options without tool param, with decision.missing_tool
4. J3-S04: Run::send and send_batch per-call cache (lead decision, rule 0)
5. J3-S05: Settings.cache + KEYS + default
6. J3-S06: Run.run_id
7. J3-S07: Exchange and Route.exchange
8. J3-S08: session_dir
9. J3-S09: session_rows
10. J3-S10: error ids and log category
11. J3-S11: private sink writer: event/append/rows/sha
12. J3-S12: decision.result fields
13. J3-S13: PyRun kwargs tool_version, cache
14. J3-S14: PyRun.run_id getter
15. J3-S15: PyO3 session_rows
16. J3-S16: decidr RunArgs fields
17. J3-S17: Python stubs
18. J3-S18: PyRun.log over `log_as` (new capability, accessor for j-4 log_event / j-6)
19. J3-S19: sitr pyfunctions `tool=` (new capability)
20. J3-S20: template tool_version wiring

Exact code for each: the dev bead agt-j-3.group-dev Signatures and design "Public surface".

## Does Not Close
- `Run.project`, `cached` on errors, an in-memory cache index.
- The template rewrite: j-4 (j-3 adds only the `tool_version` wiring of J3-S20).
- `seed_session` (OWNER_FIX 2026-11-26: not an owner mandate; offline tests use a real Run with an injected logger and a stub).

## Public surface (exact Rust)

Library pattern (rule 0): the Run owns where its logs go and who sent the query; rows and links are written by the library through the sc-observability sink, never by the caller; the cache is a setting that layered config resolves. No new struct is created for a row, a link, a usage record or a cache key: they are `serde_json::Value`s built where they are written.

```rust
// crates/sc-agent/src/lib.rs
pub struct Options {                      // j-2 shape, plus:
    pub tool_version: String,             // released version of the tool; the tool name is options.settings.tool (set by config)
    /* environment, settings, model, agent_role, timeout, base_url, root, log_root, repo unchanged */
}
pub struct Settings { /* j-2 fields */ pub cache: bool }          // key "cache", default true
pub struct RunArgs {                      // j-2 shape, plus:
    pub tool_version: String,
    pub cache: Option<bool>,              // None -> configured (DECIDR_CACHE / config files)
    /* tool, agent_role, model, project, config_dir, timeout_secs, workers, base_url unchanged */
}
pub struct Run { /* j-2 fields */ pub run_id: ulid::Ulid }          // minted in from_options
impl Run {
    /// j-2's `from_options(options, tool, logger, cancel)` loses its `tool` parameter: the tool is options.settings.tool.
    /// Err `decision.missing_tool` when the tool or tool_version is empty (before any logger, wire or file work).
    pub fn from_options(options: Options, logger: Option<Arc<Logger>>, cancel: Arc<Cancel>) -> Result<Self, Error>;
    /// j-2's `send(&self, request)` gains the per-call override: `cache` None = the Run's (`RunArgs.cache`, else configured); Some(b) wins for this call.
    pub fn send(&self, request: &Value, cache: Option<bool>) -> Result<Value, Error>;
    /// `send_batch(&self, requests, cache, on_result)`: the same `cache` applies to every item.
    pub fn send_batch(&self, requests: &[Value], cache: Option<bool>, on_result: &(dyn Fn(usize, &Value) + Sync)) -> Result<Value, Error>;
}                                         // new, close, sends_to, logger: j-2 signatures unchanged

/// Per-call exchange state the wire code fills (replaces the `sends: &Cell<u32>` field of `Route`, lib.rs Route :1076).
#[derive(Default)]
pub struct Exchange {
    pub sends: Cell<u32>,
    pub sha: RefCell<Option<String>>,     // sha256 of the exact posted body bytes
    pub raw: RefCell<Value>,              // {"model","usage"} of the first 2xx body that parsed; Null otherwise
    pub hit: RefCell<Option<Value>>,      // the cached success value when the cache answered
    pub cache: bool,                      // this call's cache setting: `Run::send` builds `Exchange { cache: cache.unwrap_or(self.options.settings.cache), ..Default::default() }`; `Route::post` reads it (no other path, no new Route field)
}
pub struct Route<'a> { /* fields unchanged except */ pub exchange: &'a Exchange }   // `sends` removed

/// The one session-log layout: <log_root>/sessions/<repo>/<agent_role>/<project>. Private: writer (Run::send) and reader (session_rows) are both in lib.rs.
fn session_dir(log_root: &Path, repo: &str, agent_role: &str, project: &str) -> PathBuf;
/// The one reader of the session log. Its log root is `log_root(agent_role)`, the same private `session_root(name)` function the Run's default logger uses with the tool name (one function, two callers). They meet when agent_role == tool, which a generated client guarantees (its role is its tool name, REQ-SAG-0003); the reader's `agent_role` argument is documented as "the tool name that owns the log root".
/// Ok(object keyed by session_id -> row); a missing id is Err `session.not_found` naming the ids; `project` None resolves like a send.
pub fn session_rows(ids: &[String], agent_role: &str, project: Option<&str>) -> Result<Value, Error>;

// private (latitude inside bodies, shapes fixed):
fn event(service: &ServiceName, target: &str, action: &str, correlation_id: Option<&str>, fields: Value) -> Result<LogEvent, Error>;     // private
pub fn log_as(logger: &Logger, target: &str, action: &str, correlation_id: Option<&str>, fields: Value) -> Result<(), Error>;   // new capability: logger.log(event(..)?); the one writer of a caller-named event
// `pub fn log(logger, action, fields)` (lib.rs:987) keeps its public signature: log_as(logger, "sc-agent", action, None, fields)
fn append(path: PathBuf, action: &str, fields: Value) -> Result<(), Error>;                 // JsonlFileSink::open(path, NEVER).write(&event) ; NEVER = RetainedLogPolicy{rotation_max_bytes: ByteCount::from_bytes(u64::MAX), rotation_max_files at its maximum, retention_max_age = 100 years (3_153_600_000 s), rest default}; sc-observability 1.5.0 has no disable/no-rotation mode (RetainedLogPolicy fields are all required; RetentionMaxAge must be non-zero) and computes `SystemTime::now() - age` (sinks.rs:277), which panics for the type's maximum, so the age is a large representable one and rotation can't trigger (REQ-SAG-0002: no panic on a library path)
fn rows(path: &Path, keep: impl FnMut(&Value) -> bool) -> Result<Vec<Value>, Error>;        // unwraps each line's `fields`; skips an unparsable (partial) line; Err `session.unreadable` when the file exists and cannot be read
fn sha(bytes: &[u8]) -> Result<String, Error>;                                               // shared by `digest` and the body hash
```

Error ids (`error()` lib.rs:75): new `decision.missing_tool` (validation), `log.session_write` (log), `session.unreadable` (log), `session.not_found` (validation; ADR-SAG-0021 Paths). The category arm `"log.write" | "log.flush" | "log.shutdown" | "log.session_write" | "session.unreadable" => "log"` (REQ-SAG-0017 and ADR-SAG-0019 decision 6 list `log`); `log.invalid_slug`, `log.invalid_event` stay `validation`; every other id unchanged. `config.invalid` for `DECIDR_CACHE=1` comes from the existing typed setting (bool).

Behaviour of `Run::send` (call order; private):
1. cancel, project and slug checks (unchanged) -> wire. `Route::post` serializes the body once to bytes (replaces `send_json`), sets `exchange.sha`, and, when `self.exchange.cache` (the call's `cache.unwrap_or(options.settings.cache)`), `rows(sessions.jsonl, same request_sha256 && same model_requested && result.ok)` -> the row with the greatest `session_id`; on a hit it stores the row's `result.value` in `exchange.hit` and returns it without a model call, holdoff or wait; `text()` passes a hit through unparsed. Unreadable file: `session.unreadable`, no model call.
2. On a 2xx body that parses, `exchange.raw` takes `{model, usage}`.
3. Hit: link `{run_id, session_id (the row's), cached: true}` to `runs.jsonl`; log `decision.result {session_id, cached: true}`; flush; return the value with `cached: true`. No row.
4. Miss: success value gains `attempts`, `session_id`, `question_digests`, `cached: false` (as `attempts`/`session_id`/`question_digests` today :1451-1458); append the row to `sessions.jsonl`, then the link `{run_id, session_id, false}` to `runs.jsonl`. Row append or link failure: log `log.session_write` once; a success outcome returns that error (carrying `session_id`), an error outcome is returned unchanged. Then log `decision.result {session_id, cached}` (success only; no `response` field), flush.
5. `send_batch` is unchanged: it calls `self.send` per item, so cache hits and links are per item (REQ-SAG-0703).

Per-call cache override (one cache mechanism): `send` and `send_batch` take `cache: Option<bool>`, None = the Run's setting. Tests that resend on purpose pass `cache=Some(false)`.

```rust
// crates/sc-agent-py/src/lib.rs (PyRun, as in j-2: never raises, stores the error envelope)
// PyRun `send`/`send_batch` gain `cache=None` (arg::<bool>, passed to Run::send/send_batch); PyRun gains `fn log(&self, event: &Bound<'_, PyDict>) -> Py<PyAny>`: Envelope over `log_as(logger, target, action, correlation_id, fields)` from the event {target, action, correlation_id, fields} (target and action required strings, correlation_id str or None, fields a dict; else `sc_agent_py.invalid_option`), then flush (new capability, the accessor j-4's `log_event` calls; the Rust `log` is not used because it fixes target `sc-agent` and has no correlation_id)
// PyRun::new signature gains `tool_version=None, cache=None` (kwargs `arg::<String>` / `arg::<bool>` into RunArgs.tool_version/.cache; None tool/tool_version -> "")
#[pymethods] impl PyRun { #[getter] fn run_id(&self) -> Option<String>; }   // Run.run_id.to_string(); None when creation failed
#[pyfunction(signature=(session_ids=None, *extra, agent_role=None, project=None, **rest))]
fn session_rows(py: Python<'_>, session_ids: Option<&Bound<'_, PyAny>>, extra: &Bound<'_, PyTuple>, agent_role: Option<&Bound<'_, PyAny>>,
                project: Option<&Bound<'_, PyAny>>, rest: Option<&Bound<'_, PyDict>>) -> Py<PyAny>;  // respond_value(None, agent_role, |_| sc_agent::session_rows(..)): errors go to the default logger named by the role
// sitr pyfunctions `languages|queries|query|run(..., tool: Option<String>)`: `respond(py, general_log, tool.as_deref().unwrap_or("sitr"), ..)` (+~6; stub `tool: str | None = None` in _native.pyi)
// template: front matter `required_variables` gains `tool_version`; `TOOL_VERSION = "{{ tool_version }}"`; the two Run call sites pass `tool_version=TOOL_VERSION`; wheel README render command and scripts/check_consumer_template.py vars gain tool_version
// crates/decidr/src/main.rs: RunArgs { tool_version: env!("CARGO_PKG_VERSION").into(), cache: None, .. }
```
j-2's "tool is required" `sc_agent_py.invalid_option` check in `PyRun::new` is dropped: the empty tool reaches `Run::new` and returns `decision.missing_tool` from one place. A given `general_log` (a public extension point of the library; tests use the same path) logs where it logs; otherwise the default logger logs to the tool's log root (j-2).

Python (names added by j-3; `_native.pyi`/`__init__.py`/`__init__.pyi`): `Run.__init__(..., tool_version: str|None=None, cache: bool|None=None)`, `Run.run_id: str|None`, `Run.send(request, *, cache: bool|None=None)`, `Run.send_batch(requests, *, cache: bool|None=None, on_result=None)`, `Run.log(event: Mapping[str, Any]) -> Envelope`, `session_rows(session_ids: Iterable[str], *, agent_role: str, project: str|None=None) -> Envelope`; `__all__` gains `"session_rows"`.

Logger: an injected logger (a public extension point of the library; tests use the same path) logs where it logs; otherwise the default logger logs to the log root defined for the tool (j-2 `Run::new`). The session-log reader uses the same tool log root; j-3 shares the one root function between writer and reader.

## Replaces (item -> existing construct; what is deleted with it)

| item | replaces | deleted with it |
|---|---|---|
| `append`, `event`, `NEVER` policy | `static SESSION_APPEND` :1284 and the `OpenOptions` append block with `config.session` :1464-1488; the body of `log` :987-992 becomes `event` + `logger.log` | the static, the lock, the manual create_dir_all/writeln, `config.session` id |
| `session_dir` (private) | inline `.join("sessions").join(repo).join(role).join(project)` :1466-1470 | the inline join (reader and writer share the layout fn) |
| `sha` | the sc-sha call in `digest` :969-974 | the duplicate call |
| `Exchange`, `Route.exchange` | `Route.sends: &Cell<u32>` and `let sends = Cell::new(0)` in `send` :1407 | the field and the local |
| body bytes in `Route::post` | `post.send_json(&sending)` :1226, `body.clone()` :1224 | both |
| row `json!` with schema/tool/provider/model/usage/sha | the row literal :1464 | the literal |
| `decision.result {session_id, cached}` | `json!({"response":value})` :1490 | the `response` field |
| `Options.tool_version`, `Run::from_options` minus `tool` | j-2's separate `tool: String` parameter of `from_options` | the parameter and the matching `args.tool` pass-through |
| `decision.missing_tool` | j-2's PyO3 "tool is required" `invalid_option` | that check |
| category arm | `_ => "validation"` for `log.write`/`log.flush`/`log.shutdown` | - |
| `sitr` pyfunctions `tool=` (`languages`, `queries`, `query`, `run`) | `respond(py, general_log, "sitr", ..)` hard-coded name, crates/sc-agent-py/src/lib.rs (j-2 end) | the literal as the only name (new capability: the tool passes its name, ANC-015/016; moved here from j-4 so one sprint owns lib.rs and the .pyi stubs) |
| template `tool_version` wiring | j-2's template call sites (no tool_version) | nothing (without it j-3's `decision.missing_tool` breaks every rendered client) |
| per-call `cache` of `send`/`send_batch`, `PyRun.log`, `log_as`, `Run.run_id`, `RunArgs.cache`, `Settings.cache`, `Exchange.hit`, links, `session_rows`, `PyRun.run_id`, PyO3 `session_rows`, `session.not_found`, `session.unreadable`, `log.session_write`, `decision.missing_tool` | none: new capability (REQ-SAG-0005, REQ-SAG-0004, REQ-SAG-0703, ADR-SAG-0019 d14, ADR-SAG-0021) | - |

## External contracts (examples)

Files (each line is an sc-observability event; the row or link is its `fields`; never rotated or pruned, REQ-SAG-0003):
```
<log_root>/sessions/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl   action "session.row"
{"version":"v1","timestamp":"...","level":"info","service":"decidr","target":"sc-agent","action":"session.row","fields":{
 "schema":"sc-agent-session/2","session_id":"01J...","repo":"github.com/randlee/sc-agent","agent_role":"rust-developer","project":"phase-b",
 "tool":"decidr","tool_version":"0.5.0","provider":"typesafe","model_requested":"jev-1.14.0","model_answered":"jev-1.14.0","usage":null,
 "request_sha256":"sha256:...","request":{...},"question_digests":{"q":"sha256:..."},"result":{"ok":true,"value":{...,"attempts":1,"session_id":"01J...","cached":false}}}}
.../runs.jsonl   action "run.link"   fields {"run_id":"01K...","session_id":"01J...","cached":false}   (true on a cache hit, session_id of the older row)
```
No `run_id`, start or end time in the row (ADR-SAG-0021). `usage` and `model_answered` are `null` when the reply has none (a 401 row has `model_answered: null`).
Config: key `cache` (default true), env `DECIDR_CACHE=false`, doctor row `{"path":"cache","value":"true","source":"default"}`; `DECIDR_CACHE=1` -> `config.invalid`.
Rust: `Run::new(RunArgs{ tool:"decidr".into(), tool_version:"0.5.0".into(), cache:None, .. }, None, cancel)?; run.run_id`; empty tool or tool_version -> `decision.missing_tool`.
Python:
```python
with sc_agent.Run(tool="plan-anchor", tool_version="1.2.0", cache=False, agent_role="plan-anchor", project="p") as run:
    run.run_id            # 26-char ULID, equals the links' run_id
    out = run.send(req)   # value has "cached": false|true, "session_id"
rows = sc_agent.session_rows([out["value"]["session_id"]], agent_role="plan-anchor", project="p")
# {"ok": True, "value": {"01J...": {...row fields...}}} ; missing id -> {"ok": False, "error": {"id": "session.not_found", ...}}
```
CLI: no new flag; `decidr send` writes `tool: "decidr"`, `tool_version` = the CLI version.

## Deletions

- crates/sc-agent/src/lib.rs: `static SESSION_APPEND` :1284; the append block :1464-1488 and `config.session`; the `sends` local :1407 and `Route.sends`; `send_json` :1226 and `body.clone()` :1224; the duplicate sc-sha call :969-974; the `response` field :1490.
- crates/sc-agent-py (j-2 plan): the "tool is required" check.
- Tests: `req_sag_0002_session_append_failure_keeps_provider_outcome_and_logs_once` (sending.rs:597; replaced by `j3_session_write_failure`). `req_sag_0005_one_mib_request_is_recorded_whole` (records.rs:59) is replaced by `j3_largest_request_that_fits_is_recorded_whole` (owner, 2026-11-24, approved): the largest request whose enveloped row fits the sink's 1 MiB event cap is sent and its row read back whole; one byte larger is still sent whole to the model, writes no row and no link, and returns `log.session_write` (logged once).
- Nothing else: `model_with`, Route fields, wire impls, batch code are untouched.

## Owner rulings applied (2026-11-24, j1-bead.txt:352-354)

1. Event-wrapped session row accepted: the sink is used as ADR-SAG-0021 says; the row is the `fields` of the event (see External contracts).
2. 1 MiB: the largest request that fits is recorded whole; a larger one gives no row and a `log.session_write` error (the request itself is still sent whole, so ANC-004 holds). Covered by `j3_largest_request_that_fits_is_recorded_whole`; the failure path is the `log.session_write` rule in Run::send step 4.
3. A cache hit writes no new row and returns the `session_id` of the row holding the result with `cached: true`: Run::send step 3 (link only; the value is the older row's `result.value`, whose `session_id` is that row's). Checked against `j3_cache_hit_one_model_call`.

## Budget (rewrite.py counting; against the 2,580 phase cap; j-1 end 2,402)

Rule 2 counting: restructuring must net <= 0 against what it replaces and is counted 0 here; only new capability grows the total. The restructure estimates (j-2 about -11, this sprint about -4) are unbuilt and are not used to buy room.

| | lines | basis |
|---|---|---|
| Restructure: sink writer + `event` + `sha` + `session_dir` + body bytes + row literal replacing SESSION_APPEND/OpenOptions block/duplicate sha/`send_json`/`sends`; must net <= 0 (reuse j-1 `log`, `digest`, `Route`; the row's fields beyond today's literal are new capability below) | 0 (plan estimate about -4) | block removed is about 31 lines |
| New capability: `cache` key + lookup + hit path + `Exchange` | about +45 | estimate |
| New capability: per-call `cache` argument on `send`/`send_batch` (Rust) | about +4 | estimate |
| New capability: `run_id`, `tool_version`, `missing_tool`, links, `log` category arm, `session.unreadable` | about +30 | estimate |
| New capability: `session_rows` + `rows` reader incl. `session.not_found` | about +25 | estimate |
| New capability: `log_as` + `PyRun.log` (event dict -> `log_as` + flush) | about +11 | estimate |
| New capability: sitr `tool=` on 4 pyfunctions (J3-S19; moved from j-4) | about +6 | estimate |
| New capability: PyO3 kwargs, `run_id` getter, `session_rows` pyfunction, per-call `cache` kwarg; decidr 2 | about +27 | estimate |
| **This sprint** | restructure 0, new capability about +148 | not built |
| **Phase** | j-1 2,402 + j-2 new capability 19 + this sprint 148 = **about 2,569 / 2,580 (about 9 under, new capability only)** | with the unbuilt restructure estimates (-15) about 2,554 (26 under) |
At the cap the dev stops at the gate and reports the line. No per-function counts.

## Done checks

- Build: `RUSTUP_TOOLCHAIN=1.98.1 cargo build --workspace --locked`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo fmt --check --all`.
- Gate: `RUSTUP_TOOLCHAIN=1.98.1 python3 scripts/gate/rewrite.py` all PASS and Rust source <= 2,580, run at every push.
- Greps: `grep -rn 'SESSION_APPEND\|OpenOptions\|send_json\|config.session' crates/sc-agent/src` none; `grep -n 'sends: &' crates/sc-agent/src` none; `grep -rn 'struct SessionRow\|struct RunLink\|struct CacheKey' crates` none.
- Rust tests (deterministic: loopback Stub, injected wait seam, controlled files; no sleeps): records.rs `j3_sinks_never_rotate` (both files pre-created sparse past `RetainedLogPolicy::default().rotation_max_bytes`, mtime UNIX_EPOCH; one send adds exactly one line each, no sibling file), `j3_row_v2_per_wire` (messages, chat/completions, systemone: exact schema/tool/tool_version/provider/model_requested/model_answered/usage, `request_sha256` = sc-sha of the stub-recorded body bytes, no `run_id`, `usage` null without usage; `decision.result` fields keys exactly `session_id`,`cached`), `j3_session_write_failure` (sessions path obstructed: 200 -> `log.session_write` with `session_id`; 401 -> `http.auth` whole; once in the general log; no runs.jsonl), `j3_link_write_failure`, `j3_batch_links` (batch of 3 -> 3 links `{run_id,row session_id,false}`), `j3_interrupt_keeps_finished_rows` (REQ-SAG-0011; workers 1, cancel after result 1 of 3 -> 1 row, 2 `decision.cancelled`); kept `old_session_file_survives_a_new_decision_unchanged`. cache.rs `j3_cache_hit_one_model_call` (second send: same value + session_id, `cached: true`, 1 row, links false then true), `j3_cache_hit_skips_holdoff_and_wait` (second stub reply would be 529 Retry-After 3600; 0 requests, 0 wait calls), `j3_cache_newest_success_only` (two hand-written ok rows -> greater session_id; error row only -> model call; unreadable file mode 000 (skipped as root) -> `session.unreadable`, 0 requests), `j3_cache_misses` (table: other model, project, agent role, `cache` false), `j3_per_call_cache_overrides_the_run` (Some(false) and Some(true) against the Run's setting, send and send_batch; the value reaches Route::post through `Exchange.cache`), `j3_cache_batch_per_item` (batch of 10 resent with 2 changed -> 2 requests, 10 results in order, 8 `cached: true`), `j3_error_row_is_not_a_hit` (401 row has `model_answered: null`; resend makes a model call). config_and_values.rs `j3_cache_setting` (default true; `DECIDR_CACHE=false`; `DECIDR_CACHE=1` -> `config.invalid`; doctor row), `j3_missing_tool` (empty tool or tool_version -> `decision.missing_tool`, 0 requests, no row), `j3_log_categories` (table over the ids above and the unchanged ids), `j3_session_rows` (found ids, missing id -> `session.not_found`, unreadable -> `session.unreadable`). wires.rs: mechanical `Route.exchange`. support/mod.rs: `options` gains `tool_version` and `settings.tool`/`settings.cache` ; `rows()` reads `fields`.
- decidr cli.rs: `j3_decidr_row_identity` (row under `<log_root>/sessions/github.com/randlee/sc-agent/rust-developer/phase-b/`, tool `decidr`, tool_version = CLI version).
- Python (tests/contract/test_session.py new, test_contract.py): `test_run_id_and_links` (26-char ULID = links' run_id; two Runs differ), `test_missing_tool_refuses` (None and "" for tool/tool_version from every call), `test_run_cache_override` (Run `cache=False` -> 2 stub requests), `test_send_cache_argument_overrides_the_run` (None follows the Run; `cache=False` on a caching Run -> 2 requests; `cache=True` on a Run with cache False -> hit), `test_run_log_writes_event_and_flushes` (event reaches the general log with the given target, action, correlation_id and fields, not target sc-agent; a bad event is `sc_agent_py.invalid_option`), `test_sitr_tool_kwarg_names_the_log` (sitr call with tool= logs under that tool), `test_rendered_template_passes_tool_version` (render with tool_version; Run opened with it; render without it fails --strict), `test_session_rows_reader` (child process, `<CLIENT>_LOG_ROOT` set, temporary git repo: found ids; missing id `session.not_found`; no log-root parameter), `test_cli_and_wheel_rows_match` (equal except session_id/tool/tool_version), `test_sitr_default_log_root` (REQ-SAG-0003, `SC_AGENT_HOME`), existing `test_session_and_general_log_share_fixed_root` kept; `test_native_stub_declares_every_export` still passes.
