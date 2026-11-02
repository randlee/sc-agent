# sc-jev-check — inputs and extraction boundary

**ADR-SAG-0015:** Extraction produces the **subjects artifact outside the judge**; the judge consumes it and **links no parser**.

Normative artifact: [`docs/sc-agent-types/subjects-schema.md`](../sc-agent-types/subjects-schema.md) (`sc/subjects/1`).  
Tree-sitter production: crate **`sc-treesitter`** ([`docs/sc-treesitter/architecture.md`](../sc-treesitter/architecture.md)).

## Extractor trait (Phase A kinds)

`sc-jev-check` implements **`file`**, **`table-column`**, **`subjects`**, and **`command`** only. **No** `tree-sitter` kind and **no** grammar crate dependencies.

```rust
/// Phase A kinds: file, table-column, subjects, command.
pub trait Extractor { /* … */ }
```

### `subjects`

Reads a **`sc/subjects/1`** JSON file (path or stdin). Validates `schema`, uses `subjects[].id` / `.text` as question subjects. Copies the `producer` block into session metadata and the equal-input record.

```toml
[inputs.branches]
kind = "subjects"
path = "out/task-hook.subjects.json"
```

### `command`

Argv subprocess; **stdout must be** a valid `sc/subjects/1` document (typical: invoke `sc-treesitter extract …`).

```toml
# Example: inline producer step
command = ["sc-treesitter", "extract", "--language", "python", "--query", "branches", "{path}"]
```

Replaces the old ad hoc `{id: text}` JSON contract.

### `file` / `table-column`

Unchanged roles (raw file bytes; markdown table column with unescaped `|` rules per REQ-SAG-0309).

## Lite gate `check.toml` shape

```toml
[inputs.hook]
kind = "file"
path = "skills/atm-bd-lite/scripts/atm-bd-task-hook.py"

[inputs.branches]
kind = "subjects"
path = "out/task-hook.subjects.json"
```

(`out/task-hook.subjects.json` is produced by `sc-treesitter` in a prior step or CI fixture.)

## Cross-impl (consumer)

`tests/fixtures/cross-impl/branches/` holds hook source, reference `.scm`, **committed `subjects.json`** (from `sc-treesitter`), and expected request bytes + sha256. Rust `sc-jev-check` and Python `jev-check` **read the same `subjects.json`**; no Python branch extractor. atm-core PR #2308 is **not** a Phase A dependency.

Producer byte-for-byte test lives in **`sc-treesitter`** (sprint **a-3b**).
