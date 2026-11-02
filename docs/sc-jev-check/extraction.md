# sc-jev-check — extraction (ADR-SAG-0015)

Extraction lives only in **`sc-jev-check`**. **`sc-jev`** never reads source files.

## Extractor trait

```rust
mod private {
    pub trait Sealed {}
}

/// Phase A kinds only (`file`, `table-column`, `tree-sitter`, `command`).
pub trait Extractor: private::Sealed {
    fn extract(&self, repo: &RepoRoot, rev: &Rev, input: &InputSpec) -> Result<Vec<Subject>, JevError>;
}

pub struct Subject {
    pub id: String,
    pub text: String,
    pub span: Option<Span>,
}

pub struct Span {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub start_byte: u32,
    pub end_byte: u32,
}
```

Kinds **`file`**, **`table-column`**, **`tree-sitter`**, and **`command`** implement this trait.

## `tree-sitter` kind

```toml
[[extractors]]
id = "branches"
kind = "tree-sitter"
language = "python"
path = "skills/atm-bd-lite/scripts/atm-bd-task-hook.py"
rev = "HEAD"
query = "queries/python-branches.scm"
captures = ["arm", "refusal"]
# id scheme default: path:line (see ADR subject rules)
```

Query path is relative to `check.toml` (or `query_inline = "..."`). Query file is
loaded via `git show` at `rev`, like other inputs.

### Lite gate query (committed as versioned config)

```scheme
;; queries/python-branches.scm
(case_clause pattern: (_) @arm)

(return_statement
  (call function: (identifier) @_fn) @refusal
  (#eq? @_fn "Refusal"))
```

Captures whose names start with `_` are helpers — never subjects.

### Subject rules (normative)

| Rule | Definition |
| --- | --- |
| **id** | `{path}:{start_line}`; duplicate captures on same line → `:{n}` suffix (`n` from 1) |
| **text** | UTF-8 slice `[start_byte, end_byte)` — no unparse, trim, or reformat |
| **order** | `(start_byte, capture name)` ascending; stable across impls |
| **empty** | zero subjects → `EXTRACTION_EMPTY` |
| **parse error** | ERROR node inside captured span → `EXTRACTION_PARSE_ERROR` (path + line) |
| **multiplicity** | same node, different capture names → two subjects; same name → one |

Session records `extractor: { kind, language, grammar_version, query_sha256 }`.

### Grammars (pinned in `Cargo.toml`)

| `language` | crate | Phase A |
| --- | --- | --- |
| `python` | `tree-sitter-python` | yes (lite gate) |
| `rust` | `tree-sitter-rust` | yes |
| `c-sharp` | `tree-sitter-c-sharp` | feature, not default |
| others | respective crates | feature flags |

Unknown/disabled language → `CONFIG_INVALID` listing available set.

## `command` kind (escape hatch)

```toml
[[extractors]]
id = "arms"
kind = "command"
path = "crates/x/src/hook.rs"
rev = "HEAD"
command = ["syn-grep", "arms", "{path}", "--rev", "{rev}"]
timeout_seconds = 30
```

Argv array only (no shell). Placeholders `{path}`, `{rev}`, `{repo_root}`. Cwd =
repo root. stdout = JSON object `{ "<id>": "<text>" }` or array of `{id, text, span?}`;
else `EXTRACTION_INVALID` (first 300 bytes). Non-zero exit → `EXTRACTION_FAILED`;
timeout → `EXTRACTION_TIMEOUT`; empty → `EXTRACTION_EMPTY`. Command + argv +
stdout sha256 recorded in session.

## Python parity dependency (atm-core)

**atm-core PR #2308** (`extractors.py`): `python-branches` must emit the **same
source slice** as tree-sitter (replace `ast.unparse`). Sprint **a-3** is blocked
on that one-line semantic change + fixture update so cross-impl `prompt_sha256`
matches when branches are present.

## Cross-impl fixture

`tests/fixtures/cross-impl/branches/` — one hook, one `.scm` query, byte-equal
subjects JSON from Python reference and Rust; full request bytes + sha256 equal.

## Phase A tests (sprint a-3)

- `test_tree_sitter_python_branches_lists_arms_and_refusals`
- `test_tree_sitter_rust_match_arms`
- `test_tree_sitter_helper_capture_is_not_a_subject`
- `test_tree_sitter_same_line_ids_are_unique_and_stable`
- `test_tree_sitter_parse_error_is_typed`
- `test_tree_sitter_unknown_language_is_config_invalid`
- cross-impl branches fixture (subjects + full request sha256)
