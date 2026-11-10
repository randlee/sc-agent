<!-- Generated from bead agt-4p2.3 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-3: sc-treesitter, tsq and sc-treesitter-py
- id: `a-3` (bead `agt-4p2.3`)
- branch: `sprint/a-3-sc-treesitter`
- depends on: a-1
- copy script: `docs/plans/phase-a/copy/a-3.sh`

## Description

Build sc-treesitter as a stand-alone library with PyO3 bindings (crates/sc-treesitter-py) and the tsq binary (non-default feature cli) for querying any grammar. Governing records: REQ-SAG-0301, REQ-SAG-0302, REQ-SAG-0303, REQ-SAG-0003 (tsq log). Code to copy from sc-agent-ref: see design.

## Design

```yaml
sprint:
  id: a-3
  closure_type: boundary
  target_boundary: sc-treesitter
  vertical_rationale: crates/sc-treesitter-py is a one-function #[pymodule] over sc-treesitter's `python` feature (section 3a) and cannot be built or tested without it, so the extension crate closes with the library it wraps.
  owned_paths: [crates/sc-treesitter/**, crates/sc-treesitter-py/**]
  must_follow:
    - sprint: a-1
      artifact: workspace root Cargo.toml (members = ["crates/*"], [workspace.package], [workspace.lints]) and clippy.toml (test allowances, disallowed-methods sleep rule)
      trigger: dev-push
  requirements: [REQ-SAG-0301, REQ-SAG-0302, REQ-SAG-0303, REQ-SAG-0003, REQ-SAG-0002, REQ-SAG-0001, REQ-SAG-0009, REQ-SAG-0010]
  adrs: [ADR-SAG-0004, ADR-SAG-0005]
  does_not_close:
    - "REQ-SAG-0302 AC2 for C# (csharp) and Go is the second release and is outside phase A (recorded in plan.md); the AC stays unchecked in the record. Adding them is table rows, queries and samples with no API change."
    - "tsq runs one short query and installs no signal handler (it writes no session log, so the default exit loses nothing)."
    - "REQ-SAG-0003 rotation/retention acceptance criteria: sc-observability's built-in policy, checked for the sc-agent logs in a-8; this sprint closes only 'tsq writes its own general log file'."
```

# Sprint A-3: sc-treesitter, the tsq binary and sc-treesitter-py (bead agt-4p2.3)

Copy script: `docs/plans/phase-a/copy/a-3.sh` (dry run 2026-11-09 after the tsq rows moved in from a-10.sh: 34 copy steps, 10 files, 751 lines before trimming).
Source: sc-agent-ref @ dce239a, `crates/sc-treesitter`. Target: `crates/sc-treesitter/`.
Governing records: ADR-SAG-0005 (sc-treesitter-py, the `python`/`cli` feature gates, the tsq bin inside sc-treesitter), REQ-SAG-0301, REQ-SAG-0302 (as amended: Python + Rust first release, C# `csharp` + Go second release), REQ-SAG-0303 (PyO3 bindings and `tsq`, the CLI a subset of PyO3), REQ-SAG-0003 (each CLI writes its own general log), REQ-SAG-0009 (no tokio), REQ-SAG-0010 (no sleep), REQ-SAG-0002 (now also denies `indexing_slicing`), REQ-SAG-0001, ADR-SAG-0004 (sc-treesitter stands alone).
Error convention: the workspace id rule (A-1) is `"<crate-prefix>.<variant_snake>"` with an id, a message and a cause chain. sc-treesitter MUST NOT depend on sc-agent-types (REQ-SAG-0301), so it does not implement A-1's `ErrorId` trait; it follows the same convention with inherent `id()`/`message()` and serializes as the same `{"id","message"}` shape as `ErrorDetail`. Ids use the prefix `treesitter`. Owner memories: owner-sc-treesitter, owner-treesitter-grammars, owner-error-contract, owner-errors-settled, owner-no-sleep, owner-lowlevel-no-tokio.
Owner corrections (2026-11-09) win over the older copy plan: KEEP the Python and Rust grammars and queries; C#/Go are the second wave; use the owner's 17-variant error list less `PositionOverflow` (unreachable, round 2 RBP-F006) plus `Write` and `Log` for the `tsq` binary, 18 variants (the owner: missing errors are added in the plan); no sc-agent deps, no git; PyO3 support types behind a `python` feature, built into the extension module by `crates/sc-treesitter-py` (section 3a).

Scope: this sprint is the library, its output types, error ids, JSON encoder and file reader, the PyO3 support types, `crates/sc-treesitter-py`, and the `tsq` binary (`src/main.rs`, `src/cli.rs`, `tests/cli.rs`, clap, sc-observability, the non-default `cli` feature, section 3b). It is the single owner of `crates/sc-treesitter/Cargo.toml`; no other sprint edits this crate. Install: `cargo install sc-treesitter --features cli`.

Decisions (2026-11-09; the crate set, the `python`/`cli` features and the tsq placement are decided in ADR-SAG-0005 (Approved)):
- PyO3 placement: a separate crate `crates/sc-treesitter-py` builds the extension module; sc-treesitter stays an rlib with the support types behind `python` (section 3a); decided in ADR-SAG-0005 (Approved).
- Output shape: flat captures sorted by (start_byte, name, end_byte), as copied; each capture is one part of a match.
- Logging: the library logs nothing (stand-alone, REQ-SAG-0301); the `tsq` binary writes its own general log (REQ-SAG-0003, section 3b).
- tsq (owner named it `tsq`): subcommands `languages`, `queries`, `query`; runs predefined queries only (REQ-SAG-0303: the CLI is a subset of PyO3). Exit 0 on success, 1 on any error.
- tsq's general log: sc-observability directly (sc-treesitter may not depend on sc-agent crates), service `tsq`, root `~/.sc-agent` (sc-observability's standard default for app sc-agent), file `~/.sc-agent/logs/tsq.log.jsonl`, one event per command; tsq reads no sc-agent config.
- `cli` is not a default feature, so a library consumer (and sc-treesitter-py) never builds clap or sc-observability; `[[bin]] tsq` has `required-features = ["cli"]`.
- Second wave (C# `csharp`, Go) is a later sprint, one predefined query each (proposed: csharp `switch-sections`, go `switch-cases`); each grammar is one table row, one `.scm` file and one test sample.

## 1. Copy list (a-3.sh)

| Source (`crates/sc-treesitter/`) | Target (`crates/sc-treesitter/`) | What |
|---|---|---|
| `LICENSE` | `LICENSE` | MIT |
| `src/grammar.rs:12-100` | `src/grammar.rs` | `Grammar` table, python/rust loaders and rows, `languages`, `language`, `lookup`, `all` |
| `src/query.rs:9-75` | `src/query.rs` | `QueryInfo`, `PYTHON`/`RUST` consts, `queries`, `query_source` |
| `src/extract.rs:7` | `src/run.rs` | tree-sitter imports |
| `src/extract.rs:132-138` | `src/run.rs` | `RawCapture` |
| `src/extract.rs:176-252` | `src/run.rs` | `extract` body: parse, compile, matches loop, sort, ERROR/MISSING check (becomes `run_query`) |
| `src/extract.rs:273-280` | `src/run.rs` | capture text via `source.get(..)` |
| `src/extract.rs:293-312` | `src/run.rs` | `capture_allowed`, `inclusive_lines` |
| `src/extract.rs:326-359` | `src/run.rs` | `error_line_inside` (becomes `error_inside`) |
| `src/extract.rs:361-363,494-523,552-611,639-658,679-702,734` | `src/run.rs` (`mod tests`) | tests: rust match arms; same-line determinism; parse error; unknown language; duplicate captures; empty result; invalid query; MISSING node; clean source |
| `src/lib.rs:34-59` | `src/lib.rs` | module wiring, re-exports |
| `src/render.rs:6-21` | `src/lib.rs` | pretty JSON + `\n` (becomes `to_json_bytes`) |
| `assets/queries/rust/match-arms.scm` | same | Rust predefined query `match-arms` |
| `assets/queries/python/branches.scm:2-5` | same | Python predefined query `branches`, case_clause half only (no Refusal/lite-gate half) |

tsq binary (feature `cli`; rows moved from a-10.sh):

| Source (`crates/sc-treesitter/`) | Target (`crates/sc-treesitter/`) | What |
|---|---|---|
| `src/cli/mod.rs:11-12` | `src/cli.rs` | clap imports |
| `src/cli/mod.rs:36-77` | `src/cli.rs` | `Cli`, `Command`, `ExtractArgs` language/query/captures (becomes `QueryArgs`) |
| `src/cli/mod.rs:87-144` | `src/cli.rs` | path positional, `QueriesArgs`, `Captures`, `Parsed`, `parse()` |
| `src/cli/mod.rs:201-235` | `src/cli.rs` | `parse_captures`, test `cli_definition_is_consistent` |
| `src/cli/mod.rs:257-265` | `src/cli.rs` | test `captures_are_trimmed_and_ordered` |
| `src/main.rs:8-9,25-35,50-77,80-83,98-103` | `src/main.rs` | imports, `main`, `run` (languages, queries), capture refs, `json_line` |
| `tests/cli.rs:12,20-22,33-37,54-62,441-465` | `tests/cli.rs` | `binary()`, `run()`, `error_json`, `trim_newline`, languages + queries listing test |

Not copied: `src/git_io.rs`, the rest of `src/cli/` and `src/main.rs` (`extract`, `--rev`, envelopes), the rest of `tests/` (git/envelope tests), `ExtractInput` and its getters (`extract.rs:12-130`), the builder/subjects code (`extract.rs:254-272,281-291`), `u32_from_usize` (`:314-324`), digest/subjects/cross-impl/manifest/newtype tests (`:364-493,524-551,613-638,659-678,703-733`), mimalloc, `PRODUCER_NAME`/`PRODUCER_MODE`, the old README and Cargo.toml.

## 2. Required edits after copying

- All files: delete every `sc_agent_types` import and use; replace `JevError` with `crate::Error` variants (section 4); delete doc text about sc/subjects/1, `query_sha256`, "Phase A", git, producers; add module docs (`//!`) to each file.
- `src/grammar.rs`: features renamed `python` -> `grammar-python`, `rust` -> `grammar-rust` (the `python` feature is PyO3). Remove `LanguageInfo` (the hand-copied `grammar`/`grammar_version` strings are no longer output) and the public `language()`; `Grammar` keeps `id`, `load`, `queries`. `languages()` returns ids. Add `KNOWN_LANGUAGES` (every id this crate version can be built with, compiled or not) so `lookup` returns `GrammarUnavailable` for a known id whose feature is off and `UnknownLanguage` otherwise. Rewrite the doctest without `LanguageInfo`.
- `src/query.rs`: `QueryInfo` derives `Serialize` and skips `source` in serde; delete the `query_sha256` paragraphs (copied lines 12-15 and 50-54 of the target); `query_not_found` -> `Error::UnknownQuery { language, name }`.
- `src/run.rs`: rename `extract` to `run_query` with the signature in section 3; delete `ExtractInput`, `path`/`rev`/`blob_sha` locals, `PRODUCER_*`, the empty-query special case (an empty query compiles to zero patterns and returns `NoMatches`). Before UTF-8: `source.len() > MAX_SOURCE_BYTES` -> `InputTooLarge`. `from_utf8` error -> `NotUtf8 { byte_offset: e.valid_up_to() }`. `set_language` error -> `GrammarLoad { detail: e.to_string() }`. `Query::new` error -> `QueryCompile` keeping `offset,row,column,kind (Debug of QueryErrorKind),message`. After compiling, each name in `captures` must be in `query.capture_names()` else `QueryCaptureInvalid`. `parse` `None` -> `ParseFailed`. Replace `capture_names[index]` with `.get(index)` (no index; a missing name is unreachable by tree-sitter contract, so skip it with `continue`, no panic) and delete both `debug_assert!`. `found.is_empty()` -> `NoMatches { language, query }`. `error_line_inside` returns the node `(row, column, kind)` of the earliest ERROR/MISSING node (no `unwrap_or(u32::MAX)`), giving `SyntaxError { row, column, node_kind }`. `inclusive_lines` returns `(usize, usize)` computed with `row.saturating_add(1)`: the source is at most `MAX_SOURCE_BYTES` (8 MiB), so a row is below 8 Mi and the add cannot saturate; no u32 narrowing, no error. The copied text lookup (lines 273-280) maps to `Capture` construction; `source.get(start..end)` on the already-validated UTF-8 text cannot be `None` at tree-sitter node boundaries, so a `None` skips that capture with `continue` (no panic, no error variant). Remove `drop(matches)` comment noise.
- `src/run.rs` tests: port to `run_query`; assert `Error` variants (`matches!`) and `id()` (e.g. `"treesitter.no_matches"`), not `.code()`; replace python tests that used the cross-impl `hook.py` fixture with inline sources; in the determinism test drop the subject-id asserts and keep `once == twice` and same-line byte ordering; gate tests with `grammar-python`/`grammar-rust`.
- `src/lib.rs`: modules `error`, `grammar`, `query`, `run`, `source`, `#[cfg(feature = "python")] python`; re-exports as in section 3; `#![warn(missing_docs)]`. Fold `render.rs` as generic `to_json_bytes`; encode error -> `Error::Encode { detail }`.
- `assets/queries/python/branches.scm`: remove the sentence about `@_fn` from the comment (there is no helper capture now).
- tsq `src/cli.rs`: `#[command(name = "tsq", bin_name = "tsq", version, about = "Run predefined tree-sitter queries.")]`; delete `NOTES`/`after_help`, `PATH_VALUE_NAME` comment, `bin_name` comments. `Command::Extract(ExtractArgs)` -> `Command::Query(QueryArgs)` with positional `language`, `query`, `file: PathBuf` and `--captures`; drop `--rev`. `parse()` maps every non-help/version clap error to `sc_treesitter::Error::Usage { kind: format!("{:?}", e.kind()), detail: e.render().to_string() }` (id `treesitter.usage`); delete `value_parser_error`, `usage_error`, `message`, `parse_rev`. `parse_captures` returns `Result<Captures, String>` (clap carries its text into `Usage.detail`). Remove `sc_agent_types` import.
- tsq `src/main.rs`: remove mimalloc, `git_io`, `JevError`, `ScAgentErrorResponse`. `query` command: `query_source` -> `read_source(&file)` -> `run_query(&language, text, &bytes, Some(&file), captures)` -> `to_json_bytes`. `json_line` generic over `Serialize`; listings print `languages()` ids and `queries()` (`QueryInfo` serializes as `{"language","query"}`). New `emit` (section 3b) with no `.expect`/`unwrap`; a failed stdout write is `Error::Write`. The general log (Decisions) starts before the command runs (`Error::Log` if it cannot) and records one `tsq.command` event (subcommand, outcome, error id); a log write failure never changes the result.
- tsq `tests/cli.rs`: `env!("CARGO_BIN_EXE_tsq")`, `HOME` set to a tempdir; `run()` = `Command::new(binary()).args(args).current_dir(cwd).output()` (no git isolation, no deadline helper); `error_json` expects exit 1 and the `{"id","message"}` line; listing test asserts `["python","rust"]` and `branches`; gate with `grammar-python`/`grammar-rust`.

## 3. Public API (exact Rust; bodies elided)

```rust
// src/lib.rs
pub use error::Error;
pub use query::{QueryInfo, queries, query_source};
pub use run::{Capture, run_query};
pub use source::{MAX_SOURCE_BYTES, read_source};
pub use grammar::languages;
pub fn to_json_bytes<T: serde::Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, Error>;

// src/grammar.rs
pub(crate) struct Grammar {
    pub(crate) id: &'static str,
    pub(crate) load: fn() -> tree_sitter::Language,
    pub(crate) queries: &'static [QueryInfo],
}
pub(crate) const KNOWN_LANGUAGES: &[&str] = &["python", "rust"];
static GRAMMARS: &[Grammar] = &[ /* #[cfg(feature = "grammar-python")] python, #[cfg(feature = "grammar-rust")] rust */ ];
pub fn languages() -> impl ExactSizeIterator<Item = &'static str>;
pub(crate) fn lookup(id: &str) -> Result<&'static Grammar, Error>;
pub(crate) fn all() -> &'static [Grammar];

// src/query.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct QueryInfo {
    pub language: &'static str,
    #[serde(rename = "query")]
    pub name: &'static str,
    #[serde(skip)]
    pub source: &'static str,
}
pub fn queries(language: Option<&str>) -> Result<Vec<&'static QueryInfo>, Error>;
pub fn query_source(language: &str, name: &str) -> Result<&'static str, Error>;

// src/run.rs
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(frozen, get_all, module = "sc_treesitter"))]
#[non_exhaustive]
pub struct Capture {
    pub name: String,
    pub text: String,
    pub start_line: usize, // 1-based, inclusive
    pub end_line: usize,   // 1-based, inclusive
    pub start_byte: usize,
    pub end_byte: usize,
}
/// Runs any query against any loaded grammar; captures sorted by (start_byte, name, end_byte).
/// Names starting with `_` are helpers and never returned; `captures` is an allow-list.
pub fn run_query(
    language: &str,
    query: &str,
    source: &[u8],
    path: Option<&std::path::Path>, // only for error detail
    captures: Option<&[&str]>,
) -> Result<Vec<Capture>, Error>;

// src/source.rs (new)
pub const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024; // the old 8 MiB input cap
pub fn read_source(path: &std::path::Path) -> Result<Vec<u8>, Error>; // NotFound -> FileNotFound, other io -> FileRead, > cap -> InputTooLarge

// src/python.rs (feature "python"; PyO3 support types; the #[pymodule] is in crates/sc-treesitter-py)
pyo3::create_exception!(sc_treesitter, TreesitterError, pyo3::exceptions::PyException);
impl From<Error> for pyo3::PyErr; // TreesitterError(args = (id, message))
#[pyo3::pyfunction] pub fn languages() -> Vec<&'static str>;
#[pyo3::pyfunction] #[pyo3(signature = (language = None))]
pub fn queries(language: Option<&str>) -> pyo3::PyResult<Vec<(&'static str, &'static str)>>;
#[pyo3::pyfunction] pub fn query_source(language: &str, name: &str) -> pyo3::PyResult<&'static str>;
#[pyo3::pyfunction] #[pyo3(signature = (language, query, source, captures = None))]
pub fn run_query(
    language: &str,
    query: &str,
    source: &pyo3::Bound<'_, pyo3::PyAny>, // bytes or str; other types -> Error::Python
    captures: Option<Vec<String>>,
) -> pyo3::PyResult<Vec<Capture>>;
/// Adds the functions, `Capture` and `TreesitterError` to whichever #[pymodule] owns them.
pub fn register(module: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()>;
```

Synchronous, no tokio, no threads, no sleep, no logging (stand-alone, REQ-SAG-0301).

## 3a. crates/sc-treesitter-py (new)

```toml
[package]
name = "sc-treesitter-py"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license = "MIT"
publish = false

[lib]
name = "sc_treesitter"
crate-type = ["cdylib"]

[lints]
workspace = true

[dependencies]
sc-treesitter = { path = "../sc-treesitter", features = ["python"] }
pyo3 = { version = "=0.29.2", features = ["abi3-py310", "extension-module"] }
```

```rust
// src/lib.rs
#[pyo3::pymodule]
fn sc_treesitter(module: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> { sc_treesitter::python::register(module) }
```

`pyproject.toml` (maturin): wheel `sc-treesitter`, module `sc_treesitter`. `tests/test_query.py` holds test 9.

## 3b. tsq (`src/cli.rs`, `src/main.rs`; feature `cli`)

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

Output: stdout JSON + `\n`, exit 0. Failure: one stderr line `{"id": "treesitter.<variant>", "message": ...}`, exit 1, empty stdout. Errors: the 18-variant `Error` (section 4): `Usage` for argv, `Encode` for JSON, `Write` for a failed stdout/stderr write, `Log` when the general log cannot start. Synchronous: no tokio, no threads, no sleep.

## 4. Error enum (18 variants; `src/error.rs`, new)

Lower errors here are external (tree-sitter, std::io, std::str, serde_json, clap (tsq), pyo3, sc-observability (tsq)); each variant keeps the lower error's kind and message in its fields. No sc-agent error is wrapped. Hand-written `Display` (`"<id>: <message>"`), no thiserror, no serde tag.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Id is not a language this crate knows.
    UnknownLanguage { language: String },                                      // "treesitter.unknown_language"
    /// Known language whose grammar feature is not compiled in.
    GrammarUnavailable { language: String },                                   // "treesitter.grammar_unavailable"
    /// tree_sitter::LanguageError (ABI/version mismatch); detail = its Display.
    GrammarLoad { language: String, detail: String },                          // "treesitter.grammar_load"
    /// No predefined query by that name for the language.
    UnknownQuery { language: String, name: String },                           // "treesitter.unknown_query"
    /// tree_sitter::QueryError, every field kept; kind = Debug of QueryErrorKind (display only; not a stable value to match on).
    QueryCompile { language: String, offset: usize, row: usize, column: usize, kind: String, detail: String }, // "treesitter.query_compile"
    /// Capture filter names a capture the query does not define.
    QueryCaptureInvalid { capture: String },                                   // "treesitter.query_capture_invalid"
    FileNotFound { path: std::path::PathBuf },                                 // "treesitter.file_not_found"
    /// io error other than NotFound; kind = Debug of io::ErrorKind (display only), detail = Display.
    FileRead { path: std::path::PathBuf, kind: String, detail: String },       // "treesitter.file_read"
    /// Utf8Error::valid_up_to.
    NotUtf8 { path: Option<std::path::PathBuf>, byte_offset: usize },          // "treesitter.not_utf8"
    /// Source over MAX_SOURCE_BYTES.
    InputTooLarge { path: Option<std::path::PathBuf>, bytes: u64, limit: u64 }, // "treesitter.input_too_large"
    /// Parser::parse returned None.
    ParseFailed { language: String },                                          // "treesitter.parse_failed"
    /// ERROR or MISSING node inside a matched span; 0-based row/column.
    SyntaxError { path: Option<std::path::PathBuf>, row: usize, column: usize, node_kind: String }, // "treesitter.syntax_error"
    /// The query ran and matched nothing (after helper/allow-list filtering).
    NoMatches { language: String, query: String },                             // "treesitter.no_matches"
    /// serde_json error Display.
    Encode { detail: String },                                                 // "treesitter.encode"
    /// tsq argument error (section 3b); kind = Debug of clap ErrorKind (display only), detail = clap's text.
    Usage { kind: String, detail: String },                                    // "treesitter.usage"
    /// PyO3 argument conversion failure (bindings only); detail = PyErr Display.
    Python { detail: String },                                                 // "treesitter.python"
    /// `tsq` could not write stdout or stderr (section 3b); kind = Debug of io::ErrorKind (display only), detail = Display.
    Write { kind: String, detail: String },                                    // "treesitter.write"
    /// `tsq`'s general log could not start (section 3b); detail = the sc-observability error's code and message.
    Log { detail: String },                                                    // "treesitter.log"
}
impl Error {
    /// Stable id "treesitter.<variant_snake>".
    pub fn id(&self) -> &'static str;
    /// One line naming the cause and every field.
    pub fn message(&self) -> String;
}
impl std::fmt::Display for Error {}            // "<id>: <message>"
impl std::error::Error for Error {}
impl serde::Serialize for Error {}             // {"id": ..., "message": ...}, the ErrorDetail shape
```

A unit test checks every id is distinct and matches `^treesitter\.[a-z_]+$`, and that the serialized `id` equals `id()`.

## 5. Cargo.toml (new)

```toml
[package]
name = "sc-treesitter"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license = "MIT"
description = "Load tree-sitter grammars and run any query against them."
include = ["Cargo.toml", "README.md", "LICENSE", "src/**", "assets/queries/**"]

[lints]
workspace = true

[features]
default = ["grammar-python", "grammar-rust"]
grammar-python = ["dep:tree-sitter-python"]
grammar-rust = ["dep:tree-sitter-rust"]
python = ["dep:pyo3"]
cli = ["dep:clap", "dep:sc-observability"]   # NOT in default: `cargo install sc-treesitter --features cli`

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tree-sitter = "=0.27.1"
tree-sitter-python = { version = "=0.25.0", optional = true }
tree-sitter-rust = { version = "=0.24.2", optional = true }
pyo3 = { version = "=0.29.2", optional = true, features = ["abi3-py310"] }
clap = { version = "4", features = ["derive"], optional = true }
sc-observability = { version = "=1.5.0", optional = true }

[[bin]]
name = "tsq"
path = "src/main.rs"
required-features = ["cli"]

[[test]]
name = "cli"
path = "tests/cli.rs"
required-features = ["cli"]
```

Workspace lints (root `Cargo.toml` and `clippy.toml`, owned by A-1; this sprint does not edit them, `members = ["crates/*"]` picks up both crates): `[workspace.lints.clippy] panic = "deny"`, `unwrap_used = "deny"`, `expect_used = "deny"`, `indexing_slicing = "deny"`, and `clippy.toml` with `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`, `allow-indexing-slicing-in-tests = true`. sc-lint does not enforce these yet. README.md: 3-4 lines, new.

Second wave (a later sprint; see Decisions): add `grammar-csharp` (`tree-sitter-c-sharp` 0.23.5, id `csharp`) and `grammar-go` (`tree-sitter-go` 0.25.0) rows to `GRAMMARS` and `KNOWN_LANGUAGES`, with their queries; no API change. Building them against tree-sitter 0.27.1 is not yet confirmed.

## 6. New code

- `src/error.rs` (section 4), ~110 lines.
- `src/source.rs`: `read_source`, `MAX_SOURCE_BYTES`, ~30 lines.
- `src/python.rs`: section 3, ~90 lines.
- `Capture` and the capture-filter check in `src/run.rs`.
- `README.md` (names `cargo install sc-treesitter --features cli`), `Cargo.toml`.
- tsq: `QueryArgs`, `Command::Query`, new `emit`, `general_log`, fixtures `tests/fixtures/match.rs` + golden `match.rs.json`, `tests/fixtures/not-utf8.py`.

## 7. Tests

Each test names the acceptance line it proves (`AC: <line key>`); the assertion itself is the acceptance line.

Ported (from the copied ranges; AC: REQ-SAG-0302 same call, REQ-SAG-0002 errors): Rust `match-arms` predefined query; same-line determinism and byte order; ERROR node -> `SyntaxError`; MISSING node -> `SyntaxError`; unknown language -> `UnknownLanguage`; duplicate captures in one match; empty result -> `NoMatches`; invalid query -> `QueryCompile` with row/column/offset/kind; clean source succeeds.
New (library, `src/*` unit tests):
1. `python_and_rust_through_the_same_call` (AC: REQ-SAG-0302 same call).
2. `every_loaded_language_runs_through_run_query` (AC: REQ-SAG-0302 every id); per-id sample table, so a new grammar adds a row and a sample.
3. Helper capture `@_x` never returned; undefined allow-list capture -> `QueryCaptureInvalid` (AC: REQ-SAG-0002 errors).
4. `NotUtf8` byte_offset, `InputTooLarge` bytes/limit; a capture on the last line of a multi-line source reports 1-based inclusive lines (AC: Positions, REQ-SAG-0002 errors).
5. `GrammarUnavailable` built without `grammar-rust` (AC: REQ-SAG-0302 every id; CI `--no-default-features --features grammar-python`).
6. `read_source`: missing file -> `FileNotFound`; a directory -> `FileRead` with kind (AC: REQ-SAG-0002 errors).
7. `queries(None)` / `queries(Some("cobol"))` / unknown `query_source` (AC: REQ-SAG-0303 listings).
8. All 18 ids distinct, `^treesitter\.[a-z_]+$`, serialized id = `id()`; `to_json_bytes` ends with `\n` (AC: REQ-SAG-0002 errors).
PyO3 (`crates/sc-treesitter-py/tests/test_query.py`, pytest over the maturin build):
9. Caller-written query returns `Capture` objects; non-UTF-8 `bytes` raises `TreesitterError(treesitter.not_utf8, message)` (AC: REQ-SAG-0303 PyO3, REQ-SAG-0002 lowest-layer PyO3).
tsq (`src/cli.rs` unit tests and `tests/cli.rs`, `--features cli`):
10. `cli_definition_is_consistent`, `captures_are_trimmed_and_ordered` (copied; AC: tsq query).
11. `languages` / `queries --language python` (AC: REQ-SAG-0303 tsq listings).
12. `query rust match-arms tests/fixtures/match.rs` equals the golden JSON; unknown language -> `treesitter.unknown_language`, exit 1, empty stdout; missing positional -> `treesitter.usage` (AC: tsq query).
13. Non-UTF-8 fixture -> stderr `treesitter.not_utf8`; closed stdout -> `treesitter.write` (AC: REQ-SAG-0002 lowest-layer tsq).
14. A run writes `$HOME/.sc-agent/logs/tsq.log.jsonl` with one `tsq.command` event (AC: REQ-SAG-0003 tsq.log).

## Required validation

- `cargo build -p sc-treesitter` and `cargo build -p sc-treesitter --features cli`; `maturin build -m crates/sc-treesitter-py/Cargo.toml`.
- `cargo clippy -p sc-treesitter -p sc-treesitter-py --all-features --all-targets -- -D warnings` with the A-1 workspace denies and clippy.toml (sleep disallowed).
- `cargo test -p sc-treesitter --all-features`; `cargo test -p sc-treesitter --no-default-features --features grammar-python`; `pytest crates/sc-treesitter-py/tests`.
- `cargo tree -p sc-treesitter -e normal,build` (default features): no `sc-agent*`/`sc-decision*`, no `sc-observability`, no `clap`, no `tokio`.
- `cargo tree -p sc-treesitter --features cli -e normal`: no `sc-agent*`/`sc-decision*`, no `tokio`.
- `grep -rnE 'thread::sleep|time::sleep|sleep_until' crates/sc-treesitter crates/sc-treesitter-py` finds nothing.

## 8. Dependencies

`tree-sitter =0.27.1`, `tree-sitter-python =0.25.0` (optional), `tree-sitter-rust =0.24.2` (optional), `serde 1` (derive), `serde_json 1`, `pyo3 =0.29.2` with `abi3-py310` (optional), `clap 4` (derive, optional, `cli`), `sc-observability =1.5.0` (optional, `cli`). No sc-agent crate, no tokio, no git, no sc-sha, no mimalloc, no thiserror. Dev: `tempfile 3` (for the `read_source` tests only; other fixtures are inline).

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-3.sh` (sc-agent-ref@dce239a, 34 copy steps incl. the tsq rows), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] REQ-SAG-0301: `cargo tree -p sc-treesitter -e normal,build` lists no sc-agent or sc-decision crate, with default features and with `--features cli`; the crate has no agent or decision-model code.
- [ ] REQ-SAG-0302 same call: sc-treesitter `run_query` runs one caller-written query against Python source and against Rust source (first release).
- [ ] REQ-SAG-0302 every id: every id from `languages()` runs through the same `run_query` call; adding a grammar adds a `GRAMMARS` row and a test sample only, no API change.
- [ ] REQ-SAG-0303 listings: sc-treesitter `languages()` lists the loaded grammars and `queries()` lists the predefined queries (python/branches, rust/match-arms).
- [ ] REQ-SAG-0303 PyO3: a caller-written query runs through crates/sc-treesitter-py (module sc_treesitter, feature python) and returns its matches as Capture objects.
- [ ] REQ-SAG-0303 tsq listings: `tsq languages` prints `["python","rust"]` and `tsq queries --language python` lists `branches`.
- [ ] tsq query: `tsq query <language> <query> <file>` runs a predefined query and prints its captures as JSON (exit 0); every failure prints one stderr line {id, message} with a treesitter.* id, exit 1, empty stdout.
- [ ] REQ-SAG-0002 errors: all 18 sc-treesitter Error variants (the owner's 17 less the unreachable PositionOverflow, plus Write and Log for tsq) exist with stable ids treesitter.<variant_snake> (the workspace convention, without depending on sc-agent-types), serialized as {id, message}; each keeps the lower error's kind and message.
- [ ] REQ-SAG-0002 lowest-layer PyO3: an injected non-UTF-8 source reaches the sc-treesitter-py entry point as TreesitterError with id treesitter.not_utf8 and its message.
- [ ] REQ-SAG-0002 lowest-layer tsq: a non-UTF-8 file reaches tsq's stderr as treesitter.not_utf8 with its message; a failed stdout write is treesitter.write.
- [ ] REQ-SAG-0002 clippy: `cargo clippy -p sc-treesitter -p sc-treesitter-py --all-features --all-targets -- -D warnings` passes with panic, unwrap_used, expect_used and indexing_slicing denied in non-test code.
- [ ] Positions: no git, no sc-sha; no u32 clamping; 1-based lines are `row.saturating_add(1)` under the documented 8 MiB MAX_SOURCE_BYTES bound, so there is no position error variant.
- [ ] REQ-SAG-0003 tsq.log: a tsq run writes its own general log `~/.sc-agent/logs/tsq.log.jsonl` (one `tsq.command` event per command), a file separate from decide's.
- [ ] Stand-alone library: the sc-treesitter library writes no log (REQ-SAG-0301); `cli` is not a default feature and `[[bin]] tsq` has `required-features = ["cli"]`, so `cargo tree -p sc-treesitter -e normal` (default features) lists no sc-observability and no clap.
- [ ] REQ-SAG-0009: `cargo tree -p sc-treesitter -e normal` and `cargo tree -p sc-treesitter --features cli -e normal` list no tokio crate.
- [ ] REQ-SAG-0010: sc-treesitter (library and tsq) and sc-treesitter-py are Rust crates in the sc-agent workspace, and their source calls no sleep function (std::thread::sleep, tokio::time::sleep or any other), enforced by the workspace clippy disallowed-methods rule.
- [ ] ADR-SAG-0005: crates/sc-treesitter-py is the only crate with the `#[pymodule]`, sc-treesitter's `python` and `cli` features are both off by default, and the `tsq` bin lives in sc-treesitter behind `required-features = ["cli"]`, checked by `cargo metadata` (features, bin targets) in CI.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); each decision is recorded once in the design:
1. PyO3 placement: crates/sc-treesitter-py (module and wheel sc_treesitter / sc-treesitter); decided in ADR-SAG-0005 (Approved).
2. Output shape: flat sorted captures (assistant).
3. Logging: none in the library (stand-alone); the tsq binary logs (this sprint, REQ-SAG-0003).
4. C#/Go: a later sprint; one predefined query each, proposed by the assistant.
5. tsq write failure: new variant Write (treesitter.write); Log added for tsq's general log.
6. Tree-sitter CLI binary name: tsq (owner); it runs predefined queries only (REQ-SAG-0303).
7. Hardening round 1 (2026-11-09): the tsq binary, its copy rows, tests and acceptance moved here from A-10 (PLAN-SCOPE-004, PLAN-CRIT-007/011/012); `cli` is a non-default feature; this sprint is the single owner of crates/sc-treesitter/Cargo.toml; must_follow A-1 (workspace root, PLAN-CRIT-004).
Round 2 (2026-11-09): ADR-SAG-0005 cited; PositionOverflow deleted (18 variants), impossible slice skipped (RBP-F006); C#/Go outside phase A (REQ-QA-008); tsq needs no signal handling (REQ-SAG-0011).
No open questions.

Dev round 1 corrections (2026-11-10, from the a-3 developer; code at 94f8039 is right, doc follows): (1) section 7 test 8 id pattern is `^treesitter\.[a-z][a-z0-9_]*$` (treesitter.not_utf8 has a digit); (2) section 3b general_log returns `sc_observability::v2::Logger` (the v1 Logger is deprecated in 1.5.0 and fails -D warnings); (3) section 3a: sc-treesitter-py has build.rs + build-dependency `pyo3-build-config = "=0.29.2"` calling add_extension_module_link_args() so plain cargo links on macOS.
