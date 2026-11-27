# j-4 plan: the consumer template on the Run

Base: sprint/j-3 @0f82d6da. `:N` = `crates/sc-agent-py/python/sc_agent/templates/consumer.py.j2` at that commit (668 lines). Wheel API = j-2 and j-3 planned end states (j-2-plan.md, j-3-plan.md); their code on this branch is partial. Written by reading the code; no j-4 code yet.

## Goal

Rand (ANC-015, ANC-016, rule 0): "judge-jev and plan-anchor simply supply tool name and follow the rules established for all decidr based tools"; "derivative tools should defer to the default tool rules and not use injection path for production code (test only)". Measurable: the rendered module opens one `sc_agent.Run(tool=CLIENT_NAME, tool_version=TOOL_VERSION, ...)` per command and sends only through it; it creates no Logger, computes no digest, run id or attempt count, and writes `jev-batch-result/2` lines with exactly the 14 generic fields; every template function returns `Ok`/`Err` (`open_run` included); `grep -nE 'hashlib|threading|_logger|Cancel|general_log=logger' consumer.py.j2` finds nothing.

## Layout

| file | holds |
|---|---|
| crates/sc-agent-py/python/sc_agent/templates/consumer.py.j2 | the consumer module: Result types, `open_run`, calls, consent gate, batch pump, `/2` lines, redaction |
| crates/sc-agent-py/python/sc_agent/testing.py (new) | `FakeRun`, `fake_open_run` (test fake of a Run; generated code never imports it) |
| crates/sc-agent-py/src/lib.rs | sitr pyfunctions gain `tool=` (see Budget; rule-7 item) |
| crates/sc-agent-py/python/sc_agent/_native.pyi | sitr stubs gain `tool: str \| None = None` |
| scripts/check_consumer_template.py | render vars gain `"tool_version": "0.0.0-test"` |
| tests/contract/test_template.py, crates/sc-agent-py/tests/loopback.py | tests below |

## Library pattern (rule 0)

- Config: the module passes the tool name and the caller's selector; the wheel's layered config resolves model, provider, cache, project, workers, timeout. The template reads no config file and no environment variable except the consent setting the tool owns.
- Logging: the module passes `tool=CLIENT_NAME`; the wheel's default logger writes `<CLIENT>_LOG_ROOT|~/.sc-agent>/logs/<client>.log.jsonl`. `general_log` exists only in `open_run(seams=...)` for tests (ANC-016). No module-level logger, lock or handle.
- Limits: none in the template (no worker cap, no request-size cap, no retries); the Run's own errors are returned.

## Public surface (exact Python; the template is Python, so the signatures are Python)

```python
# front matter: version: 2.0.0; required_variables: client_name, sitr, sc_decidr_version, tool_version
TOOL_VERSION = "{{ tool_version }}"

# moved from judge-jev result.py.j2:27-126 (CANCELED_EXIT/canceled_envelope already here :91-98, not duplicated)
T = TypeVar("T"); E = TypeVar("E")
@dataclass(frozen=True)
class Ok(Generic[T]):  value: T;  ok: Literal[True] = True
@dataclass(frozen=True)
class Err(Generic[E]): error: E;  ok: Literal[False] = False
Result = Ok[T] | Err[E]
@dataclass(frozen=True)
class ToolError: code: str; message: str; action: str | None = None; data: Any = None
def err(code: str, message: str, action: str | None = None, data: Any = None) -> Err[ToolError]
def utf8_stdio() -> None
def run_main(main: Callable[[], int]) -> int        # KeyboardInterrupt -> canceled_envelope, exit CANCELED_EXIT

@dataclass(frozen=True)
class Reply:
    answers: Mapping[str, Any]     # typed_answer applied
    session_id: str | None
    cached: bool

def load_wheel() -> Result[Any, ClientError]          # Err(DEPENDENCY.SC_DECIDR); sc_agent imported only here
def open_run(*, model: str | None = None, project: str | None = None, cache: bool | None = None,
             config_dir: str | None = None, timeout_secs: float | None = None,
             workers: int | None = None, seams: Mapping[str, Any] | None = None) -> Result[Any, ClientError]
    # sc_agent.Run(tool=CLIENT_NAME, tool_version=TOOL_VERSION, agent_role=CLIENT_NAME, model=model,
    #   project=project, cache=cache, config_dir=..., timeout_secs=..., workers=...,
    #   **(seams or {}))) ; Err(DEPENDENCY.SC_DECIDR) when the wheel cannot be imported.  seams keys: general_log, base_url, sleep, keyring_lookup (tests only)

def call_result(run: Any, request: str | Mapping[str, Any], *, cache: bool | None = None, abort_on: frozenset[str] = frozenset(),
                category_codes: Mapping[str, str] | None = None, repository_root: str = ".",
                consent_what: str = "The request text") -> Result[Reply, ClientError]
def call_batch_results(run: Any, requests: Sequence[str | Mapping[str, Any]], *, cache: bool | None = None, abort_on: frozenset[str] = frozenset(),
                       category_codes: Mapping[str, str] | None = None, repository_root: str = ".",
                       consent_what: str = "The request text") -> Result[list[Result[Reply, ClientError]], ClientError]
    # KeyboardInterrupt: run.cancel(); raise   (REQ-SAG-0018)
def log_event(run: Any, target: str, action: str, correlation_id: str | None, fields: Mapping[str, Any]) -> Result[None, ClientError]
    # redact_secrets(fields); run.log({"target","action","correlation_id","fields"}); flush on the Run's logger (the accessor j-6 needs; wheel: PyRun.log, j-3)
def decode_result(value: str | Mapping[str, Any], *, category_codes: Mapping[str, str] | None = None) -> Result[Reply, ClientError]
def remote_judge_consent(repository_root: str = ".", what: str = "The request text") -> Result[None, ClientError]

BATCH_RESULT_SCHEMA = "jev-batch-result/2"
class BatchSettings(NamedTuple): gate: str; repeat: int; prefix: str; threshold: float; dry_run: bool
class ResultLine(NamedTuple):    # field order = JSON key order = owner ruling 2026-11-24 / ADR-SAG-0019 d1
    schema: str; tool: str; tool_version: str; run_id: str | None; session_id: str | None
    request_id: str; project: str | None; agent_role: str; threshold: float
    verdict: str | None; min_confidence: float | None; leaning: str | None
    ok: bool; error: ClientError | None
class StreamedResult(NamedTuple): failed: list[ResultLine]

def batch_settings(args: Any, prefix: str, *, check_threshold: Callable[[Any], Any]) -> Result[BatchSettings, ClientError]
def fixture_answer(job: BatchJob, fixture: Mapping[str, Any]) -> Result[Reply, ClientError]   # Reply(answers, None, False)
def result_line(job: BatchJob, settings: BatchSettings, run_id: str | None, project: str | None,
                result: Result[Reply, ClientError], *,
                verdict: Callable[[BatchJob, BatchSettings, Mapping[str, Any] | None], tuple[str | None, float | None, str | None]] | None = None) -> ResultLine
    # verdict(job, settings, answers | None) -> (verdict, min_confidence, leaning): fills exactly those 3 of the 14 fields;
    # None -> all three None. decide() stays in each tool.
def run_batch(run: Any | None, jobs: Sequence[BatchJob], settings: BatchSettings, *,
              on_line: Callable[[ResultLine, Mapping[str, Any] | None], object],
              project: str | None = None, fixture: Mapping[str, Any] | None = None,
              on_request: Callable[[Any], object] | None = None, cache: bool | None = None, abort_on: frozenset[str] = frozenset(),
              category_codes: Mapping[str, str] | None = None, repository_root: str = ".",
              consent_what: str = "The request text",
              verdict: Callable[[BatchJob, BatchSettings, Mapping[str, Any] | None], tuple[str | None, float | None, str | None]] | None = None
              ) -> Result[StreamedResult, ClientError]
def answers_for(lines: Sequence[Mapping[str, Any]]) -> Result[dict[str, dict[str, Any]], ClientError]
    # {request_id: answers}; one sc_agent.session_rows(ids, agent_role=CLIENT_NAME, project=p) per distinct line project
BEARER_RE: re.Pattern[str]          # r"(?i)\b(bearer\s+)[A-Za-z0-9._~+/=-]{16,}"
def redact_secrets(value: Any) -> Any     # iterative; replaces values of sc_agent.provider_keys() env vars and bearer tokens with "[redacted]"
def encode_result_line(line: ResultLine) -> str    # _asdict, error -> {code,message,action,category,recoverable,data}, redact_secrets, json.dumps + "\n"
{% if sitr %}def extract(language: str, source: str, query: str) -> Result[Any, ClientError]   # sc_agent.sitr.query(..., tool=CLIENT_NAME){% endif %}
```

`safe_id`, `validate_request_lines`, `load_answers_fixture` keep their parameters and return `Result[...]` instead of the dict record. Unchanged verbatim: `ClientError`, `from_tool_error`, `CANCELED_EXIT`, `canceled_envelope`, the four answer NamedTuples, `typed_answer`, `wheel_error` (minus the `_CLIENT_IDS` branch), `DECIDR_CATEGORY_CODES`, `_ALLOWED_ABORT_ON`, `BatchJob`, `batch_request_id`, `BATCH_FIXTURE_ERROR`, `BATCH_ID_RE`, `REMOTE_JUDGE_*`, `REPO_ROOT_HELP`, fixture validators, `write_stdout_line`, `json_answers`. No other public name. `ToolError` stays for the tool's own `Result` errors (moved block, used by consumer logic); `ClientError` stays for wheel failures (carries status, retry_after_ms, attempts, data); the moved code already uses both and `from_tool_error` converts.

Rust (new capability, rule-7 item 1): `#[pyfunction] languages|queries|query|run(..., tool: Option<String>)` in crates/sc-agent-py/src/lib.rs :484-527; `respond(py, general_log, tool.as_deref().unwrap_or("sitr"), ..)`.

`sc_agent/testing.py`:

```python
class FakeRun:
    def __init__(self, script: Mapping[str, Any], **open_args: Any) -> None   # .open_args, .sent: list[list[Any]], .cancels: int
    def sends_to(self, provider: str) -> bool                                  # provider == script["provider"]
    def log(self, event: Mapping[str, Any]) -> dict[str, Any]                  # appends to .logged; Envelope ok
    run_id: str | None            # script["run_id"]
    def __enter__(self) -> FakeRun
    def __exit__(self, *exc: object) -> bool                                  # False
    def send(self, request: Any, *, cache: bool | None = None) -> dict[str, Any]   # send_batch([request]) item 0's result; records cache in .sent
    def send_batch(self, requests: Sequence[Any], *, cache: bool | None = None,
                   on_result: Callable[[int, dict[str, Any]], object] | None = None) -> dict[str, Any]
        # pops script["envelopes"] in order; after cancel() the rest are decision.cancelled errors;
        # raises KeyboardInterrupt at index script.get("interrupt_at")
    def cancel(self) -> dict[str, Any]
@contextlib.contextmanager
def fake_open_run(module: types.ModuleType, script: Mapping[str, Any]) -> Iterator[list[FakeRun]]
    # patches module.open_run and sc_agent.session_rows (answers from script["rows"]); restores both
```

## Replaces (each item; what is deleted with it)

| item | replaces (file:line) | deleted with it |
|---|---|---|
| `TOOL_VERSION`, front matter 2.0.0 | `version: 1.0.0` :3, required_variables :6-9 | — |
| `Ok/Err/Result/ToolError/err/utf8_stdio/run_main` | judge-jev result.py.j2:27-126 and plan-anchor's copy (deleted in j-5/j-6); in the template: the `Record` result dicts :44 | `Record` as a result type |
| `Reply` | success dict in `decode_result` :204-205, :209-210 | keys position, line, digests, usage, model_answered, attempts |
| `load_wheel` -> Result | `load_wheel` :160-165, `_failure` :150-153, `_failed` :156-157 | `_failure`, `_failed` |
| `open_run` -> Result | per-call Run wiring in `call_result` :247-262 and `call_batch_results` :270-296; `_logger` :168-177, `_logger_handle`/`_logger_lock` :45-46 | per-call `model, project, cancel, config_dir, general_log, timeout_secs, workers`; the module Logger; `import threading` |
| `call_result`, `call_batch_results` on a Run | same, :247-305 | `on_result` callback, `sc_agent.Cancel()`, synthesized cancelled results :263-265, :297-299 |
| consent gate in `_start` asks `run.sends_to(sc_agent.TYPESAFE)` (the library's per-send resolution: keyless first, then the first held key; no provider guess) | `_start` :231-244 (consent on every call), `remote_preflight` :341-343 | `remote_preflight` |
| `remote_judge_consent` -> Result | :315-338 | dict returns |
| `wheel_error` | :180-191 | `_CLIENT_IDS` :55, branch :184-185 |
| `_abort_if_selected(line, abort_on, run)` | :221-224 (`cancel: Cancel`) | `TYPE_CHECKING`/`Cancel` import :37-40 |
| `BATCH_RESULT_SCHEMA` /2 | :346 | `BATCH_RETRIES` :348, `BATCH_MAX_WORKERS` :347 (worker limits are the Run's) |
| `BatchSettings` (5 fields) | :355-363 | `model`, `timeout`, `max_workers`; max_workers check :494-495 |
| `StreamedResult(failed)` | :378-380, `streamed_result` :653-655, `total_attempts` :641-650 | `attempts` |
| `ResultLine`, `result_line` | dict line :505-539 | /1 keys item_id, anchor_id, candidate_id, repeat, request, request_sha256, questions_sha256, roles, kind, dry_run, raw_response, answers, model_answered, sends, retry_count, attempts, suggested_action; `error_message_limit` |
| (none) | `_sha256` :389-390, `questions_digest` :393-395 | `import hashlib`; local run_id minting :523 (run_id is the Run's ULID) |
| `fixture_answer` | `fixture_attempt` :416-440 | `attempt`, list entries, `transient_codes`, `Container` import |
| `run_batch` | `check_fixture_coverage` :567-579 and the send loops each consumer writes (judge-jev jev_judge.py.j2, plan-anchor jev_judge.py.j2; deleted in j-5/j-6) | `check_fixture_coverage` |
| `answers_for` | consumers reading answers from /1 lines (`answers` key) | — |
| `BEARER_RE`, `redact_secrets` | judge-jev jev_errors.py.j2:17-30 `redact_sensitive`, plan-anchor jev_judge.py.j2:236 (deleted in j-5/j-6) | — |
| `verdict=` callback of `result_line`/`run_batch` | judge-jev jev_judge.py:850-857 `batch_verdict_fields` and plan-anchor jev_judge.py:1913 `leaning_of`, :2222, :2247 (deleted in j-5) building verdict keys into the line dict; `decide()` (judge-jev :731, plan-anchor :1842) stays in each tool | the dict-merge of tool keys into the line |
| `encode_result_line(line)` | :635-638 | `redact=` opt-out |
| `extract` | :659-667 | `general_log=` and `_logger` use |
| `safe_id`, `validate_request_lines`, `load_answers_fixture`, `batch_settings` -> Result | :398-402, :560-564, :582-595, :489-502 | dict returns |
| `testing.FakeRun`, `fake_open_run` | none: new capability (ADR-SAG-0019 Implementation l.120 names `sc_agent/testing.py`) | — |
| sitr `tool=` kwarg | none: new capability (see rule-7 item 1) | — |

## External contracts (examples)

/2 line (one per job, index order):
```json
{"schema":"jev-batch-result/2","tool":"plan-anchor","tool_version":"1.2.0","run_id":"01KC...","session_id":"01KC...",
 "request_id":"plan/I-1/ANC-001/c1/r1","project":"phase-j","agent_role":"plan-anchor","threshold":0.6,
 "verdict":"pass","min_confidence":0.82,"leaning":null,"ok":true,"error":null}
```
Failed: `"ok":false,"error":{"code":"AUTH.CREDENTIALS","message":"...","action":"...","category":"auth","recoverable":false,"data":{...wheel error...}}`. Fixture run without a Run: `run_id` and `session_id` null. Dry run: no line.

Render: `sc-compose render --file consumer.py.j2 --var-file vars.json --strict` with `{"client_name":"plan-anchor","sitr":false,"sc_decidr_version":"0.6.0","tool_version":"1.2.0"}`; missing `tool_version` fails.

Use:
```python
with client.open_run(model=args.model, workers=args.workers) as run:
    out = client.run_batch(run, jobs, settings, project=args.project, on_line=write)
```
Paths: general log `<PLAN_ANCHOR_LOG_ROOT|~/.sc-agent>/logs/plan-anchor.log.jsonl` (wheel default); sitr calls in the judge-jev render log to `judge-jev.log.jsonl` (via `tool=`). Consent: `.sc/<client>/settings.yaml` `allow_remote_judge`, read only for a TypeSafe Run.

## Deletions (file:line at 0a44e3e0)

consumer.py.j2: `import hashlib` :28, `import threading` :35, `Container` :36, `TYPE_CHECKING`/`Cancel` :37-40, `_logger_handle`/`_logger_lock` :45-46, `_CLIENT_IDS` :55, `_failure` :150-153, `_failed` :156-157, `_logger` :168-177, per-call Run parameters and interrupt synthesis :247-305, `remote_preflight` :341-343, `BATCH_MAX_WORKERS`, `BATCH_RETRIES` :347-348, BatchSettings `model/timeout/max_workers` :357,:361-362, `StreamedResult.attempts` :380, `_sha256`/`questions_digest` :389-395, `fixture_attempt` :416-440, `/1` line body :511-539, `check_fixture_coverage` :567-579, `encode_result_line(redact=)` :635, `total_attempts`/`streamed_result` :641-655, `_logger` use in `extract` :664-666.
tests: test_template.py:33 assertions on `fixture_attempt`/`total_attempts`; loopback.py `test_keyboard_interrupt_cancels_the_passed_cancel` :310, `test_wheel_keyboard_interrupt_reaches_each_consumer_cancel_handler` :353, `test_logger_initialization_is_serialized` :172 (no module logger remains).

## Done checks

- Build: `maturin develop` (wheel), `python3 scripts/check_consumer_template.py` renders both configurations `--strict`; strict Pyright on both renders and testing.py.
- Gate: `RUSTUP_TOOLCHAIN=1.98.1 python3 scripts/gate/rewrite.py` all PASS; Rust source <= j-3 end + ~6 and <= 2,500.
- Greps (empty): `grep -nE 'hashlib|threading|_logger|Cancel|remote_preflight|total_attempts|fixture_attempt|questions_digest' crates/sc-agent-py/python/sc_agent/templates/consumer.py.j2`; `grep -rn 'jev-batch-result/1' crates/sc-agent-py tests/contract`; `grep -n 'sc_agent.testing' <renders>`.
- tests/contract/test_template.py (FakeRun, deterministic): `test_render_requires_tool_version_and_defines_result_types`, `test_results_are_typed_values`, `test_result_line_fields_in_order_and_error_passthrough` (`ResultLine._fields` = the 14), `test_run_batch_lines_and_answers_for` (3 jobs: run_id, session_ids in index order; answers_for returns scripted answers), `test_open_run_passes_tool_and_no_logger` (open_args has tool, tool_version, agent_role; no general_log unless seams), `test_open_run_repeat_disables_cache`, `test_open_run_without_wheel_is_err`, `test_verdict_callback_fills_only_three_fields`, `test_fixture_coverage_before_any_line`, `test_fixture_with_run_stamps_run_id_and_sends_nothing`, `test_dry_run_sends_and_writes_nothing`, `test_abort_on_auth_cancels_remaining_jobs`, `test_redact_secrets_table` (bearer rows, provider-key row, 10,000-deep list), `test_category_codes_table`, `test_rendered_module_imports_without_wheel` (`sys.modules["sc_agent"] = None`).
- crates/sc-agent-py/tests/loopback.py (real wheel, loopback stub, `<CLIENT>_LOG_ROOT` set): `test_consent_only_for_typesafe` (haiku: settings module never imported; jev without consent: `POLICY.REMOTE_JUDGE_DISABLED`, 0 stub requests), `test_haiku_runs_without_typesafe_key`, `test_keyboard_interrupt_cancels_the_run` (interrupt from the stub's seam callback; propagates), `test_default_log_path_is_the_wheels` (records in `<CLIENT>_LOG_ROOT/logs/<client>.log.jsonl`; no general_log passed), `test_sitr_extract_logs_under_the_tool` (updated), `test_rendered_modules_only_reference_the_permitted_api` (updated: `Run, session_rows, provider_keys, TYPESAFE, sitr`), `test_fakerun_conforms_to_real_run` (envelope key sets and attributes match), `test_rendered_clients_run_against_the_installed_wheel` (updated).
- No sleeps, threads or timing in any test (CLAUDE.md flaky-test rule).

## Budget

Gate count (non-test Rust, scripts/gate/rewrite.py): this branch measures 2,503/2,500 today (j-3 partial). j-4 restructure: 0 Rust. New capability: sitr `tool=` about +6 Rust (rule-7 item 1); without it, 0. Per-call `cache` pass-through and `log_event` are Python (0 Rust here; `PyRun.log` +8 and the per-call `cache` +4 Rust are counted in j-3). Python (not in the gate): consumer.py.j2 roughly 668 -> 700 (about -150 deleted, +~110 new incl. run_batch/ResultLine/redaction, +~70 moved Result block); testing.py about +45 new. Restructure net zero is a Rust rule; the template's Python delta is reported, not capped.

## Plan decisions (rule 0, no question to Rand)

1. sitr `tool=`: tools pass their name and the library picks the default logger (rule 0, ANC-015, ANC-016), so the 4 sitr pyfunctions gain `tool=` (+~6 Rust, new capability) and the template passes no Logger. Conflicting record text to delete on develop: ADR-SAG-0019 d1 l.57 "The generated client gets one Logger from the wheel, named after the client and writing `~/.sc-agent/logs/<client>.log.jsonl`, and gives it to its Run and every sitr call."
2. Repeats: j-3's `Run.send`/`send_batch` take a per-call `cache` (one cache mechanism), so `call_result`/`call_batch_results`/`run_batch` take `cache: bool | None = None` and pass it through; `run_batch` passes `False` when `settings.repeat > 1` and the caller's `cache` otherwise (plan-anchor REQ-085). `open_run` has no `repeat`; its `cache` is the Run's setting.
3. /2 is generic only (owner ruling, j1-bead.txt:364-368): no open `result_fields` dict; a typed `verdict` callback returns `(verdict, min_confidence, leaning)` and fills exactly those 3 of the 14 fields. Jev does not return them; each tool's `decide()` computes them (result interpretation is the consumer's, ADR-SAG-0019 d1).
