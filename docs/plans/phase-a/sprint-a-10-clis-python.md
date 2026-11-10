<!-- Generated from bead agt-4p2.10 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-10: decide CLI and sc-agent Python API
- id: `a-10` (bead `agt-4p2.10`)
- branch: `sprint/a-10-clis-python`
- depends on: a-6, a-7, a-9
- copy script: `docs/plans/phase-a/copy/a-10.sh`

## Description

The decide binary and library (send one request or a JSONL batch with --model, --project, --agent-role, an optional --provider override, --image and --long-context; models; doctor) and crates/sc-agent-py (one Python function per decide operation, ANC-006, with injectable logger callables). Argument parsing, backend choice, wiring, input limits, signals and output only. Records: REQ-SAG-0501, REQ-SAG-0503, REQ-SAG-0504 (docs/decide), REQ-SAG-0001, REQ-SAG-0003, REQ-SAG-0004, REQ-SAG-0007, REQ-SAG-0008, REQ-SAG-0011..0014, REQ-SAG-0101..0104, REQ-SAG-0401, REQ-SAG-0601, REQ-SAG-0602, REQ-SAG-0701, REQ-SAG-0801, ADR-SAG-0001, ADR-SAG-0002, ADR-SAG-0004.

## Design

```yaml
sprint:
  id: a-10
  closure_type: integration
  target_boundary: decide
  vertical_rationale: crates/sc-agent-py is a PyO3 shell over decide's library target with one function per decide operation (ANC-006); its parity test lists both, so the two close together.
  owned_paths: [crates/decide/**, crates/sc-agent-py/**]
  must_follow:
    - sprint: a-6
      artifact: sc_decision_systemone::{SystemOneBackend, TYPESAFE_BASE_URL, models_request} and sc_decision_local::{LocalBackend (incl. with_timeouts, probe_request, read_probe), LocalProbe (new, with_log, discover, wait_until_loaded), LocalTimeouts, DaemonKind}
      trigger: dev-push
    - sprint: a-7
      artifact: sc_llm_provider::{LlmClient (incl. with_log, models_request, decode_models), LlmProvider, catalog_model_id} and sc_decision_llm::LlmDecisionBackend (incl. with_log)
      trigger: dev-push
    - sprint: a-9
      artifact: sc_decision_runner::{DecisionRunner (incl. with_clock), RunnerConfig, RunnerError}
      trigger: dev-push
  requirements: [NFR-SAG-0001, REQ-SAG-0201, REQ-SAG-0001, REQ-SAG-0002, REQ-SAG-0003, REQ-SAG-0004, REQ-SAG-0005, REQ-SAG-0007, REQ-SAG-0008, REQ-SAG-0009, REQ-SAG-0010, REQ-SAG-0011, REQ-SAG-0012, REQ-SAG-0013, REQ-SAG-0014, REQ-SAG-0015, REQ-SAG-0101, REQ-SAG-0102, REQ-SAG-0103, REQ-SAG-0104, REQ-SAG-0401, REQ-SAG-0501, REQ-SAG-0503, REQ-SAG-0504, REQ-SAG-0601, REQ-SAG-0602, REQ-SAG-0701, REQ-SAG-0801, REQ-SAG-0901]
  adrs: [ADR-SAG-0001, ADR-SAG-0002, ADR-SAG-0004, ADR-SAG-0005]  # 0005: pattern reference for the sc-agent-py packaging (separate PyO3 crate)
  does_not_close:
    - "Discovery is a library capability of sc-decision-local; no decide or Python operation calls it in this phase (REQ-SAG-0801 requires only the library report)."
    - "tsq and REQ-SAG-0303: Sprint A-3."
```

# Sprint A-10: decide CLI and library, sc-agent Python API (bead agt-4p2.10)

Copy script: `docs/plans/phase-a/copy/a-10.sh` (dry run 2026-11-09 after the tsq rows moved to a-3.sh: 9 copy steps, 4 files, 210 lines before trimming).
Source: sc-agent-ref @ dce239a (`crates/sc-ai`, `crates/sc-jev-host/src/stream`, and the `Parsed`/`parse()` pattern from `crates/sc-treesitter/src/cli/mod.rs`).
Targets: `crates/decide/` (new lib + bin crate) and `crates/sc-agent-py/` (new PyO3 crate, the sc-agent Python API).
Governing records: REQ-SAG-0501 and REQ-SAG-0503 (under `docs/decide/requirements/`, naming the CLI `decide`), REQ-SAG-0504 (a bad batch line stops decide before any send), REQ-SAG-0003 (each CLI writes its own general log; every log under `<log_root>/logs/`), REQ-SAG-0005 (session logs under `<log_root>/logs/<repo>/<agent_role>/<project>/`), REQ-SAG-0008 (optional logger injection with defaults, in Rust and in Python), REQ-SAG-0011 (interrupt cancels cleanly), REQ-SAG-0012 (provider-wide busy pause, end to end), REQ-SAG-0013 (config bounds, end to end), REQ-SAG-0014 (input size limits), REQ-SAG-0015 (local runners: own request timeout, default 300 s, and load timeout, default 600 s, from the runner's config entry; load time never counts toward the request timeout or the 90 s budget), REQ-SAG-0101/0102 (short names through the config-built catalog), REQ-SAG-0601 (busy resend, end to end), REQ-SAG-0602 (quota passed through), REQ-SAG-0004 (CLI and PyO3 accept project and agent_role, one field for role and name; slugs; reject, do not rewrite), REQ-SAG-0103 (provider and model separate; provider chosen by ranking, never required), REQ-SAG-0104 AC2 (cancel from Python), REQ-SAG-0401 (keys: env, repo-root .env, keyring), REQ-SAG-0801 (a local runner selected by its configured name), REQ-SAG-0901, REQ-SAG-0701 (batch through the runner), REQ-SAG-0001 AC2 (no UUID anywhere in the finished workspace), REQ-SAG-0002, REQ-SAG-0007, REQ-SAG-0009 / owner-lowlevel-no-tokio, ADR-SAG-0001 (one sync `Decision` trait; `DecisionRequest` — the Jev body plus image parts and the long-context flag — is the one request form), ADR-SAG-0002 (log layout: session rows under `<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl`), ADR-SAG-0004 (decide is the top layer and the composition root; sc-agent-py depends on decide). Anchors: ANC-006 (every decide operation is also in the sc-agent Python API), ANC-007.
Rule for the binary: argument parsing and output only. All behaviour lives in the libraries (A-1, A-2, A-4..A-9); decide's own logic (choosing the backend, wiring `DecisionRunner`, models, doctor, input limits, the signal hook) is in decide's library target, which the `decide` binary and sc-agent-py share.

Decisions (assistant, 2026-11-09; round 2 owner decisions folded in):
- Names: binary and package `decide` (crates.io availability checked at publish time); subcommands `send`, `models`, `doctor`.
- `--project` and `--agent-role` are required, no default; there is no `--agent-name`.
- Provider: `--provider` is an optional override. Absent: the first provider in `Provider::ranking(line of the model)` (A-1) whose key `resolve` detects (A-4); never required (REQ-SAG-0103).
- A batch is read and validated whole before anything is sent; the first bad line stops decide (REQ-SAG-0504).
- Exit: 0 when the command ran and every result succeeded; 1 otherwise; 130 after any interrupt (REQ-SAG-0011; `ctrlc` does not distinguish SIGINT from SIGTERM, so there is no 143).
- `send` without `--batch` always calls `Decision::decide` (one request); `send --batch` always calls `Decision::decide_stream`, even for a one-line batch (CRIT-M3).
- Images and long context (REQ-QA-001, ADR-SAG-0001): `decide send --image NAME=FILE` (repeatable; format from the extension `.png`/`.jpg`/`.jpeg`) and `--long-context`. A batch line is either a bare Jev request or `{"request": {...}, "images": {"NAME": "path"}, "long_context": true}` (paths relative to the working directory). Every request is built with `DecisionRequest::new(body, image_parts, long_context)` (A-2), which checks the parts and the REQ-SAG-0014 image limits.
- Input limits (REQ-SAG-0014): one request (file, stdin or one batch line) at most 8 MiB; a batch at most 10,000 lines and 256 MiB; each image at most 20 MiB (checked from file metadata before reading) and at most 20 images per request (A-2's `DecisionRequest::new`). Over a limit: `DecideError::LimitExceeded { what: LimitKind, line, max }` (or `RequestInvalid` wrapping A-2's `DecisionError::Limits`); nothing is sent.
- Typed options (RBP-F010): `SendOptions` takes `ModelSelector` (its runner arm is a `RunnerName`), `Option<Provider>` and `Slug`s, and `ModelsOptions.runner` is `Option<RunnerName>`; the CLI and Python parse strings at the edge with `ModelSelector::parse` / `Provider::from_str` / `Slug::new` / `RunnerName::try_from`.
- Seams (REQ-SAG-0008, REQ-QA-009): every options struct holds a `Context` with `service`, `config_dir`, `repo_root`, `environment`, `keyring`, `transport`, `general` and `clock` (all but `service` optional; `None` = the real default), and `SendOptions` adds `session`. `Context` has a hand-written `Debug` (counts and flags, never values). Library tests never touch the process environment or HOME: they pass a tempdir `config_dir`, a tempdir `repo_root` (so no real `.env` is read), an explicit `environment` Vec, a `MapKeyring`-style keyring, a `ManualClock` (`ManualClock::clock()`, `sc_agent_types::testing`) as `clock` where a holdoff, provider pause or load poll is under test, a `ScriptedTransport` (`sc_agent_http::testing`) or the bounded-wait `Gate` transport of `crates/decide/tests/common/mod.rs`, and a `MemoryLog` (`let log = MemoryLog::new()?;` then `general: Some(log.general())`, `session: Some(log.session(identity))`; assertions read `log.records()?`, `MemoryRecord { level, target, action, fields }` in write order), and go through the real `DecisionRunner`. Process tests start the `decide` binary with `--config-dir <tempdir>/.sc-agent` (whose default.json sets `log_root` to that directory) and an environment cleared except PATH; no test sets HOME or USERPROFILE, so they run on every OS. The real home default is covered by sc-agent-config's `default_dir` unit test. Fake `Decision` impls are local to crates/decide's tests.
- Transport seams (A-5): `Transport::ready()` checks that a transport can send at all (`UreqTransport::ready` validates its timeouts); `HttpClient::send_once` calls it before it logs `http.send` or counts an attempt, so a transport that cannot send is refused first. `sc_agent_http::testing::BlockingTransport::wait_started(sends) -> bool` gives up after `FAILSAFE` (60 s) and releases every blocked send; it is used by the unit test in `src/signal.rs`. decide's integration tests use their own `Gate` (`tests/common/mod.rs`), whose `wait_started(sends)` panics past `HANG_GUARD` (60 s) after releasing the sends, so a broken ordering fails the test instead of hanging it.
- Signals (REQ-SAG-0011, RSH-001, RSH-004): `decide::signal::install(&cancel)` registers a handler with the `ctrlc` crate, feature `termination` (SIGINT and SIGTERM on Unix, Ctrl-C and Ctrl-Break on Windows; sync, no tokio, no `unsafe`); ctrlc starts and owns the handler thread. The handler counts every interrupt and always cancels the token; it exits the process while no logger is open, and on the third interrupt after the logs open (`LAST_RESORT_INTERRUPT = 3`). `SignalWatch::logs_opened()` is called just after `send`, `send --batch`, `models` or `doctor` has opened its general log (`context_with_general`; for `send`, after the input is read and checked). Before it, decide is parsing, loading its configuration (a `default.json` or `local.json` that is a FIFO or device is refused as `config.not_a_file` before any read), reading stdin or a FIFO or opening its general log (RSH-001, RSH-002) and nothing is logged, so an interrupt calls `std::process::exit(EXIT_INTERRUPTED)` (130) at once. After it, the first and second interrupt only cancel (cancel is idempotent): unfinished requests end `decision.cancelled` and are printed, the stream is drained, the session and general logs are flushed and closed, and main exits 130. The third interrupt exits 130 at once, whatever is still being written: the operator's escape from a drain stuck on a stalled stdout reader or log disk (RSH-003). Every other exit after a logger is open therefore goes through drain, flush and close (REQ-SAG-0011, NFR-SAG-0001); this covers `models` and `doctor` too, whose waits are bounded by the probe timeouts.
- Python interrupts (REQ-SAG-0011, RSH-005): sc-agent-py runs the decide call on a scoped worker thread and waits on its result channel with `recv_timeout(100 ms)` (a timed wait, not sleep) with the GIL released (`py.detach`); after each timeout it checks `py.check_signals()` with the GIL. `KeyboardInterrupt` (or any signal exception) cancels the token, drains the remaining results (they are still passed to `on_result`), joins the worker, then re-raises. An exception raised by `on_result` also cancels, drains (the remaining results are not passed on) and re-raises.
- Python loggers (REQ-SAG-0008, owner 2026-11-09): `general_log` and `session_log` keyword arguments take `Callable[[dict], None]`; each becomes an sc-observability `LogSink` (`PyCallableSink`) that calls the callable with the GIL, wrapped as `GeneralLogger::injected` / `SessionLogger::injected`. The callable receives `{"level", "target", "action", "fields": {..}}`. A callable that raises makes that write a refused write (counted by sc-agent-log, reported by `flush`/`close` as lost rows); it never changes a decision result.
- Logger ownership (REQ-SAG-0008): `general_logger(&Context)` and `session_logger(&SendOptions)` open the default loggers; the caller (the `decide` binary, sc-agent-py) opens them, puts them in `context.general` and `options.session`, and closes them after the operation, so every record and row is written before it returns and a lost row is reported, not dropped. The binary closes explicitly (`close_general`, `session.close()`; `report_close` prints one `decide.log` line on stderr for a close failure and does not change the exit code). sc-agent-py closes both on the worker thread (`close_loggers`) and raises a close or flush failure after the result as a `RuntimeWarning` whose text is the JSON error chain; it never changes the result. When a caller injects none, `Prepared::finish` writes the `decide.send` record and then flushes the general logger decide opened itself (the runner's detached workers may hold its last handle, so a drop would never flush); `models` and `doctor` do the same through `probe_setup` and `flushed_result`.
- Configuration directory and log root (REQ-SAG-0003 0.2.0, owner decision 2026-11-10): `Context.config_dir` is the one seam for the configuration directory; `None` is `sc_agent_config::default_dir()` (`<home>/.sc-agent`). No environment variable names a configuration directory or a log root; the Python functions take a `config_dir` option (a path) and the decide CLI takes the global flag `--config-dir <PATH>` (before or after `send`, `models` or `doctor`; an empty path is a usage error), both mapping to `Context.config_dir`. `<log_root>` is the configuration's `log_root`, else `<home>/.sc-agent` on every platform (`AgentConfig::log_root()`), whether or not `config_dir` is set. Every log goes under `<log_root>/logs/`: the general log is `<log_root>/logs/<cli>.log.jsonl` and the session logs are `<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl` (ADR-SAG-0002). Tests, Rust and Python, choose the configuration with the `config_dir` option; process tests pass `--config-dir`.
- ANC-006: this sprint delivers `crates/sc-agent-py` with one Python function per decide operation (section 3.3).

## 1. Copy list (a-10.sh)

decide (`crates/decide/`):

| Source | Target | What |
|---|---|---|
| `crates/sc-ai/LICENSE` | `LICENSE` | MIT |
| `crates/sc-treesitter/src/cli/mod.rs:111-144` | `src/cli.rs` | `Parsed` + `parse()` (help/version printed, other clap errors returned) |
| `crates/sc-ai/src/cli.rs:560-572` | `src/cli.rs` | `non_empty_text`, `non_empty_path` value parsers |
| `crates/sc-ai/src/cli.rs:610-618` | `src/cli.rs` | test `cli_definition_is_consistent` |
| `crates/sc-ai/src/command.rs:412-429` | `src/input.rs` | `request_bytes`: path or `-` for stdin |
| `crates/sc-jev-host/src/stream/mod.rs:797-845` | `src/input.rs` | `LineRead`, `read_line_bounded` (bounded JSONL line reader) |
| `crates/sc-jev-host/src/stream/tests.rs:7-51` | `src/input.rs` (`#[cfg(test)] mod tests`) | 3 line-reader tests |
| `crates/sc-ai/src/main.rs:77-82,90-104` | `src/main.rs` | `print_stdout`, `emit` (stderr line) |

Not copied: sc-ai `command.rs` dispatch (sc_jev_host compose, envelopes, readbacks, check.toml, `ask`, sessions dirs, registry test seams), sc-ai `cli.rs` jev groups and flags, `tests/argv.rs`, `tests/cli.rs` (2117 lines of envelope/scripted-backend tests), sc-jev-host `DecisionStream`/`serve_jsonl`/pool (queue and workers are A-9 behind `Decision::decide_stream`), every tsq row (moved to A-3), mimalloc, `ScAgentErrorResponse`, exit 2/3 retryable rule.

## 2. Required edits after copying

decide:
- `cli.rs`: rewrite `Cli`/`Command` (section 3.2); `parse()` maps clap errors to `DecideError::Usage`; value parsers return `Result<_, String>` (`parse_runner_name` builds a `RunnerName` at the edge); message text "See decide --help.".
- `input.rs` (library): `request_bytes` loses `RepoRoot`/`joined_path` (paths are relative to the working directory), reads at most `MAX_REQUEST_BYTES + 1` bytes (`Read::take`) and returns `LimitExceeded` past the cap; errors -> `InputNotFound`/`InputRead`; `read_line_bounded`: replace `&available[..take]` with `available.get(..take)` (indexing_slicing), generic over `R: BufRead + ?Sized`; `read_batch` passes `MAX_REQUEST_BYTES + 2` as its `max`, so the line terminator (`\n` or `\r\n`) is not counted against the 8 MiB cap (QA-005); wrap the copied tests in `#[cfg(test)] mod tests { use super::*; use std::io::Cursor; ... }`.
- `main.rs`: `print_stdout`/`emit` take `DecideError`; `emit` writes `DecideError::detail()` as one JSON line and must not `.expect`: when encoding fails it writes a fixed literal `{"id":"decide.encode","message":"..."}` line.

## 3. Rust code (exact; bodies elided)

### 3.2 decide (`crates/decide/`)

```rust
// src/cli.rs
#[derive(Debug, clap::Parser)]
#[command(name = "decide", bin_name = "decide", version, about = "Send TypeSafe (Jev) decision requests.")]
pub(crate) struct Cli {
    /// Configuration directory holding default.json (and local.json); default ~/.sc-agent. Logs go to the configured log_root, else ~/.sc-agent.
    #[arg(long, global = true, value_name = "PATH", value_parser = non_empty_path)]
    pub(crate) config_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Command {
    /// Send one request (JSON) or a batch (JSONL, --batch) through the Decision trait.
    Send(SendArgs),
    /// Print the TypeSafe models list (GET /v1/models) of TypeSafe or of a configured local runner.
    Models(ModelsArgs),
    /// List detected keys in use order; warn about provider models missing from the catalog and config models tying a built-in.
    Doctor,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SendArgs {
    /// Optional override (typesafe, anthropic, openai, openrouter, nous, together). Absent: chosen by ranking among detected keys (REQ-SAG-0103).
    #[arg(long, value_name = "PROVIDER", value_parser = non_empty_text)]
    pub(crate) provider: Option<String>,
    /// Short name (jev, haiku, luna, ...), a model id, or a configured local runner name.
    #[arg(long, value_name = "NAME", allow_hyphen_values = true, value_parser = non_empty_text)]
    pub(crate) model: String,
    /// Project slug for the session rows.
    #[arg(long, value_name = "SLUG", allow_hyphen_values = true, value_parser = non_empty_text)]
    pub(crate) project: String,
    /// Agent-role slug for the session rows (e.g. plan-anchor, judge-jev, sanity).
    #[arg(long, value_name = "SLUG", allow_hyphen_values = true, value_parser = non_empty_text)]
    pub(crate) agent_role: String,
    /// Input is JSONL: one request per line (a bare Jev request or {"request", "images", "long_context"}).
    #[arg(long)]
    pub(crate) batch: bool,
    /// Image part NAME read from FILE (png, jpg, jpeg); repeatable; single-request mode only.
    #[arg(long = "image", value_name = "NAME=FILE", value_parser = parse_image_arg, conflicts_with = "batch")]
    pub(crate) images: Vec<(String, std::path::PathBuf)>,
    /// Send TypeSafe-Danger-Zone-Long-Context: true; single-request mode only.
    #[arg(long, conflicts_with = "batch")]
    pub(crate) long_context: bool,
    /// Request file, or - for stdin.
    #[arg(value_name = "FILE|-", default_value = "-", value_parser = non_empty_text)]
    pub(crate) input: String,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ModelsArgs {
    /// A configured local runner; TypeSafe when absent.
    #[arg(long, value_name = "NAME", value_parser = parse_runner_name)]
    pub(crate) runner: Option<RunnerName>,
}

#[derive(Debug)]
pub(crate) enum Parsed { Run(Cli), Print(String) }
pub(crate) fn parse<I, T>(args: I) -> Result<Parsed, DecideError>
where I: IntoIterator<Item = T>, T: Into<std::ffi::OsString> + Clone;
fn parse_runner_name(value: &str) -> Result<RunnerName, String>;   // RunnerName::try_from; "<reason>: <value>. See decide --help."
fn non_empty_text(value: &str) -> Result<String, String>;
fn parse_image_arg(value: &str) -> Result<(String, std::path::PathBuf), String>;   // split at the first '='; both sides non-empty

// src/input.rs (library; REQ-SAG-0014 limits)
pub const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;        // one request, one batch line
pub const MAX_BATCH_LINES: u64 = 10_000;
pub const MAX_BATCH_BYTES: u64 = 256 * 1024 * 1024;          // lines plus image bytes
pub const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;           // also enforced by DecisionRequest::new (A-2)
/// Which REQ-SAG-0014 limit a DecideError::LimitExceeded names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LimitKind { RequestBytes, BatchLines, BatchBytes, ImageBytes }
impl LimitKind { pub const fn as_str(self) -> &'static str; }   // "request_bytes" | "batch_lines" | "batch_bytes" | "image_bytes"
impl std::fmt::Display for LimitKind {}                          // as_str
pub fn request_bytes(spec: &str, stdin: &mut dyn std::io::Read) -> Result<Vec<u8>, DecideError>;
pub fn open_lines(spec: &str, stdin: Box<dyn std::io::BufRead + Send>) -> Result<Box<dyn std::io::BufRead + Send>, DecideError>;
/// Reads one image file: metadata length > MAX_IMAGE_BYTES -> LimitExceeded before reading, and again after a take of
/// MAX_IMAGE_BYTES + 1 bytes if the file grew meanwhile (QA-008); extension -> ImageFormat (else ImageFormat error);
/// io errors -> ImageRead.
pub fn read_image(name: &str, path: &std::path::Path) -> Result<sc_decision_schema::ImagePart, DecideError>;
/// Builds a DecisionRequest from JSON bytes, image files and the flag: SystemOneRequest::from_json, read_image for each,
/// DecisionRequest::new(body, parts, long_context) (A-2 checks parts and image count/size); errors -> RequestInvalid { line, .. }.
pub fn build_request(json: &[u8], images: &[(String, std::path::PathBuf)], long_context: bool, line: Option<u64>) -> Result<sc_decision_schema::DecisionRequest, DecideError>;
/// The in-memory form (sc-agent-py): json.len() > MAX_REQUEST_BYTES -> LimitExceeded { what: LimitKind::RequestBytes }; each image
/// > MAX_IMAGE_BYTES -> LimitExceeded { what: LimitKind::ImageBytes }; format from magic bytes (PNG 89 50 4E 47, JPEG FF D8 FF; else
/// ImageFormat with an empty path); then DecisionRequest::new as above.
pub fn build_request_bytes(json: &[u8], images: Vec<(String, Vec<u8>)>, long_context: bool, line: Option<u64>) -> Result<sc_decision_schema::DecisionRequest, DecideError>;
/// One in-memory batch item (sc-agent-py): JSON text, image bytes by name, long-context flag.
#[derive(Debug, Clone, Default)]
pub struct BatchInput { pub json: String, pub images: Vec<(String, Vec<u8>)>, pub long_context: bool }
/// The batch caps over item sizes (JSON plus image bytes) in input order; item n is line n.
/// LimitExceeded { what: LimitKind::BatchLines | LimitKind::BatchBytes, line: Some(n), .. }; the same counters read_batch applies to file lines.
fn check_batch_sizes(sizes: impl Iterator<Item = u64>) -> Result<(), DecideError>;
/// The in-memory batch path: check_batch_sizes over every item before any item is built; each item then goes through
/// build_request_bytes with line = its 1-based index.
pub fn build_batch(items: Vec<BatchInput>) -> Result<Vec<sc_decision_schema::DecisionRequest>, DecideError>;
enum LineRead { End, Line, TooLong }
fn read_line_bounded<R: std::io::BufRead + ?Sized>(reader: &mut R, max: usize, out: &mut Vec<u8>) -> std::io::Result<LineRead>;
/// One batch line: a bare Jev request, or this wrapper (chosen by the presence of the "request" key).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchLine { request: Box<serde_json::value::RawValue>, #[serde(default)] images: Option<OrderedMap<String, std::path::PathBuf>>, #[serde(default)] long_context: bool }
/// Reads and validates the whole JSONL batch before anything is sent (REQ-SAG-0504, REQ-SAG-0014). Blank lines and a
/// leading UTF-8 BOM are skipped, but every physical line and every byte read counts against MAX_BATCH_LINES and
/// MAX_BATCH_BYTES, the BOM and blank lines included (RSH-002); line numbers count every physical line from 1. The line
/// terminator is not counted against the 8 MiB request cap (QA-005). A wrapper line's image bytes actually read count toward
/// the batch budget, not the metadata length (RSH-003). The first line that is too long, past a batch cap, not UTF-8, or
/// not a valid request returns LimitExceeded / LineNotUtf8 / RequestInvalid naming that line, the field and the reason;
/// nothing is sent.
pub fn read_batch(reader: &mut dyn std::io::BufRead) -> Result<Vec<sc_decision_schema::DecisionRequest>, DecideError>;

// src/lib.rs: decide's library target, shared by the decide binary and sc-agent-py
pub mod input; pub mod output; pub mod signal; mod doctor; mod error; mod select;
pub use doctor::{DetectedKey, DoctorOptions, DoctorReport, MissingModel, Tie, doctor};
pub use error::DecideError;
pub use select::{Context, ModelSelector, ModelsOptions, SendOptions, general_logger, load_config, models, send, send_batch, send_batch_on, send_on, session_identity, session_logger};

// src/select.rs  (the one place a backend is chosen; ADR-SAG-0004 composition root)
/// Shared seams; every field None = the real default (REQ-SAG-0008). Debug is hand-written (counts and flags only).
#[derive(Clone)]
pub struct Context {
    pub service: &'static str,                                                  // "decide" | "sc-agent-py"
    pub config_dir: Option<PathBuf>,                                            // None -> sc_agent_config::default_dir() (~/.sc-agent)
    pub repo_root: Option<PathBuf>,                                             // None -> Repo::work_tree_root(cwd); the .env of this work tree is read for keys (REQ-SAG-0401)
    pub environment: Option<Vec<(OsString, OsString)>>,                         // None -> std::env::vars_os(); SC_AGENT__ overrides and key lookup
    pub keyring: Option<Arc<dyn KeyringSource + Send + Sync>>,                  // None -> the platform keyring
    pub transport: Option<Arc<dyn Transport>>,                                  // None -> a UreqTransport with the operation's timeouts (decision_transport, probe_transport)
    pub general: Option<GeneralLogger>,                                         // None -> GeneralLogger::open_default(log_root, service)
    pub clock: Option<sc_agent_types::Clock>,                                   // None -> Clock::system(); passed to DecisionRunner::with_clock (admissions, holdoffs, provider pauses, sends), to LocalProbe (the load wait) and to the probe holdoffs; a test seam (sc_agent_types::testing::ManualClock::clock())
}
impl Context { pub fn new(service: &'static str) -> Self; }   // every seam None
/// What --model names; parsed once at the edge against the loaded config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSelector { Short(ShortName), Version(ModelId), Runner(RunnerName) }   // Runner: a key of config.local_runners
impl ModelSelector {
    /// A ShortName if it parses; else a configured local runner name; else a ModelId; else DecideError::Types (types.invalid_model_id).
    /// (Catalog::resolve is infallible for both ShortName and ModelId, A-1; no error arm.)
    pub fn parse(text: &str, config: &AgentConfig) -> Result<Self, DecideError>;
}
impl std::fmt::Display for ModelSelector {}   // the short name, the model id or the runner name
#[derive(Clone)]   // Debug is hand-written
pub struct SendOptions {
    pub model: ModelSelector,
    pub provider: Option<Provider>,
    pub project: sc_agent_log::Slug,
    pub agent_role: sc_agent_log::Slug,
    pub context: Context,
    pub session: Option<sc_agent_log::SessionLogger>,           // None -> SessionLogger::open_default(log_root, identity)
}
#[derive(Clone, Debug)]
pub struct ModelsOptions { pub runner: Option<RunnerName>, pub context: Context }   // runner: a configured local runner; TypeSafe when None
/// sc_agent_config::load(context.config_dir or default_dir()?, context.environment or vars_os()); errors -> Config.
pub fn load_config(context: &Context) -> Result<AgentConfig, DecideError>;
/// The general logger of an operation: context.general, else GeneralLogger::open_default(<log_root>, context.service) (the file is <log_root>/logs/<service>.log.jsonl);
/// <log_root> is AgentConfig::log_root() (REQ-SAG-0003). The caller opens it, puts it in context.general and closes it after the operation.
pub fn general_logger(context: &Context) -> Result<GeneralLogger, DecideError>;
/// The session logger of options: options.session, else SessionLogger::open_default(<log_root>, session_identity(..)?)
/// (<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl, ADR-SAG-0002). As general_logger: open it, put it in options.session, close it after the send.
pub fn session_logger(options: &SendOptions) -> Result<SessionLogger, DecideError>;
/// SessionIdentity::new(Repo::detect(cwd)?, project, agent_role); used by session_logger and by sc-agent-py for SessionLogger::injected.
pub fn session_identity(project: &sc_agent_log::Slug, agent_role: &sc_agent_log::Slug) -> Result<SessionIdentity, DecideError>;
/// A runner selector -> Chosen::Local { name, runner, backend: Arc<LocalBackend> }: LocalBackend::new(DaemonKind::from(runner.kind), host, port, model, key via resolve_var(key_env))
/// .with_timeouts(LocalTimeouts { load: runner.load_timeout(), request: runner.request_timeout() });
/// else model = catalog.resolve(ModelChoice) (catalog = config.catalog(), so config models are seen, REQ-SAG-0101/0102);
/// provider = options.provider, else the first of Provider::ranking(line) whose key resolve() finds with
/// KeySources { env: environment, repo_root: context.repo_root or Repo::work_tree_root(cwd)?, keyring }; none -> NoProviderKey.
/// TypeSafe -> SystemOneBackend::new(TYPESAFE_BASE_URL, key); any other provider -> LlmDecisionBackend(LlmClient::new(LlmProvider, key).with_log(general), model).with_log(general)
/// (Chosen::Remote { provider, model, backend: Arc<dyn Backend> }).
fn backend(options: &SendOptions, config: &AgentConfig, general: &GeneralLogger) -> Result<Chosen, DecideError>;
/// The decision transport: context.transport, else one UreqTransport per local runner with the runner's request timeout
/// (Timeouts::default().with_request(runner.request_timeout()), REQ-SAG-0015, default 300 s), else Timeouts::default().
fn decision_transport(context: &Context, runner: Option<&LocalRunner>) -> TransportChoice;
/// The probe transport (models, doctor, load probes): context.transport, else UreqTransport::new(Timeouts::probe()).
fn probe_transport(context: &Context) -> Arc<dyn Transport>;
/// RunnerConfig::from_config(config)? (runner.invalid_config -> DecideError::Runner), HttpClient::new(transport, ResendPolicy::new(config.max_sends), general.clone())
/// (one HttpClient per runner, so the REQ-SAG-0012 provider pause is shared by every worker);
/// DecisionRunner::with_clock(backend, http, session, general, runner_config, clock): `clock` is context.clock(), the injected Clock or Clock::system().
fn runner(clock: Clock, config: &AgentConfig, transport: Arc<dyn Transport>, session: SessionLogger, general: GeneralLogger, backend: Arc<dyn Backend>) -> Result<DecisionRunner, DecideError>;
/// Prepared::new(options) builds everything before anything is sent (load_config, general logger, backend, session logger, transport, runner);
/// Prepared::wait_loaded(cancel) is the REQ-SAG-0015 load wait (LogContext::NONE); Prepared::finish(&Tally, outcome) writes decide.send, then flushes a general logger decide opened.
/// Always `Decision::decide` (plain send); on_result gets the DecisionResult first, then flush_session_log runs and one `decide.send` general record is written.
/// A flush failure after the result was handed out is returned after it, never instead of it (QA-006): on_result's Err, else DecideError::Runner (session log), else DecideError::Log (general log lost rows).
pub fn send(options: &SendOptions, request: DecisionRequest, cancel: &CancelToken, on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>) -> Result<(), DecideError>;
/// Always `Decision::decide_stream(Box::new(requests.into_iter()), cancel)` (an owned 'static iterator), even for one
/// request; `on_result` gets each DecisionResult as it finishes; an Err from on_result cancels the token, the rest is
/// drained (and dropped), flush_session_log runs, and that Err is returned.
pub fn send_batch(options: &SendOptions, requests: Vec<DecisionRequest>, cancel: &CancelToken, on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>) -> Result<(), DecideError>;
/// Testable cores: any Decision (the runner in send/send_batch, a fake in tests).
pub fn send_on(decision: &dyn Decision, request: DecisionRequest, cancel: &CancelToken) -> DecisionResult;
pub fn send_batch_on(decision: &dyn Decision, requests: Vec<DecisionRequest>, cancel: &CancelToken, on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>) -> Result<(), DecideError>;
/// GET /v1/models of TypeSafe or a configured runner, sent once through HttpClient::send_once with Timeouts::probe() (A-5);
/// writes one `decide.models` general record; the logger setup and flush are probe_setup and flushed_result.
pub fn models(options: &ModelsOptions, cancel: &CancelToken) -> Result<ModelMetadataList, DecideError>;

// src/signal.rs (REQ-SAG-0011; the `ctrlc` crate with feature `termination`: SIGINT + SIGTERM on Unix,
// Ctrl-C/Ctrl-Break on Windows; sync, no tokio, no unsafe in decide)
/// The exit code after any interrupt; ctrlc does not report which signal fired, so there is no 143.
pub const EXIT_INTERRUPTED: i32 = 130;
/// The interrupt that exits at once even while the drain runs: the third.
pub const LAST_RESORT_INTERRUPT: u32 = 3;
/// interrupts: AtomicU32 (interrupts received); logs_open: AtomicBool (logs_opened was called, so a logger may be open).
struct State { interrupts: AtomicU32, logs_open: AtomicBool }
/// What the handler does with one interrupt.
enum Action { Cancel, Exit }   // Cancel: cancel the token (again), main drains, flushes, closes and exits 130; Exit: process::exit(130) now, before any logger is open or on the last-resort interrupt
#[derive(Debug, Clone)]
pub struct SignalWatch { state: Arc<State> }
/// ctrlc::set_handler(move || if on_interrupt(&cancel, &state) == Action::Exit { std::process::exit(EXIT_INTERRUPTED) });
/// ctrlc starts and owns its handler thread (decide's source spawns none). Err (a handler already set) -> DecideError::Signal { source: ctrlc::Error }.
pub fn install(cancel: &CancelToken) -> Result<SignalWatch, DecideError>;
/// What the handler does: always count the interrupt and cancel the token (idempotent), and return Exit while logs_open is false or once the count reaches LAST_RESORT_INTERRUPT.
/// After logs_opened the first and second interrupt are Cancel, so the exit goes through drain, flush, close, exit 130; the third is Exit (RSH-003). Unit-tested directly.
fn on_interrupt(cancel: &CancelToken, state: &State) -> Action;
impl SignalWatch {
    /// True once any interrupt was received; main then exits 130. ctrlc does not report which signal fired,
    /// so SIGINT, SIGTERM, Ctrl-C and Ctrl-Break all exit 130 (no 143).
    pub fn interrupted(&self) -> bool;
    /// Called just after send, send --batch, models or doctor has opened its general log (send: after the input is read and checked):
    /// from now on an interrupt cancels and lets the operation drain, flush and close its logs (the third one still exits at once).
    pub fn logs_opened(&self);
}

// src/doctor.rs (REQ-SAG-0503)
#[derive(Debug, serde::Serialize)]
pub struct DetectedKey { pub provider: Provider, #[serde(serialize_with = "key_source_str")] pub source: KeySource }   // never the value
#[derive(Debug, serde::Serialize)]
pub struct MissingModel { pub provider: Provider, pub model: String }
#[derive(Debug, serde::Serialize)]
pub struct Tie { pub config: ModelId, pub builtin: ModelId }
#[derive(Debug, serde::Serialize)]
pub struct DoctorReport { pub keys: Vec<DetectedKey>, pub missing: Vec<MissingModel>, pub ties: Vec<Tie> }
#[derive(Clone, Debug)]
pub struct DoctorOptions { pub context: Context }
/// Model lists fetched with Timeouts::probe(); writes one `decide.doctor` general record.
pub fn doctor(options: &DoctorOptions, cancel: &CancelToken) -> Result<DoctorReport, DecideError>;

// src/output.rs (library)
#[derive(Debug, serde::Serialize)]
pub struct ResultLine<'a> {
    pub session_id: &'a SessionId,
    pub position: u64,
    pub digests: &'a OrderedMap<String, QuestionDigest>,
    #[serde(skip_serializing_if = "Option::is_none")] pub response: Option<&'a SystemOneResponse>,
    #[serde(skip_serializing_if = "Option::is_none")] pub error: Option<ErrorDetail>,   // DecisionError::detail(): every lower id, message and recovery
}
impl<'a> ResultLine<'a> { pub fn new(result: &'a DecisionResult) -> Self; }
pub fn result_line(result: &DecisionResult) -> Result<Vec<u8>, DecideError>;      // compact JSON, no trailing newline
pub fn result_text(result: &DecisionResult) -> Result<String, DecideError>;       // the same as a String (sc-agent-py)
pub fn to_json<T: serde::Serialize + ?Sized>(value: &T) -> Result<String, DecideError>;   // a models list or doctor report

// src/main.rs (bin: argv -> decide::{send, send_batch, models, doctor} -> stdout)
fn main() -> std::process::ExitCode;   // install the SignalWatch first; exit 130 when it was interrupted
fn run(parsed: Parsed, cancel: &CancelToken, watch: &SignalWatch, stdin: &mut dyn std::io::Read, stdout: &mut dyn std::io::Write) -> Result<std::process::ExitCode, DecideError>;
/// Slugs, load_config, ModelSelector::parse and the provider first, then the whole input is read and checked, then the general logger opens (context_with_general; an interrupt exits at once until
/// logs_opened); then watch.logs_opened(), the session logger (session_logger), send or send_batch, and the explicit close of both loggers.
fn run_send(args: SendArgs, cancel: &CancelToken, watch: &SignalWatch, stdin: &mut dyn std::io::Read, stdout: &mut dyn std::io::Write) -> Result<std::process::ExitCode, DecideError>;
fn print_stdout(bytes: &[u8]) -> std::process::ExitCode;
fn emit(error: &DecideError) -> std::process::ExitCode;     // report(error), then exit 1
fn report(error: &DecideError);                              // one stderr line: error.detail() as JSON, `recovery` included when present
fn report_close(closed: Result<(), LogError>);               // a logger that lost rows: one decide.log stderr line, exit code unchanged
```

Types used: `SessionId`, `ErrorId` (incl. `cause_detail()`, provided `recovery()`), `ErrorDetail` (incl. `recovery: Option<Cow<'static, str>>`), `VarName`, `Catalog`, `ModelChoice`, `ModelId`, `ShortName`, `Provider` (incl. `PRIORITY`, `ranking`, `FromStr`), `MODELS`, `CancelToken` (A-1); `AgentConfig` (incl. `catalog`, `local_runners`, `log_root()`, `RunnerKind`, `RunnerName`, `LocalRunner`), `load`, `default_dir` (A-4); `KeySources`, `KeySource`, `KeyringSource`, `EnvSource`, `ResolvedKey`, `resolve`, `resolve_var` (A-4); `HttpClient`, `ResendPolicy`, `Transport`, `UreqTransport`, `Timeouts` (incl. `probe`) (A-5); `GeneralLogger`, `SessionLogger` (incl. `injected`), `Slug`, `Repo` (incl. `work_tree_root`), `SessionIdentity` (A-8); `Decision`, `Backend`, `DecisionRequest` (incl. `new`), `ImagePart`, `ImageFormat`, `DecisionResult`, `SystemOneRequest`, `SystemOneResponse`, `OrderedMap`, `QuestionDigest`, `ModelMetadataList` (A-2); `SystemOneBackend`, `TYPESAFE_BASE_URL` (A-6); `LocalBackend` (incl. `with_timeouts`), `LocalProbe` (incl. `with_log`, `wait_until_loaded`), `LocalTimeouts`, `DaemonKind` (A-6); `LlmClient` (incl. `models_request`, `decode_models`), `LlmProvider`, `catalog_model_id`, `LlmDecisionBackend` (A-7); `DecisionRunner` (incl. `with_clock`), `RunnerConfig`, `RunnerError` (A-9).

Behaviour:
- `send` (no `--batch`): `request_bytes` (8 MiB cap) -> `build_request(json, --image list, --long-context, None)` -> `decide::send` (always `Decision::decide`; `on_result` prints the result as soon as it exists) -> one `ResultLine` + `\n`. A failed decision is data in its result (owner-decision-trait), printed on stdout. A session-log flush failure after the result is a command error (exit 1) printed after it, never instead of it (QA-006).
- `send --batch`: `read_batch` reads and validates every line first (limits, schema, images); a bad line returns its error (line, field, reason) before anything is sent (REQ-SAG-0504). Then `decide::send_batch`; each result printed as one JSONL line as it arrives (fastest first, session ULID and input position); a failed request is data in its line and the batch continues. A stdout write failure cancels the token and returns `Write`. After the stream, `send_batch` flushes the session log (`flush_session_log`); an Err is a command error (exit 1) printed after the results, never replacing them. `main` then closes the session and general loggers.
- Local runner load (REQ-SAG-0015): before the first request of a send or batch to a local runner, decide calls `LocalProbe::new(backend, &*probe, clock).with_log(general).wait_until_loaded(&cancel, runner.load_timeout(), LogContext::NONE)` (A-6) once, with `probe` = the context transport or `UreqTransport::new(Timeouts::probe())` and `clock` = `Context.clock` or `Clock::system()`; a cancel ends the wait at once (`Loaded::Cancelled`: `decision.cancelled` with `sends: 0` for every request, nothing sent); a load that does not finish within `load_timeout()` (default 600 s) is the A-6 `DecisionError`, surfaced as `decide.decision` (cause `decision.backend` -> `local.load_timeout`), and nothing is sent. Only then does the batch start, so load time never counts toward a request's own timeout or the 90 s budget (the budget starts at the first send, A-5). The send carries no load deadline (no `HttpRequest::with_deadline`): its timeout is the per-runner `UreqTransport`'s request timeout (the runner's `request_timeout()`, default 300 s), counted from the send after the load. The backend is built with `LocalBackend::with_timeouts(LocalTimeouts { load, request })`.
- Signals (REQ-SAG-0011): see the signals decision above. Before `logs_opened` (called just after the general log opens) an interrupt exits 130 at once; after it, the first two SIGINT/SIGTERM cancel the token, unfinished requests end `decision.cancelled` and are printed, the session and general logs are flushed and closed, and main exits 130; the third exits 130 at once (`LAST_RESORT_INTERRUPT`). `models` and `doctor` call `logs_opened` too, so they close their general log before exiting.
- Session identity: the CLI and sc-agent-py parse `--project`/`--agent-role` with `Slug::new` before anything is built; an invalid value is `decide.log` (cause `log.invalid_slug`), nothing is sent and no row is written (REQ-SAG-0004). `repo` comes from `Repo::detect` on the working folder.
- `models`: `SystemOneBackend::models_request(..)?` for TypeSafe or the runner's base (returns `Result<HttpRequest, DecisionError>`: `with_secret` refuses a key on plain-http non-loopback; `?` maps it to `decide.decision`); sent with `HttpClient::send_once` over the context transport with `Timeouts::probe()` (a one-off holdoff waits on `Clock::wait_until(&cancel, t)` of the context clock, A-1/A-5); `decode_models`; prints the JSON.
- `doctor` (REQ-SAG-0503): (1) one entry per detected provider key in `Provider::PRIORITY` order with its `KeySource` (`env`, `.env`, `keyring`), never the value; (2) for each provider with a key, its model list (`LlmClient::models_request(..)?` (returns `Result<HttpRequest, LlmError>`; `?` maps it to `decide.llm`) / `decode_models`; TypeSafe through A-6 `models_request(..)?`), each id mapped back with `catalog_model_id`, one `MissingModel { provider, model }` per id `catalog.line_of(&str)` does not know; (3) one `Tie { config, builtin }` per config model whose release date equals a built-in model of its line (the config model wins, A-1). Nothing in (2) or (3) when nothing is missing or tied.
- Config: `load_config(context)`; bound violations (REQ-SAG-0013: workers, max_sends, runner host/kind/port/key_env, key on plain-http non-loopback) are A-4's `AgentConfigError`s with the key path, surfaced as `decide.config` before anything is sent.
- General-log records are written with `GeneralLogger::write(level, target, action, LogContext::NONE, fields)` (A-8; decide's own records carry no session; per-request `http.*`/`runner.*` events carry `LogContext { session_id, position }` from A-5/A-9). Records (ANC-007; never a key or key value): `decide.send` (service, model, provider, `runner` for a local runner, requests, succeeded, failed, cancelled, `error` = the error id when the operation failed), `decide.models` (service, `runner` or `typesafe`, models count, outcome id), `decide.doctor` (service, detected key count, missing count, tie count, outcome id). The same `GeneralLogger` goes into `HttpClient::new` (its `http.*` records), `DecisionRunner::with_clock`, `LocalProbe::with_log` (the local load wait), `LlmClient::with_log` and `LlmDecisionBackend::with_log`.
- Exit: 0 when the command ran and every result succeeded; 1 otherwise; 130 after an interrupt (after drain, flush and close once a logger is open). Numbers are printed as serde_json gives them (REQ-SAG-0007). No tokio, no sleep; decide's source starts no thread; the only extra thread is the handler thread `ctrlc` starts and owns.

## 4. Error list

decide (`crates/decide/src/error.rs`, new; ErrorId from A-1, Display = `id: message` only, the chain in `source()`/`detail()`; each wrapped error kept unchanged; `cause_detail` returns the wrapped error's detail, or a `foreign` detail (id `clap`, `io`, `serde_json`, `ctrlc`) for a non-sc-agent error):

```rust
#[derive(Debug)]
#[non_exhaustive]
pub enum DecideError {
    /// The command line does not parse; the clap error is the cause.
    Usage { source: Box<clap::Error> },                                            // "decide.usage"; cause id clap
    /// The global configuration could not be loaded or is out of bounds (REQ-SAG-0013; cause config.invalid with the key path, or config.key_on_plain_http).
    Config(Box<sc_agent_config::AgentConfigError>),                                // "decide.config"
    /// --provider or --model does not parse.
    Types(Box<sc_agent_types::TypesError>),                                        // "decide.types"
    /// The provider's or runner's key is missing or malformed.
    Secrets(Box<sc_agent_secrets::SecretsError>),                                  // "decide.secrets"
    /// A slug is invalid (log.invalid_slug), or repo detection or a logger failed.
    Log(Box<sc_agent_log::LogError>),                                              // "decide.log"
    /// A local runner's backend could not be built.
    Local(Box<sc_decision_local::LocalError>),                                     // "decide.local"
    /// Building a System One backend, decoding the models list, or the local-runner load wait failed
    /// (a model that does not load in time: cause decision.backend -> local.load_timeout).
    Decision(Box<sc_decision_schema::DecisionError>),                              // "decide.decision"
    /// A models or doctor send failed.
    Http(Box<sc_agent_http::HttpError>),                                           // "decide.http"
    /// An LLM client could not be built or a provider model list could not be read.
    Llm(Box<sc_llm_provider::LlmError>),                                           // "decide.llm"
    /// flush_session_log failed after the results were printed, or RunnerConfig::from_config refused the workers (runner.invalid_config).
    Runner(Box<sc_decision_runner::RunnerError>),                                  // "decide.runner"
    /// No provider in Provider::ranking for the model has a detected key (and no --provider was given).
    /// recovery: "set one of the listed providers' key variables, or pass --provider".
    NoProviderKey { model: ModelSelector, tried: Vec<Provider> },                  // "decide.no_provider_key"
    /// --runner (or models(runner=...)) names no configured local runner; name may not even be a valid RunnerName.
    /// recovery: "name a key of `local_runners` in the global configuration".
    UnknownRunner { name: String, known: Vec<sc_agent_config::RunnerName> },       // "decide.unknown_runner"
    InputNotFound { input: String },                                               // "decide.input_not_found"
    /// io error other than NotFound, kept whole as source (cause id io).
    InputRead { input: String, source: std::io::Error },                           // "decide.input_read"
    /// REQ-SAG-0014; line None for a single request.
    LimitExceeded { what: input::LimitKind, line: Option<u64>, max: u64 },         // "decide.limit_exceeded"
    LineNotUtf8 { line: u64, byte_offset: usize },                                 // "decide.line_not_utf8"
    /// An image file could not be read; the io error is kept whole as source (cause id io).
    ImageRead { name: String, path: std::path::PathBuf, source: std::io::Error },  // "decide.image_read"
    /// An image's extension (files) or magic bytes (in memory, path empty) is not png, jpg or jpeg.
    ImageFormat { name: String, path: std::path::PathBuf },                        // "decide.image_format"
    /// A request does not match the Jev request schema, or DecisionRequest::new refused it (image parts, image limits);
    /// line = JSONL line (None for a single request); the message names the line, the field and the reason;
    /// source = decision.request_parse / decision.image_parts / decision.limits, kept (REQ-SAG-0504).
    RequestInvalid { line: Option<u64>, source: Box<sc_decision_schema::DecisionError> },  // "decide.request_invalid"
    Encode { source: serde_json::Error },                                          // "decide.encode"; cause id serde_json
    /// stdout write failed.
    Write { source: std::io::Error },                                              // "decide.write"; cause id io
    /// The interrupt handler could not be installed.
    Signal { source: ctrlc::Error },                                               // "decide.signal"; cause id ctrlc
}
impl DecideError {
    pub fn request_invalid(line: Option<u64>, source: sc_decision_schema::DecisionError) -> Self;   // boxes source
    pub fn input(input: &str, source: std::io::Error) -> Self;                      // NotFound -> InputNotFound, else InputRead
}
// From impls (macro boxed_from!): AgentConfigError, TypesError, SecretsError, LogError, LocalError, DecisionError, HttpError, LlmError, RunnerError
// -> their boxed variant; serde_json::Error -> Encode; clap::Error -> Usage (boxed).
impl ErrorId for DecideError {}   // recovery(): Some(..) for every variant of this crate (wrapped errors give their own); cause_detail(): see above
```

22 variants (QA-1: `UnknownRunner` (decide.unknown_runner) added for `models --runner NAME`; `LineTooLong` folded into `LimitExceeded`; `ImageRead`, `ImageFormat` and `Signal` added). Every wrapped lower error (Config, Types, Secrets, Log, Local, Decision, Http, Llm, Runner, `Usage.source`, `RequestInvalid.source`) is boxed for clippy result_large_err, the wrapped error unchanged; io, serde_json and ctrlc errors are kept as `source` (not as kind and detail strings). A decision that fails inside `decide`/`decide_stream` is not a `DecideError`: it is the `DecisionError` inside that `DecisionResult`, printed in its `ResultLine.error`.

## 5. Cargo.toml (decide, new)

```toml
[package]
name = "decide"            # crates.io availability checked at publish time
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Command-line client for the TypeSafe (Jev) decision API."

[lints]
workspace = true

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order", "float_roundtrip", "raw_value"] }
sc-agent-types = { path = "../sc-agent-types", version = "0.1.0" }
sc-agent-config = { path = "../sc-agent-config", version = "0.1.0" }
sc-agent-secrets = { path = "../sc-agent-secrets", version = "0.1.0" }
sc-agent-log = { path = "../sc-agent-log", version = "0.1.0" }
sc-agent-http = { path = "../sc-agent-http", version = "0.1.0" }
sc-decision-schema = { path = "../sc-decision-schema", version = "0.1.0" }
sc-decision-systemone = { path = "../sc-decision-systemone", version = "0.1.0" }
sc-decision-local = { path = "../sc-decision-local", version = "0.1.0" }
sc-llm-provider = { path = "../sc-llm-provider", version = "0.1.0" }
sc-decision-llm = { path = "../sc-decision-llm", version = "0.1.0" }
sc-decision-runner = { path = "../sc-decision-runner", version = "0.1.0" }

ctrlc = { version = "3", features = ["termination"] }   # sync; SIGINT/SIGTERM, Ctrl-C/Ctrl-Break; no tokio (REQ-SAG-0011)

[dev-dependencies]
sc-agent-http = { path = "../sc-agent-http", features = ["testing"] }   # ScriptedTransport, BlockingTransport
sc-agent-log = { path = "../sc-agent-log", features = ["testing"] }     # MemoryLog
sc-observability = "=1.5.0"                                              # a notifying LogSink for ordering tests (tests/common)
tempfile = "3"

[lib]
name = "decide"
path = "src/lib.rs"

[[bin]]
name = "decide"
path = "src/main.rs"
```

### 3.3 sc-agent Python API (`crates/sc-agent-py/`, ANC-006)

Package `sc-agent-py` (`publish = false`), `[lib] name = "sc_agent", crate-type = ["cdylib"]`; maturin wheel `sc-agent`, module `sc_agent`. Deps: `decide` (path, lib), `sc-agent-types`, `sc-agent-config`, `sc-agent-log`, `sc-decision-schema`, `sc-observability = "=1.5.0"` (the `LogSink` trait), `pyo3 = { version = "=0.29.2", features = ["abi3-py310", "extension-module"] }`, `serde_json`; build-dependency `pyo3-build-config = "=0.29.2"` (macOS `-undefined dynamic_lookup`, so plain `cargo build` and `cargo test` link the cdylib). Same output shapes as the CLI (JSON text). The default general log service is `sc-agent-py`. Strings are parsed at the edge (`ModelSelector::parse`, `Provider::from_str`, `Slug::new`, `RunnerName::try_from`); errors raise `SagError`. Requests go through the same limit checks as the CLI (REQ-SAG-0014): `send` calls `decide::input::build_request_bytes` and `send_batch` calls `decide::input::build_batch` before any logger is opened, so in-memory strings and byte images meet the 8 MiB request, 10,000-item / 256 MiB batch and 20 MiB / 20-image limits, raising `SagError` `decide.limit_exceeded` (or `decide.request_invalid` with cause `decision.limits`). A worker thread that panics, or ends without a result, raises `SagError` with the new id `sc_agent_py.worker_panicked` (RBP-F003).

```rust
#[pyclass(module = "sc_agent", frozen)] pub struct CancelToken(sc_agent_types::CancelToken);   // new(), cancel(), is_cancelled()
create_exception!(sc_agent, SagError, PyException, "...");   // args = (id, message, detail JSON with every cause_detail and recovery when present)
/// A batch item: a JSON request string, or (request, images, long_context).
#[derive(FromPyObject)]
enum BatchItem { Bare(String), Full(String, Option<std::collections::BTreeMap<String, Vec<u8>>>, bool) }
#[pyfunction]
#[pyo3(signature = (request, *, model, project, agent_role, provider = None, images = None, long_context = false, cancel = None, general_log = None, session_log = None, config_dir = None))]
#[expect(clippy::too_many_arguments, reason = "one keyword argument per decide send flag (ANC-006)")]
fn send(py: Python<'_>, request: &str, model: &str, project: &str, agent_role: &str, provider: Option<&str>,
        images: Option<BTreeMap<String, Vec<u8>>>, long_context: bool, cancel: Option<&CancelToken>,
        general_log: Option<Py<PyAny>>, session_log: Option<Py<PyAny>>, config_dir: Option<PathBuf>) -> PyResult<String>;   // one ResultLine; decide::send with an on_result that keeps the line
#[pyfunction]
#[pyo3(signature = (requests, *, model, project, agent_role, provider = None, cancel = None, on_result = None, general_log = None, session_log = None, config_dir = None))]
#[expect(clippy::too_many_arguments, reason = "one keyword argument per decide send flag (ANC-006)")]
fn send_batch(py: Python<'_>, requests: Vec<BatchItem>, model: &str, project: &str, agent_role: &str, provider: Option<&str>,
              cancel: Option<&CancelToken>, on_result: Option<Py<PyAny>>, general_log: Option<Py<PyAny>>, session_log: Option<Py<PyAny>>, config_dir: Option<PathBuf>) -> PyResult<Vec<String>>;   // ResultLines in finish order; on_result(line) as each arrives
#[pyfunction] #[pyo3(signature = (runner = None, cancel = None, general_log = None, config_dir = None))]
fn models(py: Python<'_>, runner: Option<&str>, cancel: Option<&CancelToken>, general_log: Option<Py<PyAny>>, config_dir: Option<PathBuf>) -> PyResult<String>;
#[pyfunction] #[pyo3(signature = (cancel = None, general_log = None, config_dir = None))]
fn doctor(py: Python<'_>, cancel: Option<&CancelToken>, general_log: Option<Py<PyAny>>, config_dir: Option<PathBuf>) -> PyResult<String>;   // DoctorReport JSON
#[pymodule] fn sc_agent(module: &Bound<'_, PyModule>) -> PyResult<()>;

/// Calls a Python `Callable[[dict], None]` with each record under the GIL. The dict is {"level", "target", "action", "fields": {..}}.
/// A raised exception is a refused write (counted, reported by flush); never changes a result.
struct PyCallableSink { callable: Py<PyAny>, name: SinkName }
impl sc_observability::v2::LogSink for PyCallableSink {}
/// Logger built like MemoryLog (file and console sinks off, SinkRegistration::typed(PyCallableSink)) ->
/// GeneralLogger::injected / SessionLogger::injected(logger, decide::session_identity(..)?).
fn callable_logger(callable: Py<PyAny>, service: &'static str) -> Result<Arc<sc_observability::v2::Logger>, DecideError>;
/// What a worker's call gives back: its result, and any logger that failed to close.
type Outcome<T> = (Result<T, DecideError>, Vec<DecideError>);
type OnResult<'a> = &'a mut dyn FnMut(DecisionResult) -> Result<(), DecideError>;
/// Runs `work` on a std::thread::scope worker; the Python thread waits on the result channel with
/// recv_timeout(100 ms) inside py.detach, then py.check_signals() with the GIL. A signal exception cancels the token, drains the
/// channel (the drained results still go to on_result), joins, and re-raises; an on_result exception does the same but the
/// drained results are not passed on (REQ-SAG-0011). Returns the value and the result lines. A logger that failed to close (or a
/// session flush after the result) is a RuntimeWarning whose text is the JSON error chain; it never changes the result.
/// A worker panic or a worker that ended without a result is SagError sc_agent_py.worker_panicked.
fn run_interruptible<T: Send>(py: Python<'_>, cancel: &sc_agent_types::CancelToken, on_result: Option<&Py<PyAny>>, work: impl FnOnce(OnResult<'_>) -> Outcome<T> + Send) -> PyResult<(T, Vec<String>)>;
/// Closes the session and general loggers on the worker thread after the operation; a close failure is returned, not raised.
fn close_loggers(general: Option<GeneralLogger>, session: Option<SessionLogger>) -> Vec<DecideError>;
/// `runner` as a RunnerName; a name that cannot be one is decide.unknown_runner (naming the configured runners).
fn runner_name(runner: &str, config_dir: Option<PathBuf>) -> Result<RunnerName, DecideError>;
/// The Context of an operation: config_dir (None -> ~/.sc-agent), and the general-log callable, else the default general log.
fn context(general_log: Option<Py<PyAny>>, config_dir: Option<PathBuf>) -> Result<Context, DecideError>;
```

decide subcommand -> Python: `send` -> `send` (`decide::send`; `--image` -> `images`, `--long-context` -> `long_context`), `send --batch` -> `send_batch` (`decide::send_batch`; batch-line wrapper -> `BatchItem::Full`), `models` -> `models`, `doctor` -> `doctor`. The Python API is a superset where useful (ANC-006): image bytes may be passed directly, loggers may be injected, and every function takes `config_dir` (`str | os.PathLike | None`, mapping to `Context.config_dir`; absent: `~/.sc-agent`).
Transport from Python: no Python transport seam (the context transport is None -> `UreqTransport`); tests that need a scripted transport are Rust tests in crates/decide. Python tests use a loopback runner from a tempdir configuration chosen with the `config_dir` option (no environment variable; the pytest `args` fixture carries `model`, `project`, `agent_role` and `config_dir`) and no external network.

No tokio, no mimalloc. Dev: `tempfile 3`.

## 6. New code

- decide: `src/error.rs` (528 lines with its tests), `src/select.rs` (1,300), `src/signal.rs` (226), `src/doctor.rs` (204), `src/output.rs` (71), the input reader (`read_batch`, `open_lines`, `read_image`, `build_request`, `build_request_bytes`, `build_batch`, `LimitKind`) in `src/input.rs` (843), `main.rs` (249), `cli.rs` (177).
- sc-agent-py: `src/lib.rs` (610: functions, `BatchItem`, `PyCallableSink`, `callable_logger`, `run_interruptible`, `close_loggers`), `pyproject.toml`, `tests/test_api.py`, `tests/conftest.py`, `tests/loopback.py` (a TCP server that accepts and never answers, or answers one fixed error, or answers ok).

## 7. Tests

Each test names the acceptance line it proves (`AC: <line key>`). "Real runner" = `decide::send`/`send_batch` with a `Context` holding a tempdir `config_dir`, a tempdir `repo_root`, an explicit `environment`, a map keyring, a `ManualClock` (`clock`, an `sc_agent_types::Clock`) where a holdoff, pause or load poll is under test, a `ScriptedTransport` (or the bounded-wait `Gate`) and `MemoryLog` (`general: Some(log.general())`, `session: Some(log.session(identity))`), going through the real `DecisionRunner` and backend; no network, process env and HOME untouched.

decide (`crates/decide`):
1. `cli_definition_is_consistent`; the 3 `read_line_bounded` tests (copied; AC: decide send).
2. Single request from a file and from stdin reaches `send_on` unchanged; one `ResultLine` with `session_id` and `position` (AC: decide send).
3. Each TypeSafe request example (openapi.json) through the real runner + SystemOneBackend, transport answering the documented response (AC: REQ-SAG-0501 AC1).
4. Each openapi.json example that uses `assets[].part` is sent with `--image NAME=FILE` (and as a batch wrapper line), and each long-context example with `--long-context`, through the real runner: the transport sees multipart parts named as the assets and the `TypeSafe-Danger-Zone-Long-Context: true` header (AC: REQ-SAG-0501 images).
5. Batch of 3 on a fake `Decision` finishing out of order and failing one (AC: REQ-SAG-0701 output).
6. 1,000-line batch, line 412 bad, real runner; the injected transport records zero calls (AC: REQ-SAG-0504).
7. `Slug::new` rejects `"Phase B"` at the edge; no `--agent-name` flag (AC: REQ-SAG-0004).
8. `0.123456789` printed unchanged; a result carrying `decision.http` -> `http.network` prints both ids and messages (AC: REQ-SAG-0002).
9. `models` with an injected transport (AC: REQ-SAG-0501 AC2).
10. `doctor` with an injected transport and fake provider lists; `missing` and `ties` as named structs (AC: REQ-SAG-0503).
11. `select::backend` provider choice cases (AC: REQ-SAG-0103).
12. A LocalBackend and a SystemOneBackend run batches in parallel through two real runners over injected transports (AC: decide send, owner-local-decision-crate).
13. Broken stdout during a batch cancels the token and returns `decide.write` (AC: REQ-SAG-0701 output).
14. A run with `config_dir` whose default.json sets `log_root` to a tempdir writes `<log_root>/logs/decide.log.jsonl` and the session row under `<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl`; both logs land under `<log_root>/logs/` (AC: REQ-SAG-0003, ADR-SAG-0002).
15. 40-request `send_batch` over a bounded `Gate` transport: 8 sends in flight at once with the default workers, 1 with workers 1 (AC: REQ-SAG-0701 parallel).
16. Busy end to end: transport answers 529 (no Retry-After) every time -> 5 sends and `http.provider_busy` (reason `sends`, 5 attempts); `SC_AGENT__MAX_SENDS=2` in `environment` -> 2 sends (AC: REQ-SAG-0601).
17. Plain `send` calls `decide`, `send --batch` with one line calls `decide_stream` (fake `Decision` counting calls; AC: CRIT-M3 dispatch).
18. MemoryLog injected; one send, models and doctor; `log.records()?` holds `decide.send`, `decide.models`, `decide.doctor` and `http.send` actions; no record's `fields` holds a key value (AC: ANC-007).
19. `grep -rn 'thread::spawn' crates/decide/src` finds nothing; the ctrlc handler thread is ctrlc's, not decide's (AC: REQ-SAG-0701 output).
20. `send` over a `Gate` transport held by the test; cancel the token; the `decision.cancelled` result arrives on a channel within the 2 s bound; releasing the transport yields no further result (AC: REQ-SAG-0104 AC2).
21. `--model haiku` through a tempdir default.json with a later-dated `haiku` model: the transport sees that model; without it, the built-in (AC: REQ-SAG-0101, REQ-SAG-0102).
22. Anthropic 429 with the quota body: one send, result `http.quota_exhausted` with the remote message and its recovery (AC: REQ-SAG-0602).
23. A tempdir default.json `local_runners.laya` (host 127.0.0.1, port 9000): `--model laya` builds a LocalBackend and the transport sees `http://127.0.0.1:9000` (AC: REQ-SAG-0801).
24. REQ-SAG-0012, two cases through `decide::send_batch` against one provider, on a `ManualClock` (`Context.clock`) that only the test moves; the transport answers 429 `Retry-After: 1` to the first send and 200 after. `a_retry_after_pauses_every_send_to_the_host` (`workers: 1`): the single worker admits the other request only after the 429 is handled; the test reads the exact pause end from the `http.busy` record and moves the clock to exactly it, and every later send, the other request's included, is stamped with exactly that instant, because the pause lives in the one `HttpClient` the runner shares; with `Retry-After: 0` the pause end is the 250 ms floor. `with_two_workers_a_request_admitted_after_the_429_waits_the_pause` (`workers: 2`): two workers take the first two requests, the second one's send is held until the `http.busy` record of the 429 is written, so the third request is admitted only once the pause holds; it and the resend are both sent at exactly the pause end. Nothing depends on elapsed time (AC: REQ-SAG-0012).
25. default.json with `workers: 0`, `max_sends: 11`, a runner `port: 0`, and `key_env` on `http://10.0.0.5`: each -> `decide.config` (cause `config.invalid` naming the key path, or `config.key_on_plain_http`) and no send (AC: REQ-SAG-0013).
26. Limits: a 9 MiB single request, a 10,001-line batch, a line past 8 MiB, a 21 MiB image, 21 images -> `decide.limit_exceeded` / `decide.request_invalid` (cause `decision.limits`) naming the limit; nothing sent. `an_oversized_batch_sends_nothing` (providers.rs): past 256 MiB a batch read from a generated file through `read_batch` is `batch_bytes` and nothing is sent, and 10,001 in-memory items through `build_batch` are `batch_lines`; the in-memory byte cap over item sizes (`check_batch_sizes`) is unit-tested in `input.rs` (`in_memory_item_sizes_over_256_mib_are_batch_bytes`) without allocating 256 MiB. Unit tests in `input.rs` also pin the counting rules: blank lines count against `batch_lines`, a line of exactly the cap is accepted (the terminator is not counted), a wrapper line's image bytes count toward the batch budget, and an image's size is checked from metadata before the read (AC: REQ-SAG-0014).
27. Signals (AC: REQ-SAG-0011 decide, NFR-SAG-0001). Unit (`src/signal.rs`): `an_interrupt_before_the_logs_open_exits_at_once` (`on_interrupt` counts, cancels and returns `Action::Exit` while no logger is open); `a_second_interrupt_of_a_run_still_drains_and_flushes` (after `logs_opened` the first and second interrupt return `Action::Cancel`, and the run still drains every request as cancelled and flushes one row per started request); `the_third_interrupt_after_the_logs_open_exits_at_once` (RSH-003: the first two interrupts return `Action::Cancel`, the third returns `Action::Exit`, `LAST_RESORT_INTERRUPT = 3`); `an_interrupt_after_the_logs_open_still_flushes_the_general_log` (an interrupt delivered before `models` sends anything does not exit; the operation writes its `decide.models` record and flushes the general log it opened); `an_interrupt_during_a_batch_cancels_every_unfinished_request` (over a `BlockingTransport` an interrupt after `logs_opened` cancels the token, every unfinished result is `decision.cancelled` and the session log holds one row per started request). Process (Unix CI, `tests/cli.rs`, the child's environment cleared and `--config-dir <tempdir>/.sc-agent`, no HOME): `an_interrupt_cancels_flushes_and_exits_130` (a `decide send --batch` child against a loopback runner that never answers, sent SIGINT and, in a second run, SIGTERM: every line printed is `decision.cancelled`, the rows are in sessions.jsonl, exit code 130 both times) `sigterm_while_the_input_blocks_exits_130` (SIGTERM while decide blocks reading its input, a FIFO the test holds open, before any logger opens, exits 130 at once with nothing printed or sent, RSH-001) and `a_fifo_config_file_is_refused_without_blocking` (RSH-002: a `default.json` that is a FIFO is refused from its metadata, never opened: decide exits 1 at once with `decide.config` whose cause is `config.not_a_file`).
30. Local runner load (REQ-SAG-0015): a scripted local daemon (ScriptedTransport via the Context) answers the load probe not-loaded twice, then loaded; `send --model dec` (a mapika runner) makes the three probes, then one send, on the Context's `ManualClock` stepped by the test (once probe n is sent and the wait is parked on the clock, the test moves it one `LOAD_POLL_INTERVAL`; the probes land exactly 1 s apart and the end record's `waited_ms` is 2000; `decide_waits_for_the_local_model_to_load_before_sending`); the send carries no load deadline (the transport sees `deadline() == None`), so its timeout is the per-runner transport's own, counted from the send after the load, not from the first probe. The per-runner request timeout is checked on the real `decision_transport` (`select.rs` unit test), and a runner entry without `request_timeout_secs`/`load_timeout_secs` gets the 300 s and 600 s defaults. Cancelling the token while the probe transport blocks (the bounded `Gate`) ends the wait at once with `decision.cancelled` (`sends: 0`) for every request and no decision send. A model still not loaded after the runner's `load_timeout_secs` (10, the minimum; the Context's `ManualClock`, with the one scripted probe taking the whole 10 s on it, so no real time passes) ends the send with `decide.decision` (cause `decision.backend` -> `local.load_timeout`, naming the runner) and nothing but `/health` probes is sent (`a_model_that_does_not_load_in_time_is_local_load_timeout`). `the_configured_load_timeout_reaches_the_load_wait` (`select.rs` unit): a runner entry with `load_timeout_secs: 17` passes 17 s to `LocalProbe::wait_until_loaded` (recorded by a test-only seam in decide's `wait_until_loaded` wrapper), so the configured value is proven without waiting it out (AC: REQ-SAG-0015).
28a. Workspace check (`crates/decide/tests/workspace.rs`, `only_sc_decision_schema_maps_the_jev_api`; AC: REQ-SAG-0201 AC2): outside sc-decision-schema no serde-derived struct or enum carries a Jev wire field (`questions`, `answers`, `assets`, `noul`, `criteria`, `probabilities`, `legend`, `data_base64`, `release_date`, `instructions`), found by field name or `#[serde(rename)]`, not by type name (a type-name `rg` also matches SystemOneBackend, which has no serde derive and maps no Jev field). Provider usage types in sc-llm-provider carry none of those names and pass. A positive control: the same scan must find `SystemOneRequest` and `SystemOneResponse` in sc-decision-schema, so a broken scanner cannot pass vacuously.
28. Workspace searches (`workspace.rs`; AC: REQ-SAG-0001 AC2, REQ-SAG-0007): `no_uuid_text_anywhere_in_crates` finds no text file under `crates/` containing `Uuid`, `uuid::` or `uuid =` (the needles are built at run time so the file does not match itself, and the test asserts it saw over 100 files); `no_rounding_anywhere` finds no `round(` call and no `{:.N}` precision format in any Rust file under `crates/`, tests included, which is stricter than the AC.
31. REQ-SAG-0002 0.2.0, attached-key redaction end to end (`providers.rs`, `an_echoed_key_is_redacted_from_every_refusal`): a provider that echoes the attached key in a 400, 422 or 503 body (TypeSafe), and a keyed local runner (loopback, so the key is allowed) that echoes it in a 503, give a result whose error has the same id (`decision.api_bad_request`, `decision.api_validation`, `decision.api_status`), `[redacted]` where the key was, and the rest of the provider's text whole; the printed line never contains the key. `an_echoed_key_is_redacted_from_a_parse_error`: a 2xx reply whose mistyped field echoes the attached key is `decision.response_parse`, with `[redacted]` in serde's quoted text and the rest of the message kept; the same for a models list (`decode_models`), where the error is `decide.decision` with cause `decision.response_parse` (AC: REQ-SAG-0002).
32. A session log that cannot be flushed after the result still returns the result and the flush error after it (`logs.rs`, `a_session_flush_failure_keeps_the_result`); with no injected loggers a batch has written its general and session files when `send_batch` returns (`default_logs_are_written_when_a_batch_returns`; AC: REQ-SAG-0002, REQ-SAG-0003, QA-001, QA-006).

sc-agent-py (Rust unit tests in `src/lib.rs`):
29. The string sc-agent-py returns for a DecisionResult holding probability `0.123456789` contains `0.123456789` (AC: REQ-SAG-0007).

sc-agent-py (`tests/test_api.py`, pytest over the maturin build; a tempdir configuration chosen with the `config_dir` option, passed by the `args` fixture, whose default.json names a loopback runner; no external network):
1. Lists the decide subcommands from `decide --help` (send, models, doctor, plus `send --batch`) and asserts each has its Python function (AC: ANC-006).
2. A `CancelToken` created and cancelled from Python, passed to `send` and `send_batch`, ends every request with `decision.cancelled` and starts no send (AC: REQ-SAG-0104 AC2).
3. `project="Phase B"` raises `SagError` with id `decide.log` (cause `log.invalid_slug`) and writes no row (AC: REQ-SAG-0004).
4. One `decide send` subprocess and one `sc_agent.send` with the same project and agent_role against a loopback runner that answers a fixed error: both session rows carry equal `project` and `agent_role` (AC: REQ-SAG-0004 AC3), and both entry points return the loopback's 503 as the lowest-layer error (`assert_lowest_503`: id `decision.api_status`, no `cause`, message `the API answered HTTP 503: <body>` with the provider's body whole; AC: REQ-SAG-0002 AC1).
5. `general_log=records.append`, `session_log=rows.append` on one send: `records` holds a dict with action `decide.send`, `rows` holds the session row; no file is written under the log root for the injected logs (AC: REQ-SAG-0008 Python).
6. `send_batch` against a loopback runner that never answers; a second Python thread calls `_thread.interrupt_main()`; `KeyboardInterrupt` is raised within 2 s, every result was `decision.cancelled`, and the session rows are on disk; an `on_result` that raises `ValueError` cancels, drains and re-raises it (AC: REQ-SAG-0011 Python).
8. `send_batch` with 10,001 items raises `SagError` `decide.limit_exceeded` (what `batch_lines`), and `send` with a 9 MiB request string raises `decide.limit_exceeded` (what `request_bytes`); nothing reaches the loopback runner (AC: REQ-SAG-0014).
9. `session_log=rows.append` only: `rows` gets the session row and the general log file `<log_root>/logs/sc-agent-py.log.jsonl` is written under the tempdir log root (AC: REQ-SAG-0008 AC4).
10. `general_log=records.append` only: `records` gets the `decide.send` record and the session row is in `<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl` (AC: REQ-SAG-0008 AC5).
7. `send(request, images={"img1": png_bytes}, long_context=True)` against the loopback runner reaches the transport as multipart with the long-context header (the loopback server records the request) (AC: REQ-SAG-0501 images).
11. `send` and `send_batch` against a loopback runner that answers ok return the decided response: one line per request, `position` 0 for `send` and 0..n-1 for the batch, a 26-character session ULID, the probability unrounded, and one session row per request with outcome ok (AC: REQ-SAG-0501 successful path, REQ-SAG-0007).
12. A `general_log` or `session_log` callable that raises never changes the result: the send returns its normal line and the lost rows are reported as a `RuntimeWarning` naming `log.rows_lost`; callables that accept every record warn nothing (AC: REQ-SAG-0008, QA-003).

## Required validation

- `cargo build -p decide -p sc-agent-py`; `maturin build -m crates/sc-agent-py/Cargo.toml`.
- `cargo clippy -p decide -p sc-agent-py --all-targets -- -D warnings` with the A-1 workspace denies and clippy.toml (sleep disallowed).
- `cargo test -p decide -p sc-agent-py`; `pytest crates/sc-agent-py/tests`.
- `cargo tree -p decide -e normal` and `cargo tree -p sc-agent-py -e normal`: no tokio.
- `cargo tree -i decide` for each lower sc-agent crate: not listed (ADR-SAG-0004).
- `grep -rnE 'thread::sleep|time::sleep|sleep_until' crates/decide/src crates/sc-agent-py/src` finds nothing; no `thread::spawn` in crates/decide/src.
- The workspace search tests (test 28 and 28a: `no_uuid_text_anywhere_in_crates`, `no_rounding_anywhere`, `only_sc_decision_schema_maps_the_jev_api`) pass; they replace the literal `rg` searches.

## 8. Dependencies on other sprints

A-6 and A-7 (backends, `LocalProbe` incl. `with_log`, models list, LLM client incl. `with_log`, `LlmProvider`); A-9 (`DecisionRunner`, `RunnerConfig`, `RunnerError`); through them A-1, A-2 (`DecisionRequest::new`, image limits), A-4 (config bounds, `RunnerKind`, `KeySource`), A-5 (`Timeouts::probe`, provider pause, `testing`), A-8 (`Slug`, `injected` loggers, `testing::MemoryLog`) (names in section 3.2).

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-10.sh` (sc-agent-ref@dce239a, 9 copy steps), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] decide send: one request from a file or stdin, or a JSONL batch (--batch), with --model, --project and --agent-role (required; no --agent-name) and an optional --provider override; a configured local runner is selected by name (REQ-SAG-0401, REQ-SAG-0801); runs through the Decision trait only, over DecisionRunner (ADR-SAG-0001).
- [ ] REQ-SAG-0501 images: `decide send --image NAME=FILE` (repeatable) and `--long-context`, batch wrapper lines `{"request", "images", "long_context"}`, and sc-agent-py `send(..., images=, long_context=)` / batch tuples build every request with DecisionRequest::new; each openapi.json example that uses assets.part or long context is sent through the real runner and the transport sees the multipart parts and the long-context header (ADR-SAG-0001: DecisionRequest is the one request form).
- [ ] CRIT-M3 dispatch: plain `decide send` always calls Decision::decide; `decide send --batch` always calls Decision::decide_stream with an owned 'static iterator (Box::new(requests.into_iter())), even for a one-line batch.
- [ ] REQ-SAG-0103: without --provider, decide picks the first provider in Provider::ranking(line) with a detected key (process env, repo-root .env, keyring); `--model haiku` with Anthropic and OpenRouter keys goes to Anthropic, with only OpenRouter to OpenRouter as `anthropic/claude-haiku-5.5`, `--provider openrouter` overrides, `--model jev` with no TypeSafe key is decide.no_provider_key (with a recovery text).
- [ ] REQ-SAG-0101 / REQ-SAG-0102: `decide send --model haiku` resolves through the config-built catalog; with a tempdir default.json adding a later-dated haiku model, the transport receives that model id; without it, the built-in.
- [ ] REQ-SAG-0801: a `local_runners.laya` entry in a tempdir default.json (Context.config_dir) makes `--model laya` build a LocalBackend that sends to the configured host and port.
- [ ] REQ-SAG-0504: a 1,000-line batch whose line 412 has a `score` question without `criteria` stops decide with decide.request_invalid naming line 412, `criteria` and the reason, and the injected transport is never called.
- [ ] REQ-SAG-0014: a single request over 8 MiB, a batch over 10,000 lines or 256 MiB, a line over 8 MiB, an image over 20 MiB and a request with 21 images each end with a typed error naming the limit (decide.limit_exceeded, or decide.request_invalid with cause decision.limits) and nothing is sent; sc-agent-py goes through the same decide::input::build_request_bytes / build_batch checks, so 10,001 batch items or a request over 8 MiB from Python raise SagError decide.limit_exceeded.
- [ ] REQ-SAG-0701 parallel: decide --batch hands the whole batch to DecisionRunner; a 40-request batch over a BlockingTransport has 8 sends in flight at once with the default workers and 1 with workers set to 1.
- [ ] REQ-SAG-0701 output: decide starts no thread of its own (a test greps crates/decide/src for thread::spawn and finds none; the only extra thread is the interrupt handler thread the ctrlc crate starts and owns); decide prints JSON for one request and JSONL for a batch, fastest first, each line tagged with its session ULID and input position; a failed request is printed in its result as an ErrorDetail chain and the rest of the batch continues; a stdout write failure cancels the batch and returns decide.write.
- [ ] REQ-SAG-0601: end to end through decide send and the real DecisionRunner, a request whose injected transport answers busy (529) every time is sent 5 times and its result is http.provider_busy with reason sends and 5 attempts; with SC_AGENT__MAX_SENDS=2 in Context.environment it is sent 2 times.
- [ ] REQ-SAG-0012: end to end through decide send --batch, a 429 with Retry-After: 1 on the first of two parallel requests to one provider pauses every later send to that host (the other request's too) until the not-before time; Retry-After: 0 still waits the 250 ms floor.
- [ ] REQ-SAG-0602: an Anthropic 429 with the quota body ends the decide result with http.quota_exhausted after exactly one send, with the remote message and its recovery text.
- [ ] REQ-SAG-0013: decide with a tempdir default.json holding workers 0, max_sends 11, a runner port 0, or a key_env on a plain-http non-loopback host fails before any send with decide.config, cause config.invalid naming the key path (or config.key_on_plain_http).
- [ ] REQ-SAG-0011 decide: SIGINT or SIGTERM during `decide send --batch` (and Ctrl-C/Ctrl-Break on Windows) cancels the token through decide::signal (ctrlc with feature termination, no tokio); every unfinished request ends decision.cancelled and is printed, the session log is flushed (one row per started request), and decide exits 130 for any interrupt (ctrlc does not report which signal fired). An interrupt exits 130 at once before any logger is open (`SignalWatch::logs_opened`, called just after the general logger opens: parsing, configuration, input reading and the logger open); after that the first and second interrupt only cancel, and the exit goes through drain, flush and close of the session and general logs, exit 130, for send, send --batch, models and doctor; the third interrupt after `logs_opened` exits 130 at once (`LAST_RESORT_INTERRUPT = 3`, test `the_third_interrupt_after_the_logs_open_exits_at_once`).
- [ ] REQ-SAG-0015: end to end through decide send to a local runner, decide waits for the model to load with LocalProbe::wait_until_loaded (load_timeout from the runner's entry, default 600 s) before the first send, then sends over a per-runner UreqTransport whose request timeout is the runner's (default 300 s); a scripted daemon reporting not-loaded twice then loaded gets one send that carries no load deadline, so its timeout starts after the load; a cancel during the load wait ends it at once with decision.cancelled and nothing sent; a model not loaded within load_timeout ends with decide.decision (cause decision.backend -> local.load_timeout) and nothing sent.
- [ ] REQ-SAG-0011 Python: KeyboardInterrupt during sc-agent-py send/send_batch is seen within one 100 ms timed wait (py.check_signals with the GIL), cancels the token, drains the results, flushes the session rows and re-raises; an exception raised by on_result does the same.
- [ ] Each TypeSafe request example can be sent through decide send and returns the API's response (REQ-SAG-0501 AC1; ScriptedTransport injected through the Context, real runner).
- [ ] The TypeSafe models list is returned by decide models (REQ-SAG-0501 AC2; transport injected through the Context; Timeouts::probe()).
- [ ] decide doctor warns naming provider and model for every provider model in neither the built-in list nor the global configuration, prints no warning when all are present, warns when a config model has the release date of a built-in of its line ({config, builtin}), and lists every detected provider key (KeySource, never the value) in the order it would be used (REQ-SAG-0503).
- [ ] ANC-006: crates/sc-agent-py (module sc_agent) offers send, send_batch, models and doctor, one per decide operation (send, send --batch, models, doctor), checked by a pytest that lists the decide subcommands from `decide --help` and the module's functions.
- [ ] REQ-SAG-0104 AC2: a CancelToken created and cancelled from Python stops the sc-agent-py send and send_batch it is passed to (decision.cancelled, no send); in decide, cancelling a send whose transport is blocked returns decision.cancelled within the 2 s channel bound and no later result.
- [ ] REQ-SAG-0004: an invalid --project or --agent-role (e.g. 'Phase B') is rejected as decide.log with cause log.invalid_slug and nothing is sent or logged; sc-agent-py parses project and agent_role with the same Slug::new and raises SagError decide.log for 'Phase B'; AC3: a decide CLI send and an sc-agent-py send with the same project and agent_role write session rows with equal project and agent_role fields.
- [ ] REQ-SAG-0008 AC4/AC5 Python: with only session_log injected the callable receives the session row and the default general log file is written under the tempdir log root; with only general_log injected the callable receives the decide.send record and the session row is written to sessions.jsonl.
- [ ] REQ-SAG-0008 Python: sc-agent-py send/send_batch accept general_log and session_log callables (Callable[[dict], None]) and models/doctor accept general_log; a pytest receives the decide.send record and the session row through them, and the injected logs write no file.
- [ ] REQ-SAG-0003 (0.2.0): decide writes <log_root>/logs/decide.log.jsonl, its own general log file (tsq's is A-3); <log_root> is the configuration's log_root, else <home>/.sc-agent on every platform; every log goes under <log_root>/logs/ (a test covers both the general log and the session log landing there); no environment variable names a configuration directory or a log root. AC3 (no log_root configured: logs under <home>/.sc-agent/logs/) is closed by composition: sc-agent-config's `AgentConfig::log_root` (config.rs) returns the configured value, else `default_dir()` (`<home>/.sc-agent`), and decide's select.rs takes every log root from `config.log_root()` (the general and the session logger), so no test sets HOME.
- [ ] REQ-SAG-0005 (0.2.0): decide and sc-agent-py write session rows to <log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl (test every_log_lands_under_log_root_logs).
- [ ] ADR-SAG-0002: a decide send with a tempdir log_root writes its session row to <log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl with session_id, repo, project and agent_role first in fields.
- [ ] REQ-SAG-0001 AC2: no text file under crates/ of the finished workspace contains `Uuid`, `uuid::` or `uuid =` (crates/decide/tests/workspace.rs, no_uuid_text_anywhere_in_crates).
- [ ] REQ-SAG-0007: a probability 0.123456789 is printed unchanged by decide and returned unchanged in sc-agent-py's result string; no Rust file under crates/ has a `round(` call or a `{:.N}` precision format (crates/decide/tests/workspace.rs, no_rounding_anywhere; tests included, stricter than the AC).
- [ ] REQ-SAG-0002 (0.2.0): the error type DecideError (decide.* ids, implements ErrorId) is the return type of every public function of the decide library, and SagError is the error returned to Python by sc-agent-py; each wraps the real lower error unchanged and carries its id and message (AgentConfigError, TypesError, SecretsError, LogError, LocalError, DecisionError, HttpError, LlmError, RunnerError; io, clap, serde_json and ctrlc errors kept whole as `source`); a lowest-layer error (http.network) appears with its id and message in decide output, and a provider's 503 is the lowest-layer error at both the CLI and the Python entry (test_api.py, `assert_lowest_503`: id `decision.api_status`, no cause, message with the body whole); the lower-layer io or keyring error of a secrets failure is kept whole as `source` in SecretsError (reachable through cause_detail and source()); the attached-key exception: a key that a provider or local runner echoes in an error body is replaced by `[redacted]` while the error id and the rest of the text are kept (crates/decide/tests/providers.rs, an_echoed_key_is_redacted_from_every_refusal); values are not rounded (REQ-SAG-0007).
- [ ] ADR-SAG-0004: every dependency in crates/decide/Cargo.toml points down the layer order (types; config, secrets, log; http; decision schema; decision backends; runner): decide is the top layer, above sc-decision-runner, and depends on sc-agent-types, sc-agent-config, sc-agent-secrets, sc-agent-log, sc-agent-http, sc-decision-schema, sc-decision-systemone, sc-decision-local, sc-decision-llm and sc-decision-runner; crates/sc-agent-py depends on decide; no lower-layer sc-agent library crate lists decide, tsq or sc-agent-py as a dependency, so none depends on a higher layer, checked by `cargo tree` for each lower crate.
- [ ] REQ-SAG-0009: Cargo.toml dependencies: neither decide nor sc-agent-py adds a tokio dependency, and no low-level sc-agent crate gains tokio with default features through them; `cargo tree -p decide -e normal` and `cargo tree -p sc-agent-py -e normal` list no tokio crate.
- [ ] REQ-SAG-0010: decide and sc-agent-py source call no sleep function, enforced by the workspace clippy disallowed-methods rule; clippy passes with the workspace denies (REQ-SAG-0002).
- [ ] ANC-007: the decide library writes one decide.send, decide.models and decide.doctor record per operation (never a key) to the GeneralLogger in its Context (injected, or the default for its service) and passes that same logger into HttpClient::new, DecisionRunner::with_clock, LocalProbe::with_log, LlmClient::with_log and LlmDecisionBackend::with_log; a test injects sc_agent_log::testing::MemoryLog (general: Some(log.general()), session: Some(log.session(identity))), runs one send, models and doctor over a ScriptedTransport and finds those actions and http.send in log.records(); sc-agent-py calls the same decide library (default loggers, service sc-agent-py, or the Python callables), so its operations write the same records.
- [ ] REQ-SAG-0201 AC2: no crate in the finished workspace other than sc-decision-schema defines a serde-derived type that carries a Jev wire field (crates/decide/tests/workspace.rs, only_sc_decision_schema_maps_the_jev_api: a derive-aware scan by field name and serde rename, with a positive control that finds SystemOneRequest and SystemOneResponse in sc-decision-schema); provider usage types in sc-llm-provider map provider wire fields, carry none of the Jev names, and pass.
- [ ] NFR-SAG-0001 (end-to-end contribution; owner a-8, 40,000-session logger test): after an interrupted decide --batch, the session log holds exactly one row per started request (test 27's flushed-rows check), also after a second interrupt.
- [ ] REQ-SAG-0008 / REQ-QA-009: `decide --config-dir <PATH>` (global, on send, models and doctor) selects the configuration directory, the same seam as Python `config_dir=` (tests `config_dir_is_a_global_flag` and the cli.rs process tests, which set no HOME).

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); decisions are in the design:
1. decide package/bin/subcommands: decide; send, models, doctor; crates.io availability checked at publish time (assistant task).
2. The rename to decide is the owner's ("call the cli binaries 'decide'").
3. --project/--agent-role required (assistant); --provider is an optional override, provider chosen by ranking among detected keys (owner, REQ-SAG-0103); exit 0/1 (assistant).
4. ANC-006 (replaces ANC-002): sc-agent-py in this sprint.
Also: a bad batch line stops decide before any send (owner 'fail fast on first', REQ-SAG-0504); each CLI writes its own general log (REQ-SAG-0003).
Hardening round 1 (2026-11-09): tsq (binary, copy rows, tests, REQ-SAG-0303) moved to A-3 and the A-3 dependency was dropped (PLAN-SCOPE-004, PLAN-CRIT-007/011/012); injection seams in SendOptions/ModelsOptions/DoctorOptions (PLAN-CRIT-006); send vs send_batch dispatch (CRIT-M3); this sprint owns the end-to-end REQ criteria the boundary sprints list in does_not_close.
Round 2 (2026-11-09): ADR-SAG-0004 amendment approved by the owner (open-question lines removed, ARCH-002); ADR-SAG-0002 added; images/long-context input, REQ-SAG-0101/0102/0602/0001 AC2/0007/0004 AC3/0801 tests, Context seams, Python logger callables, signals (REQ-SAG-0011), REQ-SAG-0012/0013/0014 end to end, typed options (RBP-F010), cause -> cause_detail.
- QA-1 (2026-11-10): a local model that fails to load surfaces as decide.decision (cause decision.backend -> local.*), not decide.local, because A-6 LocalProbe::wait_until_loaded returns DecisionError; the chain is unchanged (REQ-SAG-0002). decide.local is a LocalError from building a local runner's backend. SIGINT and SIGTERM both exit 130 (ctrlc does not report which signal fired; REQ-SAG-0011 only requires non-zero). ADR-SAG-0005 listed as the packaging pattern for sc-agent-py.
- QA-2 (2026-11-10): the signal hook exits at once only before any logger is open; from `logs_opened` (just before send, send --batch, models or doctor opens its general log) every interrupt only cancels (idempotent), so each exit goes through drain, flush, close, exit 130 (REQ-SAG-0011, NFR-SAG-0001). No environment variable names a configuration directory or a log root: `Context.config_dir` (with the Python `config_dir` option and the CLI flag `--config-dir`, QA-3) is the seam; tests never set HOME or USERPROFILE. Owner decision 2026-11-10: `<log_root>` defaults to `<home>/.sc-agent` on every platform and every log goes under `<log_root>/logs/` (general `<log_root>/logs/<cli>.log.jsonl`, session `<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl`; REQ-SAG-0003 0.2.0). `Context.clock` reaches `DecisionRunner::with_clock`. The send APIs hand each result to `on_result` before the session flush; callers open and close the loggers (QA-006). Typed options (`RunnerName`, `LimitKind`), boxed error variants and `UnknownRunner` follow the code.
