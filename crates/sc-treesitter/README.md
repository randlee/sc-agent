# sc-treesitter

Loads tree-sitter grammars and runs any query against them. The library stands alone: it is synchronous, starts no threads, does no logging and has no sc-agent dependency. It sits outside the sc-agent layering. The **`sitr`** CLI ships from the separate **`sitr`** crate; the Python bindings ship as the PyPI **`sitr`** project (`import sc_treesitter`).

Captures come back sorted by (start byte, name, end byte), with 1-based inclusive lines.

## What it provides

- `run_query(language, query, source, path, captures)`: runs one caller-written query against one source with any loaded grammar, and returns `Vec<Capture>`. `captures` is an optional allow-list of capture names. Names that start with `_` are helpers and are never returned.
- `LanguageId` (`LanguageId::new("python")`, a checked id) and `languages()`.
- `queries(language)`, `query_source(language, name)` and `QueryInfo`: the predefined queries (`python` `branches`, `rust` `match-arms`).
- `read_source(path)`, a file reader.
- `to_json_bytes`: pretty JSON plus a trailing newline.
- `Capture` and `Error` (stable `treesitter.*` ids).

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

The **`sitr`** CLI runs the predefined queries:

```bash
cargo install sitr
sitr languages
sitr queries --language python
sitr query python branches path/to/file.py
sitr query rust match-arms src/lib.rs --captures arm
```

`sitr` prints JSON and a newline on stdout and exits 0. On failure it prints one stderr line `{"id": "treesitter.<variant>", "message": ...}`, prints nothing on stdout and exits 1. When the general log opens, each run writes one `sitr.command` event to `~/.sc-agent/logs/sitr.log.jsonl`, or to `$SITR_LOG_ROOT/logs/sitr.log.jsonl` when `SITR_LOG_ROOT` is set and not empty.

## Features

- `python`: the PyO3 support types, used by the `sc-treesitter-py` extension crate.

## Depends on

No sc-agent crate. This crate stands alone (ADR-SAG-0004).

## Requirements and decisions

- REQ-SAG-0301: sc-treesitter is a stand-alone library.
- REQ-SAG-0302: any tree-sitter grammar.
- REQ-SAG-0303: PyO3 and CLI query surfaces.
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top (`treesitter.*` ids).
- REQ-SAG-0003: log retention (the `sitr` general log).
- ADR-SAG-0005: sc-treesitter-py is a separate PyO3 crate; the `sitr` CLI is a separate crate.
