# sc-jev-check — inputs and extraction boundary

**ADR-SAG-0015:** Extraction produces the **subjects artifact outside the judge**; the judge consumes it and **links no parser**.

Normative artifact: [`docs/sc-agent-types/subjects-schema.md`](../sc-agent-types/subjects-schema.md).  
Producer: [`sc-treesitter`](../sc-treesitter/architecture.md).

## Extractor trait (sealed)

```rust
pub trait Extractor: private::Sealed {
    fn extract(
        &self,
        repo: &RepoRoot,
        rev: &Rev,
        input: &InputSpec,
    ) -> Result<Vec<Subject>, JevError>;
}
```

Only **`file`**, **`table-column`**, **`subjects`**, and **`command`** implement this trait in Phase A. No public `impl Extractor` outside `sc-jev-check`.

### `subjects`

Reads **`sc/subjects/1`** from path or stdin; validates schema; maps `subjects[].id` / `.text` into question subjects. Returns the validated document + subjects (producer copy into equal-input happens in **a-6**).

### `command`

Argv array only — **no shell**. Placeholders: `{path}`, `{rev}`, `{repo_root}`. **cwd** = repo root. **`timeout_seconds`** default **30**. Non-zero exit → `EXTRACTION_FAILED`; timeout → `EXTRACTION_TIMEOUT`; invalid stdout → `EXTRACTION_INVALID`; zero subjects → `EXTRACTION_EMPTY`. **stdout** must be valid `sc/subjects/1`.

```toml
command = ["sc-treesitter", "extract", "--language", "python", "--query", "branches", "{path}"]
```

### `file` / `table-column`

Unchanged; table-column uses unescaped `|` rules (REQ-SAG-0309).

## Lite gate `check.toml`

```toml
[inputs.hook]
kind = "file"
path = "skills/atm-bd-lite/scripts/atm-bd-task-hook.py"

[inputs.branches]
kind = "subjects"
path = "tests/fixtures/cross-impl/branches/subjects.json"
```

## Cross-impl (consumer)

End-to-end digest test lives on sprint **a-7** (`must_follow` **a-11**). Fixture tree is owned by **a-11**; both Rust and Python read the same committed `subjects.json`.
