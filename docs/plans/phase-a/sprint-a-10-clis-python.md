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
      artifact: sc_decision_systemone::{SystemOneBackend, TYPESAFE_BASE_URL, models_request} and sc_decision_local::{LocalBackend (incl. with_log), DaemonKind}
      trigger: dev-push
    - sprint: a-7
      artifact: sc_llm_provider::{LlmClient (incl. with_log, models_request, decode_models), LlmProvider, catalog_model_id} and sc_decision_llm::LlmDecisionBackend (incl. with_log)
      trigger: dev-push
    - sprint: a-9
      artifact: sc_decision_runner::{DecisionRunner, RunnerConfig, RunnerError}
      trigger: dev-push
  requirements: [NFR-SAG-0001, REQ-SAG-0201, REQ-SAG-0001, REQ-SAG-0002, REQ-SAG-0003, REQ-SAG-0004, REQ-SAG-0007, REQ-SAG-0008, REQ-SAG-0009, REQ-SAG-0010, REQ-SAG-0011, REQ-SAG-0012, REQ-SAG-0013, REQ-SAG-0014, REQ-SAG-0015, REQ-SAG-0101, REQ-SAG-0102, REQ-SAG-0103, REQ-SAG-0104, REQ-SAG-0401, REQ-SAG-0501, REQ-SAG-0503, REQ-SAG-0504, REQ-SAG-0601, REQ-SAG-0602, REQ-SAG-0701, REQ-SAG-0801, REQ-SAG-0901]
  adrs: [ADR-SAG-0001, ADR-SAG-0002, ADR-SAG-0004]
  does_not_close:
    - "Discovery is a library capability of sc-decision-local; no decide or Python operation calls it in this phase (REQ-SAG-0801 requires only the library report)."
    - "tsq and REQ-SAG-0303: Sprint A-3."
```

# Sprint A-10: decide CLI and library, sc-agent Python API (bead agt-4p2.10)

Copy script: `docs/plans/phase-a/copy/a-10.sh` (dry run 2026-11-09 after the tsq rows moved to a-3.sh: 9 copy steps, 4 files, 210 lines before trimming).
Source: sc-agent-ref @ dce239a (`crates/sc-ai`, `crates/sc-jev-host/src/stream`, and the `Parsed`/`parse()` pattern from `crates/sc-treesitter/src/cli/mod.rs`).
Targets: `crates/decide/` (new lib + bin crate) and `crates/sc-agent-py/` (new PyO3 crate, the sc-agent Python API).
Governing records: REQ-SAG-0501 and REQ-SAG-0503 (under `docs/decide/requirements/`, naming the CLI `decide`), REQ-SAG-0504 (a bad batch line stops decide before any send), REQ-SAG-0003 (each CLI writes its own general log), REQ-SAG-0008 (optional logger injection with defaults, in Rust and in Python), REQ-SAG-0011 (interrupt cancels cleanly), REQ-SAG-0012 (provider-wide busy pause, end to end), REQ-SAG-0013 (config bounds, end to end), REQ-SAG-0014 (input size limits), REQ-SAG-0015 (local runners: own request timeout, default 300 s, and load timeout, default 600 s, from the runner's config entry; load time never counts toward the request timeout or the 90 s budget), REQ-SAG-0101/0102 (short names through the config-built catalog), REQ-SAG-0601 (busy resend, end to end), REQ-SAG-0602 (quota passed through), REQ-SAG-0004 (CLI and PyO3 accept project and agent_role, one field for role and name; slugs; reject, do not rewrite), REQ-SAG-0103 (provider and model separate; provider chosen by ranking, never required), REQ-SAG-0104 AC2 (cancel from Python), REQ-SAG-0401 (keys: env, repo-root .env, keyring), REQ-SAG-0801 (a local runner selected by its configured name), REQ-SAG-0901, REQ-SAG-0701 (batch through the runner), REQ-SAG-0001 AC2 (no UUID anywhere in the finished workspace), REQ-SAG-0002, REQ-SAG-0007, REQ-SAG-0009 / owner-lowlevel-no-tokio, ADR-SAG-0001 (one sync `Decision` trait; `DecisionRequest` — the Jev body plus image parts and the long-context flag — is the one request form), ADR-SAG-0002 (log layout: session rows under `<log_root>/<repo>/<agent_role>/<project>/sessions.jsonl`), ADR-SAG-0004 (decide is the top layer and the composition root; sc-agent-py depends on decide). Anchors: ANC-006 (every decide operation is also in the sc-agent Python API), ANC-007.
Rule for the binary: argument parsing and output only. All behaviour lives in the libraries (A-1, A-2, A-4..A-9); decide's own logic (choosing the backend, wiring `DecisionRunner`, models, doctor, input limits, the signal hook) is in decide's library target, which the `decide` binary and sc-agent-py share.

Decisions (assistant, 2026-11-09; round 2 owner decisions folded in):
- Names: binary and package `decide` (crates.io availability checked at publish time); subcommands `send`, `models`, `doctor`.
- `--project` and `--agent-role` are required, no default; there is no `--agent-name`.
- Provider: `--provider` is an optional override. Absent: the first provider in `Provider::ranking(line of the model)` (A-1) whose key `resolve` detects (A-4); never required (REQ-SAG-0103).
- A batch is read and validated whole before anything is sent; the first bad line stops decide (REQ-SAG-0504).
- Exit: 0 when the command ran and every result succeeded; 1 otherwise; 130 after any interrupt (REQ-SAG-0011; `ctrlc` does not distinguish SIGINT from SIGTERM, so there is no 143).
- `send` without `--batch` always calls `Decision::decide` (one request); `send --batch` always calls `Decision::decide_stream`, even for a one-line batch (CRIT-M3).
- Images and long context (REQ-QA-001, ADR-SAG-0001): `decide send --image NAME=FILE` (repeatable; format from the extension `.png`/`.jpg`/`.jpeg`) and `--long-context`. A batch line is either a bare Jev request or `{"request": {...}, "images": {"NAME": "path"}, "long_context": true}` (paths relative to the working directory). Every request is built with `DecisionRequest::new(body, image_parts, long_context)` (A-2), which checks the parts and the REQ-SAG-0014 image limits.
- Input limits (REQ-SAG-0014): one request (file, stdin or one batch line) at most 8 MiB; a batch at most 10,000 lines and 256 MiB; each image at most 20 MiB (checked from file metadata before reading) and at most 20 images per request (A-2's `DecisionRequest::new`). Over a limit: `DecideError::LimitExceeded { what, line, max }` (or `RequestInvalid` wrapping A-2's `DecisionError::Limits`); nothing is sent.
- Typed options (RBP-F010): `SendOptions` takes `ModelSelector`, `Option<Provider>` and `Slug`s; the CLI and Python parse strings at the edge with `ModelSelector::parse` / `Provider::from_str` / `Slug::new`.
- Seams (REQ-SAG-0008, REQ-QA-009): every options struct holds a `Context` with `config_dir`, `environment`, `keyring`, `transport` and `general` (all optional; `None` = the real default), and `SendOptions` adds `session`. Tests never touch the process environment or HOME: they pass a tempdir `config_dir`, an explicit `environment` Vec, a `MapKeyring`-style keyring, a `ScriptedTransport`/`BlockingTransport` (`sc_agent_http::testing`) and a `MemoryLog` (`let log = MemoryLog::new()?;` then `general: Some(log.general())`, `session: Some(log.session(identity))`; assertions read `log.records()?`, `MemoryRecord { level, target, action, fields }` in write order), and go through the real `DecisionRunner`. Fake `Decision` impls are local to crates/decide's tests.
- Signals (REQ-SAG-0011, RSH-004): `decide::signal::install(&cancel)` registers a handler with the `ctrlc` crate, feature `termination` (SIGINT and SIGTERM on Unix, Ctrl-C and Ctrl-Break on Windows; sync, no tokio, no `unsafe`); ctrlc starts and owns the handler thread. On an interrupt the handler cancels the token. The normal path then returns `decision.cancelled` for every unfinished request, the stream is drained and printed, `flush_session_log` runs, and main exits 130.
- Python interrupts (REQ-SAG-0011, RSH-005): sc-agent-py runs the decide call on a scoped worker thread and waits on its result channel with `recv_timeout(100 ms)` (a timed wait, not sleep) outside the GIL; after each timeout it reacquires the GIL and calls `py.check_signals()`. `KeyboardInterrupt` (or any signal exception) cancels the token, drains the remaining results, then re-raises. An exception raised by `on_result` does the same: cancel, drain, re-raise.
- Python loggers (REQ-SAG-0008, owner 2026-11-09): `general_log` and `session_log` keyword arguments take `Callable[[dict], None]`; each becomes an sc-observability `LogSink` (`PyCallableSink`) that calls the callable with the GIL, wrapped as `GeneralLogger::injected` / `SessionLogger::injected`. A callable that raises makes that write a refused write (counted by sc-agent-log, reported by `flush`); it never changes a decision result.
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
- `cli.rs`: rewrite `Cli`/`Command` (section 3.2); `parse()` maps clap errors to `DecideError::Usage`; value parsers return `Result<_, String>`; message text "See decide --help.".
- `input.rs` (library): `request_bytes` loses `RepoRoot`/`joined_path` (paths are relative to the working directory), reads at most `MAX_REQUEST_BYTES + 1` bytes (`Read::take`) and returns `LimitExceeded` past the cap; errors -> `InputNotFound`/`InputRead`; `read_line_bounded`: replace `&available[..take]` with `available.get(..take)` (indexing_slicing); its `max` is `MAX_REQUEST_BYTES`; wrap the copied tests in `#[cfg(test)] mod tests { use super::*; use std::io::Cursor; ... }`.
- `main.rs`: `print_stdout`/`emit` take `DecideError`; `emit` writes `DecideError::detail()` as one JSON line and must not `.expect`: when encoding fails it writes a fixed literal `{"id":"decide.encode","message":"..."}` line.

## 3. Rust code (exact; bodies elided)

### 3.2 decide (`crates/decide/`)

```rust
// src/cli.rs
#[derive(Debug, clap::Parser)]
#[command(name = "decide", bin_name = "decide", version, about = "Send TypeSafe (Jev) decision requests.")]
pub(crate) struct Cli { #[command(subcommand)] pub(crate) command: Command }

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
    #[arg(long, value_name = "NAME", value_parser = non_empty_text)]
    pub(crate) runner: Option<String>,
}

#[derive(Debug)]
pub(crate) enum Parsed { Run(Cli), Print(String) }
pub(crate) fn parse<I, T>(args: I) -> Result<Parsed, DecideError>
where I: IntoIterator<Item = T>, T: Into<std::ffi::OsString> + Clone;
fn non_empty_text(value: &str) -> Result<String, String>;
fn parse_image_arg(value: &str) -> Result<(String, std::path::PathBuf), String>;   // split at the first '='; both sides non-empty

// src/input.rs (library; REQ-SAG-0014 limits)
pub const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;        // one request, one batch line
pub const MAX_BATCH_LINES: u64 = 10_000;
pub const MAX_BATCH_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;           // also enforced by DecisionRequest::new (A-2)
pub fn request_bytes(spec: &str, stdin: &mut dyn std::io::Read) -> Result<Vec<u8>, DecideError>;
pub fn open_lines(spec: &str, stdin: Box<dyn std::io::BufRead + Send>) -> Result<Box<dyn std::io::BufRead + Send>, DecideError>;
/// Reads one image file: metadata length > MAX_IMAGE_BYTES -> LimitExceeded before reading; extension -> ImageFormat
/// (else ImageFormat error); io errors -> ImageRead.
pub fn read_image(name: &str, path: &std::path::Path) -> Result<sc_decision_schema::ImagePart, DecideError>;
/// Builds a DecisionRequest from JSON bytes, image files and the flag: SystemOneRequest::from_json, read_image for each,
/// DecisionRequest::new(body, parts, long_context) (A-2 checks parts and image count/size); errors -> RequestInvalid { line, .. }.
pub fn build_request(json: &[u8], images: &[(String, std::path::PathBuf)], long_context: bool, line: Option<u64>) -> Result<sc_decision_schema::DecisionRequest, DecideError>;
/// The in-memory form (sc-agent-py): json.len() > MAX_REQUEST_BYTES -> LimitExceeded { what: "request_bytes" }; each image
/// > MAX_IMAGE_BYTES -> LimitExceeded { what: "image_bytes" }; format from magic bytes (PNG 89 50 4E 47, JPEG FF D8 FF; else
/// ImageFormat); then DecisionRequest::new as above. build_request reads the files and calls this.
pub fn build_request_bytes(json: &[u8], images: Vec<(String, Vec<u8>)>, long_context: bool, line: Option<u64>) -> Result<sc_decision_schema::DecisionRequest, DecideError>;
/// One in-memory batch item (sc-agent-py): JSON text, image bytes by name, long-context flag.
pub struct BatchInput { pub json: String, pub images: Vec<(String, Vec<u8>)>, pub long_context: bool }
/// The in-memory batch path: > MAX_BATCH_LINES items or > MAX_BATCH_BYTES total (json plus image bytes) -> LimitExceeded
/// { what: "batch_lines" | "batch_bytes", line } before any item is built; each item through build_request_bytes with
/// line = its 1-based index. Same limits and errors as read_batch, which applies the same counters to file lines.
pub fn build_batch(items: Vec<BatchInput>) -> Result<Vec<sc_decision_schema::DecisionRequest>, DecideError>;
enum LineRead { End, Line, TooLong }
fn read_line_bounded<R: std::io::BufRead>(reader: &mut R, max: usize, out: &mut Vec<u8>) -> std::io::Result<LineRead>;
/// One batch line: a bare Jev request, or this wrapper (chosen by the presence of the "request" key).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchLine { request: Box<serde_json::value::RawValue>, #[serde(default)] images: OrderedMap<String, std::path::PathBuf>, #[serde(default)] long_context: bool }
/// Reads and validates the whole JSONL batch before anything is sent (REQ-SAG-0504, REQ-SAG-0014). Blank lines and a
/// leading BOM are skipped. The first line that is too long, past MAX_BATCH_LINES / MAX_BATCH_BYTES, not UTF-8, or not a
/// valid request returns LimitExceeded / LineNotUtf8 / RequestInvalid naming that line, the field and the reason; nothing is sent.
pub fn read_batch(reader: &mut dyn std::io::BufRead) -> Result<Vec<sc_decision_schema::DecisionRequest>, DecideError>;

// src/lib.rs: decide's library target, shared by the decide binary and sc-agent-py
pub mod input; pub mod output; pub mod signal; mod doctor; mod error; mod select;
pub use doctor::{doctor, DetectedKey, DoctorOptions, DoctorReport, MissingModel, Tie};
pub use error::DecideError;
pub use select::{load_config, models, send, send_batch, send_batch_on, send_on, session_identity, Context, ModelSelector, ModelsOptions, SendOptions};

// src/select.rs  (the one place a backend is chosen; ADR-SAG-0004 composition root)
/// Shared seams; every field None = the real default (REQ-SAG-0008).
#[derive(Clone)]
pub struct Context {
    pub service: &'static str,                                                  // "decide" | "sc-agent-py"
    pub config_dir: Option<std::path::PathBuf>,                                 // None -> sc_agent_config::default_dir()
    pub environment: Option<Vec<(std::ffi::OsString, std::ffi::OsString)>>,     // None -> std::env::vars_os(); SC_AGENT__ overrides and key lookup
    pub keyring: Option<Arc<dyn sc_agent_secrets::KeyringSource + Send + Sync>>, // None -> the platform keyring
    pub transport: Option<Arc<dyn sc_agent_http::Transport>>,                   // None -> UreqTransport::new(Timeouts::default())
    pub general: Option<sc_agent_log::GeneralLogger>,                           // None -> GeneralLogger::open_default(log_root, service)
}
impl Context { pub fn new(service: &'static str) -> Self; }   // every seam None
/// What --model names; parsed once at the edge against the loaded config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSelector { Short(ShortName), Version(ModelId), Runner(String) }   // Runner: a key of config.local_runners
impl ModelSelector {
    /// A ShortName if it parses; else a configured local runner name; else a ModelId; else decide.types.
    /// (Catalog::resolve is infallible for both ShortName and ModelId, A-1; no error arm.)
    pub fn parse(text: &str, config: &AgentConfig) -> Result<Self, DecideError>;
}
#[derive(Clone)]
pub struct SendOptions {
    pub model: ModelSelector,
    pub provider: Option<Provider>,
    pub project: sc_agent_log::Slug,
    pub agent_role: sc_agent_log::Slug,
    pub context: Context,
    pub session: Option<sc_agent_log::SessionLogger>,           // None -> SessionLogger::open_default(log_root, identity)
}
#[derive(Clone)]
pub struct ModelsOptions { pub runner: Option<String>, pub context: Context }   // runner: a configured local runner; TypeSafe when None
/// sc_agent_config::load(context.config_dir or default_dir()?, context.environment or vars_os()); errors -> Config.
pub fn load_config(context: &Context) -> Result<AgentConfig, DecideError>;
/// SessionIdentity::new(Repo::detect(cwd)?, project, agent_role); used by send and by sc-agent-py for SessionLogger::injected.
pub fn session_identity(project: &sc_agent_log::Slug, agent_role: &sc_agent_log::Slug) -> Result<SessionIdentity, DecideError>;
/// Runner -> LocalBackend (DaemonKind from the runner's RunnerKind, key via resolve_var(key_env)) with_log(general);
/// else model = catalog.resolve(ModelChoice) (catalog = config.catalog(), so config models are seen, REQ-SAG-0101/0102);
/// provider = options.provider, else the first of Provider::ranking(line) whose key resolve() finds with
/// KeySources { env: environment, repo_root: Repo::work_tree_root(cwd)?, keyring }; none -> NoProviderKey.
/// TypeSafe -> SystemOneBackend::new(TYPESAFE_BASE_URL, key); any other provider -> LlmDecisionBackend(LlmClient::new(LlmProvider, key).with_log(general), model).with_log(general).
fn backend(options: &SendOptions, config: &AgentConfig, general: &GeneralLogger) -> Result<Arc<dyn Backend>, DecideError>;
/// transport = context.transport, else for a local runner UreqTransport::new(Timeouts::default().with_request(runner.request_timeout()))
/// (one transport per local runner, REQ-SAG-0015, default 300 s), else UreqTransport::new(Timeouts::default()); HttpClient::new(transport, ResendPolicy::new(config.max_sends), general.clone())
/// (one HttpClient per runner, so the REQ-SAG-0012 provider pause is shared by every worker);
/// DecisionRunner::new(backend, http, session, general, RunnerConfig::from_config(config)).
fn runner(config: &AgentConfig, context: &Context, session: SessionLogger, general: GeneralLogger, backend: Arc<dyn Backend>) -> Result<DecisionRunner, DecideError>;
/// Always `Decision::decide` (plain send); then flush_session_log; writes one `decide.send` general record.
pub fn send(options: &SendOptions, request: DecisionRequest, cancel: &CancelToken) -> Result<DecisionResult, DecideError>;
/// Always `Decision::decide_stream(Box::new(requests.into_iter()), cancel)` (an owned 'static iterator), even for one
/// request; `on_result` gets each DecisionResult as it finishes; an Err from on_result cancels the token, the rest is
/// drained (and dropped), flush_session_log runs, and that Err is returned.
pub fn send_batch(options: &SendOptions, requests: Vec<DecisionRequest>, cancel: &CancelToken, on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>) -> Result<(), DecideError>;
/// Testable cores: any Decision (the runner in send/send_batch, a fake in tests).
pub fn send_on(decision: &dyn Decision, request: DecisionRequest, cancel: &CancelToken) -> DecisionResult;
pub fn send_batch_on(decision: &dyn Decision, requests: Vec<DecisionRequest>, cancel: &CancelToken, on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>) -> Result<(), DecideError>;
/// GET /v1/models of TypeSafe or a configured runner, through HttpClient::send_once with Timeouts::probe() (A-5);
/// writes one `decide.models` general record.
pub fn models(options: &ModelsOptions, cancel: &CancelToken) -> Result<ModelMetadataList, DecideError>;

// src/signal.rs (REQ-SAG-0011; the `ctrlc` crate with feature `termination`: SIGINT + SIGTERM on Unix,
// Ctrl-C/Ctrl-Break on Windows; sync, no tokio, no unsafe in decide)
pub struct SignalWatch { /* interrupted: Arc<AtomicBool> */ }
/// ctrlc::set_handler(move || on_interrupt(&cancel, &interrupted)); ctrlc starts and owns its handler thread
/// (decide's source spawns none). Err (a handler already set) -> DecideError::Signal.
pub fn install(cancel: &CancelToken) -> Result<SignalWatch, DecideError>;
/// What the handler does: set `interrupted`, cancel the token. Unit-tested directly.
fn on_interrupt(cancel: &CancelToken, interrupted: &AtomicBool);
impl SignalWatch {
    /// True once any interrupt was received; main then exits 130. ctrlc does not report which signal fired,
    /// so SIGINT, SIGTERM, Ctrl-C and Ctrl-Break all exit 130 (no 143).
    pub fn interrupted(&self) -> bool;
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
#[derive(Clone)]
pub struct DoctorOptions { pub context: Context }
/// Model lists fetched with Timeouts::probe(); writes one `decide.doctor` general record.
pub fn doctor(options: &DoctorOptions, cancel: &CancelToken) -> Result<DoctorReport, DecideError>;

// src/output.rs (library)
#[derive(serde::Serialize)]
pub struct ResultLine<'a> {
    pub session_id: &'a SessionId,
    pub position: u64,
    pub digests: &'a OrderedMap<String, QuestionDigest>,
    #[serde(skip_serializing_if = "Option::is_none")] pub response: Option<&'a SystemOneResponse>,
    #[serde(skip_serializing_if = "Option::is_none")] pub error: Option<ErrorDetail>,   // DecisionError::detail(): every lower id, message and recovery
}
pub fn result_line(result: &DecisionResult) -> Result<Vec<u8>, DecideError>;

// src/main.rs (bin: argv -> decide::{send, send_batch, models, doctor} -> stdout)
fn main() -> std::process::ExitCode;   // install the SignalWatch first; exit 130 when it was interrupted
fn run(parsed: Parsed, cancel: &CancelToken, stdin: &mut dyn std::io::Read, stdout: &mut dyn std::io::Write) -> Result<std::process::ExitCode, DecideError>;
fn print_stdout(bytes: &[u8]) -> std::process::ExitCode;
fn emit(error: &DecideError) -> std::process::ExitCode;     // one stderr line: error.detail() as JSON, `recovery` included when present
```

Types used: `SessionId`, `ErrorId` (incl. `cause_detail()`, provided `recovery()`), `ErrorDetail` (incl. `recovery: Option<Cow<'static, str>>`), `VarName`, `Catalog`, `ModelChoice`, `ModelId`, `ShortName`, `Provider` (incl. `PRIORITY`, `ranking`, `FromStr`), `MODELS`, `CancelToken` (A-1); `AgentConfig` (incl. `catalog`, `local_runners`, `RunnerKind`), `load`, `default_dir` (A-4); `KeySources`, `KeySource`, `KeyringSource`, `EnvSource`, `ResolvedKey`, `resolve`, `resolve_var` (A-4); `HttpClient`, `ResendPolicy`, `Transport`, `UreqTransport`, `Timeouts` (incl. `probe`) (A-5); `GeneralLogger`, `SessionLogger` (incl. `injected`), `Slug`, `Repo` (incl. `work_tree_root`), `SessionIdentity` (A-8); `Decision`, `Backend`, `DecisionRequest` (incl. `new`), `ImagePart`, `ImageFormat`, `DecisionResult`, `SystemOneRequest`, `SystemOneResponse`, `OrderedMap`, `QuestionDigest`, `ModelMetadataList` (A-2); `SystemOneBackend`, `TYPESAFE_BASE_URL` (A-6); `LocalBackend`, `DaemonKind` (A-6); `LlmClient` (incl. `models_request`, `decode_models`), `LlmProvider`, `catalog_model_id`, `LlmDecisionBackend` (A-7); `DecisionRunner`, `RunnerConfig` (A-9).

Behaviour:
- `send` (no `--batch`): `request_bytes` (8 MiB cap) -> `build_request(json, --image list, --long-context, None)` -> `decide::send` (always `Decision::decide`) -> one `ResultLine` + `\n`. A failed decision is data in its result (owner-decision-trait), printed on stdout.
- `send --batch`: `read_batch` reads and validates every line first (limits, schema, images); a bad line returns its error (line, field, reason) before anything is sent (REQ-SAG-0504). Then `decide::send_batch`; each result printed as one JSONL line as it arrives (fastest first, session ULID and input position); a failed request is data in its line and the batch continues. A stdout write failure cancels the token and returns `Write`. After the stream, `runner.flush_session_log()`; an Err is a command error (exit 1) printed after the results, never replacing them.
- Local runner load (REQ-SAG-0015): before the first request of a send or batch to a local runner, decide calls `LocalBackend::wait_until_loaded(probe_transport, &cancel, runner.load_timeout(), ctx)` (A-6) once, with `probe_transport` = the context transport or `UreqTransport::new(Timeouts::probe())`; a cancel ends the wait at once (`decision.cancelled` for every request, nothing sent); a load that does not finish within `load_timeout()` (default 600 s) is the A-6 error, surfaced as `decide.local`, and nothing is sent. Only then does the batch start, so load time never counts toward a request's own timeout or the 90 s budget (the budget starts at the first send, A-5). For daemons that load on the first request, the first request is built with `HttpRequest::with_deadline(self, Duration)` (A-5; replaces the transport's request timeout for that request's sends only), and A-6 decides which request gets it and the duration (load timeout plus request timeout).
- Signals: SIGINT/SIGTERM cancel the token; unfinished requests end `decision.cancelled` and are printed; `flush_session_log` runs; exit 130 (REQ-SAG-0011).
- Session identity: the CLI and sc-agent-py parse `--project`/`--agent-role` with `Slug::new` before anything is built; an invalid value is `decide.log` (cause `log.invalid_slug`), nothing is sent and no row is written (REQ-SAG-0004). `repo` comes from `Repo::detect` on the working folder.
- `models`: `SystemOneBackend::models_request(..)?` for TypeSafe or the runner's base (returns `Result<HttpRequest, DecisionError>`: `with_secret` refuses a key on plain-http non-loopback; `?` maps it to `decide.decision`); sent with `HttpClient::send_once` over the context transport with `Timeouts::probe()` (a one-off holdoff waits on `CancelToken::wait_until`, A-5); `decode_models`; prints the JSON.
- `doctor` (REQ-SAG-0503): (1) one entry per detected provider key in `Provider::PRIORITY` order with its `KeySource` (`env`, `.env`, `keyring`), never the value; (2) for each provider with a key, its model list (`LlmClient::models_request(..)?` (returns `Result<HttpRequest, LlmError>`; `?` maps it to `decide.llm`) / `decode_models`; TypeSafe through A-6 `models_request(..)?`), each id mapped back with `catalog_model_id`, one `MissingModel { provider, model }` per id `catalog.line_of(&str)` does not know; (3) one `Tie { config, builtin }` per config model whose release date equals a built-in model of its line (the config model wins, A-1). Nothing in (2) or (3) when nothing is missing or tied.
- Config: `load_config(context)`; bound violations (REQ-SAG-0013: workers, max_sends, runner host/kind/port/key_env, key on plain-http non-loopback) are A-4's `AgentConfigError`s with the key path, surfaced as `decide.config` before anything is sent.
- General-log records are written with `GeneralLogger::write(level, target, action, LogContext::default(), fields)` (A-8; decide's own records carry no session; per-request `http.*`/`runner.*` events carry `LogContext { session_id, position }` from A-5/A-9). Records (ANC-007; never a key or key value): `decide.send` (service, model, provider, requests, succeeded, failed, cancelled), `decide.models` (runner or `typesafe`, model count, outcome id), `decide.doctor` (detected key count, missing count, tie count, outcome id). The same `GeneralLogger` goes into `HttpClient::new` (its `http.*` records), `DecisionRunner::new`, `LocalBackend::with_log`, `LlmClient::with_log` and `LlmDecisionBackend::with_log`.
- Exit: 0 when the command ran and every result succeeded; 1 otherwise; 130 after an interrupt. Numbers are printed as serde_json gives them (REQ-SAG-0007). No tokio, no sleep; decide's source starts no thread; the only extra thread is the handler thread `ctrlc` starts and owns.

## 4. Error list

decide (`crates/decide/src/error.rs`, new; ErrorId from A-1, Display = `id: message` only, the chain in `source()`/`detail()`; each wrapped error kept unchanged; `cause_detail` returns the wrapped error's detail):

```rust
#[derive(Debug)]
#[non_exhaustive]
pub enum DecideError {
    /// clap error; kind = Debug of clap::error::ErrorKind (display only), detail = clap's text.
    Usage { kind: String, detail: String },                                       // "decide.usage"
    /// The global configuration could not be loaded or is out of bounds (REQ-SAG-0013; cause config.invalid with the key path, or config.key_on_plain_http).
    Config(sc_agent_config::AgentConfigError),                                     // "decide.config"
    /// --provider or --model does not parse.
    Types(sc_agent_types::TypesError),                                             // "decide.types"
    /// The provider's or runner's key is missing or malformed.
    Secrets(sc_agent_secrets::SecretsError),                                       // "decide.secrets"
    /// Slug invalid (log.invalid_slug), repo detection or logger open failed.
    Log(sc_agent_log::LogError),                                                   // "decide.log"
    /// A local runner's kind cannot be served.
    Local(sc_decision_local::LocalError),                                          // "decide.local"
    /// Building a System One backend or decoding the models list failed.
    Decision(sc_decision_schema::DecisionError),                                   // "decide.decision"
    /// A models or doctor send failed.
    Http(sc_agent_http::HttpError),                                                // "decide.http"
    /// An LLM client could not be built or a provider model list could not be read.
    Llm(sc_llm_provider::LlmError),                                                // "decide.llm"
    /// flush_session_log failed after the results were printed.
    Runner(sc_decision_runner::RunnerError),                                       // "decide.runner"
    /// No provider in Provider::ranking for the model has a detected key (and no --provider was given).
    /// recovery: "set one of the listed providers' key variables, or pass --provider".
    NoProviderKey { model: String, tried: Vec<Provider> },                         // "decide.no_provider_key"
    InputNotFound { input: String },                                               // "decide.input_not_found"
    /// io error other than NotFound; kind = Debug of io::ErrorKind (display only), detail = Display.
    InputRead { input: String, kind: String, detail: String },                     // "decide.input_read"
    /// REQ-SAG-0014: what = "request_bytes" | "batch_lines" | "batch_bytes" | "image_bytes"; line None for a single request.
    LimitExceeded { what: &'static str, line: Option<u64>, max: u64 },             // "decide.limit_exceeded"
    LineNotUtf8 { line: u64, byte_offset: usize },                                 // "decide.line_not_utf8"
    /// An image file could not be read; kind = Debug of io::ErrorKind (display only).
    ImageRead { name: String, path: std::path::PathBuf, kind: String, detail: String },   // "decide.image_read"
    /// An image file's extension is not png, jpg or jpeg.
    ImageFormat { name: String, path: std::path::PathBuf },                        // "decide.image_format"
    /// A request does not match the Jev request schema, or DecisionRequest::new refused it (image parts, image limits);
    /// line = JSONL line (None for a single request); the message names the line, the field and the reason;
    /// source = decision.request_parse / decision.image_parts / decision.limits, kept (REQ-SAG-0504).
    RequestInvalid { line: Option<u64>, source: sc_decision_schema::DecisionError },  // "decide.request_invalid"
    Encode { detail: String },                                                     // "decide.encode"
    /// stdout write failed; kind = Debug of io::ErrorKind (display only).
    Write { kind: String, detail: String },                                        // "decide.write"
    /// The interrupt handler could not be installed (ctrlc::Error Display).
    Signal { detail: String },                                                     // "decide.signal"
}
impl ErrorId for DecideError {}   // recovery(): Some(..) for NoProviderKey; the wrapped errors carry their own
```

21 variants (round 2: `LineTooLong` folded into `LimitExceeded`; `ImageRead`, `ImageFormat` and `Signal` added). A decision that fails inside `decide`/`decide_stream` is not a `DecideError`: it is the `DecisionError` inside that `DecisionResult`, printed in its `ResultLine.error`.

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
tempfile = "3"

[lib]
name = "decide"
path = "src/lib.rs"

[[bin]]
name = "decide"
path = "src/main.rs"
```

### 3.3 sc-agent Python API (`crates/sc-agent-py/`, ANC-006)

Package `sc-agent-py` (`publish = false`), `[lib] name = "sc_agent", crate-type = ["cdylib"]`; maturin wheel `sc-agent`, module `sc_agent`. Deps: `decide` (path, lib), `sc-agent-types`, `sc-agent-log`, `sc-decision-schema`, `sc-observability = "=1.5.0"` (the `LogSink` trait), `pyo3 = { version = "=0.29.2", features = ["abi3-py310", "extension-module"] }`, `serde_json`. Same output shapes as the CLI (JSON text). The default general log service is `sc-agent-py`. Strings are parsed at the edge (`ModelSelector::parse`, `Provider::from_str`, `Slug::new`); errors raise `SagError`. Requests go through the same limit checks as the CLI (REQ-SAG-0014): `send` calls `decide::input::build_request_bytes` and `send_batch` calls `decide::input::build_batch`, so in-memory strings and byte images meet the 8 MiB request, 10,000-item / 256 MiB batch and 20 MiB / 20-image limits, raising `SagError` `decide.limit_exceeded` (or `decide.request_invalid` with cause `decision.limits`).

```rust
#[pyclass(module = "sc_agent", frozen)] pub struct CancelToken(sc_agent_types::CancelToken);   // new(), cancel(), is_cancelled()
create_exception!(sc_agent, SagError, PyException);   // args = (id, message, detail JSON with every cause_detail and recovery when present)
/// A batch item: a JSON request string, or (request, images, long_context).
#[derive(FromPyObject)]
enum BatchItem { Bare(String), Full(String, Option<std::collections::BTreeMap<String, Vec<u8>>>, bool) }
#[pyfunction]
#[pyo3(signature = (request, *, model, project, agent_role, provider = None, images = None, long_context = false, cancel = None, general_log = None, session_log = None))]
fn send(py: Python<'_>, request: &str, model: &str, project: &str, agent_role: &str, provider: Option<&str>,
        images: Option<std::collections::BTreeMap<String, Vec<u8>>>, long_context: bool, cancel: Option<&CancelToken>,
        general_log: Option<PyObject>, session_log: Option<PyObject>) -> PyResult<String>;   // one ResultLine
#[pyfunction]
#[pyo3(signature = (requests, *, model, project, agent_role, provider = None, cancel = None, on_result = None, general_log = None, session_log = None))]
fn send_batch(py: Python<'_>, requests: Vec<BatchItem>, model: &str, project: &str, agent_role: &str, provider: Option<&str>,
              cancel: Option<&CancelToken>, on_result: Option<PyObject>, general_log: Option<PyObject>, session_log: Option<PyObject>) -> PyResult<Vec<String>>;   // ResultLines in finish order; on_result(line) as each arrives
#[pyfunction] #[pyo3(signature = (runner = None, cancel = None, general_log = None))]
fn models(py: Python<'_>, runner: Option<&str>, cancel: Option<&CancelToken>, general_log: Option<PyObject>) -> PyResult<String>;
#[pyfunction] #[pyo3(signature = (cancel = None, general_log = None))]
fn doctor(py: Python<'_>, cancel: Option<&CancelToken>, general_log: Option<PyObject>) -> PyResult<String>;   // DoctorReport JSON
#[pymodule] fn sc_agent(module: &Bound<'_, PyModule>) -> PyResult<()>;

/// Calls a Python `Callable[[dict], None]` with each record (fields plus level, target, action) under the GIL.
/// A raised exception is a refused write (counted, reported by flush); never changes a result.
struct PyCallableSink { callable: Py<PyAny> }
impl sc_observability::v2::LogSink for PyCallableSink {}
/// Logger built like MemoryLog (file and console sinks off, SinkRegistration::typed(PyCallableSink)) ->
/// GeneralLogger::injected / SessionLogger::injected(logger, decide::session_identity(..)?).
fn callable_logger(callable: Py<PyAny>) -> Result<Arc<sc_observability::v2::Logger>, SagError>;
/// Runs `work` on a std::thread::scope worker; the Python thread waits on the result channel with
/// recv_timeout(100 ms) inside py.allow_threads, then py.check_signals() with the GIL. A signal exception or an
/// on_result exception cancels the token, drains the channel, joins, and re-raises (REQ-SAG-0011).
fn run_interruptible<T: Send>(py: Python<'_>, cancel: &sc_agent_types::CancelToken, work: impl FnOnce(&mut dyn FnMut(DecisionResult) -> Result<(), DecideError>) -> Result<T, DecideError> + Send, on_result: Option<&PyObject>) -> PyResult<T>;
```

decide subcommand -> Python: `send` -> `send` (`decide::send`; `--image` -> `images`, `--long-context` -> `long_context`), `send --batch` -> `send_batch` (`decide::send_batch`; batch-line wrapper -> `BatchItem::Full`), `models` -> `models`, `doctor` -> `doctor`. The Python API is a superset where useful (ANC-006): image bytes may be passed directly, and loggers may be injected.
Transport from Python: no Python transport seam (the context transport is None -> `UreqTransport`); tests that need a scripted transport are Rust tests in crates/decide. Python tests use a loopback runner from a tempdir config (`HOME` set before import) and no external network.

No tokio, no mimalloc. Dev: `tempfile 3`.

## 6. New code

- decide: `src/error.rs` (~150 lines), `src/select.rs` (~160), `src/signal.rs` (~30), `src/doctor.rs` (~80), `src/output.rs` (~40), `read_batch`/`open_lines`/`read_image`/`build_request` in `src/input.rs` (~110), `run`/`main` (~90), `Cli` (~80).
- sc-agent-py: `src/lib.rs` (~260: functions, `BatchItem`, `PyCallableSink`, `run_interruptible`), `pyproject.toml`, `tests/test_api.py`, `tests/loopback.py` (a TCP server that accepts and never answers, or answers one fixed error).

## 7. Tests

Each test names the acceptance line it proves (`AC: <line key>`). "Real runner" = `decide::send`/`send_batch` with a `Context` holding a tempdir `config_dir`, an explicit `environment`, a map keyring, a `ScriptedTransport` (or `BlockingTransport`) and `MemoryLog` (`general: Some(log.general())`, `session: Some(log.session(identity))`), going through the real `DecisionRunner` and backend; no network, process env and HOME untouched.

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
14. A run with `config_dir` whose default.json sets `log_root` to a tempdir writes `<log_root>/logs/decide.log.jsonl` and the session row under `<log_root>/<repo>/<agent_role>/<project>/sessions.jsonl` (AC: REQ-SAG-0003, ADR-SAG-0002).
15. 40-request `send_batch` over a `BlockingTransport`: 8 sends in flight at once with the default workers, 1 with workers 1 (AC: REQ-SAG-0701 parallel).
16. Busy end to end: transport answers 529 (no Retry-After) every time -> 5 sends and `http.provider_busy` (reason `sends`, 5 attempts); `SC_AGENT__MAX_SENDS=2` in `environment` -> 2 sends (AC: REQ-SAG-0601).
17. Plain `send` calls `decide`, `send --batch` with one line calls `decide_stream` (fake `Decision` counting calls; AC: CRIT-M3 dispatch).
18. MemoryLog injected; one send, models and doctor; `log.records()?` holds `decide.send`, `decide.models`, `decide.doctor` and `http.send` actions; no record's `fields` holds a key value (AC: ANC-007).
19. `grep -rn 'thread::spawn' crates/decide/src` finds nothing; the ctrlc handler thread is ctrlc's, not decide's (AC: REQ-SAG-0701 output).
20. `send` over a `BlockingTransport` held by the test; cancel the token; the `decision.cancelled` result arrives on a channel within the 2 s bound; releasing the transport yields no further result (AC: REQ-SAG-0104 AC2).
21. `--model haiku` through a tempdir default.json with a later-dated `haiku` model: the transport sees that model; without it, the built-in (AC: REQ-SAG-0101, REQ-SAG-0102).
22. Anthropic 429 with the quota body: one send, result `http.quota_exhausted` with the remote message and its recovery (AC: REQ-SAG-0602).
23. A tempdir default.json `local_runners.laya` (host 127.0.0.1, port 9000): `--model laya` builds a LocalBackend and the transport sees `http://127.0.0.1:9000` (AC: REQ-SAG-0801).
24. Two parallel requests to one provider through `decide::send_batch` (default workers); the `ScriptedTransport` answers 429 `Retry-After: 1` to the first send and 200 after; the transport's recorded send times show no later send to that host (the other request's included) before the pause ends, because the pause lives in the one `HttpClient` the runner shares; with `Retry-After: 0` the pause still lasts at least the 250 ms floor. Real clock, recorded send times: the runner's clock seam is crate-private to sc-decision-runner (its tests use `sc_agent_http::testing::ManualClock`), so decide cannot supply a ManualClock; the test waits about 1.25 s (AC: REQ-SAG-0012).
25. default.json with `workers: 0`, `max_sends: 11`, a runner `port: 0`, and `key_env` on `http://10.0.0.5`: each -> `decide.config` (cause `config.invalid` naming the key path, or `config.key_on_plain_http`) and no send (AC: REQ-SAG-0013).
26. Limits: a 9 MiB single request, a 10,001-line batch, a line past 8 MiB, a 21 MiB image, 21 images -> `decide.limit_exceeded` / `decide.request_invalid` (cause `decision.limits`) naming the limit; nothing sent (AC: REQ-SAG-0014).
27. Unit: `on_interrupt` during a `send_batch` over a `BlockingTransport` cancels the token; every unfinished result is `decision.cancelled` and the session rows are flushed (MemoryLog has one row per started request). Process (Unix CI): a `decide send --batch` child against a loopback runner that never answers, sent SIGINT and, in a second run, SIGTERM (`kill -INT` / `kill -TERM`): every line printed is `decision.cancelled`, the rows are in sessions.jsonl, exit code 130 both times (AC: REQ-SAG-0011 decide).
30. Local runner load (REQ-SAG-0015): a scripted local daemon (ScriptedTransport via the Context) answers the load probe not-loaded twice, then loaded; `send --model laya` makes the probes, then one send, and the send's deadline (recorded by the transport) is the runner's request timeout measured from after the load, not from the first probe; a runner entry with `request_timeout: 300`/`load_timeout: 600` defaults applies when absent. Cancelling the token while the probe transport blocks (BlockingTransport) ends the wait at once with `decision.cancelled` and no send (AC: REQ-SAG-0015).
28a. Workspace check: in every crate except sc-decision-schema, no type maps the Jev API (no `SystemOneRequest`/`SystemOneResponse`/`Question`/`Asset`/`ModelMetadata`-shaped struct deriving serde with Jev field names: `rg -n 'struct (SystemOne|Question|Asset|ModelMetadata|NoulCriteria)' crates/ -g '!crates/sc-decision-schema/**'` finds nothing); provider usage types in sc-llm-provider map provider wire fields and are excluded (AC: REQ-SAG-0201 AC2).
28. Workspace searches: `rg -n 'Uuid|uuid::|uuid =' crates/` and `rg -n 'round\(|\{:\.[0-9]+\}' crates/ -g '*.rs'` (outside tests) find nothing (AC: REQ-SAG-0001 AC2, REQ-SAG-0007).

sc-agent-py (Rust unit tests in `src/lib.rs`):
29. The string sc-agent-py returns for a DecisionResult holding probability `0.123456789` contains `0.123456789` (AC: REQ-SAG-0007).

sc-agent-py (`tests/test_api.py`, pytest over the maturin build; `HOME` a tempdir with a default.json naming a loopback runner; no external network):
1. Lists the decide subcommands from `decide --help` (send, models, doctor, plus `send --batch`) and asserts each has its Python function (AC: ANC-006).
2. A `CancelToken` created and cancelled from Python, passed to `send` and `send_batch`, ends every request with `decision.cancelled` and starts no send (AC: REQ-SAG-0104 AC2).
3. `project="Phase B"` raises `SagError` with id `decide.log` (cause `log.invalid_slug`) and writes no row (AC: REQ-SAG-0004).
4. One `decide send` subprocess and one `sc_agent.send` with the same project and agent_role against a loopback runner that answers a fixed error: both session rows carry equal `project` and `agent_role` (AC: REQ-SAG-0004 AC3).
5. `general_log=records.append`, `session_log=rows.append` on one send: `records` holds a dict with action `decide.send`, `rows` holds the session row; no file is written under the log root for the injected logs (AC: REQ-SAG-0008 Python).
6. `send_batch` against a loopback runner that never answers; a second Python thread calls `_thread.interrupt_main()`; `KeyboardInterrupt` is raised within 2 s, every result was `decision.cancelled`, and the session rows are on disk; an `on_result` that raises `ValueError` cancels, drains and re-raises it (AC: REQ-SAG-0011 Python).
8. `send_batch` with 10,001 items raises `SagError` `decide.limit_exceeded` (what `batch_lines`), and `send` with a 9 MiB request string raises `decide.limit_exceeded` (what `request_bytes`); nothing reaches the loopback runner (AC: REQ-SAG-0014).
9. `session_log=rows.append` only: `rows` gets the session row and the general log file `<log_root>/logs/sc-agent-py.log.jsonl` is written under the tempdir log root (AC: REQ-SAG-0008 AC4).
10. `general_log=records.append` only: `records` gets the `decide.send` record and the session row is in `<log_root>/<repo>/<agent_role>/<project>/sessions.jsonl` (AC: REQ-SAG-0008 AC5).
7. `send(request, images={"img1": png_bytes}, long_context=True)` against the loopback runner reaches the transport as multipart with the long-context header (the loopback server records the request) (AC: REQ-SAG-0501 images).

## Required validation

- `cargo build -p decide -p sc-agent-py`; `maturin build -m crates/sc-agent-py/Cargo.toml`.
- `cargo clippy -p decide -p sc-agent-py --all-targets -- -D warnings` with the A-1 workspace denies and clippy.toml (sleep disallowed).
- `cargo test -p decide -p sc-agent-py`; `pytest crates/sc-agent-py/tests`.
- `cargo tree -p decide -e normal` and `cargo tree -p sc-agent-py -e normal`: no tokio.
- `cargo tree -i decide` for each lower sc-agent crate: not listed (ADR-SAG-0004).
- `grep -rnE 'thread::sleep|time::sleep|sleep_until' crates/decide/src crates/sc-agent-py/src` finds nothing; no `thread::spawn` in crates/decide/src.
- `rg -n 'Uuid|uuid::|uuid =' crates/` and the rounding search (test 28) find nothing.

## 8. Dependencies on other sprints

A-6 and A-7 (backends incl. `with_log`, models list, LLM client, `LlmProvider`); A-9 (`DecisionRunner`, `RunnerConfig`, `RunnerError`); through them A-1, A-2 (`DecisionRequest::new`, image limits), A-4 (config bounds, `RunnerKind`, `KeySource`), A-5 (`Timeouts::probe`, provider pause, `testing`), A-8 (`Slug`, `injected` loggers, `testing::MemoryLog`) (names in section 3.2).

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
- [ ] REQ-SAG-0011 decide: SIGINT or SIGTERM during `decide send --batch` (and Ctrl-C/Ctrl-Break on Windows) cancels the token through decide::signal (ctrlc with feature termination, no tokio); every unfinished request ends decision.cancelled and is printed, the session log is flushed (one row per started request), and decide exits 130 for any interrupt (ctrlc does not report which signal fired).
- [ ] REQ-SAG-0015: end to end through decide send to a local runner, decide waits for the model to load with LocalBackend::wait_until_loaded (load_timeout from the runner's entry, default 600 s) before the first send, then sends over a per-runner UreqTransport whose request timeout is the runner's (default 300 s); a scripted daemon reporting not-loaded twice then loaded gets one send whose timeout starts after the load, and a cancel during the load wait ends it at once with decision.cancelled and nothing sent.
- [ ] REQ-SAG-0011 Python: KeyboardInterrupt during sc-agent-py send/send_batch is seen within one 100 ms timed wait (py.check_signals with the GIL), cancels the token, drains the results, flushes the session rows and re-raises; an exception raised by on_result does the same.
- [ ] Each TypeSafe request example can be sent through decide send and returns the API's response (REQ-SAG-0501 AC1; ScriptedTransport injected through the Context, real runner).
- [ ] The TypeSafe models list is returned by decide models (REQ-SAG-0501 AC2; transport injected through the Context; Timeouts::probe()).
- [ ] decide doctor warns naming provider and model for every provider model in neither the built-in list nor the global configuration, prints no warning when all are present, warns when a config model has the release date of a built-in of its line ({config, builtin}), and lists every detected provider key (KeySource, never the value) in the order it would be used (REQ-SAG-0503).
- [ ] ANC-006: crates/sc-agent-py (module sc_agent) offers send, send_batch, models and doctor, one per decide operation (send, send --batch, models, doctor), checked by a pytest that lists the decide subcommands from `decide --help` and the module's functions.
- [ ] REQ-SAG-0104 AC2: a CancelToken created and cancelled from Python stops the sc-agent-py send and send_batch it is passed to (decision.cancelled, no send); in decide, cancelling a send whose transport is blocked returns decision.cancelled within the 2 s channel bound and no later result.
- [ ] REQ-SAG-0004: an invalid --project or --agent-role (e.g. 'Phase B') is rejected as decide.log with cause log.invalid_slug and nothing is sent or logged; sc-agent-py parses project and agent_role with the same Slug::new and raises SagError decide.log for 'Phase B'; AC3: a decide CLI send and an sc-agent-py send with the same project and agent_role write session rows with equal project and agent_role fields.
- [ ] REQ-SAG-0008 AC4/AC5 Python: with only session_log injected the callable receives the session row and the default general log file is written under the tempdir log root; with only general_log injected the callable receives the decide.send record and the session row is written to sessions.jsonl.
- [ ] REQ-SAG-0008 Python: sc-agent-py send/send_batch accept general_log and session_log callables (Callable[[dict], None]) and models/doctor accept general_log; a pytest receives the decide.send record and the session row through them, and the injected logs write no file.
- [ ] REQ-SAG-0003: decide writes <log_root>/logs/decide.log.jsonl, its own general log file (tsq's is A-3).
- [ ] ADR-SAG-0002: a decide send with a tempdir log_root writes its session row to <log_root>/<repo>/<agent_role>/<project>/sessions.jsonl with session_id, repo, project and agent_role first in fields.
- [ ] REQ-SAG-0001 AC2: `rg -n 'Uuid|uuid::|uuid =' crates/` over the finished workspace finds no hit.
- [ ] REQ-SAG-0007: a probability 0.123456789 is printed unchanged by decide and returned unchanged in sc-agent-py's result string; `rg -n 'round\(|\{:\.[0-9]+\}' crates/ -g '*.rs'` finds no rounding of returned values outside tests.
- [ ] REQ-SAG-0002: the error type DecideError (decide.* ids, implements ErrorId) is the return type of every public function of the decide library, and SagError is the error returned to Python by sc-agent-py; each wraps the real lower error unchanged and carries its id and message (AgentConfigError, TypesError, SecretsError, LogError, LocalError, DecisionError, HttpError, LlmError, RunnerError); a lowest-layer error (http.network) appears with its id and message in decide output; values are not rounded (REQ-SAG-0007).
- [ ] ADR-SAG-0004: every dependency in crates/decide/Cargo.toml points down the layer order (types; config, secrets, log; http; decision schema; decision backends; runner): decide is the top layer, above sc-decision-runner, and depends on sc-agent-types, sc-agent-config, sc-agent-secrets, sc-agent-log, sc-agent-http, sc-decision-schema, sc-decision-systemone, sc-decision-local, sc-decision-llm and sc-decision-runner; crates/sc-agent-py depends on decide; no lower-layer sc-agent library crate lists decide, tsq or sc-agent-py as a dependency, so none depends on a higher layer, checked by `cargo tree` for each lower crate.
- [ ] REQ-SAG-0009: Cargo.toml dependencies: neither decide nor sc-agent-py adds a tokio dependency, and no low-level sc-agent crate gains tokio with default features through them; `cargo tree -p decide -e normal` and `cargo tree -p sc-agent-py -e normal` list no tokio crate.
- [ ] REQ-SAG-0010: decide and sc-agent-py source call no sleep function, enforced by the workspace clippy disallowed-methods rule; clippy passes with the workspace denies (REQ-SAG-0002).
- [ ] ANC-007: the decide library writes one decide.send, decide.models and decide.doctor record per operation (never a key) to the GeneralLogger in its Context (injected, or the default for its service) and passes that same logger into HttpClient::new, DecisionRunner::new, LocalBackend::with_log, LlmClient::with_log and LlmDecisionBackend::with_log; a test injects sc_agent_log::testing::MemoryLog (general: Some(log.general()), session: Some(log.session(identity))), runs one send, models and doctor over a ScriptedTransport and finds those actions and http.send in log.records(); sc-agent-py calls the same decide library (default loggers, service sc-agent-py, or the Python callables), so its operations write the same records.
- [ ] REQ-SAG-0201 AC2: no crate in the finished workspace other than sc-decision-schema defines a type that maps the Jev API (workspace search over crates/ excluding sc-decision-schema finds none); provider usage types in sc-llm-provider map provider wire fields and are excluded.
- [ ] NFR-SAG-0001 (end-to-end contribution; owner a-8, 40,000-session logger test): after an interrupted decide --batch, the session log holds exactly one row per started request (test 27's flushed-rows check).

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); decisions are in the design:
1. decide package/bin/subcommands: decide; send, models, doctor; crates.io availability checked at publish time (assistant task).
2. The rename to decide is the owner's ("call the cli binaries 'decide'").
3. --project/--agent-role required (assistant); --provider is an optional override, provider chosen by ranking among detected keys (owner, REQ-SAG-0103); exit 0/1 (assistant).
4. ANC-006 (replaces ANC-002): sc-agent-py in this sprint.
Also: a bad batch line stops decide before any send (owner 'fail fast on first', REQ-SAG-0504); each CLI writes its own general log (REQ-SAG-0003).
Hardening round 1 (2026-11-09): tsq (binary, copy rows, tests, REQ-SAG-0303) moved to A-3 and the A-3 dependency was dropped (PLAN-SCOPE-004, PLAN-CRIT-007/011/012); injection seams in SendOptions/ModelsOptions/DoctorOptions (PLAN-CRIT-006); send vs send_batch dispatch (CRIT-M3); this sprint owns the end-to-end REQ criteria the boundary sprints list in does_not_close.
Round 2 (2026-11-09): ADR-SAG-0004 amendment approved by the owner (open-question lines removed, ARCH-002); ADR-SAG-0002 added; images/long-context input, REQ-SAG-0101/0102/0602/0001 AC2/0007/0004 AC3/0801 tests, Context seams, Python logger callables, signals (REQ-SAG-0011), REQ-SAG-0012/0013/0014 end to end, typed options (RBP-F010), cause -> cause_detail.
