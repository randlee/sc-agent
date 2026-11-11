# sc-treesitter-py

The `sc_treesitter` Python extension module: the PyO3 bindings of `sc-treesitter`. The functions and types live in `sc_treesitter::python` (feature `python`). This crate only owns the `#[pymodule]` and registers them (ADR-SAG-0005). It has `publish = false` and builds with maturin as the Python package `sc-treesitter`.

It stands alone with `sc-treesitter`: no sc-agent crate is involved.

## What it provides

- `sc_treesitter.run_query(language, query, source, captures=None)`: `source` is `bytes` or `str`. It returns a list of `Capture` objects with `name`, `text`, `start_line`, `end_line`, `start_byte` and `end_byte`. Lines are 1-based and inclusive; bytes are 0-based with the end exclusive.
- `sc_treesitter.languages()`: the ids of the compiled-in grammars.
- `sc_treesitter.queries(language=None)`: the predefined queries as `(language, query)` pairs.
- `sc_treesitter.query_source(language, name)`: the text of a predefined query.
- `sc_treesitter.Capture` and `sc_treesitter.TreesitterError`. `TreesitterError.args` is `(id, message)`, where the id is a stable `treesitter.*` id.

The GIL is released while tree-sitter parses and queries.

## Usage

```bash
cd crates/sc-treesitter-py && maturin develop --release
```

```python
import sc_treesitter

print(sc_treesitter.languages())  # ['python', 'rust']

captures = sc_treesitter.run_query(
    "python",
    "(function_definition name: (identifier) @name)",
    b"def hello():\n    pass\n",
)
for capture in captures:
    print(capture.name, capture.text, capture.start_line, capture.end_line)

try:
    sc_treesitter.run_query("cobol", "(x) @x", b"x")
except sc_treesitter.TreesitterError as error:
    error_id, message = error.args  # error_id == "treesitter.unknown_language"
```

## Features

None of its own. It builds `sc-treesitter` with the `python` feature and the default grammars (`grammar-python`, `grammar-rust`). PyO3 uses `abi3-py310` and `extension-module`.

## Depends on

- `sc-treesitter` (with feature `python`). No sc-agent crate.

## Requirements and decisions

- REQ-SAG-0303: PyO3 and CLI query surfaces.
- REQ-SAG-0302: any tree-sitter grammar.
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top.
- ADR-SAG-0005: sc-treesitter-py is a separate PyO3 crate; `tsq` lives in sc-treesitter behind a `cli` feature.
