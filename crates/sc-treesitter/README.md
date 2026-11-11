# sc-treesitter

Loads tree-sitter grammars and runs any query against them. The library stands alone: it is synchronous, starts no threads, does no logging and has no sc-agent dependency. It sits outside the sc-agent layering. The `tsq` binary and the Python module are built from it.

Captures come back sorted by (start byte, name, end byte), with 1-based inclusive lines.

## What it provides

- `run_query(language, query, source, path, captures)`: runs one caller-written query against one source with any loaded grammar, and returns `Vec<Capture>`. `captures` is an optional allow-list of capture names. Names that start with `_` are helpers and are never returned.
- `LanguageId` (`LanguageId::new("python")`, a checked id) and `languages()`.
- `queries(language)`, `query_source(language, name)` and `QueryInfo`: the predefined queries (`python` `branches`, `rust` `match-arms`).
- `read_source(path)`, a size-limited file reader, with `MAX_SOURCE_BYTES`.
- `to_json_bytes`: pretty JSON plus a trailing newline.
- `Capture`, `Error` (stable `treesitter.*` ids) and `Limit`, with the limits `MAX_CAPTURES`, `MAX_CAPTURE_TEXT_BYTES` and `MATCH_LIMIT`.

## Usage

Rust:

```rust,ignore
let python = sc_treesitter::LanguageId::new("python")?;
let captures = sc_treesitter::run_query(
    python,
    "(function_definition name: (identifier) @name)",
    b"def hello():\n    pass\n",
    None, // path, used only in error details
    None, // capture allow-list
)?;
assert_eq!(captures[0].text, "hello");
let json = sc_treesitter::to_json_bytes(&captures)?;
```

`tsq` runs the predefined queries:

```bash
cargo install sc-treesitter --features cli
tsq languages
tsq queries --language python
tsq query python branches path/to/file.py
tsq query rust match-arms src/lib.rs --captures arm
```

`tsq` prints JSON and a newline on stdout and exits 0. On failure it prints one stderr line `{"id": "treesitter.<variant>", "message": ...}`, prints nothing on stdout and exits 1. Each run writes one `tsq.command` event to `~/.sc-agent/logs/tsq.log.jsonl`, or to `<PATH>/logs/tsq.log.jsonl` with `--log-root PATH`.

## Features

- `grammar-python` and `grammar-rust` (default): compile in that grammar.
- `python`: the PyO3 support types, used by the `sc-treesitter-py` extension crate.
- `cli` (not default): builds the `tsq` binary. It adds `clap` and `sc-observability`.

## Depends on

No sc-agent crate. This crate stands alone (ADR-SAG-0004).

## Requirements and decisions

- REQ-SAG-0301: sc-treesitter is a stand-alone library.
- REQ-SAG-0302: any tree-sitter grammar.
- REQ-SAG-0303: PyO3 and CLI query surfaces.
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top (`treesitter.*` ids).
- REQ-SAG-0003: log retention (the `tsq` general log).
- ADR-SAG-0005: sc-treesitter-py is a separate PyO3 crate; `tsq` lives in sc-treesitter behind a `cli` feature.
