# sc-treesitter

Load tree-sitter grammars (Python, Rust) and run any query against them; captures come back sorted, with 1-based inclusive lines.
Stand-alone: no sc-agent dependency, no logging. PyO3 support types sit behind the `python` feature (the extension module is `crates/sc-treesitter-py`).
The `tsq` binary runs the predefined queries: `cargo install sc-treesitter --features cli`.
