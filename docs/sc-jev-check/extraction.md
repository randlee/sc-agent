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

`file` reads the repo-relative path at `rev`. `WORKTREE` is the working tree:
the path is canonicalized and must stay under the canonical repo root, so a
symlink that leaves the repo is `INPUT_INVALID`. Any other rev resolves
`<rev>:./<path>` once with `git rev-parse --verify --end-of-options`, then
reads that object id with `git cat-file`. Both reads stop at 8 MiB
(`INPUT_TOO_LARGE`).

**Known test limitation (Windows).** The symlink-escape tests
(`extract::read::tests::committed_symlink_outside_the_repo_is_input_invalid`
and `symlink_inside_the_repo_is_read`) are `#[cfg(unix)]`: creating a symlink
on Windows needs Developer Mode or `SeCreateSymbolicLinkPrivilege`, which a
test cannot rely on. The containment check itself (`canonicalize` then
`starts_with` the canonical root) is platform-neutral code and runs on
Windows; only the symlink fixture is Unix-only.

`table-column` splits rows on unescaped `|` only. A cell may contain `\|`, which is one literal `|` and does not open a new column (REQ-SAG-0309).

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
