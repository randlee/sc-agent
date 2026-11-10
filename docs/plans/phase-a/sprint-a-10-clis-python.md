<!-- Generated from bead agt-4p2.10 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-10: decide and tsq CLIs, sc-agent Python API
- id: `a-10` (bead `agt-4p2.10`)
- branch: `sprint/a-10-clis-python`
- depends on: agt-4p2.7, agt-4p2.3, agt-4p2.6, agt-4p2.9
- copy script: `docs/plans/phase-a/copy/a-10.sh`

## Description

Two thin binaries and the Python API: decide (send one request or a JSONL batch with --model, --project, --agent-role and an optional --provider override; models; doctor), tsq (languages, queries, query) and crates/sc-agent-py (one Python function per decide operation, ANC-006). Argument parsing, backend choice and output only. Records: REQ-SAG-0303, REQ-SAG-0501, REQ-SAG-0503, REQ-SAG-0504 (docs/decide), REQ-SAG-0003, REQ-SAG-0004, REQ-SAG-0103, REQ-SAG-0401, REQ-SAG-0801.

## Design

# Sprint A-10: decide and tsq CLIs, sc-agent Python API (bead agt-4p2.10)

Copy script: `docs/plans/phase-a/copy/a-10.sh` (dry run 2026-11-09, reconciliation: 24 copy steps, 7 files, 450 lines before trimming).
Source: sc-agent-ref @ dce239a (`crates/sc-treesitter`, `crates/sc-ai`, `crates/sc-jev-host/src/stream`).
Targets: `crates/sc-treesitter/` (`tsq` binary behind the `cli` feature), `crates/decide/` (new lib + bin crate) and `crates/sc-agent-py/` (new PyO3 crate, the sc-agent Python API).
Governing records: REQ-SAG-0501 and REQ-SAG-0503 (under `docs/decide/requirements/`, naming the CLI `decide`), REQ-SAG-0504 (a bad batch line stops decide before any send), REQ-SAG-0303 (`tsq`), REQ-SAG-0003 (each CLI writes its own general log), REQ-SAG-0004 (CLI and PyO3 accept project and agent_role, one field for role and name; slugs; reject, do not rewrite), REQ-SAG-0103 (provider and model separate; provider chosen by ranking, never required), REQ-SAG-0104 AC2 (cancel from Python), REQ-SAG-0401 (keys: env, repo-root .env, keyring), REQ-SAG-0801 (a local runner selected by its configured name), REQ-SAG-0901, REQ-SAG-0701 (batch through the runner), REQ-SAG-0002, REQ-SAG-0007, REQ-SAG-0009 / owner-lowlevel-no-tokio, ADR-SAG-0001 (one sync `Decision` trait), ADR-SAG-0004 (decide is the top layer: it may use every crate; it is where a backend is chosen and wired into the runner). Anchors: ANC-006 (every decide operation is also in the sc-agent Python API); batch parallelism is REQ-SAG-0701.
Rule for both binaries: argument parsing and output only. All behaviour lives in the libraries (A-3 sc-treesitter; A-1, A-2, A-4..A-9 for decide); decide's own logic (choosing the backend for `--model` with the optional `--provider` override, wiring `DecisionRunner`, models, doctor) is in decide's library target, which the `decide` binary and sc-agent-py share.

Decisions (assistant, 2026-11-09):
- Names: binaries `tsq` and `decide`; package `decide` (crates.io availability checked at publish time); subcommands `send`, `models`, `doctor`; tsq runs predefined queries only (REQ-SAG-0303: the CLI is a subset of PyO3).
- `--project` and `--agent-role` are required, no default; there is no `--agent-name`.
- Provider: `--provider` is an optional override. Absent: the first provider in `Provider::ranking(line of the model)` (A-1) whose key `resolve` detects (A-4); never required (REQ-SAG-0103).
- A batch is read and validated whole before anything is sent; the first bad line stops decide (REQ-SAG-0504).
- Exit: 0 when the command ran and every result succeeded, else 1.
- tsq's general log: sc-observability directly (sc-treesitter may not depend on sc-agent crates), service `tsq`, root `~/.sc-agent` (sc-observability's standard default for app sc-agent), file `~/.sc-agent/logs/tsq.log.jsonl`, one event per command; tsq reads no sc-agent config.
- ANC-006: this sprint delivers `crates/sc-agent-py` with one Python function per decide operation (section 3.3).

## 1. Copy list (a-10.sh)

tsq (`crates/sc-treesitter/`):

| Source | Target | What |
|---|---|---|
| `src/cli/mod.rs:11-12` | `src/cli.rs` | clap imports |
| `src/cli/mod.rs:36-77` | `src/cli.rs` | `Cli`, `Command`, `ExtractArgs` language/query/captures (becomes `QueryArgs`) |
| `src/cli/mod.rs:87-144` | `src/cli.rs` | path positional, `QueriesArgs`, `Captures`, `Parsed`, `parse()` |
| `src/cli/mod.rs:201-235` | `src/cli.rs` | `parse_captures`, test `cli_definition_is_consistent` |
| `src/cli/mod.rs:257-265` | `src/cli.rs` | test `captures_are_trimmed_and_ordered` |
| `src/main.rs:8-9,25-35,50-77,80-83,98-103` | `src/main.rs` | imports, `main`, `run` (languages, queries), capture refs, `json_line` |
| `tests/cli.rs:12,20-22,33-37,54-62,441-465` | `tests/cli.rs` | `binary()`, `run()`, `error_json`, `trim_newline`, languages + queries listing test |

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

Not copied: sc-ai `command.rs` dispatch (sc_jev_host compose, envelopes, readbacks, check.toml, `ask`, sessions dirs, registry test seams), sc-ai `cli.rs` jev groups and flags, `tests/argv.rs`, `tests/cli.rs` (2117 lines of envelope/scripted-backend tests), sc-jev-host `DecisionStream`/`serve_jsonl`/pool (queue and workers are A-9 behind `Decision::decide_stream`), the sc-treesitter `extract`/`--rev`/`git_io`, mimalloc, `ScAgentErrorResponse`, exit 2/3 retryable rule.

## 2. Required edits after copying

tsq:
- `cli.rs`: `#[command(name = "tsq", bin_name = "tsq", version, about = "Run predefined tree-sitter queries.")]`; delete `NOTES`/`after_help`, `PATH_VALUE_NAME` comment, `bin_name` comments. `Command::Extract(ExtractArgs)` -> `Command::Query(QueryArgs)` with positional `language`, `query`, `file: PathBuf` and `--captures`; drop `--rev`. `parse()` maps every non-help/version clap error to `sc_treesitter::Error::Usage { kind: format!("{:?}", e.kind()), detail: e.render().to_string() }` (id `treesitter.usage`); delete `value_parser_error`, `usage_error`, `message`, `parse_rev`. `parse_captures` returns `Result<Captures, String>` (clap carries its text into `Usage.detail`). Remove `sc_agent_types` import.
- `main.rs`: remove mimalloc, `git_io`, `JevError`, `ScAgentErrorResponse`. `run_query` command: `query_source` -> `read_source(&file)` -> `run_query(&language, text, &bytes, Some(&file), captures)` -> `to_json_bytes`. `json_line` generic over `Serialize`; listings print `languages()` ids and `queries()` (`QueryInfo` serializes as `{"language","query"}`). New `emit` (section 3) with no `.expect`/`unwrap`; a failed stdout write is `Error::Write`. The general log (Decisions) starts before the command runs (`Error::Log` if it cannot) and records one `tsq.command` event (subcommand, outcome, error id); a log write failure never changes the result.
- `tests/cli.rs`: `env!("CARGO_BIN_EXE_tsq")`, `HOME` set to a tempdir; `run()` = `Command::new(binary()).args(args).current_dir(cwd).output()` (no git isolation, no deadline helper); `error_json` expects exit 1 and the `{"id","message"}` line; listing test asserts `["python","rust"]` and `branches`; gate with `grammar-python`/`grammar-rust`.
- `Cargo.toml` (sc-treesitter, extends A-3): `cli = ["dep:clap", "dep:sc-observability"]` added to `default`; `clap = { version = "4", features = ["derive"], optional = true }`; `sc-observability = { version = "=1.5.0", optional = true }`; `[[bin]] name = "tsq", path = "src/main.rs", required-features = ["cli"]`; `[[test]] name = "cli", path = "tests/cli.rs", required-features = ["cli"]`.

decide:
- `cli.rs`: rewrite `Cli`/`Command` (section 3.2); `parse()` maps clap errors to `DecideError::Usage`; value parsers return `Result<_, String>`; message text "See decide --help.".
- `input.rs` (library): `request_bytes` loses `RepoRoot`/`joined_path` (paths are relative to the working directory), errors -> `InputNotFound`/`InputRead`; `read_line_bounded`: replace `&available[..take]` with `available.get(..take)` (indexing_slicing); wrap the copied tests in `#[cfg(test)] mod tests { use super::*; use std::io::Cursor; ... }`.
- `main.rs`: `print_stdout`/`emit` take `DecideError`; `emit` writes `DecideError::detail()` as one JSON line and must not `.expect`: when encoding fails it writes a fixed literal `{"id":"decide.encode","message":"..."}` line.

## 3. Rust code (exact; bodies elided)

### 3.1 tsq (`crates/sc-treesitter/src/cli.rs`, `src/main.rs`)

```rust
#[derive(Debug, clap::Parser)]
#[command(name = "tsq", bin_name = "tsq", version, about = "Run predefined tree-sitter queries.")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Command {
    /// Print the loaded grammars as a JSON array of language ids.
    Languages,
    /// Print the predefined queries as a JSON array of {language, query}.
    Queries(QueriesArgs),
    /// Run a predefined query on a file and print its captures as JSON.
    Query(QueryArgs),
}

#[derive(Debug, clap::Args)]
pub(crate) struct QueriesArgs {
    #[arg(long, value_name = "L", allow_hyphen_values = true)]
    pub(crate) language: Option<String>,
}

#[derive(Debug, clap::Args)]
pub(crate) struct QueryArgs {
    #[arg(value_name = "LANGUAGE")]
    pub(crate) language: String,
    #[arg(value_name = "QUERY")]
    pub(crate) query: String,
    #[arg(value_name = "FILE")]
    pub(crate) file: std::path::PathBuf,
    #[arg(long, value_name = "NAME,NAME", allow_hyphen_values = true, value_parser = parse_captures)]
    pub(crate) captures: Option<Captures>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Captures(Vec<String>);

#[derive(Debug)]
pub(crate) enum Parsed {
    Run(Cli),
    Print(String),
}

pub(crate) fn parse<I, T>(args: I) -> Result<Parsed, sc_treesitter::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone;
fn parse_captures(value: &str) -> Result<Captures, String>;

// main.rs
fn main() -> std::process::ExitCode;
fn run(parsed: Parsed) -> Result<Vec<u8>, sc_treesitter::Error>;
fn json_line<T: serde::Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, sc_treesitter::Error>;
/// One stderr line: the serialized Error `{"id": ..., "message": ...}`; exit 1.
fn emit(error: &sc_treesitter::Error) -> std::process::ExitCode;
/// sc_observability Logger, service "tsq", log_root = home_dir()/.sc-agent; Err -> Error::Log.
fn general_log() -> Result<sc_observability::Logger, sc_treesitter::Error>;
```

Output: stdout JSON + `\n`, exit 0. Failure: one stderr line `{"id": "treesitter.<variant>", "message": ...}`, exit 1, empty stdout. Errors: the 19-variant `sc_treesitter::Error` from A-3: `Usage` for argv, `Encode` for JSON, `Write` for a failed stdout/stderr write, `Log` when the general log cannot start.

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
pub use doctor::{doctor, DetectedKey, DoctorReport};
pub use error::DecideError;
pub use select::{models, send, send_on, SendOptions};

// src/select.rs  (the one place a backend is chosen; ADR-SAG-0004 top layer)
#[derive(Debug, Clone)]
pub struct SendOptions { pub model: String, pub provider: Option<String>, pub project: String, pub agent_role: String, pub service: &'static str } // service: "decide" | "sc-agent-py"
/// model names a config local runner -> LocalBackend (DaemonKind from runner.kind, key via resolve_var(key_env));
/// else model = catalog.resolve(ModelChoice); provider = options.provider, else the first of
/// Provider::ranking(line) (line = the short name, or catalog.line_of(id)) whose key resolve() finds, with
/// KeySources::system(Repo::work_tree_root(cwd)?); none -> NoProviderKey. TypeSafe -> SystemOneBackend::new(TYPESAFE_BASE_URL, key);
/// any other provider -> LlmDecisionBackend(LlmClient::new(provider, key), model).
fn backend(options: &SendOptions, config: &AgentConfig, catalog: &Catalog, sources: &KeySources<'_>) -> Result<Arc<dyn Backend>, DecideError>;
/// Loggers::open(config.log_root()?, options.service, SessionIdentity::new(Repo::detect(cwd)?, project, agent_role)?, None, None),
/// HttpClient::new(Arc::new(UreqTransport::default()), ResendPolicy::new(config.max_sends), loggers.general.clone()),
/// DecisionRunner::new(backend, http, loggers.session, loggers.general, RunnerConfig::from_config(config)).
fn runner(options: &SendOptions, config: &AgentConfig, backend: Arc<dyn Backend>) -> Result<DecisionRunner, DecideError>;
/// Session identity is validated before the backend is built; loads config from default_dir(); one request -> decide,
/// several -> decide_stream; `on_result` gets each DecisionResult as it finishes; then flush_session_log.
pub fn send(options: &SendOptions, requests: Vec<DecisionRequest>, cancel: &CancelToken, on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>) -> Result<(), DecideError>;
/// Testable core of send: any Decision (the runner in send, a fake in tests).
pub fn send_on(decision: &dyn Decision, requests: Vec<DecisionRequest>, cancel: &CancelToken, on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>) -> Result<(), DecideError>;
/// GET /v1/models of TypeSafe or a configured runner, through HttpClient::send_once (one-off holdoff on cancel.wait_until).
pub fn models(runner: Option<&str>, cancel: &CancelToken) -> Result<ModelMetadataList, DecideError>;

// src/doctor.rs (REQ-SAG-0503)
#[derive(Debug, serde::Serialize)]
pub struct DetectedKey { pub provider: Provider, pub source: &'static str }   // never the value
#[derive(Debug, serde::Serialize)]
pub struct DoctorReport { pub keys: Vec<DetectedKey>, pub missing: Vec<(Provider, String)>, pub ties: Vec<(ModelId, ModelId)> }
pub fn doctor(cancel: &CancelToken) -> Result<DoctorReport, DecideError>;

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

Types used: `SessionId`, `ErrorId`, `ErrorDetail`, `Catalog`, `ModelChoice`, `Provider` (incl. `PRIORITY`, `ranking`), `MODELS`, `CancelToken` (A-1); `AgentConfig`, `load`, `default_dir` (A-4); `KeySources`, `ResolvedKey`, `resolve`, `resolve_var` (A-4); `HttpClient`, `ResendPolicy`, `UreqTransport`, `Attempts`, `Next` (A-5); `Loggers`, `Repo` (incl. `work_tree_root`), `SessionIdentity` (A-8); `Decision`, `Backend`, `DecisionRequest`, `DecisionResult`, `SystemOneResponse`, `OrderedMap`, `QuestionDigest`, `ModelMetadataList` (A-2); `SystemOneBackend`, `TYPESAFE_BASE_URL` (A-6); `LocalBackend`, `DaemonKind` (A-6); `LlmClient` (incl. `models_request`, `decode_models`), `catalog_model_id`, `LlmDecisionBackend` (A-7); `DecisionRunner`, `RunnerConfig` (A-9).

Behaviour:
- `send` (single): `request_bytes` -> `SystemOneRequest::from_json` (failure: `RequestInvalid { line: None }`) -> `DecisionRequest::from` -> `decide::send` with one request (`decision.decide`) -> one `ResultLine` + `\n`. A failed decision is data in its result (owner-decision-trait), printed on stdout.
- `send --batch`: `read_batch` reads and validates every line first; a bad line returns its error (line, field, reason) before anything is sent (REQ-SAG-0504). Then `decision.decide_stream(Box::new(requests.into_iter()), &cancel)`; each result printed as one JSONL line as it arrives (fastest first, session ULID and input position); a failed request is data in its line and the batch continues. A stdout write failure cancels the token and returns `Write`. After the stream, `runner.flush_session_log()`; an Err is a command error (exit 1) printed after the results, never replacing them.
- Session identity: `--project`/`--agent-role` validated by `SessionIdentity::new` before anything is sent; an invalid value is `decide.log` (cause `log.invalid_slug`), nothing is sent and no row is written (REQ-SAG-0004). `repo` comes from `Repo::detect` on the working folder.
- `models`: `SystemOneBackend::models_request` for TypeSafe or the runner's base; sent with `HttpClient::send_once` (a one-off holdoff waits on `CancelToken::wait_until`, A-5); `decode_models`; prints the JSON.
- `doctor` (REQ-SAG-0503): (1) one line per detected provider key in `Provider::PRIORITY` order with its source (`env`, `.env`, `keyring`), never the value; (2) for each provider with a key, its model list (`LlmClient::models_request`/`decode_models`; TypeSafe through A-6 `models_request`), each id mapped back with `catalog_model_id`, one warning `{provider, model}` per id `catalog.line_of` does not know; (3) one warning per config model whose release date equals a built-in model of its line (the config model wins, A-1). Nothing in (2) or (3) when nothing is missing or tied.
- The configuration directory passed to `sc_agent_config::load` is `default_dir()` (`~/.sc-agent/`).
- Exit: 0 when the command ran and every result succeeded; 1 otherwise. Numbers are printed as serde_json gives them (REQ-SAG-0007). No tokio, no sleep, no threads of its own.

## 4. Error list

tsq: no new enum; uses `sc_treesitter::Error` (A-3).

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
#[pyfunction] #[pyo3(signature = (runner = None, cancel = None))] fn models(py: Python<'_>, runner: Option<&str>, cancel: Option<&CancelToken>) -> PyResult<String>;
#[pyfunction] #[pyo3(signature = (cancel = None))] fn doctor(py: Python<'_>, cancel: Option<&CancelToken>) -> PyResult<String>;   // DoctorReport JSON
#[pymodule] fn sc_agent(module: &Bound<'_, PyModule>) -> PyResult<()>;
```

decide subcommand -> Python: `send` -> `send`, `send --batch` -> `send_batch`, `models` -> `models`, `doctor` -> `doctor`.

No tokio, no mimalloc. Dev: `tempfile 3`.

## 6. New code

- decide: `src/error.rs` (~130 lines), `src/select.rs` (~110), `src/doctor.rs` (~70), `src/output.rs` (~40), `read_batch`/`open_lines` in `src/input.rs` (~50), `run`/`main` (~80), `Cli` (~70).
- sc-agent-py: `src/lib.rs` (~150), `pyproject.toml`, `tests/test_api.py`.
- tsq: `QueryArgs`, `Command::Query`, new `emit`, `general_log`, fixtures `tests/fixtures/match.rs` + golden `match.rs.json`, `tests/fixtures/not-utf8.py`.

## 7. Tests

tsq:
1. `cli_definition_is_consistent`, `captures_are_trimmed_and_ordered` (copied).
2. `languages` prints `["python","rust"]`; `queries --language python` lists `branches` (REQ-SAG-0303 AC2).
3. `query rust match-arms tests/fixtures/match.rs` stdout equals the golden JSON.
4. Unknown language -> stderr id `treesitter.unknown_language`, exit 1, empty stdout.
5. Lowest-layer failure (REQ-SAG-0002 AC1): non-UTF-8 fixture -> stderr `treesitter.not_utf8` with its message.
6. Missing positional -> `treesitter.usage` with clap's kind and text.
7. A run writes `$HOME/.sc-agent/logs/tsq.log.jsonl` with one `tsq.command` event (REQ-SAG-0003); a closed stdout -> `treesitter.write`.

decide (fake `Decision` or the real runner over a test transport; no network):
1. `cli_definition_is_consistent`; the 3 `read_line_bounded` tests (copied).
2. Single request from a file and from stdin: the fake receives the request unchanged; one `ResultLine` with `session_id` and `position`.
3. Each TypeSafe request example (openapi.json) parses and reaches `decide` unchanged; through the real runner + SystemOneBackend + a test transport answering the documented response, the API's response is printed (REQ-SAG-0501 AC1).
4. Batch of 3 where the fake finishes out of order and fails one: 3 lines in finish order, each with session ULID and position, the failed one with `error`; exit 1 (REQ-SAG-0701).
5. A 1,000-line batch whose line 412 has a `score` question without `criteria` -> stderr `decide.request_invalid` naming line 412, `criteria` and the reason (cause `decision.request_parse`), exit 1, and the transport is never called (REQ-SAG-0504 AC).
6. `--project "Phase B"` -> stderr `decide.log` with cause `log.invalid_slug`, exit 1, the backend is never built or called (REQ-SAG-0004 AC4). There is no `--agent-name` flag (clap rejects it).
7. A probability `0.123456789` is printed as `0.123456789` (REQ-SAG-0007).
8. A result carrying `decision.http` -> `http.network` prints both ids and messages in `error` (REQ-SAG-0002 AC1).
9. `models` with a test transport prints the TypeSafe models list (REQ-SAG-0501 AC2).
10. `doctor` with a fake provider list: one warning naming provider and model for a model in neither list; none when all are present; with Anthropic and OpenRouter keys both set, both are listed, Anthropic first, with their sources and no value; a config model dated like a built-in of its line is warned (REQ-SAG-0503 ACs).
11. `select::backend`: a config local runner name builds a LocalBackend; `--model haiku` with Anthropic and OpenRouter keys and no `--provider` goes to Anthropic; with only an OpenRouter key it goes to OpenRouter sending `anthropic/claude-haiku-5.5`; `--provider openrouter` overrides; a key found only in the repo-root `.env` is used; `--model jev` with no TypeSafe key -> `decide.no_provider_key` (REQ-SAG-0103 AC2).
12. A LocalBackend and a SystemOneBackend run batches in parallel through two runners over test transports (owner-local-decision-crate).
13. Broken stdout during a batch cancels the token and returns `decide.write`.
14. A run writes its general log to `<log_root>/logs/decide.log.jsonl` (REQ-SAG-0003).

sc-agent-py (`tests/test_api.py`, pytest over the maturin build and a test transport hook):
1. ANC-006: a test lists the decide subcommands from `decide --help` (send, models, doctor, plus `send --batch`) and asserts each has its Python function.
2. `send` and `send_batch` return the same ResultLines as the CLI for the same requests; `send_batch` results arrive fastest first through `on_result`.
3. REQ-SAG-0104 AC2: a `CancelToken` cancelled from Python ends a held-off request with `decision.cancelled`.
4. REQ-SAG-0004 AC3/AC4: a CLI call and a Python call with the same project and agent_role write rows with the same values; `project="Phase B"` raises `SagError` with id `decide.log` (cause `log.invalid_slug`) and writes no row.

CI: `cargo tree -p decide` and `-p sc-agent-py` list no tokio; clippy with the workspace denies (REQ-SAG-0002).

## 8. Dependencies on other sprints

A-3 (`sc_treesitter::{Error, languages, queries, QueryInfo, query_source, read_source, run_query, to_json_bytes}`); A-6 and A-7 (backends, models list, LLM client); A-9 (`DecisionRunner`, `RunnerConfig`, `RunnerError`); through them A-1, A-2, A-4, A-5, A-8 (names in section 3.2).

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-10.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] tsq: languages lists the loaded grammars and queries lists the predefined queries (REQ-SAG-0303).
- [ ] tsq: query <language> <query> <file> runs a predefined query and prints its captures as JSON; failures print one stderr line {id, message} with the treesitter.* id, exit 1.
- [ ] tsq: a non-UTF-8 file (lowest-layer failure) reaches stderr as treesitter.not_utf8 with its message (REQ-SAG-0002); a failed stdout write is treesitter.write.
- [ ] REQ-SAG-0003: tsq writes ~/.sc-agent/logs/tsq.log.jsonl and decide writes <log_root>/logs/decide.log.jsonl, each its own file.
- [ ] decide send: one request from a file or stdin, or a JSONL batch (--batch), with --model, --project and --agent-role (required; no --agent-name) and an optional --provider override; without --provider the provider is the first in the fixed ranking with a detected key (process env, repo-root .env, keyring), and a configured local runner is selected by name (REQ-SAG-0103, REQ-SAG-0401, REQ-SAG-0801); runs through the Decision trait only, over DecisionRunner (ADR-SAG-0001).
- [ ] REQ-SAG-0504: a batch line that does not match the Jev request schema stops decide before any request is sent, with an error naming the line, the field and the reason (1,000-line test, line 412).
- [ ] REQ-SAG-0701: batch handling in decide: decide --batch hands the whole batch to DecisionRunner, which sends the requests of the batch in parallel across its workers setting (8 by default, configurable); decide starts no thread of its own, checked by a test that greps crates/decide for thread::spawn. decide prints JSON for one request and JSONL for a batch, fastest first, each line tagged with its session ULID and input position; a failed request is printed in its result as an ErrorDetail chain and the rest of the batch continues.
- [ ] Each TypeSafe request example can be sent through decide send and returns the API's response (REQ-SAG-0501 AC1; test transport).
- [ ] The TypeSafe models list is returned by decide models (REQ-SAG-0501 AC2).
- [ ] decide doctor warns naming provider and model for every provider model in neither the built-in list nor the global configuration, prints no warning when all are present, warns when a config model has the release date of a built-in of its line, and lists every detected provider key (source, never the value) in the order it would be used (REQ-SAG-0503).
- [ ] ANC-006: crates/sc-agent-py (module sc_agent) offers send, send_batch, models and doctor, one per decide operation, checked by a test that lists both; a CancelToken cancelled from Python stops the request (REQ-SAG-0104 AC2); project and agent_role are accepted and validated through PyO3 as through the CLI (REQ-SAG-0004).
- [ ] An invalid --project or --agent-role (e.g. 'Phase B') is rejected as decide.log with cause log.invalid_slug and nothing is sent or logged (REQ-SAG-0004).
- [ ] REQ-SAG-0002: the error type DecideError (decide.* ids, implements ErrorId) is the return type of every public function of the decide library, and SagError is the error returned to Python by sc-agent-py; each wraps the real lower error unchanged and carries its id and message (AgentConfigError, TypesError, SecretsError, LogError, LocalError, DecisionError, HttpError, LlmError, RunnerError); a lowest-layer error (http.network) appears with its id and message in decide output; values are not rounded (REQ-SAG-0007).
- [ ] ADR-SAG-0004: every dependency in crates/decide/Cargo.toml points down the layer order (types; config, secrets, log; http; decision schema; decision backends; runner): decide is the top layer, above sc-decision-runner, and depends on sc-agent-types, sc-agent-config, sc-agent-secrets, sc-agent-log, sc-agent-http, sc-decision-schema, sc-decision-systemone, sc-decision-local, sc-decision-llm and sc-decision-runner; crates/sc-agent-py depends on decide; no lower-layer sc-agent library crate lists decide, tsq or sc-agent-py as a dependency, so none depends on a higher layer, checked by `cargo tree` for each lower crate.
- [ ] REQ-SAG-0009: Cargo.toml dependencies: neither decide nor tsq adds a tokio dependency, and no low-level sc-agent crate gains tokio with default features through them; `cargo tree -p decide -e normal` and `cargo tree -p tsq -e normal` list no tokio crate.
- [ ] REQ-SAG-0010: decide and tsq source call no sleep function, enforced by the workspace clippy disallowed-methods rule; clippy passes with the workspace denies (REQ-SAG-0002).
- [ ] ANC-007: the decide library and sc-agent-py write a log record for each send, models and doctor operation (never a key) to a GeneralLogger they inject into every library constructor they call, and sc-agent-py accepts an injected GeneralLogger; a test injects an in-memory GeneralLogger, runs one send and finds the records.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); decisions are in the design:
1. Tree-sitter CLI binary: tsq (owner).
2. decide package/bin/subcommands: decide; send, models, doctor; crates.io availability checked at publish time (assistant task).
3. The rename to decide is the owner's ("call the cli binaries 'decide'").
4. --project/--agent-role required (assistant); --provider is an optional override, provider chosen by ranking among detected keys (owner, REQ-SAG-0103); exit 0/1 (assistant).
5. ANC-006 (replaces ANC-002): sc-agent-py in this sprint.
6. tsq runs predefined queries only (REQ-SAG-0303).
Also: a bad batch line stops decide before any send (owner 'fail fast on first', REQ-SAG-0504); each CLI writes its own general log (REQ-SAG-0003).
No open questions.
