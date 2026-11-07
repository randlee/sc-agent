# sc-jev-check — inputs and extraction boundary

**ADR-SAG-0015:** Extraction produces the **subjects artifact outside the judge**; the judge consumes it and **links no parser**.

Normative artifact: [`docs/sc-agent-types/subjects-schema.md`](../sc-agent-types/subjects-schema.md).  
Producer: [`sc-treesitter`](../sc-treesitter/architecture.md).

## Input kinds (no extractor trait)

`extract::extract_input` matches the four `config::InputSpec` kinds directly:
**`file`**, **`table-column`**, **`subjects`**, and **`command`**. There is no
`Extractor` trait (withdrawn in r-2, Rust API patterns amendment); a new kind
is a new `InputSpec` variant plus an ADR. Every path in `check.toml` is a
`RepoRelativePath`, validated when the document is parsed (`CONFIG_INVALID`
for an absolute path, `..`, or an empty path).

### `subjects`

Reads **`sc/subjects/1`** from path or stdin; validates schema; maps `subjects[].id` / `.text` into question subjects. Returns the validated document + subjects (producer copy into equal-input happens in **a-6**).

### `command`

Argv array only — **no shell**. Placeholders: `{path}`, `{rev}`, `{repo_root}`,
replaced in one left-to-right pass. **cwd** = repo root. **`timeout_seconds`**
default **30**. **stdout** must be valid `sc/subjects/1`.

`path` (optional, REQ-SAG-0312) is the repo-relative file substituted for
`{path}`. It is validated by the `file` rules below before the command runs
(at `WORKTREE`: canonicalized, a regular file, inside the canonical repo root;
at any other rev: a blob at that rev). `{path}` in the argv with no `path`
field is `CONFIG_INVALID`.

The child gets `env_clear()` plus the a-10 allowlist (`PATH`, `HOME`,
`USERPROFILE`, `SYSTEMROOT`, `TEMP`, `TMP`, `TMPDIR`, `LANG`, `LC_*`, `TERM`); no
API-key variable is ever passed (ADR-SAG-0020).

Failures: timeout → `EXTRACTION_TIMEOUT`; invalid stdout → `EXTRACTION_INVALID`;
zero subjects → `EXTRACTION_EMPTY`. On a non-zero exit the last non-empty
stderr line (within the 4 KiB stderr tail) is tried as an
`ScAgentErrorResponse` (REQ-SAG-0313): a known id is re-raised as
`ProducerError` with that id and the producer name (file stem of `argv[0]`),
so `LANGUAGE_UNAVAILABLE` from `sc-treesitter` keeps its code; anything else
(plain text, an unknown id) is `EXTRACTION_FAILED` with the redacted tail.

```toml
[inputs.branches]
kind = "command"
command = ["sc-treesitter", "extract", "--language", "python", "--query", "branches", "{path}"]
path = "src/hook.py"
```

The engine applies no payload byte cap: each backend refuses an oversize
request in `validate_for_send` before any send (REQ-SAG-0314). The 8 MiB
per-file and stdout caps stay as resource guards.

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
