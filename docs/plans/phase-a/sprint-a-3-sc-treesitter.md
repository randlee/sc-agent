<!-- Generated from bead agt-4p2.3 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-3: sc-treesitter
- id: `a-3` (bead `agt-4p2.3`)
- branch: `sprint/a-3-sc-treesitter`
- depends on: none
- copy script: `docs/plans/phase-a/copy/a-3.sh`

## Description

Build sc-treesitter as a stand-alone library with PyO3 and CLI support types for querying any grammar. Governing records: REQ-SAG-0301, REQ-SAG-0302, REQ-SAG-0303. Code to copy from sc-agent-ref: see design.

## Design

# Sprint A-3: sc-treesitter (bead agt-4p2.3)

Copy script: `docs/plans/phase-a/copy/a-3.sh` (dry run 2026-11-09, reconciliation: 19 copy steps, 7 files, 511 lines before trimming).
Source: sc-agent-ref @ dce239a, `crates/sc-treesitter`. Target: `crates/sc-treesitter/`.
Governing records: REQ-SAG-0301, REQ-SAG-0302 (as amended: Python + Rust first release, C# `csharp` + Go second release), REQ-SAG-0303, REQ-SAG-0002 (now also denies `indexing_slicing`), REQ-SAG-0001, ADR-SAG-0004 (sc-treesitter stands alone).
Error convention: the workspace id rule (A-1) is `"<crate-prefix>.<variant_snake>"` with an id, a message and a cause chain. sc-treesitter MUST NOT depend on sc-agent-types (REQ-SAG-0301), so it does not implement A-1's `ErrorId` trait; it follows the same convention with inherent `id()`/`message()` and serializes as the same `{"id","message"}` shape as `ErrorDetail`. Ids use the prefix `treesitter`. Owner memories: owner-sc-treesitter, owner-treesitter-grammars, owner-error-contract, owner-errors-settled, owner-no-sleep, owner-lowlevel-no-tokio.
Owner corrections (2026-11-09) win over the older copy plan: KEEP the Python and Rust grammars and queries; C#/Go are the second wave; use the owner's 17-variant error list plus `Write` and `Log` for the `tsq` binary (the owner: missing errors are added in the plan); no sc-agent deps, no git; PyO3 support types behind a `python` feature, built into the extension module by `crates/sc-treesitter-py` (section 3a).

Scope split with A-10: this sprint is the library, its CLI support types (output types, error ids, JSON encoder, file reader), the PyO3 support types and `crates/sc-treesitter-py`. The `tsq` binary (`src/main.rs`, `src/cli.rs`, `tests/cli.rs`, clap, the `cli` feature) is Sprint A-10.

Decisions (assistant, 2026-11-09; owner reviews the placement at architecture review):
- PyO3 placement: a separate crate `crates/sc-treesitter-py` builds the extension module; sc-treesitter stays an rlib with the support types behind `python` (section 3a).
- Output shape: flat captures sorted by (start_byte, name, end_byte), as copied; each capture is one part of a match.
- Logging: the library logs nothing (stand-alone, REQ-SAG-0301); the `tsq` binary writes its own general log (REQ-SAG-0003, A-10).
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

Not copied: `src/git_io.rs`, `src/cli/`, `src/main.rs` (A-10), `tests/` (git/envelope tests; `tests/cli.rs` parts go to A-10), `ExtractInput` and its getters (`extract.rs:12-130`), the builder/subjects code (`extract.rs:254-272,281-291`), `u32_from_usize` (`:314-324`), digest/subjects/cross-impl/manifest/newtype tests (`:364-493,524-551,613-638,659-678,703-733`), mimalloc, `PRODUCER_NAME`/`PRODUCER_MODE`, the old README and Cargo.toml.

## 2. Required edits after copying

- All files: delete every `sc_agent_types` import and use; replace `JevError` with `crate::Error` variants (section 4); delete doc text about sc/subjects/1, `query_sha256`, "Phase A", git, producers; add module docs (`//!`) to each file.
- `src/grammar.rs`: features renamed `python` -> `grammar-python`, `rust` -> `grammar-rust` (the `python` feature is PyO3). Remove `LanguageInfo` (the hand-copied `grammar`/`grammar_version` strings are no longer output) and the public `language()`; `Grammar` keeps `id`, `load`, `queries`. `languages()` returns ids. Add `KNOWN_LANGUAGES` (every id this crate version can be built with, compiled or not) so `lookup` returns `GrammarUnavailable` for a known id whose feature is off and `UnknownLanguage` otherwise. Rewrite the doctest without `LanguageInfo`.
- `src/query.rs`: `QueryInfo` derives `Serialize` and skips `source` in serde; delete the `query_sha256` paragraphs (copied lines 12-15 and 50-54 of the target); `query_not_found` -> `Error::UnknownQuery { language, name }`.
- `src/run.rs`: rename `extract` to `run_query` with the signature in section 3; delete `ExtractInput`, `path`/`rev`/`blob_sha` locals, `PRODUCER_*`, the empty-query special case (an empty query compiles to zero patterns and returns `NoMatches`). Before UTF-8: `source.len() > MAX_SOURCE_BYTES` -> `InputTooLarge`. `from_utf8` error -> `NotUtf8 { byte_offset: e.valid_up_to() }`. `set_language` error -> `GrammarLoad { detail: e.to_string() }`. `Query::new` error -> `QueryCompile` keeping `offset,row,column,kind (Debug of QueryErrorKind),message`. After compiling, each name in `captures` must be in `query.capture_names()` else `QueryCaptureInvalid`. `parse` `None` -> `ParseFailed`. Replace `capture_names[index]` with `.get(index)` (no index; a missing name is unreachable by tree-sitter contract, so skip it with `continue`, no panic) and delete both `debug_assert!`. `found.is_empty()` -> `NoMatches { language, query }`. `error_line_inside` returns the node `(row, column, kind)` of the earliest ERROR/MISSING node (no `unwrap_or(u32::MAX)`), giving `SyntaxError { row, column, node_kind }`. `inclusive_lines` returns `Result<(usize, usize), Error>` with `checked_add(1)` -> `PositionOverflow { row }`; no u32 narrowing. The copied text lookup (lines 273-280) maps to `Capture` construction; a `None` slice cannot occur for UTF-8 node boundaries, but return `Error::NotUtf8 { byte_offset: start_byte }` rather than panic. Remove `drop(matches)` comment noise.
- `src/run.rs` tests: port to `run_query`; assert `Error` variants (`matches!`) and `id()` (e.g. `"treesitter.no_matches"`), not `.code()`; replace python tests that used the cross-impl `hook.py` fixture with inline sources; in the determinism test drop the subject-id asserts and keep `once == twice` and same-line byte ordering; gate tests with `grammar-python`/`grammar-rust`.
- `src/lib.rs`: modules `error`, `grammar`, `query`, `run`, `source`, `#[cfg(feature = "python")] python`; re-exports as in section 3; `#![warn(missing_docs)]`. Fold `render.rs` as generic `to_json_bytes`; encode error -> `Error::Encode { detail }`.
- `assets/queries/python/branches.scm`: remove the sentence about `@_fn` from the comment (there is no helper capture now).

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

## 4. Error enum (19 variants; `src/error.rs`, new)

Lower errors here are external (tree-sitter, std::io, std::str, serde_json, clap, pyo3, sc-observability); each variant keeps the lower error's kind and message in its fields. No sc-agent error is wrapped. Hand-written `Display` (`"<id>: <message>"`), no thiserror, no serde tag.

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
    /// tree_sitter::QueryError, every field kept; kind = Debug of QueryErrorKind.
    QueryCompile { language: String, offset: usize, row: usize, column: usize, kind: String, detail: String }, // "treesitter.query_compile"
    /// Capture filter names a capture the query does not define.
    QueryCaptureInvalid { capture: String },                                   // "treesitter.query_capture_invalid"
    FileNotFound { path: std::path::PathBuf },                                 // "treesitter.file_not_found"
    /// io error other than NotFound; kind = Debug of io::ErrorKind, detail = Display.
    FileRead { path: std::path::PathBuf, kind: String, detail: String },       // "treesitter.file_read"
    /// Utf8Error::valid_up_to.
    NotUtf8 { path: Option<std::path::PathBuf>, byte_offset: usize },          // "treesitter.not_utf8"
    /// Source over MAX_SOURCE_BYTES.
    InputTooLarge { path: Option<std::path::PathBuf>, bytes: u64, limit: u64 }, // "treesitter.input_too_large"
    /// Parser::parse returned None.
    ParseFailed { language: String },                                          // "treesitter.parse_failed"
    /// ERROR or MISSING node inside a matched span; 0-based row/column.
    SyntaxError { path: Option<std::path::PathBuf>, row: usize, column: usize, node_kind: String }, // "treesitter.syntax_error"
    /// A 1-based line does not fit usize.
    PositionOverflow { path: Option<std::path::PathBuf>, row: usize },         // "treesitter.position_overflow"
    /// The query ran and matched nothing (after helper/allow-list filtering).
    NoMatches { language: String, query: String },                             // "treesitter.no_matches"
    /// serde_json error Display.
    Encode { detail: String },                                                 // "treesitter.encode"
    /// CLI argument error (A-10); kind = Debug of clap ErrorKind, detail = clap's text.
    Usage { kind: String, detail: String },                                    // "treesitter.usage"
    /// PyO3 argument conversion failure (bindings only); detail = PyErr Display.
    Python { detail: String },                                                 // "treesitter.python"
    /// `tsq` could not write stdout or stderr (A-10); kind = Debug of io::ErrorKind, detail = Display.
    Write { kind: String, detail: String },                                    // "treesitter.write"
    /// `tsq`'s general log could not start (A-10); detail = the sc-observability error's code and message.
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
# `cli` and clap are added by Sprint A-10.

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tree-sitter = "=0.27.1"
tree-sitter-python = { version = "=0.25.0", optional = true }
tree-sitter-rust = { version = "=0.24.2", optional = true }
pyo3 = { version = "=0.29.2", optional = true, features = ["abi3-py310"] }
```

Workspace lints (root `Cargo.toml`, owned by whichever sprint creates the workspace, A-1 expected): `[workspace.lints.clippy] panic = "deny"`, `unwrap_used = "deny"`, `expect_used = "deny"`, `indexing_slicing = "deny"`, and `clippy.toml` with `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`, `allow-indexing-slicing-in-tests = true`. sc-lint does not enforce these yet. README.md: 3-4 lines, new.

Second wave (a later sprint; see Decisions): add `grammar-csharp` (`tree-sitter-c-sharp` 0.23.5, id `csharp`) and `grammar-go` (`tree-sitter-go` 0.25.0) rows to `GRAMMARS` and `KNOWN_LANGUAGES`, with their queries; no API change. Building them against tree-sitter 0.27.1 is not yet confirmed.

## 6. New code

- `src/error.rs` (section 4), ~110 lines.
- `src/source.rs`: `read_source`, `MAX_SOURCE_BYTES`, ~30 lines.
- `src/python.rs`: section 3, ~90 lines.
- `Capture` and the capture-filter check in `src/run.rs`.
- `README.md`, `Cargo.toml`.

## 7. Tests

Ported (from the copied ranges): Rust `match-arms` predefined query; same-line determinism and byte order; ERROR node -> `SyntaxError`; MISSING node -> `SyntaxError`; unknown language -> `UnknownLanguage`; duplicate captures in one match; empty result -> `NoMatches`; invalid query -> `QueryCompile` with row/column/offset/kind; clean source succeeds.
New:
1. `python_and_rust_through_the_same_call` (REQ-0302 AC1): one caller-written query run through `run_query` on Python source and on Rust source.
2. `every_loaded_language_runs_through_run_query` (REQ-0302 AC2): iterates `languages()`, runs `(_) @node` over a per-id sample source from a test table; asserts every id has a sample, so a new grammar only adds a row and a sample.
3. Helper capture `@_x` is never returned; allow-list naming an undefined capture -> `QueryCaptureInvalid`.
4. `NotUtf8` carries `byte_offset`; `InputTooLarge` carries bytes/limit; `PositionOverflow` from `inclusive_lines` at `usize::MAX`.
5. `GrammarUnavailable` when built without `grammar-rust` (`#[cfg(not(feature = "grammar-rust"))]`, run in CI with `--no-default-features --features grammar-python`).
6. `read_source`: missing file -> `FileNotFound`; a directory -> `FileRead` with kind.
7. `queries(None)` lists python/branches and rust/match-arms; `queries(Some("cobol"))` -> `UnknownLanguage`; `query_source` unknown name -> `UnknownQuery`.
8. `id()` equals the serialized `id` for all 19 variants and follows `treesitter.<variant_snake>`; `to_json_bytes` ends with `\n`.
9. PyO3 (feature `python`, REQ-0303 AC1 and REQ-0002 AC1): a caller-written query through `run_query` returns `Capture` objects; a non-UTF-8 `bytes` source raises `TreesitterError` whose args are (`treesitter.not_utf8`, its message). Runs as a pytest (`crates/sc-treesitter-py/tests/test_query.py`) against the module built with maturin.
10. CI: `cargo tree -p sc-treesitter -e normal,build` contains no `sc-agent`/`sc-decision` crate (REQ-0301); `cargo clippy -p sc-treesitter --all-features -- -D warnings` with the workspace lints (REQ-0002).

## 8. Dependencies

`tree-sitter =0.27.1`, `tree-sitter-python =0.25.0` (optional), `tree-sitter-rust =0.24.2` (optional), `serde 1` (derive), `serde_json 1`, `pyo3 =0.29.2` with `abi3-py310` (optional). No sc-agent crate, no tokio, no git, no sc-sha, no mimalloc, no thiserror. Dev: `tempfile 3` (for the `read_source` tests only; other fixtures are inline).

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-3.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] cargo tree -p sc-treesitter lists no sc-agent crate (REQ-SAG-0301); no agent or decision-model code.
- [ ] A caller-written query runs against Python source and against Rust source through the same run_query call (REQ-SAG-0302, first release).
- [ ] Every id from languages() runs through the same run_query call; adding a grammar adds a table row only, no API change (REQ-SAG-0302).
- [ ] languages() lists the loaded grammars and queries() lists the predefined queries (python/branches, rust/match-arms) (REQ-SAG-0303 support).
- [ ] A caller-written query runs through the PyO3 bindings (crates/sc-treesitter-py over feature python, module sc_treesitter) and returns its matches as Capture objects (REQ-SAG-0303).
- [ ] All 19 Error variants (the owner's 17 plus Write and Log for tsq) exist with stable ids treesitter.<variant_snake> (the workspace convention, without depending on sc-agent-types), serialized as {id, message}; each keeps the lower error's kind and message (REQ-SAG-0002).
- [ ] An injected lowest-layer failure (non-UTF-8 source) reaches the PyO3 entry point with id treesitter.not_utf8 and its message (REQ-SAG-0002).
- [ ] Clippy passes with panic, unwrap_used, expect_used and indexing_slicing denied in non-test code (REQ-SAG-0002).
- [ ] No git, no sc-sha; no u32 clamping of positions (PositionOverflow instead).
- [ ] Second wave (C# as csharp, Go) is a later sprint that adds table rows, queries and samples with no API change (REQ-SAG-0302).
- [ ] The library writes no log (REQ-SAG-0301); `cargo tree -p sc-treesitter --no-default-features --features grammar-python,grammar-rust` lists no sc-observability.
- [ ] REQ-SAG-0010: sc-treesitter and sc-treesitter-py are Rust crates in the sc-agent workspace, and their source calls no sleep function (std::thread::sleep, tokio::time::sleep or any other), enforced by the workspace clippy disallowed-methods rule.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); each decision is recorded once in the design:
1. PyO3 placement: crates/sc-treesitter-py (module and wheel sc_treesitter / sc-treesitter); assistant decision shown to the owner at architecture review.
2. Output shape: flat sorted captures (assistant).
3. Logging: none in the library (stand-alone); the tsq binary logs (A-10).
4. C#/Go: a later sprint; one predefined query each, proposed by the assistant.
5. tsq write failure: new variant Write (treesitter.write); Log added for tsq's general log.
No open questions.
