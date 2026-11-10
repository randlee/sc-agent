<!-- Generated from bead agt-4p2.10 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-10: decide CLI and sc-agent Python API
- id: `a-10` (bead `agt-4p2.10`)
- branch: `sprint/a-10-clis-python`
- depends on: a-7, a-6, a-9
- copy script: `docs/plans/phase-a/copy/a-10.sh`

## Description

The decide binary and library (send one request or a JSONL batch with --model, --project, --agent-role and an optional --provider override; models; doctor) and crates/sc-agent-py (one Python function per decide operation, ANC-006). Argument parsing, backend choice, wiring and output only. Records: REQ-SAG-0501, REQ-SAG-0503, REQ-SAG-0504 (docs/decide), REQ-SAG-0003, REQ-SAG-0004, REQ-SAG-0103, REQ-SAG-0104, REQ-SAG-0401, REQ-SAG-0601, REQ-SAG-0701, REQ-SAG-0801.

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
      artifact: sc_llm_provider::{LlmClient (incl. with_log, models_request, decode_models), catalog_model_id} and sc_decision_llm::LlmDecisionBackend (incl. with_log)
      trigger: dev-push
    - sprint: a-9
      artifact: sc_decision_runner::{DecisionRunner, RunnerConfig, RunnerError}
      trigger: dev-push
  requirements: [REQ-SAG-0501, REQ-SAG-0503, REQ-SAG-0504, REQ-SAG-0003, REQ-SAG-0004, REQ-SAG-0008, REQ-SAG-0103, REQ-SAG-0104, REQ-SAG-0401, REQ-SAG-0601, REQ-SAG-0701, REQ-SAG-0801, REQ-SAG-0901, REQ-SAG-0002, REQ-SAG-0007, REQ-SAG-0009, REQ-SAG-0010]
  adrs: [ADR-SAG-0001, ADR-SAG-0004]
  does_not_close:
    - "Discovery is a library capability of sc-decision-local; no decide or Python operation calls it in this phase (REQ-SAG-0801 requires only the library report)."
    - "ADR-SAG-0004 amendment (decide as composition root, sc-agent-py on decide, ErrorId/ErrorDetail convention): open owner question; records unchanged."
    - "tsq and REQ-SAG-0303: Sprint A-3."
```

# Sprint A-10: decide CLI and library, sc-agent Python API (bead agt-4p2.10)

Copy script: `docs/plans/phase-a/copy/a-10.sh` (dry run 2026-11-09 after the tsq rows moved to a-3.sh: 9 copy steps, 4 files, 210 lines before trimming).
Source: sc-agent-ref @ dce239a (`crates/sc-ai`, `crates/sc-jev-host/src/stream`, and the `Parsed`/`parse()` pattern from `crates/sc-treesitter/src/cli/mod.rs`).
Targets: `crates/decide/` (new lib + bin crate) and `crates/sc-agent-py/` (new PyO3 crate, the sc-agent Python API).
Governing records: REQ-SAG-0501 and REQ-SAG-0503 (under `docs/decide/requirements/`, naming the CLI `decide`), REQ-SAG-0504 (a bad batch line stops decide before any send), REQ-SAG-0003 (each CLI writes its own general log), REQ-SAG-0008 (optional logger injection with defaults), REQ-SAG-0601 (busy resend, end to end through decide), REQ-SAG-0004 (CLI and PyO3 accept project and agent_role, one field for role and name; slugs; reject, do not rewrite), REQ-SAG-0103 (provider and model separate; provider chosen by ranking, never required), REQ-SAG-0104 AC2 (cancel from Python), REQ-SAG-0401 (keys: env, repo-root .env, keyring), REQ-SAG-0801 (a local runner selected by its configured name), REQ-SAG-0901, REQ-SAG-0701 (batch through the runner), REQ-SAG-0002, REQ-SAG-0007, REQ-SAG-0009 / owner-lowlevel-no-tokio, ADR-SAG-0001 (one sync `Decision` trait), ADR-SAG-0004 (decide is the top layer: it may use every crate; it is where a backend is chosen and wired into the runner). Anchors: ANC-006 (every decide operation is also in the sc-agent Python API); batch parallelism is REQ-SAG-0701.
Rule for the binary: argument parsing and output only. All behaviour lives in the libraries (A-1, A-2, A-4..A-9); decide's own logic (choosing the backend for `--model` with the optional `--provider` override, wiring `DecisionRunner`, models, doctor) is in decide's library target, which the `decide` binary and sc-agent-py share.

Decisions (assistant, 2026-11-09):
- Names: binary and package `decide` (crates.io availability checked at publish time); subcommands `send`, `models`, `doctor`.
- `--project` and `--agent-role` are required, no default; there is no `--agent-name`.
- Provider: `--provider` is an optional override. Absent: the first provider in `Provider::ranking(line of the model)` (A-1) whose key `resolve` detects (A-4); never required (REQ-SAG-0103).
- A batch is read and validated whole before anything is sent; the first bad line stops decide (REQ-SAG-0504).
- Exit: 0 when the command ran and every result succeeded, else 1.
- `send` without `--batch` always calls `Decision::decide` (one request); `send --batch` always calls `Decision::decide_stream`, even for a one-line batch (CRIT-M3).
- Injection seams (REQ-SAG-0008): `SendOptions`, `ModelsOptions` and `DoctorOptions` carry an optional transport and optional loggers; `None` uses the defaults (`UreqTransport`, the default general/session loggers under `log_root`). Tests inject `sc_agent_http::testing::ScriptedTransport`/`BlockingTransport` and an `sc_agent_log::testing::MemoryLog` (`let log = MemoryLog::new()?;` then `general: Some(log.general())`, `session: Some(log.session(identity))`) through them and go through the real `DecisionRunner`; assertions read `log.records()?` (`Vec<MemoryRecord>` with `level`, `target`, `action`, `fields`, in write order). Fake `Decision` impls in tests are local to crates/decide's tests (no `sc_decision_schema::testing`).
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
- `input.rs` (library): `request_bytes` loses `RepoRoot`/`joined_path` (paths are relative to the working directory), errors -> `InputNotFound`/`InputRead`; `read_line_bounded`: replace `&available[..take]` with `available.get(..take)` (indexing_slicing); wrap the copied tests in `#[cfg(test)] mod tests { use super::*; use std::io::Cursor; ... }`.
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
    /// Input is JSONL: one TypeSafe request per line.
    #[arg(long)]
    pub(crate) batch: bool,
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

// src/input.rs (library)
pub const MAX_LINE_BYTES: usize = 1 << 20;
pub fn request_bytes(spec: &str, stdin: &mut dyn std::io::Read) -> Result<Vec<u8>, DecideError>;
pub fn open_lines(spec: &str, stdin: Box<dyn std::io::BufRead + Send>) -> Result<Box<dyn std::io::BufRead + Send>, DecideError>;
enum LineRead { End, Line, TooLong }
fn read_line_bounded<R: std::io::BufRead>(reader: &mut R, max: usize, out: &mut Vec<u8>) -> std::io::Result<LineRead>;
/// Reads and validates the whole JSONL batch before anything is sent (REQ-SAG-0504). Blank lines and a
/// leading BOM are skipped. The first line that is too long, not UTF-8 or not a Jev request returns
/// LineTooLong / LineNotUtf8 / RequestInvalid naming that line, the field and the reason; nothing is sent.
pub fn read_batch(reader: &mut dyn std::io::BufRead) -> Result<Vec<sc_decision_schema::DecisionRequest>, DecideError>;

// src/lib.rs: decide's library target, shared by the decide binary and sc-agent-py
pub mod input; pub mod output; mod doctor; mod error; mod select;
pub use doctor::{doctor, DetectedKey, DoctorOptions, DoctorReport};
pub use error::DecideError;
pub use select::{models, send, send_batch, send_on, send_batch_on, ModelsOptions, SendOptions};

// src/select.rs  (the one place a backend is chosen; ADR-SAG-0004 top layer)
#[derive(Clone)]
pub struct SendOptions {
    pub model: String,
    pub provider: Option<String>,
    pub project: String,
    pub agent_role: String,
    pub service: &'static str,                                  // "decide" | "sc-agent-py"
    pub transport: Option<Arc<dyn sc_agent_http::Transport>>,   // None -> Arc::new(UreqTransport::new(Timeouts::default()))
    pub general: Option<sc_agent_log::GeneralLogger>,           // None -> GeneralLogger::open_default(log_root, service)
    pub session: Option<sc_agent_log::SessionLogger>,           // None -> SessionLogger::open_default(log_root, identity)
}
#[derive(Clone)]
pub struct ModelsOptions {
    pub runner: Option<String>,                                 // a configured local runner; TypeSafe when None
    pub service: &'static str,
    pub transport: Option<Arc<dyn sc_agent_http::Transport>>,
    pub general: Option<sc_agent_log::GeneralLogger>,
}
/// model names a config local runner -> LocalBackend (DaemonKind from runner.kind, key via resolve_var(key_env));
/// else model = catalog.resolve(ModelChoice); provider = options.provider, else the first of
/// Provider::ranking(line) (line = the short name, or catalog.line_of(id)) whose key resolve() finds, with
/// KeySources::system(Repo::work_tree_root(cwd)?); none -> NoProviderKey. TypeSafe -> SystemOneBackend::new(TYPESAFE_BASE_URL, key);
/// any other provider -> LlmDecisionBackend(LlmClient::new(provider, key), model).
fn backend(options: &SendOptions, config: &AgentConfig, catalog: &Catalog, sources: &KeySources<'_>, general: &GeneralLogger) -> Result<Arc<dyn Backend>, DecideError>;
/// general = options.general.clone() or GeneralLogger::open_default(config.log_root()?, options.service);
/// session = options.session.clone() or SessionLogger::open_default(config.log_root()?, identity) (identity already validated);
/// transport = options.transport.clone() or Arc::new(UreqTransport::new(Timeouts::default()));
/// HttpClient::new(transport, ResendPolicy::new(config.max_sends), general.clone()),
/// DecisionRunner::new(backend, http, session, general, RunnerConfig::from_config(config)).
/// The same `general` is passed to LocalBackend::with_log / LlmClient::with_log / LlmDecisionBackend::with_log in `backend`.
fn runner(options: &SendOptions, config: &AgentConfig, identity: SessionIdentity, general: GeneralLogger, backend: Arc<dyn Backend>) -> Result<DecisionRunner, DecideError>;
/// Session identity is validated before the backend is built; loads config from default_dir(); always
/// `Decision::decide` (plain send); then flush_session_log; writes one `decide.send` general record.
pub fn send(options: &SendOptions, request: DecisionRequest, cancel: &CancelToken) -> Result<DecisionResult, DecideError>;
/// As `send`, but always `Decision::decide_stream(Box::new(requests.into_iter()), cancel)` (an owned 'static
/// iterator), even for one request; `on_result` gets each DecisionResult as it finishes.
pub fn send_batch(options: &SendOptions, requests: Vec<DecisionRequest>, cancel: &CancelToken, on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>) -> Result<(), DecideError>;
/// Testable cores: any Decision (the runner in send/send_batch, a fake in tests).
pub fn send_on(decision: &dyn Decision, request: DecisionRequest, cancel: &CancelToken) -> DecisionResult;
pub fn send_batch_on(decision: &dyn Decision, requests: Vec<DecisionRequest>, cancel: &CancelToken, on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>) -> Result<(), DecideError>;
/// GET /v1/models of TypeSafe or a configured runner, through HttpClient::send_once (one-off holdoff on cancel.wait_until)
/// over options.transport (or the default); writes one `decide.models` general record.
pub fn models(options: &ModelsOptions, cancel: &CancelToken) -> Result<ModelMetadataList, DecideError>;

// src/doctor.rs (REQ-SAG-0503)
#[derive(Debug, serde::Serialize)]
pub struct DetectedKey { pub provider: Provider, pub source: &'static str }   // never the value
#[derive(Debug, serde::Serialize)]
pub struct DoctorReport { pub keys: Vec<DetectedKey>, pub missing: Vec<(Provider, String)>, pub ties: Vec<(ModelId, ModelId)> }
#[derive(Clone)]
pub struct DoctorOptions {
    pub service: &'static str,
    pub transport: Option<Arc<dyn sc_agent_http::Transport>>,   // None -> UreqTransport
    pub general: Option<sc_agent_log::GeneralLogger>,           // None -> GeneralLogger::open_default(log_root, service)
}
/// Writes one `decide.doctor` general record.
pub fn doctor(options: &DoctorOptions, cancel: &CancelToken) -> Result<DoctorReport, DecideError>;

// src/output.rs (library)
#[derive(serde::Serialize)]
pub struct ResultLine<'a> {
    pub session_id: &'a SessionId,
    pub position: u64,
    pub digests: &'a OrderedMap<String, QuestionDigest>,
    #[serde(skip_serializing_if = "Option::is_none")] pub response: Option<&'a SystemOneResponse>,
    #[serde(skip_serializing_if = "Option::is_none")] pub error: Option<ErrorDetail>,   // DecisionError::detail(): every lower id and message
}
pub fn result_line(result: &DecisionResult) -> Result<Vec<u8>, DecideError>;

// src/main.rs (bin: argv -> decide::{send, models, doctor} -> stdout)
fn main() -> std::process::ExitCode;
fn run(parsed: Parsed, stdin: &mut dyn std::io::Read, stdout: &mut dyn std::io::Write) -> Result<std::process::ExitCode, DecideError>;
fn print_stdout(bytes: &[u8]) -> std::process::ExitCode;
fn emit(error: &DecideError) -> std::process::ExitCode;     // one stderr line: error.detail() as JSON
```

Types used: `SessionId`, `ErrorId`, `ErrorDetail`, `Catalog`, `ModelChoice`, `Provider` (incl. `PRIORITY`, `ranking`), `MODELS`, `CancelToken` (A-1); `AgentConfig`, `load`, `default_dir` (A-4); `KeySources`, `ResolvedKey`, `resolve`, `resolve_var` (A-4); `HttpClient`, `ResendPolicy`, `Transport`, `UreqTransport`, `Timeouts`, `Attempts`, `Next` (A-5); `GeneralLogger`, `SessionLogger`, `Repo` (incl. `work_tree_root`), `SessionIdentity` (A-8); `Decision`, `Backend`, `DecisionRequest`, `DecisionResult`, `SystemOneResponse`, `OrderedMap`, `QuestionDigest`, `ModelMetadataList` (A-2); `SystemOneBackend`, `TYPESAFE_BASE_URL` (A-6); `LocalBackend`, `DaemonKind` (A-6); `LlmClient` (incl. `models_request`, `decode_models`), `catalog_model_id`, `LlmDecisionBackend` (A-7); `DecisionRunner`, `RunnerConfig` (A-9).

Behaviour:
- `send` (no `--batch`): `request_bytes` -> `SystemOneRequest::from_json` (failure: `RequestInvalid { line: None }`) -> `DecisionRequest::from` -> `decide::send` (always `Decision::decide`) -> one `ResultLine` + `\n`. A failed decision is data in its result (owner-decision-trait), printed on stdout.
- `send --batch`: `read_batch` reads and validates every line first; a bad line returns its error (line, field, reason) before anything is sent (REQ-SAG-0504). Then `decide::send_batch`, which always calls `decision.decide_stream(Box::new(requests.into_iter()), &cancel)` (owned `'static` iterator), also for a one-line batch; each result printed as one JSONL line as it arrives (fastest first, session ULID and input position); a failed request is data in its line and the batch continues. A stdout write failure cancels the token and returns `Write`. After the stream, `runner.flush_session_log()`; an Err is a command error (exit 1) printed after the results, never replacing them.
- Session identity: `--project`/`--agent-role` validated by `SessionIdentity::new` before anything is sent; an invalid value is `decide.log` (cause `log.invalid_slug`), nothing is sent and no row is written (REQ-SAG-0004). `repo` comes from `Repo::detect` on the working folder.
- `models`: `SystemOneBackend::models_request` for TypeSafe or the runner's base; sent with `HttpClient::send_once` over `ModelsOptions.transport` (default `UreqTransport`) (a one-off holdoff waits on `CancelToken::wait_until`, A-5); `decode_models`; prints the JSON.
- `doctor` (REQ-SAG-0503): (1) one line per detected provider key in `Provider::PRIORITY` order with its source (`env`, `.env`, `keyring`), never the value; (2) for each provider with a key, its model list (`LlmClient::models_request`/`decode_models`; TypeSafe through A-6 `models_request`), each id mapped back with `catalog_model_id`, one warning `{provider, model}` per id `catalog.line_of` does not know; (3) one warning per config model whose release date equals a built-in model of its line (the config model wins, A-1). Nothing in (2) or (3) when nothing is missing or tied.
- The configuration directory passed to `sc_agent_config::load` is `default_dir()` (`~/.sc-agent/`).
- General-log records (ANC-007; never a key or key value): `decide.send` (service, model, provider, requests, succeeded, failed), `decide.models` (runner or `typesafe`, model count, outcome id), `decide.doctor` (detected key count, missing count, tie count, outcome id). The same `GeneralLogger` goes into `HttpClient::new` (its `http.send` records), `DecisionRunner::new`, `LocalBackend::with_log`, `LlmClient::with_log` and `LlmDecisionBackend::with_log`.
- Exit: 0 when the command ran and every result succeeded; 1 otherwise. Numbers are printed as serde_json gives them (REQ-SAG-0007). No tokio, no sleep, no threads of its own.

## 4. Error list

decide (`crates/decide/src/error.rs`, new; ErrorId from A-1, Display = detail(); each wrapped error kept unchanged):

```rust
#[derive(Debug)]
#[non_exhaustive]
pub enum DecideError {
    /// clap error; kind = Debug of clap::error::ErrorKind, detail = clap's text.
    Usage { kind: String, detail: String },                                       // "decide.usage"
    /// The global configuration could not be loaded.
    Config(sc_agent_config::AgentConfigError),                                     // "decide.config"
    /// --provider or --model does not parse, or the short name has no model.
    Types(sc_agent_types::TypesError),                                             // "decide.types"
    /// The provider's or runner's key is missing or malformed.
    Secrets(sc_agent_secrets::SecretsError),                                       // "decide.secrets"
    /// Session identity invalid (log.invalid_slug), repo detection or logger open failed.
    Log(sc_agent_log::LogError),                                                   // "decide.log"
    /// A local runner's kind is unknown.
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
    NoProviderKey { model: String, tried: Vec<Provider> },                         // "decide.no_provider_key"
    InputNotFound { input: String },                                               // "decide.input_not_found"
    /// io error other than NotFound; kind = Debug of io::ErrorKind, detail = Display.
    InputRead { input: String, kind: String, detail: String },                     // "decide.input_read"
    LineTooLong { line: u64, limit: usize },                                       // "decide.line_too_long"
    LineNotUtf8 { line: u64, byte_offset: usize },                                 // "decide.line_not_utf8"
    /// A request does not match the Jev request schema; line = JSONL line (None for a single request); the message names
    /// the line, the field (RequestParse path) and the reason; source = decision.request_parse, kept (REQ-SAG-0504).
    RequestInvalid { line: Option<u64>, source: sc_decision_schema::DecisionError },  // "decide.request_invalid"
    Encode { detail: String },                                                     // "decide.encode"
    /// stdout write failed.
    Write { kind: String, detail: String },                                        // "decide.write"
}
impl ErrorId for DecideError {}
```

18 variants. A decision that fails inside `decide`/`decide_stream` is not a `DecideError`: it is the `DecisionError` inside that `DecisionResult`, printed in its `ResultLine.error`.

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
serde_json = { version = "1", features = ["preserve_order", "float_roundtrip"] }
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

Package `sc-agent-py` (`publish = false`), `[lib] name = "sc_agent", crate-type = ["cdylib"]`; maturin wheel `sc-agent`, module `sc_agent`. Deps: `decide` (path, lib), `sc-agent-types`, `sc-decision-schema`, `pyo3 = { version = "=0.29.2", features = ["abi3-py310", "extension-module"] }`, `serde_json`. Each call releases the GIL (`py.allow_threads`). Same output shapes as the CLI (JSON text). The general log service is `sc-agent-py`.

```rust
#[pyclass(module = "sc_agent", frozen)] pub struct CancelToken(sc_agent_types::CancelToken);   // new(), cancel(), is_cancelled()
create_exception!(sc_agent, SagError, PyException);   // args = (id, message, detail JSON with every cause)
#[pyfunction] #[pyo3(signature = (request, *, model, project, agent_role, provider = None, cancel = None))]
fn send(py: Python<'_>, request: &str, model: &str, project: &str, agent_role: &str, provider: Option<&str>, cancel: Option<&CancelToken>) -> PyResult<String>;   // one ResultLine
#[pyfunction] #[pyo3(signature = (requests, *, model, project, agent_role, provider = None, cancel = None, on_result = None))]
fn send_batch(py: Python<'_>, requests: Vec<String>, model: &str, project: &str, agent_role: &str, provider: Option<&str>, cancel: Option<&CancelToken>, on_result: Option<PyObject>) -> PyResult<Vec<String>>;   // ResultLines in finish order; on_result(line) as each arrives
#[pyfunction] #[pyo3(signature = (runner = None, cancel = None))] fn models(py: Python<'_>, runner: Option<&str>, cancel: Option<&CancelToken>) -> PyResult<String>;   // decide::models with ModelsOptions { transport: None, general: None, .. }
#[pyfunction] #[pyo3(signature = (cancel = None))] fn doctor(py: Python<'_>, cancel: Option<&CancelToken>) -> PyResult<String>;   // DoctorReport JSON
#[pymodule] fn sc_agent(module: &Bound<'_, PyModule>) -> PyResult<()>;
```

decide subcommand -> Python: `send` -> `send` (`decide::send`), `send --batch` -> `send_batch` (`decide::send_batch`), `models` -> `models`, `doctor` -> `doctor`.
Loggers and transport from Python: no record supports a Python `log_root` or logger argument, so the Python functions pass `transport: None, general: None, session: None` and use decide's default loggers (service `sc-agent-py`) and `UreqTransport`; ANC-007 is met by the decide library they call. Tests that need a scripted transport are Rust tests in crates/decide.

No tokio, no mimalloc. Dev: `tempfile 3`.

## 6. New code

- decide: `src/error.rs` (~130 lines), `src/select.rs` (~110), `src/doctor.rs` (~70), `src/output.rs` (~40), `read_batch`/`open_lines` in `src/input.rs` (~50), `run`/`main` (~80), `Cli` (~70).
- sc-agent-py: `src/lib.rs` (~150), `pyproject.toml`, `tests/test_api.py`.

## 7. Tests

Each test names the acceptance line it proves (`AC: <line key>`). "Real runner" = `decide::send`/`send_batch` with a `ScriptedTransport` (or `BlockingTransport`) and `MemoryLog` injected through `SendOptions` (`general: Some(log.general())`, `session: Some(log.session(identity))`), going through the real `DecisionRunner` and backend; no network.

decide (`crates/decide`):
1. `cli_definition_is_consistent`; the 3 `read_line_bounded` tests (copied; AC: decide send).
2. Single request from a file and from stdin reaches `send_on` unchanged; one `ResultLine` with `session_id` and `position` (AC: decide send).
3. Each TypeSafe request example (openapi.json) through the real runner + SystemOneBackend, transport answering the documented response (AC: REQ-SAG-0501 AC1).
4. Batch of 3 on a fake `Decision` finishing out of order and failing one (AC: REQ-SAG-0701 output).
5. 1,000-line batch, line 412 bad, real runner; the injected transport records zero calls (AC: REQ-SAG-0504).
6. `--project "Phase B"`; no `--agent-name` flag (AC: REQ-SAG-0004).
7. `0.123456789` printed unchanged (AC: REQ-SAG-0002).
8. A result carrying `decision.http` -> `http.network` (AC: REQ-SAG-0002).
9. `models` with an injected transport (AC: REQ-SAG-0501 AC2).
10. `doctor` with an injected transport and fake provider lists (AC: REQ-SAG-0503).
11. `select::backend` provider choice cases (AC: REQ-SAG-0103).
12. A LocalBackend and a SystemOneBackend run batches in parallel through two real runners over injected transports (AC: decide send, owner-local-decision-crate).
13. Broken stdout during a batch cancels the token and returns `decide.write` (AC: REQ-SAG-0701 output).
14. A run writes `<log_root>/logs/decide.log.jsonl` (AC: REQ-SAG-0003).
15. 40-request `send_batch` over a `BlockingTransport`: 8 sends in flight at once with the default workers, 1 with workers 1 (AC: REQ-SAG-0701 parallel).
16. Busy end to end: transport answers 529 with `Retry-After: 0` every time -> 5 sends and `http.provider_busy` (reason `sends`, 5 attempts) in the result; `SC_AGENT__MAX_SENDS=2` -> 2 sends (AC: REQ-SAG-0601).
17. Plain `send` calls `decide`, `send --batch` with one line calls `decide_stream` (fake `Decision` counting calls; AC: CRIT-M3 dispatch).
18. `MemoryLog::new()?` injected (`general: Some(log.general())` in all three options, `session: Some(log.session(identity))` in SendOptions); one send, models and doctor; `log.records()?` contains `MemoryRecord`s whose `action` is `decide.send`, `decide.models`, `decide.doctor` and `http.send`, and no record's `fields` holds a key value (AC: ANC-007).
19. `grep -rn 'thread::spawn' crates/decide/src` finds nothing (AC: REQ-SAG-0701 output).
20. `send` over a `BlockingTransport` held by the test; cancel the token; the `decision.cancelled` result returns within 100 ms; releasing the transport yields no further result (AC: REQ-SAG-0104 AC2, the Rust half).

sc-agent-py (`tests/test_api.py`, pytest over the maturin build; no transport, no network):
1. Lists the decide subcommands from `decide --help` (send, models, doctor, plus `send --batch`) and asserts each has its Python function (AC: ANC-006).
2. A `CancelToken` created and cancelled from Python, passed to `send` and to `send_batch` (fake TypeSafe key in the test environment, `HOME` a tempdir), ends every request with `decision.cancelled` and starts no send, so no network is touched (AC: REQ-SAG-0104 AC2). In-flight cancel is decide test 20.
3. `project="Phase B"` raises `SagError` with id `decide.log` (cause `log.invalid_slug`) and writes no row; valid project/agent_role pass the same `SessionIdentity::new` check as the CLI (AC: REQ-SAG-0004).

## Required validation

- `cargo build -p decide -p sc-agent-py`; `maturin build -m crates/sc-agent-py/Cargo.toml`.
- `cargo clippy -p decide -p sc-agent-py --all-targets -- -D warnings` with the A-1 workspace denies and clippy.toml (sleep disallowed).
- `cargo test -p decide`; `pytest crates/sc-agent-py/tests`.
- `cargo tree -p decide -e normal` and `cargo tree -p sc-agent-py -e normal`: no tokio.
- `cargo tree -i decide` for each lower sc-agent crate: not listed (ADR-SAG-0004).
- `grep -rnE 'thread::sleep|time::sleep|sleep_until|thread::spawn' crates/decide/src crates/sc-agent-py/src` finds nothing.

## 8. Dependencies on other sprints

A-6 and A-7 (backends incl. `with_log`, models list, LLM client); A-9 (`DecisionRunner`, `RunnerConfig`, `RunnerError`); through them A-1, A-2, A-4, A-5, A-8 (names in section 3.2).

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-10.sh` (sc-agent-ref@dce239a, 9 copy steps), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] decide send: one request from a file or stdin, or a JSONL batch (--batch), with --model, --project and --agent-role (required; no --agent-name) and an optional --provider override; a configured local runner is selected by name (REQ-SAG-0401, REQ-SAG-0801); runs through the Decision trait only, over DecisionRunner (ADR-SAG-0001).
- [ ] CRIT-M3 dispatch: plain `decide send` always calls Decision::decide; `decide send --batch` always calls Decision::decide_stream with an owned 'static iterator (Box::new(requests.into_iter())), even for a one-line batch.
- [ ] REQ-SAG-0103: without --provider, decide picks the first provider in Provider::ranking(line) with a detected key (process env, repo-root .env, keyring); `--model haiku` with Anthropic and OpenRouter keys goes to Anthropic, with only OpenRouter to OpenRouter as `anthropic/claude-haiku-5.5`, `--provider openrouter` overrides, `--model jev` with no TypeSafe key is decide.no_provider_key.
- [ ] REQ-SAG-0504: a 1,000-line batch whose line 412 has a `score` question without `criteria` stops decide with decide.request_invalid naming line 412, `criteria` and the reason, and the injected transport is never called.
- [ ] REQ-SAG-0701 parallel: decide --batch hands the whole batch to DecisionRunner; a 40-request batch over a BlockingTransport has 8 sends in flight at once with the default workers and 1 with workers set to 1.
- [ ] REQ-SAG-0701 output: decide starts no thread of its own (a test greps crates/decide for thread::spawn); decide prints JSON for one request and JSONL for a batch, fastest first, each line tagged with its session ULID and input position; a failed request is printed in its result as an ErrorDetail chain and the rest of the batch continues; a stdout write failure cancels the batch and returns decide.write.
- [ ] REQ-SAG-0601: end to end through decide send and the real DecisionRunner, a request whose injected transport answers busy (529, Retry-After: 0) every time is sent 5 times and its result is http.provider_busy with reason sends and 5 attempts; with SC_AGENT__MAX_SENDS=2 it is sent 2 times.
- [ ] Each TypeSafe request example can be sent through decide send and returns the API's response (REQ-SAG-0501 AC1; ScriptedTransport injected through SendOptions, real runner).
- [ ] The TypeSafe models list is returned by decide models (REQ-SAG-0501 AC2; transport injected through ModelsOptions).
- [ ] decide doctor warns naming provider and model for every provider model in neither the built-in list nor the global configuration, prints no warning when all are present, warns when a config model has the release date of a built-in of its line, and lists every detected provider key (source, never the value) in the order it would be used (REQ-SAG-0503).
- [ ] ANC-006: crates/sc-agent-py (module sc_agent) offers send, send_batch, models and doctor, one per decide operation (send, send --batch, models, doctor), checked by a pytest that lists the decide subcommands from `decide --help` and the module's functions.
- [ ] REQ-SAG-0104 AC2: a CancelToken created and cancelled from Python stops the sc-agent-py send and send_batch it is passed to (decision.cancelled, no send); in decide, cancelling a send whose transport is blocked returns decision.cancelled within 100 ms and no later result.
- [ ] REQ-SAG-0004: an invalid --project or --agent-role (e.g. 'Phase B') is rejected as decide.log with cause log.invalid_slug and nothing is sent or logged; sc-agent-py validates project and agent_role through the same SessionIdentity::new and raises SagError decide.log for 'Phase B'.
- [ ] REQ-SAG-0003: decide writes <log_root>/logs/decide.log.jsonl, its own general log file (tsq's is A-3).
- [ ] REQ-SAG-0002: the error type DecideError (decide.* ids, implements ErrorId) is the return type of every public function of the decide library, and SagError is the error returned to Python by sc-agent-py; each wraps the real lower error unchanged and carries its id and message (AgentConfigError, TypesError, SecretsError, LogError, LocalError, DecisionError, HttpError, LlmError, RunnerError); a lowest-layer error (http.network) appears with its id and message in decide output; values are not rounded (REQ-SAG-0007).
- [ ] ADR-SAG-0004: every dependency in crates/decide/Cargo.toml points down the layer order (types; config, secrets, log; http; decision schema; decision backends; runner): decide is the top layer, above sc-decision-runner, and depends on sc-agent-types, sc-agent-config, sc-agent-secrets, sc-agent-log, sc-agent-http, sc-decision-schema, sc-decision-systemone, sc-decision-local, sc-decision-llm and sc-decision-runner; crates/sc-agent-py depends on decide; no lower-layer sc-agent library crate lists decide, tsq or sc-agent-py as a dependency, so none depends on a higher layer, checked by `cargo tree` for each lower crate.
- [ ] REQ-SAG-0009: Cargo.toml dependencies: neither decide nor sc-agent-py adds a tokio dependency, and no low-level sc-agent crate gains tokio with default features through them; `cargo tree -p decide -e normal` and `cargo tree -p sc-agent-py -e normal` list no tokio crate.
- [ ] REQ-SAG-0010: decide and sc-agent-py source call no sleep function, enforced by the workspace clippy disallowed-methods rule; clippy passes with the workspace denies (REQ-SAG-0002).
- [ ] ANC-007: the decide library writes one decide.send, decide.models and decide.doctor record per operation (never a key) to the GeneralLogger in its options (injected, or the default for its service) and passes that same logger into HttpClient::new, DecisionRunner::new, LocalBackend::with_log, LlmClient::with_log and LlmDecisionBackend::with_log; a test injects sc_agent_log::testing::MemoryLog (general: Some(log.general()), session: Some(log.session(identity))), runs one send, models and doctor over a ScriptedTransport and finds those actions and http.send in log.records(); sc-agent-py calls the decide library with the default loggers (service sc-agent-py), so its operations write the same records.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); decisions are in the design:
1. decide package/bin/subcommands: decide; send, models, doctor; crates.io availability checked at publish time (assistant task).
2. The rename to decide is the owner's ("call the cli binaries 'decide'").
3. --project/--agent-role required (assistant); --provider is an optional override, provider chosen by ranking among detected keys (owner, REQ-SAG-0103); exit 0/1 (assistant).
4. ANC-006 (replaces ANC-002): sc-agent-py in this sprint.
Also: a bad batch line stops decide before any send (owner 'fail fast on first', REQ-SAG-0504); each CLI writes its own general log (REQ-SAG-0003).
Hardening round 1 (2026-11-09): tsq (binary, copy rows, tests, REQ-SAG-0303) moved to A-3 and the A-3 dependency was dropped (PLAN-SCOPE-004, PLAN-CRIT-007/011/012); injection seams in SendOptions/ModelsOptions/DoctorOptions (PLAN-CRIT-006); send vs send_batch dispatch (CRIT-M3); this sprint owns the end-to-end REQ criteria the boundary sprints list in does_not_close.
Open owner question (not blocking this sprint's text): ADR-SAG-0004 amendment, see does_not_close.
