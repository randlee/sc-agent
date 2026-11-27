# j-2 plan: the Run, on Rust, decidr and the wheel

Base: sprint/j-1 @b4be55e6 (gate 2342/2500). Line refs are file:line at that commit. Written by reading the code; the Rust parts were prototyped on this branch (see Budget).

## Goal

Rand (ANC-008, ANC-010, ANC-011): "Related values are strategically grouped in one struct to remove repeated method parameters"; "any copy/paste was a big red flag"; "I expect the python api may allow more flexibility than cli, NOT less." Measurable: one `Run` struct replaces the send/batch parameter lists in Rust, decidr and the wheel; no PyCapsule, no `unsafe`, no string-keyed option table in crates/sc-agent-py; restructuring adds 0 net lines; phase total <= 2,342 (j-1 end) + new capability, and <= 2,500.

Premise found in the code: j-1 @b4be55e6 has no `Run`, no `Model`, no J1-13 compat block and no `configured` (j-1's `load` :1379 is that helper). The old j-2 bead names those; they are dropped, not planned.

## Layout

| file | holds |
|---|---|
| crates/sc-agent/src/lib.rs | `Run` (struct, new, logger, close, send, send_batch); `Options` without `provider`; `TYPESAFE` |
| crates/decidr/src/{main,cli}.rs | decidr on one `Run` per send; `--provider` removed |
| crates/sc-agent-py/src/lib.rs | pyclasses `Logger`, `Run`; helpers; `provider_keys`; module `TYPESAFE` |
| crates/sc-agent-py/python/sc_agent/{__init__.py,__init__.pyi,_native.pyi} | the Python contract |
| crates/sc-agent-py/python/sc_agent/templates/consumer.py.j2 | the two call sites only |
| scripts/integration.py | credential names from `decidr models` |
| crates/sc-agent-py/README.md, README.md (stale lines), CHANGELOG.md, docs/integration/prerelease.md | docs |
| tests: crates/sc-agent/tests/{support/mod.rs,sending.rs,records.rs,wires.rs}, crates/decidr/tests/cli.rs, tests/contract/{test_contract,test_template,test_integration}.py, crates/sc-agent-py/tests/loopback.py | ported to the Run |

## Public surface (exact Rust)

Library pattern (rule 0): the tool builds one `Run`; the library owns the default logger and the default wait/keyring; injection (logger, `wait`, `keyring`, `base_url`) is the test seam. A closed owned logger makes later sends return the logger's own error; no `run_closed` id.

```rust
// crates/sc-agent/src/lib.rs
pub const TYPESAFE: &str = "typesafe";          // new capability; the PROVIDERS row uses it

pub struct Options {                              // :843 minus `provider`
    pub environment: Vec<(OsString, OsString)>,
    pub settings: Settings,
    pub model: String,
    pub agent_role: String,
    pub timeout: Duration,
    pub base_url: Option<String>,
    pub root: PathBuf,
    pub log_root: PathBuf,
    pub repo: String,
}

/// Caller inputs of a Run (ANC-008: one struct, not 8 parameters). None = library default.
pub struct RunArgs {
    pub tool: String,                 // names the default logger and its log root (<TOOL>_LOG_ROOT or ~/.sc-agent)
    pub agent_role: String,
    pub model: Option<String>,        // None -> "jev"
    pub project: Option<String>,
    pub config_dir: Option<PathBuf>,
    pub timeout_secs: Option<f64>,    // None -> 120
    pub workers: Option<i64>,         // None -> configured
    pub base_url: Option<String>,     // test seam
}

pub struct Run {
    pub options: Options,             // the layered result; options.model is the selector the caller gave
    pub cancel: Arc<Cancel>,
    pub wait: Box<dyn Fn(Duration) + Send + Sync>,     // test seam; default set by from_options: cancel.wait
    pub keyring: Box<dyn Fn(&str) -> Result<String, KeyringError> + Send + Sync>, // test seam; default: os_keyring
    provider: &'static Provider,
    model: String,                    // the resolved wire id (differs from options.model)
    logger: Arc<Logger>,
    owned: bool,
}
impl Run {
    /// The library way: runs `load` (layered config), applies the overrides (decidr.invalid_workers,
    /// decidr.invalid_request_timeout), reads the process environment, derives log_root from the logger's service
    /// name (or `tool`), resolves the model once. `None` logger: owns `default_logger(&args.tool)`.
    pub fn new(args: RunArgs, logger: Option<Arc<Logger>>, cancel: Arc<Cancel>) -> Result<Self, Error>;
    /// Test seam: a fully built Options (injected environment, root, log_root, repo); same defaults for wait/keyring.
    pub fn from_options(options: Options, tool: String, logger: Option<Arc<Logger>>, cancel: Arc<Cancel>) -> Result<Self, Error>;
    pub fn provider(&self) -> &'static Provider;
    pub fn logger(&self) -> &Logger;
    /// flush; shut down only an owned logger (an injected one is never changed, ANC-006).
    pub fn close(&self) -> Result<(), Error>;
    pub fn send(&self, request: &Value) -> Result<Value, Error>;
    pub fn send_batch(&self, requests: &[Value], on_result: &(dyn Fn(usize, &Value) + Sync)) -> Result<Value, Error>;
}
```
Config and logging follow rule 0: a tool passes its name and `RunArgs`; decidr and the wheel do no `load`, `vars_os`, `log_root` or `default_logger` wiring. The tool name also drives the layered loader (ADR-SAG-0020: each CLI's file is `<cli>.json`/`<cli>.local.json`, its variables are `<CLI>_`; `sc-agent.json` and `SC_PROJECT` stay shared):
```rust
pub fn config(tool: &str, local: Option<&Path>, global: Option<&Path>, environment: &[(OsString, OsString)], project: Option<&str>) -> Result<Configuration, Error>;
pub fn load(tool: &str, config_dir: Option<&Path>, project: Option<&str>) -> Result<(Configuration, PathBuf, String), Error>;
pub fn doctor(tool: &str, config_dir: Option<&Path>, project: Option<&str>) -> Result<Value, Error>;   // log_root(tool)
struct ConfigEnv(String, Vec<(OsString, OsString)>);   // prefix = tool upper-cased, '-' -> '_', then '_'
```
For tool "decidr" behaviour is byte-for-byte what it is now (REQ-SAG-0003, REQ-SAG-0020). Replaces config() hard-coded `"decidr.json"`, `"decidr.local.json"` (lib.rs:257) and `"DECIDR_"` (:196, :204), and `load`/`doctor` copies of "decidr" (:1467). The `missing_setting` texts (:147, :168) still name DECIDR_/decidr files; reworded with the tool identity in j-3.

Python `tool`: `Run(tool=...)` is required unless `general_log` is given; with `general_log` the tool is that logger's service name (`logger("my-tool")` names the tool); neither -> `sc_agent_py.invalid_option` "tool is required" (stored, returned by every call). No `"decidr"` default exists in the wheel; decidr passes "decidr".

`Route` (:936), `Jev`/`OpenAiCompat`/`AnthropicCompat`, `model_with` and every j-1 error id are unchanged; `Run::send` builds the `Route` from `self`.

```rust
// crates/sc-agent-py/src/lib.rs (private Rust names, Python names in quotes)
#[pyclass(name = "Logger", frozen)] struct PyLogger(Arc<Logger>);
#[pyclass(name = "Run", frozen)]    struct PyRun(Result<sc_agent::Run, Value>); // Err = stored, already-logged error envelope

fn logger_of(value: &Bound<'_, PyAny>) -> Result<Arc<Logger>, Error>;
fn arg<T: for<'a, 'py> FromPyObject<'a, 'py>>(value: Option<&Bound<'_, PyAny>>, name: &str) -> Result<Option<T>, Error>;
fn extras(extra: &Bound<'_, PyTuple>, rest: Option<&Bound<'_, PyDict>>) -> Result<(), Error>;
fn call_back<A, T>(callback: &Py<PyAny>, argument: A, cancel: &sc_agent::Cancel) -> Result<T, PyErr>
where A: for<'py> IntoPyObject<'py>, A: Send,   // (A,) is the call tuple
      T: for<'a, 'py> FromPyObject<'a, 'py>;   // f64 delay -> Py<PyAny>; &str name -> String
fn respond_value(given: Option<&Bound<'_, PyAny>>, name: &str, run: impl FnOnce(&Logger) -> Result<Value, Error>) -> Value;

#[pymethods] impl PyRun {
    #[new]
    #[expect(clippy::too_many_arguments, reason = "PyO3 needs a parameter per public keyword")]
    #[pyo3(signature=(*extra, tool=None, model=None, project=None, agent_role=None, config_dir=None, general_log=None,
                      timeout_secs=None, workers=None, base_url=None, sleep=None, keyring_lookup=None, **rest))]
    fn new(py: Python<'_>, extra: &Bound<'_, PyTuple>, tool: Option<&Bound<'_, PyAny>>, model: Option<&Bound<'_, PyAny>>,
           project: Option<&Bound<'_, PyAny>>, agent_role: Option<&Bound<'_, PyAny>>, config_dir: Option<&Bound<'_, PyAny>>,
           general_log: Option<&Bound<'_, PyAny>>, timeout_secs: Option<&Bound<'_, PyAny>>, workers: Option<&Bound<'_, PyAny>>,
           base_url: Option<&Bound<'_, PyAny>>, sleep: Option<&Bound<'_, PyAny>>, keyring_lookup: Option<&Bound<'_, PyAny>>,
           rest: Option<&Bound<'_, PyDict>>) -> Self;     // never raises; failure is logged once and stored
    fn __enter__(slf: Py<Self>) -> Py<Self>;
    fn __exit__(&self, kind: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>, traceback: &Bound<'_, PyAny>) -> bool; // Run::close; False
    #[getter] fn provider(&self) -> Option<&'static str>;  // Run::provider().name; None when creation failed
    #[pyo3(signature=(request=None, *extra, **rest))]
    fn send(&self, py: Python<'_>, request: Option<&Bound<'_, PyAny>>, extra: &Bound<'_, PyTuple>, rest: Option<&Bound<'_, PyDict>>) -> Py<PyAny>;
    #[pyo3(signature=(requests=None, *extra, on_result=None, **rest))]
    fn send_batch(&self, py: Python<'_>, requests: Option<&Bound<'_, PyAny>>, extra: &Bound<'_, PyTuple>,
                  on_result: Option<&Bound<'_, PyAny>>, rest: Option<&Bound<'_, PyDict>>) -> Py<PyAny>;
    #[pyo3(signature=(*extra, **rest))]
    fn cancel(&self, py: Python<'_>, extra: &Bound<'_, PyTuple>, rest: Option<&Bound<'_, PyDict>>) -> Py<PyAny>;
}
#[pyfunction] fn provider_keys() -> Vec<&'static str>;      // new capability: non-empty `key` of each PROVIDERS row, in order
// module: add_class Logger, Run; add TYPESAFE; register! keeps logger, log, flush, digest, languages, queries, query, run
```
Python (names frozen after j-2; `tool` is a Run keyword now because Run::new needs it; j-3 adds `run_id`, `session_rows`, and the Run keywords `tool_version`, `cache` together with the code that stores them, so no stub declares a name the native module lacks):
- `__init__.py`: `from ._native import Logger, Run, TYPESAFE, digest, flush, log, logger, provider_keys`; `__all__ = ["Logger","Run","TYPESAFE","digest","flush","log","logger","provider_keys","sitr"]`; `from . import sitr`.
- `_native.pyi`: `class Run: __init__(self, *, tool: str|None=None, model=None, project=None, agent_role=None, config_dir=None, general_log: Logger|None=None, timeout_secs=None, workers=None, base_url=None, sleep=None, keyring_lookup=None) -> None; __enter__ -> Self; __exit__(...) -> bool; provider: str|None; cancel() -> Envelope; send(request: Request) -> Envelope; send_batch(requests: Iterable[Request], *, on_result: Callable[[int, Envelope], object]|None=None) -> Envelope`; `class Logger`; `TYPESAFE: str`; `provider_keys() -> list[str]`; the unchanged logger/log/flush/digest/sitr stubs. Not declared: module send/send_batch, Cancel, `_new_cancel`, `_cancel`, TestSeams, Run.close, Run.project, Model, seed_session.

## Replaces (item -> existing construct; what is deleted with it)

| item | replaces | deleted with it |
|---|---|---|
| `RunArgs`, `Run::new`, `from_options`, `provider`, `logger`, `close` | the repeated `(options, logger, cancel, wait, keyring)` parameters of `send` lib.rs:1170 and `batch` :1283; decidr main.rs:41-49 (`load`), :62-67 (workers), :80-92 (Options literal, `log_root`, `vars_os`); PyO3 `options` :144-179 and its `log_root` call :166 | the 5 parameters in 2 signatures; the 3 hand-built wirings; `Options` literals outside tests |
| `config`, `load`, `doctor` take `tool` | hard-coded names in `config` lib.rs:196, :204, :257 and `doctor` :1467 | the literals; none of ADR-SAG-0020's names change for decidr |
| `PyRun.__enter__`, `__exit__`, `provider` getter | none (ADR-SAG-0019 d14: `with sc_agent.Run(...)`; `run.provider`); new capability | - |
| `Run::send` | `pub fn send` lib.rs:1170 | free fn |
| `Run::send_batch` | `pub fn batch` lib.rs:1283 | free fn |
| `Options` minus `provider` | `Options.provider` :848; `model_with(.., options.provider.as_deref(), ..)` call in send :1190; decidr `--provider` cli.rs:19; PyO3 `provider` keyword. REQ-SAG-0103: no caller names a provider; config `models.<name>.provider` does | the field, flag, keyword |
| `PyLogger`, `logger_of` | `handle` :68-80, `capsule` :81-95, c"sc_agent.Logger" capsule | PyCapsule, `unsafe`, `CStr` |
| `PyRun` + methods | module `send` :338-386, `send_batch` :383-460, `_new_cancel`, `_cancel` :461-481, `cancel_flag` :195-205, Python `Cancel` (__init__.py:7-22) | all of those, the `cancel=` keyword |
| `arg`, `extras` | `option` :127-143 and the `&[(&str, Option<..>)]` option table | table, `options` helper |
| `call_back` | `wait` :218-230 and `lookup` :257-276 | both |
| `respond_value` | body of `respond` :96-114 (respond becomes `raw(py, respond_value(..))`) | none |
| `register!` id | `sc_agent_py.invalid_call` :547 -> `sc_agent_py.invalid_option` (one id, `invalid` :15 already uses it) | the id; 4 registrations (send, send_batch, _new_cancel, _cancel) |
| decidr on `Run` | main.rs: free `send`/`batch` calls :60-110, `provider` binding, local logger clone | `--provider`, per-call param lists |
| template call sites | consumer.py.j2:249-302 `cancel`/`sc_agent.Cancel()`/`sc_agent.send*`; :221 `_abort_if_selected(.., cancel: Cancel)` -> `Run`; :40 TYPE_CHECKING import | `cancel` params x2, `cancel = cancel or sc_agent.Cancel()` x2, Cancel import |
| `integration.py` names from `decidr models` | regex over lib.rs :16-18 and hard-coded `("typesafe","anthropic","openai")` :35, "TypeSafe key is required" :38 | both, the error |
| new capability (counted apart): `TYPESAFE`, `provider_keys`, `Run.provider` getter | none; ADR-SAG-0019 d1 lists `provider_keys` and `TYPESAFE` as public, j-4's consent gate keys on `run.provider == TYPESAFE` | - |

## External contracts (examples)

Wire: unchanged (j-1).

Python:
```python
with sc_agent.Run(model="haiku", project="p", agent_role="r", timeout_secs=30, workers=2,
                  general_log=sc_agent.logger("my-tool")["value"]) as run:
    one = run.send({"questions": {...}})                         # {"ok": True, "value": {...}} | {"ok": False, "error": {...}}
    many = run.send_batch([r1, r2], on_result=lambda i, v: ...)  # {"ok": True, "value": [{"index": 0, "result": {...}}, ...]}
    run.cancel(); run.provider   # "anthropic"
sc_agent.provider_keys()  # ["TYPESAFE_API_KEY","ANTHROPIC_API_KEY",...]; sc_agent.TYPESAFE == "typesafe"
```
Nothing raises. Bad or extra argument, wrong type, bad `timeout_secs`/`workers`, unknown model: the Run is created, the error is logged once and stored, and `send`, `send_batch`, `cancel` return it (`sc_agent_py.invalid_option` message names the argument; `decidr.invalid_request_timeout`; `decidr.invalid_workers`; `types.unknown_short_name`; `config.missing_project` at send). `run.send(r, "x")`, `run.send(r, model="m")`, `run.send_batch([r], workers=2)`, `run.cancel(1)` -> `sc_agent_py.invalid_option`. Per-call overrides stay refused (REQ-SAG-0702). Callback exception -> `sc_agent_py.callback`; KeyboardInterrupt in `sleep`/`keyring_lookup`/`on_result` cancels the Run.

Rust:
```rust
let run = Run::new(options, None, Arc::new(Cancel::default()))?;   // owns default_logger("decidr")
let v = run.send(&request)?; run.close()?;
```
CLI: `decidr send --agent-role r [--model jev] [--input f] [--batch] [--workers n] [--timeout-secs 120]`; `--provider x` -> clap error -> `decidr.usage`. `decidr models` unchanged (`json!(PROVIDERS)`). Config: `models.<name>.provider` selects the provider (REQ-SAG-0103).
Paths: default logger `<DECIDR_LOG_ROOT|~/.sc-agent>/logs/decidr.log.jsonl`; sessions unchanged.

## Deletions

- crates/sc-agent/src/lib.rs: `pub fn send` :1170, `pub fn batch` :1283 (become methods), `Options.provider` :848.
- crates/decidr/src/cli.rs:19-20 `provider`; main.rs `provider` binding and free calls.
- crates/sc-agent-py/src/lib.rs: `handle` :68, `capsule` :81, `option` :127, `options` :144, `cancel_flag` :195, `wait` :218, `lookup` :257, `send` :338, `send_batch` :387, `_new_cancel` :461, `_cancel` :468, `invalid_call` :547, imports `PyCapsule`, `CStr`.
- __init__.py: `Cancel` class and `send, send_batch, _new_cancel, _cancel` imports; __init__.pyi `Cancel`, `send`, `send_batch`; _native.pyi `send`, `send_batch`, `_new_cancel`, `_cancel`.
- consumer.py.j2: `cancel` params, `sc_agent.Cancel()` x2, Cancel import :40.
- scripts/integration.py :16-18, :35, :38.
- Tests/fixtures of the removed API (ANC-003 style, deleted not skipped): `Cancel` use in test_contract/loopback, `provider=` cases, `opts.provider = Some(..)` in sending.rs:31 (selected by a `settings.models` entry instead).
- Not deleted (j-1's): `model_with` and its `provider` argument (test seam of the table), Route, Decision/Llm.

## Budget (rewrite.py counting; against 2,500 phase cap, base 2,342)

| | lines | basis |
|---|---|---|
| Restructure: sc-agent `RunArgs`, `Run`, `new`, `from_options`, close, provider, minus send/batch parameter lists incl. the `tool` parameter of config/load/doctor (lib.rs 1253 -> 1340) | +87 | measured on this branch |
| Restructure: decidr main.rs 144 -> 128, cli.rs 33 -> 31 | -18 | measured (decidr no longer calls load, vars_os, log_root, workers) |
| Restructure: PyO3 lib.rs 562 -> about 450 (module send/send_batch/Cancel/capsule/options/wait/lookup out, PyRun/PyLogger in) | about -110 | estimate, not yet built |
| **Restructure net** | **about -41** (target <= 0) | |
| New capability: `TYPESAFE` +1, `provider_keys` +5, `PyRun.provider` getter +3, `__enter__`/`__exit__` +6 | about +15 | estimate |
| **Sprint total** | **about 2,316 / 2,500** (worst case 2,364) | |
Python (uncounted by the gate): __init__.py 28 -> about 9, stubs about +30, template net <= +4, integration.py net about 0. Docs uncapped. No per-function counts.

## Done checks

- Build: `RUSTUP_TOOLCHAIN=1.98.1 cargo build --workspace --locked`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo fmt --check --all`.
- Gate: `RUSTUP_TOOLCHAIN=1.98.1 python3 scripts/gate/rewrite.py` all PASS; Rust source <= 2,342 + new capability (about 2,357); the config tests pass `tool`; fails the sprint above 2,364.
- Greps: `grep -rn 'PyCapsule\|unsafe\|invalid_call\|_new_cancel' crates/sc-agent-py/src` none; `grep -n 'pub fn send(\|pub fn batch(\|provider: Option' crates/sc-agent/src crates/decidr/src` none; `grep -rn 'recv_timeout' crates/decidr/src` none; `grep -rnE 'seed_session|TestSeams|def close|Cancel' crates/sc-agent-py/python/sc_agent/*.pyi` none.
- Rust tests (ported to `Run`; `support::run(opts, &logger)` helper): sending.rs and records.rs existing names kept; new `run_new_resolves_the_model_once_and_rejects_unknown` (REQ-SAG-0101), `run_close_shuts_down_only_an_owned_logger` (ANC-006), `run_timeout_and_workers_apply_to_every_call` (REQ-SAG-0702), `run_with_invalid_timeout_or_workers_is_an_error` (REQ-SAG-0702); wires.rs unchanged; decidr cli.rs: `send_provider_flag_is_a_usage_error`, `local_timeout_default_is_120_seconds`.
- Python (`python3 -m unittest discover -s tests/contract`, loopback.py, strict pyright, `check_consumer_template.py`): existing wheel tests ported by call shape (names unchanged); new `test_run_sends_through_loopback_and_closes`, `test_run_cancel_from_on_result_cancels_the_rest`, `test_run_invalid_arguments_return_invalid_option`, `test_module_exports_only_the_run_api`, `test_run_model_errors`, `test_provider_keys_typesafe_and_run_provider`, `test_native_stub_declares_every_export`; integration: `test_export_uses_current_provider_variables_and_injected_lookup` (rewritten), `test_export_with_only_an_anthropic_key`, `test_export_refuses_before_writing_without_credentials`. All deterministic (stub replies on events, sleep seam, barrier).
- Docs: wheel README (Run API, no Cancel/send), root README stale `--provider`/model lines, CHANGELOG Breaking entry (removed `send`, `send_batch`, `Cancel`, `provider=`, `--provider`, `invalid_call`), prerelease.md haiku/luna cases.

## Assumptions

- ANC-001 "config can override default": met by a `models.jev` entry (j-1); no `Settings.model` key added, `--model`/`model=` default stays "jev".
- Old-bead items dropped as not in the code: Model class, J1-13 block, `configured`, `decidr models` `{providers, models}` shape, `tool`/`tool_version`/`cache` (moved to j-3 with their storage).
